// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! NZXT Hue 2 / Hue 2 Ambient, Smart Device V2, RGB & Fan Controller,
//! Kraken X3 / Elite 2024, contrôleurs RGB des boîtiers et cartes NZXT.
//! Le nombre de LED de chaque canal est lu dans l'appareil (ventilateurs
//! et bandes détectés). Protocole d'après OpenRGB.

use super::hid::{self, HidEntry};
use crate::device::{Device, Result};
use crate::model::*;
use hidapi::{DeviceInfo as HidInfo, HidApi, HidDevice};

pub const HID: &[HidEntry] = &[HidEntry {
    vid: 0x1E71,
    pids: &[
        0x2001, 0x2002, 0x2005, 0x200B, 0x2006, 0x200D, 0x200F, 0x3012, 0x2009, 0x2010, 0x200E, 0x2011, 0x2019,
        0x2020, 0x201F, 0x2022, 0x201B, 0x2012, 0x2021, 0x2007, 0x2014,
    ],
    usage_page: None,
    usage: None,
    interface: None,
    open,
}];

/// Nombre de canaux RGB selon le modèle.
fn channels(pid: u16) -> usize {
    match pid {
        0x2001 => 4,
        0x2011 | 0x2019 | 0x2020 | 0x201F | 0x2022 | 0x201B => 6,
        0x2007 | 0x2014 | 0x2012 | 0x2021 => 3,
        _ => 2,
    }
}

/// LED d'un accessoire NZXT selon son identifiant.
fn accessory_leds(id: u8) -> usize {
    match id {
        0x01 | 0x04 | 0x0A => 10,
        0x02 | 0x05 | 0x0B | 0x0C | 0x10 | 0x17 | 0x18 | 0x19 => 8,
        0x06 => 6,
        0x08 => 14,
        0x09 => 15,
        0x11 => 1,
        0x13 | 0x14 => 18,
        0x15 | 0x16 => 20,
        0x1C => 16,
        0x1D | 0x1E | 0x1F => 24,
        _ => 0,
    }
}

pub struct NzxtHue2 {
    info: DeviceInfo,
    dev: HidDevice,
    effects_2023: bool,
}

fn open(api: &HidApi, i: &HidInfo) -> Result<Vec<Box<dyn Device>>> {
    let dev = api.open_path(i.path())?;
    let pid = i.product_id();
    let mut b = [0u8; 64];
    b[0] = 0x20;
    b[1] = 0x03;
    dev.write(&b)?;
    let mut r = [0u8; 64];
    for _ in 0..10 {
        let n = dev.read_timeout(&mut r, 1000)?;
        if n == 64 && r[0] == 0x21 && r[1] == 0x03 {
            break;
        }
    }
    let mut zones = Vec::new();
    for ch in 0..channels(pid) {
        let start = 0x0F + 6 * ch;
        let leds: usize = (0..6).map(|k| accessory_leds(r.get(start + k).copied().unwrap_or(0))).sum();
        zones.push(Zone::resizable(format!("Canal {}", ch + 1), if leds == 0 { 8 } else { leds }, 1, 40));
    }
    let modes = vec![
        ModeInfo::new("Statique").colors(1, 1),
        ModeInfo::new("Fondu").speed().colors(1, 8),
        ModeInfo::new("Cycle de couleurs").speed().dirs(&[Direction::Right, Direction::Left]),
        ModeInfo::new("Défilement").speed().colors(1, 1).dirs(&[Direction::Right, Direction::Left]),
        ModeInfo::new("Défilement couvrant").speed().colors(1, 8).dirs(&[Direction::Right, Direction::Left]),
        ModeInfo::new("Alternance").speed().colors(1, 2).dirs(&[Direction::Right, Direction::Left]),
        ModeInfo::new("Pulsation").speed().colors(1, 8),
        ModeInfo::new("Respiration").speed().colors(1, 8),
        ModeInfo::new("Bougie").colors(1, 1),
        ModeInfo::new("Nuit étoilée").speed().colors(1, 1),
        ModeInfo::new("Flux arc-en-ciel").speed().dirs(&[Direction::Right, Direction::Left]),
        ModeInfo::new("Super arc-en-ciel").speed().dirs(&[Direction::Right, Direction::Left]),
        ModeInfo::new("Pulsation arc-en-ciel").speed().dirs(&[Direction::Right, Direction::Left]),
        ModeInfo::new("Éteint").off(),
    ];
    Ok(vec![Box::new(NzxtHue2 {
        info: DeviceInfo {
            id: hid::stable_id("nzxt", i),
            name: pretty_name(&hid::product(i, "NZXT Hue 2")),
            vendor: "NZXT".into(),
            kind: if matches!(pid, 0x2007 | 0x2014 | 0x3012) { DeviceKind::Cooler } else { DeviceKind::FanController },
            location: hid::location(i),
            zones,
            modes,
            direct: true,
        },
        dev,
        effects_2023: pid == 0x3012,
    })])
}

