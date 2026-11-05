// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Cartes mères Gigabyte RGB Fusion 2 pilotées par SMBus (B450 / X470 / Z370…,
//! antérieures aux modèles USB). L'adresse 0x68 est partagée avec d'autres
//! puces sur certaines cartes, la détection est donc limitée à la liste connue.
//! Protocole d'après OpenRGB.

use super::smbus::Bus;
use crate::device::{Device, Result};
use crate::model::*;
use std::sync::Arc;

const ADDR: u8 = 0x68;

const BOARDS: &[&str] = &[
    "B450 AORUS ELITE",
    "B450 AORUS ELITE V2",
    "B450 AORUS M",
    "B450 AORUS PRO WIFI-CF",
    "B450 AORUS PRO-CF",
    "B450 AORUS PRO-CF4",
    "B450 I AORUS PRO WIFI-CF",
    "B450M DS3H-CF",
    "X299 DESIGNARE EX-CF",
    "X399 AORUS XTREME-CF",
    "X399 DESIGNARE EX-CF",
    "X470 AORUS GAMING 5 WIFI",
    "X470 AORUS GAMING 5 WIFI-CF",
    "X470 AORUS GAMING 7 WIFI-CF",
    "X470 AORUS GAMING 7 WIFI-50-CF",
    "X470 AORUS ULTRA GAMING",
    "X470 AORUS ULTRA GAMING-CF",
    "B360M AORUS Gaming 3-CF",
    "B360 AORUS GAMING 3 WIFI-CF",
    "Z370 AORUS Gaming 5-CF",
    "Z370 AORUS Ultra Gaming-CF",
];

/// Emplacements utilisés parmi les 10 du protocole.
const SLOTS: &[(usize, &str)] = &[
    (0, "Processeur"),
    (2, "Logo"),
    (3, "Arrière du boîtier"),
    (4, "Boîtier"),
    (7, "Connecteur ARGB 1"),
    (8, "Connecteur ARGB 2"),
];

const STATIC: u8 = 0x04;

/// (nom, code, vitesse lente → rapide, luminosité réglable)
const MODES: &[(&str, u8, u8, u8, bool)] = &[
    ("Statique", STATIC, 0, 0, false),
    ("Pulsation", 0x01, 0x04, 0x01, false),
    ("Clignotement", 0x05, 0x01, 0x04, false),
    ("Cycle de couleurs", 0x03, 0x04, 0x01, true),
    ("Vague", 0x0A, 0xFF, 0x49, true),
];

pub fn detect(bus: &Arc<Bus>, manufacturer: &str, board: &str, out: &mut Vec<Box<dyn Device>>) {
    let (_, _, sub_vendor, _) = bus.pci;
    if !manufacturer.starts_with("Gigabyte") || sub_vendor != 0x1458 || !BOARDS.iter().any(|b| b.eq_ignore_ascii_case(board)) {
        return;
    }
    if bus.quick(ADDR).is_err() {
        return;
    }
    let mut modes: Vec<ModeInfo> = MODES
        .iter()
        .map(|m| {
            let mut i = ModeInfo::new(m.0).colors(1, 1);
            if m.2 != m.3 {
                i = i.speed();
            }
            if m.4 {
                i = i.brightness();
            }
            i
        })
        .collect();
    modes.push(ModeInfo::new("Éteint").off());
    out.push(Box::new(Fusion2Smbus {
        info: DeviceInfo {
            id: format!("gb-fusion2-smbus-{}", bus.name),
            name: format!("Gigabyte {}", pretty_name(board)),
            vendor: "Gigabyte".into(),
            kind: DeviceKind::Motherboard,
            location: bus.location(ADDR),
            zones: SLOTS.iter().map(|s| Zone::single(s.1)).collect(),
            modes,
            direct: true,
        },
        bus: bus.clone(),
        data: [[0; 16]; 10],
    }));
}

pub struct Fusion2Smbus {
    info: DeviceInfo,
    bus: Arc<Bus>,
    data: [[u8; 16]; 10],
}

impl Fusion2Smbus {
    fn set_slot(&mut self, slot: usize, mode: u8, speed: u8, bright: u8, c: Color) {
        let d = &mut self.data[slot];
        *d = [0; 16];
        d[1] = mode;
        d[2] = 0x64;
        d[4] = c.b;
        d[5] = c.g;
        d[6] = c.r;
        match mode {
            0x01 => {
                d[8] = 0x20u8.wrapping_mul(speed);
                d[9] = 0x03 * speed;
                d[10] = d[8];
                d[11] = d[9];
            }
            0x03 => {
                d[9] = 0x03 * speed;
                d[14] = 0x07;
                d[2] = bright;
            }
            0x05 => {
                d[8] = 0x64;
                d[10] = 0xC8;
                d[12] = 0xD0;
                d[13] = 0x07;
                d[14] = speed;
            }
            0x0A => {
                d[8] = speed;
                d[2] = bright;
            }
            _ => {}
        }
    }

    /// Les registres s'écrivent par paires d'emplacements (32 octets), puis on valide.
    fn flush(&self) -> Result<()> {
        for pair in 0..5 {
            let mut b = [0u8; 32];
            b[..16].copy_from_slice(&self.data[pair * 2]);
            b[16..].copy_from_slice(&self.data[pair * 2 + 1]);
            self.bus.write_block_data(ADDR, 0x20 + 2 * pair as u8, &b)?;
        }
        self.bus.write_word_data(ADDR, 0x17, 0x01FF)
    }
}

impl Device for Fusion2Smbus {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, mode: usize, p: &HwParams) -> Result<()> {
        let m = self.info.modes[mode].clone();
        let (_, code, slow, fast, _) = if m.is_off { MODES[0] } else { MODES[mode.min(MODES.len() - 1)] };
        let c = if m.is_off { Color::BLACK } else { p.color(0) };
        let t = p.speed_in(0, 100) as i32;
        let speed = (slow as i32 + (fast as i32 - slow as i32) * t / 100) as u8;
        let bright = 0x0F + (p.brightness_in(0, 100) as u32 * (0x64 - 0x0F) / 100) as u8;
        for &(slot, _) in SLOTS {
            self.set_slot(slot, code, speed, bright, c);
        }
        self.flush()
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        for (k, &(slot, _)) in SLOTS.iter().enumerate() {
            self.set_slot(slot, STATIC, 0, 0x64, colors.get(k).copied().unwrap_or(Color::BLACK));
        }
        self.flush()
    }

    fn max_fps(&self) -> u32 {
        20
    }
}
