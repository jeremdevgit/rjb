// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! RAM Kingston FURY Beast / Renegade RGB (DDR4 et DDR5). Toutes les barrettes
//! forment un seul appareil synchronisé. Protocole d'après OpenRGB.

use super::smbus::Bus;
use super::spd::Slot;
use crate::device::{err, Device, Result};
use crate::model::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

const DELAY: Duration = Duration::from_millis(10);
const REG_MODEL: u8 = 0x06;
const REG_APPLY: u8 = 0x08;
const REG_MODE: u8 = 0x09;
const REG_INDEX: u8 = 0x0B;
const REG_DIRECTION: u8 = 0x0C;
const REG_DELAY: u8 = 0x0D;
const REG_SPEED: u8 = 0x0E;
const REG_BRIGHTNESS: u8 = 0x20;
const REG_BG: u8 = 0x23;
const REG_LENGTH: u8 = 0x26;
const REG_NUM_SLOTS: u8 = 0x27;
const REG_NUM_COLORS: u8 = 0x30;
const REG_MODE_COLORS: u8 = 0x31;
const REG_LED_COLORS: u8 = 0x50;
const BEGIN: u8 = 0x53;
const END: u8 = 0x44;
const MODE_DIRECT: u8 = 0x10;

/// Effet matériel. Plusieurs effets partagent un code et ne diffèrent que par
/// le délai et la longueur (comme dans le logiciel Kingston).
struct M {
    name: &'static str,
    code: u8,
    slow: u32,
    fast: u32,
    colors: (u8, u8),
    dirs: bool,
    length: Option<u8>,
    delay: u8,
    background: bool,
}

const MODES: &[M] = &[
    M { name: "Statique", code: 0x00, slow: 0, fast: 0, colors: (1, 1), dirs: false, length: None, delay: 0, background: false },
    M { name: "Arc-en-ciel", code: 0x01, slow: 60, fast: 0, colors: (0, 0), dirs: true, length: None, delay: 0, background: false },
    M { name: "Cycle de couleurs", code: 0x01, slow: 60, fast: 0, colors: (0, 0), dirs: true, length: None, delay: 4, background: false },
    M { name: "Respiration", code: 0x03, slow: 10, fast: 1, colors: (1, 8), dirs: false, length: None, delay: 0, background: false },
    M { name: "Dynamique", code: 0x04, slow: 1000, fast: 100, colors: (1, 8), dirs: false, length: None, delay: 0, background: false },
    M { name: "Glissement", code: 0x05, slow: 255, fast: 0, colors: (1, 8), dirs: true, length: Some(4), delay: 3, background: true },
    M { name: "Vent", code: 0x05, slow: 255, fast: 0, colors: (1, 8), dirs: true, length: Some(12), delay: 0, background: true },
    M { name: "Comète", code: 0x06, slow: 255, fast: 0, colors: (1, 8), dirs: true, length: Some(7), delay: 0, background: false },
    M { name: "Pluie", code: 0x06, slow: 28, fast: 8, colors: (1, 8), dirs: true, length: Some(3), delay: 0, background: false },
    M { name: "Feu", code: 0x09, slow: 64, fast: 40, colors: (0, 0), dirs: true, length: None, delay: 0, background: false },
    M { name: "Crépuscule", code: 0x0A, slow: 255, fast: 0, colors: (0, 0), dirs: false, length: None, delay: 0, background: false },
    M { name: "Fury", code: 0x0B, slow: 255, fast: 0, colors: (1, 8), dirs: true, length: None, delay: 0, background: true },
    M { name: "Prisme", code: 0x11, slow: 60, fast: 0, colors: (0, 0), dirs: false, length: None, delay: 2, background: false },
];

pub struct KingstonFury {
    info: DeviceInfo,
    bus: Arc<Bus>,
    addrs: Vec<u8>,
    per_dimm: usize,
    cache: Vec<HashMap<u8, u8>>,
    mode: Option<u8>,
}

