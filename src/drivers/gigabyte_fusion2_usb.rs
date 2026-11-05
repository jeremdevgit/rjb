// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Gigabyte / AORUS RGB Fusion 2 USB (contrôleurs ITE 8297/5702/5711/8950 des
//! cartes mères 2019+). Protocole d'après OpenRGB.

use super::hid::{self, HidEntry};
use crate::device::{err, Device, Result};
use crate::model::*;
use hidapi::{DeviceInfo as HidInfo, HidApi, HidDevice};

const RID: u8 = 0xCC;

pub const HID: &[HidEntry] = &[
    HidEntry { vid: 0x048D, pids: &[0x8297, 0x8950, 0x5702, 0x5711], usage_page: Some(0xFF89), usage: Some(0xCC), interface: None, open },
    HidEntry { vid: 0x0414, pids: &[0xA100], usage_page: Some(0xFF89), usage: Some(0xCC), interface: None, open },
];

const STRIP_HEADERS: [u8; 4] = [0x58, 0x59, 0x62, 0x63];
const STRIP_MASKS: [u8; 4] = [0x01, 0x02, 0x08, 0x10];

pub struct Fusion2Usb {
    info: DeviceInfo,
    dev: HidDevice,
    pid: u16,
    /// ordre des octets (r, g, b) pour chaque connecteur ARGB
    order: [(usize, usize, usize); 4],
    strips: usize,
    has_fixed: bool,
    builtin_off: Option<u8>,
    counts: [u8; 6],
}

fn decode_order(v: u32) -> (usize, usize, usize) {
    let (b, g, r) = ((v & 0xFF) as usize, ((v >> 8) & 0xFF) as usize, ((v >> 16) & 0xFF) as usize);
    let valid = v >> 24 == 0 && r < 3 && g < 3 && b < 3 && r != g && r != b && g != b;
    if valid {
        (r, g, b)
    } else {
        (0, 1, 2)
    }
}

fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn open(api: &HidApi, i: &HidInfo) -> Result<Vec<Box<dyn Device>>> {
    let dev = api.open_path(i.path())?;
    let pid = i.product_id();
    let d = Fusion2Usb::cc(&dev, 0x60, 0, 0);
    if d.is_err() {
        return err("RGB Fusion : pas de réponse");
    }
    let mut r = [0u8; 64];
    r[0] = RID;
    if dev.get_feature_report(&mut r)? < 64 {
        return err("RGB Fusion : rapport incomplet");
    }
    let mut order = [decode_order(u32le(&r, 44)), decode_order(u32le(&r, 48)), (0, 1, 2), (0, 1, 2)];
    let strips = if pid == 0x5711 { 4 } else if pid == 0xA100 { 1 } else { 2 };
    if pid == 0x5711 {
        let _ = Fusion2Usb::cc(&dev, 0x61, 0, 0);
        let mut c = [0u8; 64];
        c[0] = RID;
        if dev.get_feature_report(&mut c).unwrap_or(0) >= 20 {
            order[2] = decode_order(u32le(&c, 4));
            order[3] = decode_order(u32le(&c, 8));
        }
    }
    let flags = r[11];
    let mut zones = Vec::new();
    let has_fixed = pid != 0xA100;
    if has_fixed {
        zones.push(Zone::single("Carte mère"));
    }
    for s in 0..strips {
        zones.push(Zone::resizable(format!("ARGB {}", s + 1), 20, 1, 1024));
    }
    let modes = vec![
        ModeInfo::new("Statique").colors(1, 1).brightness(),
        ModeInfo::new("Respiration").colors(1, 1).brightness().speed().random(),
        ModeInfo::new("Flash").colors(1, 1).brightness().speed().random(),
        ModeInfo::new("Double flash").colors(1, 1).brightness().speed().random(),
        ModeInfo::new("Cycle de couleurs").brightness().speed(),
        ModeInfo::new("Vague").brightness().speed(),
        ModeInfo::new("Aléatoire").brightness(),
        ModeInfo::new("Éteint").off(),
    ];
    let (_, board) = super::smbus::board();
    let name = if pid == 0xA100 {
        "Gigabyte GC-USB".to_string()
    } else if board.is_empty() {
        "Gigabyte RGB Fusion 2".to_string()
    } else {
        format!("Gigabyte {}", pretty_name(&board))
    };
    let d = Fusion2Usb {
        info: DeviceInfo {
            id: hid::stable_id("fusion2", i),
            name,
            vendor: "Gigabyte".into(),
            kind: DeviceKind::Motherboard,
            location: hid::location(i),
            zones,
            modes,
            direct: true,
        },
        dev,
        pid,
        order,
        strips,
        has_fixed,
        builtin_off: None,
        counts: [r[8] & 0x0F, r[8] >> 4, r[9] & 0x0F, r[9] >> 4, r[10] & 0x0F, r[10] >> 4],
    };
    if flags & 0x02 != 0 {
        let _ = Fusion2Usb::cc(&d.dev, 0x48, 0, 0); // désactive LampArray
    }
    d.reset();
    let _ = Fusion2Usb::cc(&d.dev, 0x31, 0, 0); // désactive le mode « beat »
    Ok(vec![Box::new(d)])
}

