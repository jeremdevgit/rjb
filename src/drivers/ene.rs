// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Contrôleurs ENE (« ASUS Aura » SMBus) : RAM G.Skill Trident Z RGB / Neo /
//! Royal, ASUS, XPG Spectrix, Geil, T-Force…, cartes mères ASUS 2017-2020 et
//! cartes graphiques ASUS. Protocole d'après OpenRGB.

use super::smbus::{Bus, I2c};
use crate::device::{Device, Result};
use crate::model::*;
use std::sync::Arc;
use std::time::Duration;

const REG_DEVICE_NAME: u16 = 0x1000;
const REG_MICRON_CHECK: u16 = 0x1030;
const REG_CONFIG_TABLE: u16 = 0x1C00;
const REG_COLORS_DIRECT: u16 = 0x8000;
const REG_COLORS_EFFECT: u16 = 0x8010;
const REG_DIRECT: u16 = 0x8020;
const REG_MODE: u16 = 0x8021;
const REG_SPEED: u16 = 0x8022;
const REG_DIRECTION: u16 = 0x8023;
const REG_APPLY: u16 = 0x80A0;
const REG_SLOT_INDEX: u16 = 0x80F8;
const REG_I2C_ADDRESS: u16 = 0x80F9;
const REG_COLORS_DIRECT_V2: u16 = 0x8100;
const REG_COLORS_EFFECT_V2: u16 = 0x8160;

const RAM_ADDRESSES: &[u8] = &[
    0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x4F, 0x66, 0x67, 0x39, 0x3A, 0x3B, 0x3C, 0x3D,
];
const MOBO_ADDRESSES: &[u8] = &[0x40, 0x4E, 0x4F];

fn reg_read(bus: &dyn I2c, dev: u8, reg: u16) -> Result<u8> {
    bus.write_word_data(dev, 0x00, reg.swap_bytes())?;
    bus.read_byte_data(dev, 0x81)
}

fn reg_write(bus: &dyn I2c, dev: u8, reg: u16, v: u8) -> Result<()> {
    bus.write_word_data(dev, 0x00, reg.swap_bytes())?;
    bus.write_byte_data(dev, 0x01, v)
}

fn reg_write_block(bus: &dyn I2c, dev: u8, reg: u16, data: &[u8]) -> Result<()> {
    bus.write_word_data(dev, 0x00, reg.swap_bytes())?;
    if bus.write_block_data(dev, 0x03, data).is_err() {
        for &b in data {
            bus.write_byte_data(dev, 0x01, b)?;
        }
    }
    Ok(())
}

/// Vérifie la signature d'un contrôleur ENE (registres 0xA0..0xAF = 0..15).
fn test(bus: &dyn I2c, addr: u8) -> bool {
    if bus.read_byte(addr).is_err() && bus.read_byte_data(addr, 0).is_err() {
        return false;
    }
    for i in 0xA0u8..0xB0 {
        if bus.read_byte_data(addr, i).ok() != Some(i - 0xA0) {
            return false;
        }
    }
    let micron: Vec<u8> = (0..6).filter_map(|i| reg_read(bus, addr, REG_MICRON_CHECK + i).ok()).collect();
    micron != b"Micron"
}

