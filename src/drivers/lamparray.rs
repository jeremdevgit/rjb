// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! HID LampArray (« Éclairage dynamique » de Windows) : norme commune à de
//! nombreux périphériques récents. Chaque LED déclare sa position physique.
//! Protocole d'après la spécification HID (page d'usage 0x59) et OpenRGB.

use super::hid::{self, HidEntry};
use crate::device::{Device, Result};
use crate::layout;
use crate::model::*;
use hidapi::{DeviceInfo as HidInfo, HidApi, HidDevice};

pub const HID: &[HidEntry] = &[HidEntry {
    vid: 0,
    pids: &[],
    usage_page: Some(0x59),
    usage: Some(0x01),
    interface: None,
    open,
}];

#[derive(Default)]
struct Ids {
    attributes: u8,
    lamp_request: u8,
    lamp_response: u8,
    multi_update: u8,
    control: u8,
}

pub struct LampArray {
    info: DeviceInfo,
    dev: HidDevice,
    ids: Ids,
    lamp_ids: Vec<u16>,
    autonomous: Option<bool>,
    min_interval_ms: u32,
}

/// Parcourt le descripteur de rapport pour trouver les identifiants des rapports LampArray.
fn parse_ids(desc: &[u8]) -> Ids {
    let mut ids = Ids::default();
    let (mut pos, mut usage_page, mut usage, mut report_id, mut coll_usage) = (0usize, 0u32, 0u32, 0u8, 0u32);
    let mut map: Vec<(u32, u8)> = Vec::new();
    while pos < desc.len() {
        let key = desc[pos];
        let (len, ksize) = if key & 0xF0 == 0xF0 {
            (desc.get(pos + 1).copied().unwrap_or(0) as usize, 3)
        } else {
            (match key & 3 {
                3 => 4,
                n => n as usize,
            }, 1)
        };
        let val = |n: usize| -> u32 {
            (0..n).fold(0u32, |acc, i| acc | (desc.get(pos + 1 + i).copied().unwrap_or(0) as u32) << (8 * i))
        };
        match key {
            0xA1 => {
                coll_usage = usage;
                if usage_page == 0x59 && report_id != 0 {
                    map.push((coll_usage, report_id));
                }
            }
            0xC0 => {
                coll_usage = 0;
                report_id = 0;
            }
            _ => {}
        }
        match key & 0xFC {
            0x04 => usage_page = val(len),
            0x08 => {
                usage = val(len);
                if len == 4 {
                    let page = usage >> 16;
                    usage &= 0xFFFF;
                    if page == 0x59 && report_id != 0 {
                        map.push((usage, report_id));
                    }
                }
            }
            0x84 => {
                report_id = val(len) as u8;
                if usage_page == 0x59 && coll_usage != 0 {
                    map.push((coll_usage, report_id));
                }
            }
            _ => {}
        }
        pos += len + ksize;
    }
    for (u, id) in map {
        match u {
            0x02 => ids.attributes = id,
            0x20 => ids.lamp_request = id,
            0x22 => ids.lamp_response = id,
            0x50 => ids.multi_update = id,
            0x70 => ids.control = id,
            _ => {}
        }
    }
    ids
}

fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn open(api: &HidApi, i: &HidInfo) -> Result<Vec<Box<dyn Device>>> {
    let dev = api.open_path(i.path())?;
    let mut desc = vec![0u8; 4096];
    let n = dev.get_report_descriptor(&mut desc)?;
    let ids = parse_ids(&desc[..n]);
    if ids.attributes == 0 || ids.lamp_request == 0 || ids.lamp_response == 0 || ids.multi_update == 0 || ids.control == 0 {
        return Ok(Vec::new());
    }
    // attributs du tableau : nombre de LED, boîte englobante, type
    let mut a = [0u8; 23];
    a[0] = ids.attributes;
    dev.get_feature_report(&mut a)?;
    let count = u16::from_le_bytes([a[1], a[2]]) as usize;
    if count == 0 || count > 2048 {
        return Ok(Vec::new());
    }
    let (bw, bh) = (u32le(&a, 3).max(1) as f32, u32le(&a, 7).max(1) as f32);
    let kind = u32le(&a, 15);
    let min_interval_ms = u32le(&a, 19);

    // attributs de chaque LED : la première requête fixe l'index, puis lecture successive
    let mut req = [0u8; 3];
    req[0] = ids.lamp_request;
    dev.send_feature_report(&req)?;
    let mut lamps = Vec::with_capacity(count);
    for _ in 0..count {
        let mut r = [0u8; 65];
        r[0] = ids.lamp_response;
        dev.get_feature_report(&mut r)?;
        let id = u16::from_le_bytes([r[1], r[2]]);
        let (x, y) = (u32le(&r, 3) as f32, u32le(&r, 7) as f32);
        let key = r[28] as u16;
        lamps.push((id, x, y, key));
    }

    let keyboard = kind == 1;
    let mut leds = Vec::with_capacity(count);
    let mut lamp_ids = Vec::with_capacity(count);
    for (n, &(id, x, y, key)) in lamps.iter().enumerate() {
        let name = layout::hid_usage_key(key)
            .map(layout::display_key)
            .unwrap_or_else(|| format!("LED {}", n + 1));
        leds.push(Led {
            name,
            x: (x / bw).clamp(0.0, 1.0),
            y: (y / bh).clamp(0.0, 1.0),
            w: 1.0,
        });
        lamp_ids.push(id);
    }
    // grille approximative (pas de touche ≈ 19 mm) pour l'affichage des claviers
    let grid = keyboard.then(|| (((bw / 19_000.0).round() as u16).max(1), ((bh / 19_000.0).round() as u16).max(1)));
    let zone = Zone {
        name: if keyboard { "Touches".into() } else { "LED".into() },
        leds,
        resizable: None,
        grid,
    };
    let vendor = i.manufacturer_string().unwrap_or("").trim().to_string();
    Ok(vec![Box::new(LampArray {
        info: DeviceInfo {
            id: hid::stable_id("lamparray", i),
            name: pretty_name(&hid::product(i, "Périphérique LampArray")),
            vendor: if vendor.is_empty() { "Éclairage dynamique".into() } else { vendor },
            kind: match kind {
                1 => DeviceKind::Keyboard,
                2 => DeviceKind::Mouse,
                3 => DeviceKind::Gamepad,
                7 => DeviceKind::Case,
                5 | 9 | 10 => DeviceKind::Light,
                _ => DeviceKind::Other,
            },
            location: hid::location(i),
            zones: vec![zone],
            modes: vec![ModeInfo::new("Effet de l'appareil")],
            direct: true,
        },
        dev,
        ids,
        lamp_ids,
        autonomous: None,
        min_interval_ms,
    })])
}

impl LampArray {
    fn set_autonomous(&mut self, on: bool) -> Result<()> {
        if self.autonomous != Some(on) {
            self.dev.send_feature_report(&[self.ids.control, on as u8])?;
            self.autonomous = Some(on);
        }
        Ok(())
    }
}

impl Device for LampArray {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, _mode: usize, _p: &HwParams) -> Result<()> {
        self.set_autonomous(true)
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        self.set_autonomous(false)?;
        let n = self.lamp_ids.len();
        let mut i = 0;
        while i < n {
            let k = (n - i).min(8);
            let mut p = [0u8; 51];
            p[0] = self.ids.multi_update;
            p[1] = k as u8;
            p[2] = (i + k >= n) as u8; // dernière série : mise à jour complète
            for j in 0..k {
                p[3 + j * 2..5 + j * 2].copy_from_slice(&self.lamp_ids[i + j].to_le_bytes());
                let c = colors.get(i + j).copied().unwrap_or(Color::BLACK);
                let o = 19 + j * 4;
                p[o..o + 4].copy_from_slice(&[c.r, c.g, c.b, 0xFF]);
            }
            self.dev.send_feature_report(&p)?;
            i += k;
        }
        Ok(())
    }

    fn max_fps(&self) -> u32 {
        if self.min_interval_ms > 0 {
            (1000 / self.min_interval_ms).clamp(5, 60)
        } else {
            60
        }
    }

    fn release(&mut self) {
        let _ = self.set_autonomous(true);
    }
}
