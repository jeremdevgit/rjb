// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! RAM Crucial Ballistix RGB / MAX RGB. Protocole d'après OpenRGB.

use super::smbus::Bus;
use crate::device::{Device, Result};
use crate::model::*;
use std::sync::Arc;
use std::time::Duration;

const ADDRESSES: &[u8] = &[0x39, 0x3A, 0x3B, 0x3C, 0x20, 0x21, 0x22, 0x23];

fn reg_read(bus: &Bus, dev: u8, reg: u16) -> Result<u8> {
    bus.write_word_data(dev, 0x00, reg.swap_bytes())?;
    bus.read_byte_data(dev, 0x81)
}

fn reg_write(bus: &Bus, dev: u8, reg: u16, v: u8) -> Result<()> {
    bus.write_word_data(dev, 0x00, reg.swap_bytes())?;
    bus.write_byte_data(dev, 0x01, v)
}

fn reg_write_block(bus: &Bus, dev: u8, reg: u16, data: &[u8]) -> Result<()> {
    bus.write_word_data(dev, 0x00, reg.swap_bytes())?;
    if bus.write_block_data(dev, 0x03, data).is_err() {
        for &b in data {
            bus.write_byte_data(dev, 0x01, b)?;
        }
    }
    Ok(())
}

fn is_micron(bus: &Bus, addr: u8, reg: u16) -> bool {
    let s: Vec<u8> = (0..6).filter_map(|i| reg_read(bus, addr, reg + i).ok()).collect();
    s == b"Micron"
}

fn test(bus: &Bus, addr: u8) -> bool {
    if bus.read_byte(addr).is_err() {
        return false;
    }
    for i in 0xA0u8..0xB0 {
        if bus.read_byte_data(addr, i).ok() != Some(i - 0xA0) {
            return false;
        }
    }
    is_micron(bus, addr, 0x1025) || is_micron(bus, addr, 0x1030)
}

pub fn detect(bus: &Arc<Bus>, out: &mut Vec<Box<dyn Device>>) {
    if !bus.is_dram() {
        return;
    }
    // réadressage des barrettes qui démarrent en 0x27
    let mut idx = 0usize;
    for slot in 0..4u8 {
        if bus.read_byte(0x27).is_err() {
            break;
        }
        while idx < ADDRESSES.len() && bus.read_byte(ADDRESSES[idx]).is_ok() {
            idx += 1;
        }
        if idx >= ADDRESSES.len() {
            break;
        }
        let _ = reg_write(bus, 0x27, 0x82EE, slot);
        let _ = reg_write(bus, 0x27, 0x82EF, ADDRESSES[idx] << 1);
        let _ = reg_write(bus, 0x27, 0x82F0, 0xF0);
        idx += 1;
        std::thread::sleep(Duration::from_millis(1));
    }
    let mut n = 0;
    for &a in ADDRESSES {
        if test(bus, a) {
            n += 1;
            out.push(Box::new(Crucial::new(bus.clone(), a, n)));
        }
    }
}

const MODES: &[(&str, u8)] = &[
    ("Statique", 0xCF),
    ("Décalage", 0x1F),
    ("Décalage en dégradé", 0x2F),
    ("Remplissage", 0x3F),
    ("Empilement", 0x4F),
    ("Double empilement", 0x5F),
    ("Respiration", 0x6F),
    ("Point mobile", 0x7F),
    ("Du centre vers l'extérieur", 0x8F),
    ("Pas de couleur", 0x9F),
    ("Vague d'eau", 0xAF),
    ("Flash", 0xBF),
];

pub struct Crucial {
    info: DeviceInfo,
    bus: Arc<Bus>,
    addr: u8,
    direct_ready: bool,
}

impl Crucial {
    fn new(bus: Arc<Bus>, addr: u8, n: usize) -> Self {
        let mut modes: Vec<ModeInfo> = MODES.iter().map(|m| ModeInfo::new(m.0).colors(1, 1)).collect();
        modes.push(ModeInfo::new("Éteint").off());
        Crucial {
            info: DeviceInfo {
                id: format!("crucial:{}:{addr:02x}", bus.name),
                name: format!("Crucial Ballistix · barrette {n}"),
                vendor: "Crucial".into(),
                kind: DeviceKind::Ram,
                location: bus.location(addr),
                zones: vec![Zone::linear("Barrette", 8)],
                modes,
                direct: true,
            },
            bus,
            addr,
            direct_ready: false,
        }
    }

    fn effect_colors(&self, c: Color) -> Result<()> {
        for led in 0..8u8 {
            let (b, a) = (&self.bus, self.addr);
            reg_write(b, a, 0x82E9, 1 << led)?;
            for r in [0x82EA, 0x82EB, 0x82EC] {
                reg_write(b, a, r, 0)?;
            }
            reg_write(b, a, 0x82ED, c.r)?;
            reg_write(b, a, 0x82EE, c.g)?;
            reg_write(b, a, 0x82EF, c.b)?;
            reg_write(b, a, 0x82F0, 0x01)?;
        }
        Ok(())
    }

    fn effect_mode(&self, mode: u8) -> Result<()> {
        let (b, a) = (&self.bus, self.addr);
        reg_write(b, a, 0x820F, mode)?;
        reg_write(b, a, 0x82EE, 0x00)?;
        reg_write(b, a, 0x82EF, 0x10)?;
        reg_write(b, a, 0x82F0, 0x84)
    }
}

impl Device for Crucial {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, mode: usize, p: &HwParams) -> Result<()> {
        self.direct_ready = false;
        if self.info.modes[mode].is_off {
            self.effect_colors(Color::BLACK)?;
            return self.effect_mode(0xCF);
        }
        self.effect_colors(p.color(0))?;
        self.effect_mode(MODES[mode].1)
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        if !self.direct_ready {
            // le mode direct s'appuie sur l'effet statique
            self.effect_mode(0xCF)?;
            self.direct_ready = true;
        }
        let get = |i: usize| colors.get(i).copied().unwrap_or(Color::BLACK);
        let r: Vec<u8> = (0..8).map(|i| get(i).r).collect();
        let g: Vec<u8> = (0..8).map(|i| get(i).g).collect();
        let b: Vec<u8> = (0..8).map(|i| get(i).b).collect();
        reg_write_block(&self.bus, self.addr, 0x8300, &r)?;
        reg_write_block(&self.bus, self.addr, 0x8340, &g)?;
        reg_write_block(&self.bus, self.addr, 0x8380, &b)
    }

    fn max_fps(&self) -> u32 {
        25
    }
}