fn signature(bus: &Bus, addr: u8, ddr5: bool) -> bool {
    if bus.write_byte_data(addr, REG_APPLY, BEGIN).is_err() {
        return false;
    }
    sleep(DELAY);
    let mut ok = true;
    for (i, ch) in b"FURY".iter().enumerate() {
        match bus.read_word_data(addr, i as u8 + 1) {
            Ok(w) if (w >> 8) as u8 == *ch => {}
            _ => {
                ok = false;
                break;
            }
        }
        sleep(DELAY);
    }
    if ok {
        let model = bus.read_word_data(addr, REG_MODEL).map(|w| (w >> 8) as u8).unwrap_or(0);
        ok = if ddr5 {
            matches!(model, 0x10 | 0x11 | 0x12 | 0x15)
        } else {
            matches!(model, 0x21 | 0x23)
        };
    }
    let _ = bus.write_byte_data(addr, REG_APPLY, END);
    sleep(DELAY);
    ok
}

/// Modèles DDR5 reconnus directement dans le SPD (variante, modèle).
fn ddr5_spd_ok(s: &Slot) -> bool {
    let (variant, model) = (s.manuf.get(0x18).copied().unwrap_or(0xFF), s.manuf.get(0x19).copied().unwrap_or(0));
    match variant {
        0x00 => matches!(model, 0x11 | 0x13 | 0x15),
        0x01 => matches!(model, 0x10 | 0x13 | 0x15),
        0x02 | 0x03 => matches!(model, 0x12 | 0x13 | 0x15),
        _ => false,
    }
}

/// N'interroge que les barrettes Kingston identifiées par leur SPD.
/// `taken` : adresses déjà attribuées à d'autres pilotes (RAM Corsair…).
/// Retourne true si des barrettes DDR4 ont été trouvées.
pub fn detect(bus: &Arc<Bus>, spd: &[Slot], taken: &[String], out: &mut Vec<Box<dyn Device>>) -> bool {
    let mut ddr4_found = false;
    if !bus.is_dram() {
        return false;
    }
    for (base, ddr5) in [(0x58u8, false), (0x60u8, true)] {
        let addrs: Vec<u8> = spd
            .iter()
            .filter(|s| s.ddr5 == ddr5 && s.is_kingston())
            .filter(|s| !taken.contains(&bus.location(base + s.index)))
            .filter(|s| (ddr5 && ddr5_spd_ok(s)) || signature(bus, base + s.index, ddr5))
            .map(|s| base + s.index)
            .collect();
        ddr4_found |= !ddr5 && !addrs.is_empty();
        if addrs.is_empty() {
            continue;
        }
        crate::log(&format!("Kingston Fury {} : {addrs:02x?}", if ddr5 { "DDR5" } else { "DDR4" }));
        let per_dimm = if ddr5 { 12 } else { 10 };
        let (mut modes, _): (Vec<ModeInfo>, Vec<()>) = MODES
            .iter()
            .map(|m| {
                let mut i = ModeInfo::new(m.name).brightness();
                if m.slow != m.fast {
                    i = i.speed();
                }
                if m.colors.1 > 0 {
                    i = i.colors(m.colors.0, m.colors.1);
                }
                if m.dirs {
                    i = i.dirs(&[Direction::Up, Direction::Down]);
                }
                (i, ())
            })
            .unzip();
        modes.push(ModeInfo::new("Éteint").off());
        let zones = (0..addrs.len()).map(|i| Zone::linear(format!("Barrette {}", i + 1), per_dimm)).collect();
        out.push(Box::new(KingstonFury {
            info: DeviceInfo {
                id: format!("kingston-fury:{}:{base:02x}", bus.name),
                name: format!("Kingston FURY {}", if ddr5 { "DDR5" } else { "DDR4" }),
                vendor: "Kingston".into(),
                kind: DeviceKind::Ram,
                location: bus.location(base),
                zones,
                modes,
                direct: true,
            },
            bus: bus.clone(),
            cache: vec![HashMap::new(); addrs.len()],
            addrs,
            per_dimm,
            mode: None,
        }));
    }
    ddr4_found
}

impl KingstonFury {
    fn write(&mut self, slot: usize, reg: u8, v: u8) -> Result<bool> {
        if self.cache[slot].get(&reg) == Some(&v) {
            return Ok(false);
        }
        for attempt in 1..=3 {
            if self.bus.write_byte_data(self.addrs[slot], reg, v).is_ok() {
                self.cache[slot].insert(reg, v);
                return Ok(true);
            }
            sleep(DELAY * attempt);
        }
        err("barrette Kingston injoignable")
    }

