// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! RAM Corsair (Vengeance RGB Pro / Pro SL / RS, Dominator Platinum / Titanium,
//! Vengeance DDR5…) sur le bus SMBus. Protocole d'après OpenRGB.

use super::smbus::{crc8, Bus};
use crate::device::{err, Device, Result};
use crate::model::*;
use std::sync::Arc;
use std::time::Duration;

const REG_RESET_BUFFER: u8 = 0x0B;
const REG_SET_BINARY_DATA: u8 = 0x20;
const REG_BINARY_START: u8 = 0x21;
const REG_STATUS: u8 = 0x30;
const REG_COLOR_BLOCK_1: u8 = 0x31;
const REG_COLOR_BLOCK_2: u8 = 0x32;
const REG_GET_BINARY_DATA: u8 = 0x40;
const REG_GET_CHECKSUM: u8 = 0x42;
const REG_GET_DEVICE_INFO: u8 = 0x61;
const REG_WRITE_CONFIGURATION: u8 = 0x82;
const ID_EFFECT: u8 = 1;
const ID_COLORS: u8 = 2;

/// (nom, PID, nombre de LED, ordre inversé)
const MODELS: &[(&str, &[u16], usize, bool)] = &[
    ("Corsair Vengeance RGB Pro DDR4", &[0x0100, 0x0101], 10, false),
    ("Corsair Dominator Platinum RGB DDR4", &[0x0200, 0x0201], 12, true),
    ("Corsair Vengeance RGB Pro SL DDR4", &[0x0300, 0x0301], 10, false),
    ("Corsair Vengeance RGB RS DDR4", &[0x0400, 0x0401], 6, false),
    ("Corsair Dominator Platinum RGB DDR5", &[0x0600, 0x0601], 12, true),
    ("Corsair Dominator Titanium RGB DDR5", &[0x0800, 0x0801, 0x0810, 0x0811], 12, true),
    ("Corsair Vengeance RGB DDR5", &[0x0700, 0x0701, 0x0900, 0x0901, 0x0910, 0x0911], 10, false),
    ("Corsair Vengeance Shugo Series DDR5", &[0x0A00, 0x0A01, 0x0A10, 0x0A11], 10, false),
    ("Corsair Vengeance RGB RS DDR5", &[0x0B00, 0x0B01], 6, false),
];

/// Modes matériels : (nom, code, couleurs, aléatoire, directions)
fn modes() -> (Vec<ModeInfo>, Vec<u8>) {
    use Direction::*;
    let list: Vec<(ModeInfo, u8)> = vec![
        (ModeInfo::new("Statique").colors(1, 1), 0x10),
        (ModeInfo::new("Changement de couleur").speed().brightness().colors(2, 2).random(), 0x00),
        (ModeInfo::new("Respiration").speed().brightness().colors(2, 2).random(), 0x01),
        (ModeInfo::new("Vague arc-en-ciel").speed().dirs(&[Down, Up, Right, Left]), 0x03),
        (ModeInfo::new("Vague de couleurs").speed().brightness().colors(2, 2).random().dirs(&[Down, Up, Right, Left]), 0x04),
        (ModeInfo::new("Visière").speed().brightness().colors(2, 2).random().dirs(&[Down, Right]), 0x05),
        (ModeInfo::new("Pluie").speed().brightness().colors(2, 2).random().dirs(&[Down, Up]), 0x06),
        (ModeInfo::new("Défilement").speed().brightness().colors(1, 1), 0x07),
        (ModeInfo::new("Arc-en-ciel").speed(), 0x08),
        (ModeInfo::new("Séquentiel").speed().brightness().colors(1, 1).random().dirs(&[Down, Up]), 0x09),
        (ModeInfo::new("Éteint").off(), 0x10),
    ];
    list.into_iter().unzip()
}

pub struct CorsairDram {
    info: DeviceInfo,
    bus: Arc<Bus>,
    addr: u8,
    leds: usize,
    reverse: bool,
    codes: Vec<u8>,
}

fn present(bus: &Bus, addr: u8) -> bool {
    matches!(bus.read_byte_data(addr, 0x43), Ok(0x1A..=0x1C))
        && matches!(bus.read_byte_data(addr, 0x44), Ok(0x01 | 0x03 | 0x04))
}