impl Fusion2Usb {
    fn cc(dev: &HidDevice, a: u8, b: u8, c: u8) -> Result<()> {
        let mut p = [0u8; 64];
        p[0] = RID;
        p[1] = a;
        p[2] = b;
        p[3] = c;
        dev.send_feature_report(&p)?;
        Ok(())
    }

    fn reset(&self) {
        for reg in 0x20..=0x27 {
            let _ = Self::cc(&self.dev, reg, 0, 0);
        }
        if self.pid == 0x5711 {
            for reg in 0x90..=0x92 {
                let _ = Self::cc(&self.dev, reg, 0, 0);
            }
        }
        self.apply_fast();
    }

    fn apply_fast(&self) {
        let _ = Self::cc(&self.dev, 0x28, 0xFF, if self.pid == 0x5711 { 0x07 } else { 0x00 });
    }

    fn set_builtin(&mut self, disabled_mask: u8) -> Result<()> {
        if self.builtin_off != Some(disabled_mask) {
            Self::cc(&self.dev, 0x32, disabled_mask, 0)?;
            std::thread::sleep(std::time::Duration::from_millis(50));
            self.builtin_off = Some(disabled_mask);
        }
        Ok(())
    }

    /// Effet sur toutes les LED fixes de la carte.
    fn effect(&self, mode: u8, speed: u32, brightness: u8, random: bool, c: Color) -> Result<()> {
        let mut p = [0u8; 64];
        p[0] = RID;
        p[1] = 0x20;
        let zone0: u32 = if self.pid == 0x5711 { 0x07FF } else { 0xFF };
        p[2..6].copy_from_slice(&zone0.to_le_bytes());
        p[11] = mode;
        p[12] = brightness;
        p[14..18].copy_from_slice(&[c.b, c.g, c.r, 0]);
        p[30] = if random { 7 } else { 0 };
        let mut per = [0u16; 4];
        match mode {
            2 => {
                per[0] = if speed <= 6 { 400 + speed as u16 * 100 } else { 1000 + (speed as u16 - 6) * 200 };
                per[1] = per[0];
                per[2] = 200;
            }
            3 | 15 => {
                if mode == 15 {
                    p[11] = 3;
                    p[31] = 1;
                    p[32] = 2;
                }
                per[0] = 100;
                per[1] = 100;
                per[2] = speed as u16 * 200 + 700;
            }
            4 => {
                per[0] = (speed * 100 + 300 + if speed > 8 { 1300 * (speed - 8) } else { 0 }) as u16;
                per[1] = per[0] - 200;
                p[30] = 7;
            }
            6 => {
                per[0] = (((speed + 1) * (speed + 1) + (speed + 1) + 10) * 5 / 2) as u16;
                p[30] = 7;
                p[31] = 1;
            }
            8 => {
                per[0] = 100;
                p[30] = 1;
                p[31] = 5;
            }
            _ => {}
        }
        for (k, v) in per.iter().enumerate() {
            p[22 + k * 2..24 + k * 2].copy_from_slice(&v.to_le_bytes());
        }
        self.dev.send_feature_report(&p)?;
        // application sur les zones concernées
        let mut a = [0u8; 64];
        a[0] = RID;
        a[1] = 0x28;
        a[2..6].copy_from_slice(&zone0.to_le_bytes());
        self.dev.send_feature_report(&a)?;
        Ok(())
    }