pub fn detect_ram(bus: &Arc<Bus>, out: &mut Vec<Box<dyn Device>>) {
    if !bus.is_dram() {
        return;
    }
    // Les barrettes ENE démarrent toutes à 0x77 : on leur attribue une adresse libre.
    let mut idx = 0usize;
    for slot in 0..8u8 {
        if bus.read_byte(0x77).is_err() {
            break;
        }
        while idx < RAM_ADDRESSES.len() && bus.read_byte(RAM_ADDRESSES[idx]).is_ok() {
            idx += 1;
        }
        if idx >= RAM_ADDRESSES.len() {
            break;
        }
        let _ = reg_write(bus.as_ref(), 0x77, REG_SLOT_INDEX, slot);
        let _ = reg_write(bus.as_ref(), 0x77, REG_I2C_ADDRESS, RAM_ADDRESSES[idx] << 1);
        idx += 1;
    }
    let mut n = 0;
    for &a in RAM_ADDRESSES {
        if test(bus.as_ref(), a) {
            n += 1;
            if let Some(d) = Ene::new(bus.clone() as Arc<dyn I2c>, a, DeviceKind::Ram, format!("Mémoire RGB · barrette {n}")) {
                out.push(Box::new(d));
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

pub fn detect_motherboard(bus: &Arc<Bus>, board: &str, out: &mut Vec<Box<dyn Device>>) {
    let (_, _, sub_vendor, _) = bus.pci;
    if !bus.is_dram() || !(sub_vendor == 0x1043 || sub_vendor == 0 || sub_vendor == 0xFFFF) {
        return;
    }
    for &a in MOBO_ADDRESSES {
        if test(bus.as_ref(), a) {
            if let Some(d) = Ene::new(bus.clone() as Arc<dyn I2c>, a, DeviceKind::Motherboard, format!("ASUS {}", pretty_name(board))) {
                out.push(Box::new(d));
            }
        }
    }
}

/// Carte graphique ASUS : contrôleur ENE en 0x67 sur le bus I2C de la carte.
pub fn detect_gpu(bus: &Arc<dyn I2c>, name: &str, out: &mut Vec<Box<dyn Device>>) {
    if test(bus.as_ref(), 0x67) {
        if let Some(d) = Ene::new(bus.clone(), 0x67, DeviceKind::Gpu, name.to_string()) {
            out.push(Box::new(d));
        }
    }
}

fn channel_name(c: u8) -> &'static str {
    match c {
        0x84 => "Audio",
        0x88 => "Plaque arrière",
        0x85 => "Panneau E/S",
        0x82 | 0x83 => "Centre",
        0x8A | 0x05 | 0x0E => "LED",
        0x8B => "PCIe",
        0x86 | 0x91 => "Connecteur RGB",
        0x87 => "Connecteur RGB 2",
        _ => "Zone",
    }
}

pub struct Ene {
    info: DeviceInfo,
    bus: Arc<dyn I2c>,
    addr: u8,
    leds: usize,
    direct_reg: u16,
    effect_reg: u16,
    in_direct: Option<bool>,
    codes: Vec<u8>,
}

impl Ene {
    fn new(bus: Arc<dyn I2c>, addr: u8, kind: DeviceKind, name: String) -> Option<Ene> {
        let version: String = (0..15)
            .filter_map(|i| reg_read(bus.as_ref(), addr, REG_DEVICE_NAME + i).ok())
            .take_while(|&b| b != 0)
            .map(|b| b as char)
            .collect();
        let mut cfg = [0u8; 64];
        for (i, c) in cfg.iter_mut().enumerate() {
            *c = reg_read(bus.as_ref(), addr, REG_CONFIG_TABLE + i as u16).ok()?;
        }
        let v1 = ["LED-0116", "DIMM_LED-0102", "DIMM_LED-0103", "AUMA0-E8K4-0101"];
        let (direct_reg, effect_reg, channel_cfg, led_count) = if v1.contains(&version.as_str()) {
            (REG_COLORS_DIRECT, REG_COLORS_EFFECT, 0x13usize, cfg[2])
        } else if version == "AUDA0-E6K5-0101" {
            (REG_COLORS_DIRECT_V2, REG_COLORS_EFFECT_V2, 0x13, cfg[2])
        } else if version == "AUMA0-E6K5-0008" {
            (REG_COLORS_DIRECT_V2, REG_COLORS_EFFECT_V2, 0x13, cfg[3])
        } else if ["AUMA0-E6K5-0104", "AUMA0-E6K5-0105", "AUMA0-E6K5-0106"].contains(&version.as_str()) {
            (REG_COLORS_DIRECT_V2, REG_COLORS_EFFECT_V2, 0x1B, cfg[2])
        } else if version.starts_with("AUMA0-E6K5-") {
            (REG_COLORS_DIRECT_V2, REG_COLORS_EFFECT_V2, 0x1B, cfg[3])
        } else {
            crate::log(&format!("ENE {addr:#04x} : version inconnue « {version} »"));
            return None;
        };
        let leds = led_count as usize;
        if leds == 0 {
            return None;
        }
        crate::log(&format!("ENE {addr:#04x} {version} : {leds} LED"));

        // zones regroupées par canal
        let mut zones: Vec<(String, usize)> = Vec::new();
        for z in 0..8 {
            let n = cfg[3 + z] as usize;
            if n == 0 {
                continue;
            }
            let name = channel_name(cfg[channel_cfg + z]).to_string();
            match zones.iter_mut().find(|(zn, _)| *zn == name) {
                Some(e) => e.1 += n,
                None => zones.push((name, n)),
            }
        }
        let total: usize = zones.iter().map(|z| z.1).sum();
        let zones = if total == leds && !zones.is_empty() {
            zones.into_iter().map(|(n, c)| Zone::linear(n, c)).collect()
        } else {
            vec![Zone::linear("LED", leds)]
        };

        let double_fade = version == "AUDA0-E6K5-0101" && cfg[0x13..0x1B].contains(&0x0E);
        let mut list: Vec<(ModeInfo, u8)> = vec![
            (ModeInfo::new("Statique").colors(1, 1), 1),
            (ModeInfo::new("Respiration").speed().colors(1, 1).random(), 2),
            (ModeInfo::new("Flash").speed().colors(1, 1), 3),
            (ModeInfo::new("Cycle de couleurs").speed(), 4),
            (ModeInfo::new("Arc-en-ciel").speed().dirs(&[Direction::Left, Direction::Right]), 5),
            (ModeInfo::new("Poursuite en fondu").speed().colors(1, 1).random().dirs(&[Direction::Left, Direction::Right]), 7),
            (ModeInfo::new("Poursuite").speed().colors(1, 1).random().dirs(&[Direction::Left, Direction::Right]), 9),
            (ModeInfo::new("Scintillement aléatoire").speed(), 13),
        ];
        if double_fade {
            list.push((ModeInfo::new("Double fondu").speed(), 14));
        }
        list.push((ModeInfo::new("Éteint").off(), 0));
        let (modes, codes) = list.into_iter().unzip();
        let vendor = if version.contains("DIMM_LED") || version.contains("AUDA") { "ENE" } else { "ASUS" };
        Some(Ene {
            info: DeviceInfo {
                id: format!("ene:{}:{addr:02x}", bus.name()),
                name,
                vendor: vendor.into(),
                kind,
                location: bus.location(addr),
                zones,
                modes,
                direct: true,
            },
            bus,
            addr,
            leds,
            direct_reg,
            effect_reg,
            in_direct: None,
            codes,
        })
    }

    fn write_colors(&self, reg: u16, colors: &[Color]) -> Result<()> {
        let mut buf = Vec::with_capacity(self.leds * 3);
        for i in 0..self.leds {
            let c = colors.get(i).copied().unwrap_or(Color::BLACK);
            buf.extend_from_slice(&[c.r, c.b, c.g]);
        }
        for (i, chunk) in buf.chunks(3).enumerate() {
            reg_write_block(self.bus.as_ref(), self.addr, reg + (i * 3) as u16, chunk)?;
        }
        Ok(())
    }

    fn set_direct(&mut self, on: bool) -> Result<()> {
        if self.in_direct != Some(on) {
            reg_write(self.bus.as_ref(), self.addr, REG_DIRECT, on as u8)?;
            reg_write(self.bus.as_ref(), self.addr, REG_APPLY, 0x01)?;
            self.in_direct = Some(on);
        }
        Ok(())
    }
}

impl Device for Ene {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, mode: usize, p: &HwParams) -> Result<()> {
        let m = self.info.modes[mode].clone();
        let mut code = self.codes[mode];
        if m.random && p.random {
            code = match code {
                2 => 6,
                7 => 8,
                9 => 10,
                c => c,
            };
        }
        let speed = if m.speed { p.speed_in(4, 0) as u8 } else { 0 };
        let dir = (p.direction == Direction::Right) as u8;
        let bus = self.bus.clone();
        let (b, a) = (bus.as_ref(), self.addr);
        reg_write(b, a, REG_MODE, code)?;
        reg_write(b, a, REG_SPEED, speed)?;
        reg_write(b, a, REG_DIRECTION, dir)?;
        reg_write(b, a, REG_APPLY, 0x01)?;
        self.in_direct = None;
        self.set_direct(false)?;
        if m.colors.1 > 0 {
            self.write_colors(self.effect_reg, &vec![p.color(0); self.leds])?;
            reg_write(b, a, REG_APPLY, 0x01)?;
        }
        Ok(())
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        self.set_direct(true)?;
        self.write_colors(self.direct_reg, colors)
    }

    fn max_fps(&self) -> u32 {
        25
    }
}
