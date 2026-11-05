// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Cartes mères ASRock Polychrome v2 (SMBus, firmware 3.x : B450 / X470 /
//! B550 / X570 / Z390…). Le nombre de LED de chaque zone est lu dans la carte.
//! Protocole d'après OpenRGB.

use super::smbus::Bus;
use crate::device::{Device, Result};
use crate::model::*;
use std::sync::Arc;
use std::time::Duration;

const ADDR: u8 = 0x6A;
const REG_MODE: u8 = 0x30;
const REG_LED_SELECT: u8 = 0x31;
const REG_LED_COUNT: u8 = 0x32;
const REG_LED_CONFIG: u8 = 0x33;
const REG_COLOR: u8 = 0x34;
const STATIC: u8 = 0x11;
const OFF: u8 = 0x10;

const ZONES: &[&str] = &["Connecteur RGB 1", "Connecteur RGB 2", "Audio", "PCH", "Cache E/S", "Connecteur ARGB"];

/// (nom, code, vitesse lente, vitesse rapide, couleur choisie)
const MODES: &[(&str, u8, u8, u8, bool)] = &[
    ("Statique", STATIC, 0, 0, true),
    ("Respiration", 0x12, 0x0A, 0x02, true),
    ("Stroboscope", 0x13, 0xA0, 0x05, true),
    ("Cycle de couleurs", 0x14, 0xA0, 0x0A, false),
    ("Aléatoire", 0x15, 0xA0, 0x05, false),
    ("Vague", 0x17, 0x06, 0x01, false),
    ("Ressort", 0x18, 0x20, 0x02, false),
    ("Empilement", 0x19, 0x20, 0x02, false),
    ("Remplissage", 0x1A, 0x20, 0x02, false),
    ("Balayage", 0x1B, 0x20, 0x02, false),
    ("Néon", 0x1C, 0x20, 0x02, false),
    ("Eau", 0x1D, 0x20, 0x02, false),
    ("Arc-en-ciel", 0x1E, 0x12, 0x02, false),
];

pub fn detect(bus: &Arc<Bus>, board: &str, out: &mut Vec<Box<dyn Device>>) {
    let (_, _, sub_vendor, _) = bus.pci;
    if sub_vendor != 0x1849 || bus.quick(ADDR).is_err() {
        return;
    }
    // version du firmware sur deux octets : 3.x = Polychrome v2
    match bus.read_block_data(ADDR, 0x00) {
        Ok(v) if v.len() == 2 && v[0] == 3 => {}
        _ => return,
    }
    let counts = match bus.read_block_data(ADDR, REG_LED_CONFIG) {
        Ok(c) if c.len() == 6 => c,
        _ => return,
    };
    let mut zones = Vec::new();
    for (z, &n) in counts.iter().enumerate() {
        if n > 0 {
            // le connecteur adressable ne se pilote qu'en couleur unique
            let n = if z == 5 { 1 } else { n as usize };
            zones.push(Zone::linear(ZONES[z], n));
        }
    }
    if zones.is_empty() {
        return;
    }
    let mut modes: Vec<ModeInfo> = MODES
        .iter()
        .map(|m| {
            let mut i = ModeInfo::new(m.0);
            if m.4 {
                i = i.colors(1, 1);
            }
            if m.2 != m.3 {
                i = i.speed();
            }
            i
        })
        .collect();
    modes.push(ModeInfo::new("Éteint").off());
    out.push(Box::new(Polychrome {
        info: DeviceInfo {
            id: format!("asrock-poly-{}", bus.name),
            name: format!("ASRock {}", pretty_name(board)),
            vendor: "ASRock".into(),
            kind: DeviceKind::Motherboard,
            location: bus.location(ADDR),
            zones,
            modes,
            direct: true,
        },
        bus: bus.clone(),
        mode: None,
        speed: 0,
        last: Vec::new(),
    }));
}

pub struct Polychrome {
    info: DeviceInfo,
    bus: Arc<Bus>,
    mode: Option<u8>,
    speed: u8,
    last: Vec<Color>,
}

fn pause() {
    std::thread::sleep(Duration::from_millis(1));
}

impl Polychrome {
    fn write_mode(&mut self, mode: u8, speed: u8) -> Result<()> {
        self.bus.write_block_data(ADDR, REG_MODE, &[mode])?;
        pause();
        self.bus.write_block_data(ADDR, REG_LED_COUNT, &[])?;
        pause();
        if mode != OFF && mode != STATIC {
            self.bus.write_block_data(ADDR, mode, &[speed])?;
            pause();
        }
        self.mode = Some(mode);
        self.speed = speed;
        self.last.clear();
        Ok(())
    }

    fn write_color(&self, led: usize, c: Color) -> Result<()> {
        self.bus.write_block_data(ADDR, REG_LED_SELECT, &[led as u8])?;
        pause();
        self.bus.write_block_data(ADDR, REG_COLOR, &[c.r, c.g, c.b])?;
        pause();
        Ok(())
    }
}

impl Device for Polychrome {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, mode: usize, p: &HwParams) -> Result<()> {
        let m = self.info.modes[mode].clone();
        if m.is_off {
            return self.write_mode(OFF, 0);
        }
        let (_, code, slow, fast, colored) = MODES[mode.min(MODES.len() - 1)];
        let t = p.speed_in(0, 100) as i32;
        let speed = (slow as i32 + (fast as i32 - slow as i32) * t / 100) as u8;
        self.write_mode(code, speed)?;
        if colored {
            let n: usize = self.info.zones.iter().map(|z| z.leds.len()).sum();
            for led in 0..n {
                self.write_color(led, p.color(0))?;
            }
        }
        Ok(())
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        if self.mode != Some(STATIC) {
            self.write_mode(STATIC, 0)?;
        }
        // chaque LED coûte deux transactions : on n'envoie que ce qui change
        for (led, &c) in colors.iter().enumerate() {
            if self.last.get(led) != Some(&c) {
                self.write_color(led, c)?;
            }
        }
        self.last = colors.to_vec();
        Ok(())
    }

    fn max_fps(&self) -> u32 {
        15
    }
}