    /// Écrit le même registre sur toutes les barrettes.
    fn set(&mut self, reg: u8, v: u8) -> Result<()> {
        for s in 0..self.addrs.len() {
            self.write(s, reg, v)?;
        }
        Ok(())
    }

    fn all(&self, reg: u8, v: u8) {
        for &a in &self.addrs {
            let _ = self.bus.write_byte_data(a, reg, v);
        }
        sleep(DELAY);
    }

    fn begin(&mut self, mode: u8) -> Result<()> {
        self.all(REG_APPLY, BEGIN);
        if self.mode != Some(mode) {
            // préambule au changement de mode
            for &a in &self.addrs {
                let _ = self.bus.write_byte_data(a, REG_INDEX, 0);
            }
            sleep(DELAY);
            self.all(REG_APPLY, END);
            self.all(REG_APPLY, BEGIN);
            self.mode = Some(mode);
        }
        self.set(REG_MODE, mode)
    }

    fn finish(&mut self, brightness: u8) -> Result<()> {
        self.set(REG_BRIGHTNESS, brightness)?;
        self.set(REG_NUM_SLOTS, self.addrs.len().min(4) as u8)?;
        self.all(REG_APPLY, END);
        Ok(())
    }

    fn mode_colors(&mut self, colors: &[Color]) -> Result<()> {
        self.set(REG_NUM_COLORS, colors.len() as u8)?;
        for (i, c) in colors.iter().enumerate() {
            let r = REG_MODE_COLORS + (i * 3) as u8;
            self.set(r, c.r)?;
            self.set(r + 1, c.g)?;
            self.set(r + 2, c.b)?;
        }
        Ok(())
    }
}

impl Device for KingstonFury {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, mode: usize, p: &HwParams) -> Result<()> {
        if self.info.modes[mode].is_off {
            let n = self.info.led_count();
            return self.set_leds(&vec![Color::BLACK; n]);
        }
        let m = &MODES[mode];
        self.begin(m.code)?;
        self.set(REG_DELAY, m.delay)?;
        if let Some(l) = m.length {
            self.set(REG_LENGTH, l)?;
        }
        let dir = if p.direction == Direction::Down { 2 } else { 1 };
        self.set(REG_DIRECTION, dir)?;
        if m.colors.1 > 0 {
            let n = p.colors.len().clamp(m.colors.0 as usize, m.colors.1 as usize);
            let cols: Vec<Color> = (0..n).map(|i| p.color(i)).collect();
            self.mode_colors(&cols)?;
            if m.background {
                for (i, v) in [16u8, 16, 16].into_iter().enumerate() {
                    self.set(REG_BG + i as u8, v)?;
                }
            }
        }
        if m.slow != m.fast {
            let s = p.speed_in(m.slow, m.fast);
            if m.code == 0x03 {
                // respiration : durées de chaque phase
                let s = s as u8;
                for (r, v) in [(0x16, s * 3), (0x17, s), (0x18, s), (0x19, s * 3), (0x1A, 1), (0x1B, 100), (0x1C, 64), (0x1D, 0)] {
                    self.set(r, v)?;
                }
                self.set(REG_SPEED, 0)?;
            } else if m.code == 0x04 {
                self.set(REG_SPEED, 0)?;
                self.set(0x12, (s >> 8) as u8)?;
                self.set(0x13, 1)?;
                self.set(0x14, ((s * 5) >> 8) as u8)?;
                self.set(0x15, 1)?;
            } else {
                self.set(REG_SPEED, s.min(255) as u8)?;
            }
        } else {
            self.set(REG_SPEED, 0)?;
        }
        self.finish(p.brightness_in(0, 100) as u8)
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        self.begin(MODE_DIRECT)?;
        for led in 0..self.per_dimm {
            for s in 0..self.addrs.len() {
                let c = colors.get(s * self.per_dimm + led).copied().unwrap_or(Color::BLACK);
                let r = REG_LED_COLORS + (led * 3) as u8;
                self.write(s, r, c.r)?;
                self.write(s, r + 2, c.b)?;
                self.write(s, r + 1, c.g)?;
            }
        }
        self.finish(100)
    }

    fn max_fps(&self) -> u32 {
        15
    }
}