    fn strip(&self, s: usize, colors: &[Color]) -> Result<()> {
        let (ro, go, bo) = self.order[s];
        let mut offset = 0u16;
        for chunk in colors.chunks(19) {
            let mut p = [0u8; 64];
            p[0] = RID;
            p[1] = STRIP_HEADERS[s];
            p[2..4].copy_from_slice(&offset.to_le_bytes());
            p[4] = (chunk.len() * 3) as u8;
            for (i, c) in chunk.iter().enumerate() {
                let o = 5 + i * 3;
                p[o + ro] = c.r;
                p[o + go] = c.g;
                p[o + bo] = c.b;
            }
            self.dev.send_feature_report(&p)?;
            offset += (chunk.len() * 3) as u16;
        }
        Ok(())
    }

    fn update_counts(&mut self) {
        let enc = |n: usize| -> u8 {
            match n {
                0..=32 => 0,
                33..=64 => 1,
                65..=256 => 2,
                257..=512 => 3,
                _ => 4,
            }
        };
        let first = self.has_fixed as usize;
        let mut c = self.counts;
        for s in 0..self.strips {
            c[s] = enc(self.info.zones[first + s].leds.len());
        }
        if c != self.counts {
            self.counts = c;
            let mut p = [0u8; 64];
            p[0] = RID;
            p[1] = 0x34;
            p[2] = (c[1] << 4) | c[0];
            p[3] = (c[3] << 4) | c[2];
            p[4] = (c[5] << 4) | c[4];
            let _ = self.dev.send_feature_report(&p);
        }
    }
}

impl Device for Fusion2Usb {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, mode: usize, p: &HwParams) -> Result<()> {
        // effets intégrés : on réactive le moteur interne des connecteurs ARGB
        self.set_builtin(0)?;
        let m = &self.info.modes[mode];
        let code = match m.name.as_str() {
            "Statique" => 1,
            "Respiration" => 2,
            "Flash" => 3,
            "Double flash" => 15,
            "Cycle de couleurs" => 4,
            "Vague" => 6,
            "Aléatoire" => 8,
            _ => 1,
        };
        let (c, bright) = if m.is_off { (Color::BLACK, 0) } else { (p.color(0), p.brightness_in(0, 255) as u8) };
        self.effect(code, p.speed_in(9, 0), bright, p.random && m.random, c)
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        let mask: u8 = STRIP_MASKS[..self.strips].iter().fold(0, |a, b| a | b);
        self.set_builtin(mask)?;
        let mut off = 0;
        if self.has_fixed {
            self.effect(1, 0, 255, false, colors[0])?;
            off = 1;
        }
        for s in 0..self.strips {
            let n = self.info.zones[self.has_fixed as usize + s].leds.len();
            self.strip(s, &colors[off..off + n])?;
            off += n;
        }
        Ok(())
    }

    fn resize_zone(&mut self, zone: usize, leds: usize) -> Result<()> {
        if zone == 0 && self.has_fixed {
            return err("zone non redimensionnable");
        }
        let name = self.info.zones[zone].name.clone();
        self.info.zones[zone] = Zone::resizable(name, leds.clamp(1, 1024), 1, 1024);
        self.update_counts();
        Ok(())
    }

    fn max_fps(&self) -> u32 {
        30
    }
}