fn grb(colors: &[Color]) -> Vec<u8> {
    colors.iter().flat_map(|c| [c.g, c.r, c.b]).collect()
}

impl NzxtHue2 {
    fn send(&self, p: &[u8]) -> Result<()> {
        let mut b = [0u8; 64];
        b[..p.len()].copy_from_slice(p);
        self.dev.write(&b)?;
        Ok(())
    }

    fn effect(&self, ch: usize, mode: u8, speed: u8, dir: bool, colors: &[Color]) -> Result<()> {
        let mask = 1u8 << ch;
        let data = grb(colors);
        if !self.effects_2023 {
            let mut p = vec![0x28, 0x03, mask, 0x28, mode, speed, 1, dir as u8, colors.len() as u8, 0];
            p.extend_from_slice(&data);
            return self.send(&p);
        }
        // contrôleurs 2023+ : durées explicites et format différent
        let s = speed.min(4) as usize;
        let (sp, modifier): ([u8; 2], u8) = match mode {
            0x01 => ([[0x50, 0], [0x3C, 0], [0x28, 0], [0x14, 0], [0x0A, 0]][s], 0x08),
            0x02 | 0x0B | 0x0C | 0x0D => ([[0x5E, 1], [0x2C, 1], [0xFA, 0], [0x96, 0], [0x50, 0]][s], 0),
            0x05 => ([[0x40, 6], [0x14, 5], [0xE8, 3], [0x20, 3], [0x58, 2]][s], 0),
            0x06 | 0x09 => ([[0x19, 0], [0x14, 0], [0x0F, 0], [0x07, 0], [0x04, 0]][s], if mode == 0x06 { 0x08 } else { 0 }),
            0x07 => ([[0x28, 0], [0x1E, 0], [0x14, 0], [0x0A, 0], [0x04, 0]][s], 0x08),
            _ => ([0x32, 0], 0),
        };
        let mut b = [0u8; 64];
        b[0] = 0x2A;
        b[1] = 0x04;
        b[2] = mask;
        b[3] = mask;
        b[4] = mode;
        b[5] = sp[0];
        b[6] = sp[1];
        let n = data.len().min(48);
        b[7..7 + n].copy_from_slice(&data[..n]);
        b[0x37] = if dir { 2 } else { 0 };
        b[0x38] = colors.len() as u8;
        b[0x39] = modifier;
        b[0x3A] = 0x08;
        b[0x3B] = 0x03;
        self.dev.write(&b)?;
        Ok(())
    }
}

impl Device for NzxtHue2 {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, mode: usize, p: &HwParams) -> Result<()> {
        let m = self.info.modes[mode].clone();
        let code = if m.is_off { 0x00 } else { [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0B, 0x0C, 0x0D][mode.min(12)] };
        let cols: Vec<Color> = if m.is_off {
            vec![Color::BLACK]
        } else if m.colors.1 > 0 {
            let n = p.colors.len().clamp(m.colors.0 as usize, m.colors.1 as usize);
            (0..n).map(|i| p.color(i)).collect()
        } else {
            Vec::new()
        };
        let speed = p.speed_in(0, 4) as u8;
        for ch in 0..self.info.zones.len() {
            self.effect(ch, code, speed, p.direction == Direction::Left, &cols)?;
        }
        Ok(())
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        let mut off = 0;
        for ch in 0..self.info.zones.len() {
            let n = self.info.zones[ch].leds.len();
            let data = grb(&colors[off..off + n]);
            let mask = 1u8 << ch;
            for (group, chunk) in data.chunks(60).enumerate() {
                let mut p = vec![0x22, 0x10 | group as u8, mask, 0x00];
                p.extend_from_slice(chunk);
                self.send(&p)?;
            }
            self.send(&[0x22, 0xA0, mask, 0, 0x01, 0, 0, 0x28, 0, 0, 0x80, 0, 0x32, 0, 0, 0x01])?;
            off += n;
        }
        Ok(())
    }

    fn resize_zone(&mut self, zone: usize, leds: usize) -> Result<()> {
        let name = self.info.zones[zone].name.clone();
        self.info.zones[zone] = Zone::resizable(name, leds.clamp(1, 40), 1, 40);
        Ok(())
    }

    fn max_fps(&self) -> u32 {
        30
    }
}
