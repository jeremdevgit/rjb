// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! RAM HyperX Predator RGB / Fury RGB DDR4 (contrôleur unique en 0x27,
//! identifiée par le SPD Kingston). Protocole d'après OpenRGB.

use super::smbus::Bus;
use super::spd::Slot;
use crate::device::{Device, Result};
use crate::model::*;
use std::sync::Arc;

const DEV: u8 = 0x27;
const REG_APPLY: u8 = 0xE1;
const REG_MODE_RANDOM: u8 = 0xE3;
const REG_MODE_CUSTOM: u8 = 0xE4;
const REG_MODE_INDEPENDENT: u8 = 0xE5;
const SLOT_BASE: [u8; 4] = [0x11, 0x41, 0x71, 0xA1];
const SLOT_MAP: [usize; 4] = [0, 2, 1, 3];

/// (nom, valeur, registre aléatoire possible, vitesse lente, rapide)
const MODES: &[(&str, u8, bool, u16, u16)] = &[
    ("Statique", 0x09, false, 0, 0),
    ("Arc-en-ciel", 0x05, false, 0x07D0, 0x0064),
    ("Comète", 0x08, true, 0x07D0, 0x0064),
    ("Battement de cœur", 0x07, true, 0x07D0, 0x01F4),
    ("Cycle de couleurs", 0x04, false, 0x05DC, 0x00FA),
    ("Respiration", 0x03, true, 0x07D0, 0x0064),
    ("Rebond", 0x02, true, 0x07D0, 0x0064),
    ("Clignotement", 0x06, true, 0x07D0, 0x01F4),
];

pub struct HyperXDram {
    info: DeviceInfo,
    bus: Arc<Bus>,
    slots: Vec<usize>,
    direct: bool,
}

pub fn detect(bus: &Arc<Bus>, spd: &[Slot], out: &mut Vec<Box<dyn Device>>) {
    let kingston: Vec<&Slot> = spd.iter().filter(|s| !s.ddr5 && s.jedec == super::spd::JEDEC_KINGSTON).collect();
    if kingston.is_empty() || bus.read_byte(DEV).is_err() {
        return;
    }
    let mut valid = 0u8;
    let (mut fury, mut predator) = (false, false);
    for s in &kingston {
        valid |= 1 << s.index;
        if s.manuf.get(6) == Some(&0x01) {
            fury = true;
        } else {
            predator = true;
        }
    }
    let slots: Vec<usize> = SLOT_MAP
        .iter()
        .copied()
        .filter(|&k| valid & (1 << k) != 0 || valid & (0x10 << k) != 0)
        .collect();
    let name = match (fury, predator) {
        (true, false) => "HyperX Fury RGB",
        (false, true) => "HyperX Predator RGB",
        _ => "HyperX RGB DRAM",
    };
    let mut modes: Vec<ModeInfo> = MODES
        .iter()
        .map(|m| {
            let mut i = ModeInfo::new(m.0);
            if m.3 != m.4 {
                i = i.speed();
            }
            if m.1 != 0x05 && m.1 != 0x04 {
                i = i.colors(1, 1);
            }
            if m.2 {
                i = i.random();
            }
            i
        })
        .collect();
    modes.push(ModeInfo::new("Éteint").off());
    let zones = (0..slots.len()).map(|i| Zone::linear(format!("Barrette {}", i + 1), 5)).collect();
    out.push(Box::new(HyperXDram {
        info: DeviceInfo {
            id: format!("hyperx-dram:{}", bus.name),
            name: name.into(),
            vendor: "HyperX".into(),
            kind: DeviceKind::Ram,
            location: bus.location(DEV),
            zones,
            modes,
            direct: true,
        },
        bus: bus.clone(),
        slots,
        direct: false,
    }));
}

impl HyperXDram {
    fn w(&self, reg: u8, v: u8) -> Result<()> {
        self.bus.write_byte_data(DEV, reg, v)
    }

    fn apply(&self) -> Result<()> {
        self.w(REG_APPLY, 0x02)?;
        self.w(REG_APPLY, 0x03)
    }

    fn led(&self, slot: usize, led: usize, c: Color) -> Result<()> {
        let base = SLOT_BASE[slot] + (3 * led) as u8;
        self.w(base, c.r)?;
        self.w(base + 1, c.g)?;
        self.w(base + 2, c.b)?;
        self.w(base + 0x10, 0x64)
    }
}

impl Device for HyperXDram {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, mode: usize, p: &HwParams) -> Result<()> {
        self.direct = false;
        if self.info.modes[mode].is_off {
            let n = self.info.led_count();
            return self.set_leds(&vec![Color::BLACK; n]);
        }
        let (_, value, random_ok, slow, fast) = MODES[mode];
        let speed = p.speed_in(slow as u32, fast as u32) as u16;
        let (hi, lo) = ((speed >> 8) as u8, speed as u8);
        let c = p.color(0);
        // couleur de l'effet
        self.w(REG_APPLY, 0x01)?;
        self.w(0xEC, c.r)?;
        self.w(0xED, c.g)?;
        self.w(0xEE, c.b)?;
        self.w(0xDD, 0x64)?;
        self.apply()?;
        self.w(REG_APPLY, 0x01)?;
        let reg = match value {
            0x05 | 0x04 => REG_MODE_RANDOM,
            _ if random_ok && p.random => REG_MODE_RANDOM,
            _ => REG_MODE_CUSTOM,
        };
        self.w(reg, value)?;
        match value {
            0x05 | 0x08 | 0x02 => {
                self.w(0xD1, hi)?;
                self.w(0xD2, lo)?;
            }
            0x07 => {
                for r in [0xDB, 0xD3] {
                    self.w(r, hi)?;
                    self.w(r + 1, lo)?;
                }
                self.w(0xEA, 0x03)?;
                self.w(0xEB, 0xE8)?;
            }
            0x04 => {
                for r in [0xD3, 0xD5] {
                    self.w(r, hi)?;
                    self.w(r + 1, lo)?;
                }
            }
            0x03 => {
                for r in [0xD7, 0xD9] {
                    self.w(r, hi)?;
                    self.w(r + 1, lo)?;
                }
                self.w(0xDB, 0)?;
                self.w(0xDC, 0)?;
            }
            0x06 => {
                self.w(0xDB, hi)?;
                self.w(0xDC, lo)?;
                self.w(0xD3, 0x07)?;
                self.w(0xD4, 0xD4)?;
            }
            _ => {}
        }
        self.apply()
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        self.w(REG_APPLY, 0x01)?;
        if !self.direct {
            self.w(REG_MODE_INDEPENDENT, 0x21)?;
            self.direct = true;
        }
        for (si, &slot) in self.slots.iter().enumerate() {
            for led in 0..5 {
                let c = colors.get(si * 5 + led).copied().unwrap_or(Color::BLACK);
                self.led(slot, led, c)?;
            }
        }
        self.apply()
    }

    fn max_fps(&self) -> u32 {
        20
    }
}
