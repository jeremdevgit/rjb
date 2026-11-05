// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Corsair Lighting Node Core / Pro, Commander Pro, LS100, LT100, 1000D,
//! Spec Omega : canaux de ventilateurs et bandes LED adressables.
//! Protocole d'après OpenRGB.

use super::hid::{self, HidEntry};
use crate::device::{Device, Result};
use crate::model::*;
use hidapi::{DeviceInfo as HidInfo, HidApi, HidDevice};
use std::time::Duration;

const ONE_CHANNEL: &[u16] = &[0x0C1A, 0x0C1E];

pub const HID: &[HidEntry] = &[HidEntry {
    vid: 0x1B1C,
    pids: &[0x0C1A, 0x0C0B, 0x0C10, 0x0C1E, 0x1D00, 0x1D04, 0x0C23],
    usage_page: None,
    usage: None,
    interface: None,
    open,
}];

pub struct LightingNode {
    info: DeviceInfo,
    dev: HidDevice,
    software: [bool; 2],
}

fn open(api: &HidApi, i: &HidInfo) -> Result<Vec<Box<dyn Device>>> {
    let dev = api.open_path(i.path())?;
    let channels = if ONE_CHANNEL.contains(&i.product_id()) { 1 } else { 2 };
    let zones = (0..channels)
        .map(|c| Zone::resizable(format!("Canal {}", c + 1), 24, 0, 204))
        .collect();
    let modes = vec![
        ModeInfo::new("Statique").colors(1, 1),
        ModeInfo::new("Vague arc-en-ciel").speed().dirs(&[Direction::Right, Direction::Left]),
        ModeInfo::new("Arc-en-ciel").speed(),
        ModeInfo::new("Changement de couleur").speed().colors(2, 2).random(),
        ModeInfo::new("Pulsation").speed().colors(2, 2).random(),
        ModeInfo::new("Vague de couleurs").speed().colors(2, 2).random().dirs(&[Direction::Right, Direction::Left]),
        ModeInfo::new("Visière").speed().colors(2, 2).random(),
        ModeInfo::new("Défilement").speed().colors(1, 1),
        ModeInfo::new("Clignotement").speed().colors(2, 2).random(),
        ModeInfo::new("Séquentiel").speed().colors(1, 1).random().dirs(&[Direction::Right, Direction::Left]),
        ModeInfo::new("Éteint").off(),
    ];
    let d = LightingNode {
        info: DeviceInfo {
            id: hid::stable_id("corsair-ln", i),
            name: pretty_name(&hid::product(i, "Corsair Lighting Node")),
            vendor: "Corsair".into(),
            kind: if i.product_id() == 0x0C10 { DeviceKind::FanController } else { DeviceKind::LedStrip },
            location: hid::location(i),
            zones,
            modes,
            direct: true,
        },
        dev,
        software: [false; 2],
    };
    d.cmd(&[0x02]); // version du firmware : réveille le contrôleur
    Ok(vec![Box::new(d)])
}

impl LightingNode {
    fn cmd(&self, data: &[u8]) -> Option<[u8; 17]> {
        let mut b = [0u8; 65];
        b[1..1 + data.len()].copy_from_slice(data);
        self.dev.write(&b).ok()?;
        let mut r = [0u8; 17];
        let _ = self.dev.read_timeout(&mut r, 15);
        Some(r)
    }

    fn commit(&self) {
        self.cmd(&[0x33, 0xFF]);
    }
}

impl Device for LightingNode {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, mode: usize, p: &HwParams) -> Result<()> {
        let m = self.info.modes[mode].clone();
        let code = match m.name.as_str() {
            "Vague arc-en-ciel" => 0x00,
            "Changement de couleur" => 0x01,
            "Pulsation" => 0x02,
            "Vague de couleurs" => 0x03,
            "Visière" => 0x06,
            "Défilement" => 0x07,
            "Clignotement" => 0x08,
            "Séquentiel" => 0x09,
            "Arc-en-ciel" => 0x0A,
            _ => 0x04,
        };
        let (c1, c2, c3) = if m.is_off { (Color::BLACK, Color::BLACK, Color::BLACK) } else { (p.color(0), p.color(1), p.color(2)) };
        let speed = p.speed_in(2, 0) as u8;
        let dir = (p.direction != Direction::Left) as u8;
        for ch in 0..self.info.zones.len() {
            let n = self.info.zones[ch].leds.len() as u8;
            self.software[ch] = false;
            self.cmd(&[0x37, ch as u8]); // réinitialisation
            self.cmd(&[0x34, ch as u8]); // début d'effet
            self.cmd(&[0x38, ch as u8, 0x01]); // contrôle matériel
            let cfg = [
                0x35, ch as u8, 0, n, code, speed, dir, (m.random && p.random) as u8, 0,
                c1.r, c1.g, c1.b, c2.r, c2.g, c2.b, c3.r, c3.g, c3.b,
            ];
            self.cmd(&cfg);
        }
        self.commit();
        Ok(())
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        let mut off = 0;
        for ch in 0..self.info.zones.len() {
            let n = self.info.zones[ch].leds.len();
            if !self.software[ch] {
                self.cmd(&[0x38, ch as u8, 0x02]);
                self.software[ch] = true;
            }
            let zone = &colors[off..off + n];
            for (start, chunk) in zone.chunks(50).enumerate() {
                let s = (start * 50) as u8;
                for (plane, get) in [(0u8, 0usize), (1, 1), (2, 2)] {
                    let mut p = vec![0x32, ch as u8, s, chunk.len() as u8, plane];
                    p.extend(chunk.iter().map(|c| [c.r, c.g, c.b][get]));
                    self.cmd(&p);
                }
            }
            off += n;
        }
        self.commit();
        Ok(())
    }

    fn resize_zone(&mut self, zone: usize, leds: usize) -> Result<()> {
        let name = self.info.zones[zone].name.clone();
        self.info.zones[zone] = Zone::resizable(name, leds.min(204), 0, 204);
        Ok(())
    }

    fn keepalive(&self) -> Option<Duration> {
        Some(Duration::from_secs(4))
    }

    fn max_fps(&self) -> u32 {
        30
    }
}