pub fn detect(bus: &Arc<Bus>, out: &mut Vec<Box<dyn Device>>) {
    if !bus.is_dram() {
        return;
    }
    for addr in (0x58..=0x5F).chain(0x18..=0x1F) {
        if present(bus, addr) {
            if let Ok(d) = CorsairDram::new(bus.clone(), addr) {
                out.push(Box::new(d));
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

impl CorsairDram {
    fn new(bus: Arc<Bus>, addr: u8) -> Result<Self> {
        bus.write_byte_data(addr, REG_GET_DEVICE_INFO, 0)?;
        bus.write_byte_data(addr, REG_BINARY_START, 0)?;
        let mut d = [0u8; 32];
        for b in d.iter_mut() {
            *b = bus.read_byte_data(addr, REG_GET_BINARY_DATA)?;
        }
        let pid = u16::from_le_bytes([d[2], d[3]]);
        let protocol = d[28];
        let (name, leds, reverse) = MODELS
            .iter()
            .find(|m| m.1.contains(&pid))
            .map(|m| (m.0, m.2, m.3))
            .unwrap_or(("Corsair RGB DRAM", 10, false));
        let slot = (addr & 0x07) + 1;
        let (modes, codes) = modes();
        crate::log(&format!("Corsair DRAM {:#04x} pid={pid:#06x} protocole={protocol}", addr));
        Ok(CorsairDram {
            info: DeviceInfo {
                id: format!("corsair-dram:{}:{addr:02x}", bus.name),
                name: format!("{name} · barrette {slot}"),
                vendor: "Corsair".into(),
                kind: DeviceKind::Ram,
                location: bus.location(addr),
                zones: vec![Zone::linear("Barrette", leds)],
                modes,
                direct: protocol >= 4,
            },
            bus,
            addr,
            leds,
            reverse,
            codes,
        })
    }

    fn wait_ready(&self) {
        for _ in 0..5 {
            if matches!(self.bus.read_byte_data(self.addr, REG_STATUS), Ok(s) if s & 0x08 == 0) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Envoie un tampon binaire puis l'applique si le CRC est bon.
    fn send_buffer(&self, data: &[u8], id: u8) -> Result<()> {
        let (b, a) = (&self.bus, self.addr);
        b.write_byte_data(a, REG_RESET_BUFFER, 0)?;
        b.write_byte_data(a, REG_BINARY_START, 0)?;
        for &x in data {
            b.write_byte_data(a, REG_SET_BINARY_DATA, x)?;
        }
        if b.read_byte_data(a, REG_GET_CHECKSUM)? != crc8(data) {
            return err("somme de contrôle incorrecte");
        }
        b.write_byte_data(a, REG_WRITE_CONFIGURATION, id)?;
        self.wait_ready();
        Ok(())
    }

    fn ordered(&self, colors: &[Color]) -> Vec<Color> {
        let mut v: Vec<Color> = (0..self.leds).map(|i| colors.get(i).copied().unwrap_or(Color::BLACK)).collect();
        if self.reverse {
            v.reverse();
        }
        v
    }
}

impl Device for CorsairDram {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, mode: usize, p: &HwParams) -> Result<()> {
        let m = &self.info.modes[mode];
        let code = self.codes[mode];
        let (c1, c2) = if m.is_off {
            (Color::BLACK, Color::BLACK)
        } else {
            (p.color(0), p.color(1))
        };
        let dir = match p.direction {
            Direction::Up => 0,
            Direction::Down => 1,
            Direction::Left => 2,
            _ => 3,
        };
        let bright = if m.brightness { p.brightness_in(0, 255) as u8 } else { 255 };
        let mut e = [0u8; 20];
        e[0] = code;
        e[1] = p.speed_in(0, 2) as u8;
        e[2] = if p.random && m.random { 0 } else { 1 };
        e[3] = dir;
        e[4..7].copy_from_slice(&[c1.r, c1.g, c1.b]);
        e[7] = bright;
        e[8..11].copy_from_slice(&[c2.r, c2.g, c2.b]);
        e[11] = bright;
        self.send_buffer(&e, ID_EFFECT)?;
        if code == 0x10 {
            // statique : couleurs LED par LED dans le tampon de couleurs
            let mut data = Vec::with_capacity(self.leds * 4);
            for c in self.ordered(&vec![c1; self.leds]) {
                data.extend_from_slice(&[c.r, c.g, c.b, 0xFF]);
            }
            self.send_buffer(&data, ID_COLORS)?;
        }
        Ok(())
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        let v = self.ordered(colors);
        let mut pkt = Vec::with_capacity(self.leds * 3 + 2);
        pkt.push(self.leds as u8);
        for c in &v {
            pkt.extend_from_slice(&[c.r, c.g, c.b]);
        }
        pkt.push(crc8(&pkt));
        let first = pkt.len().min(32);
        self.bus.write_block_data(self.addr, REG_COLOR_BLOCK_1, &pkt[..first])?;
        if pkt.len() > 32 {
            self.bus.write_block_data(self.addr, REG_COLOR_BLOCK_2, &pkt[32..])?;
        }
        Ok(())
    }

    fn max_fps(&self) -> u32 {
        30
    }
}
