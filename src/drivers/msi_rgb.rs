// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Cartes mères MSI à éclairage piloté par la puce Super I/O Nuvoton
//! (« MSI-RGB » : B350/B450/X470/Z370… ; connecteurs JRGB 12 V).
//! Protocole d'après OpenRGB.

use super::smbus::{SuperIo, SIO_REG_LOGDEV};
use crate::device::{Device, Result};
use crate::model::*;

const LOGDEV_RGB: u8 = 0x12;

/// Codes « MS-xxxx » compatibles, et inversion des canaux.
const BOARDS: &[(&str, bool)] = &[
    ("7A40", false), ("7A34", false), ("7A39", false), ("7A38", false), ("7B79", false),
    ("7B73", false), ("7B61", false), ("7B54", false), ("7B49", false), ("7B48", false),
    ("7B45", false), ("7B44", false), ("7A59", false), ("7A57", false), ("7A68", false),
    ("7B40", false), ("7A94", false), ("7B06", false), ("7B08", false), ("7B09", false),
    ("7A58", false), ("7A62", false), ("7A69", false), ("7A70", false), ("7A72", false),
    ("7A78", false), ("7A79", false), ("7A37", false), ("7B89", true), ("7B90", true),
    ("7B19", true), ("7C02", true), ("7B75", true), ("7B22", true), ("7B23", true),
    ("7B24", true), ("7B27", true), ("7B30", true), ("7B31", true), ("7B51", true),
    ("7C04", true), ("7C00", true), ("7B98", true), ("7C22", true), ("7C24", true),
    ("7C01", true), ("7C39", true), ("7B86", true), ("7B87", true), ("7D95", false),
];

pub struct MsiRgb {
    info: DeviceInfo,
    sio: SuperIo,
}

pub fn detect(manufacturer: &str, product: &str, out: &mut Vec<Box<dyn Device>>) {
    if !manufacturer.starts_with("Micro-Star") && manufacturer != "MSI" {
        return;
    }
    let Some(&(_, invert)) = BOARDS.iter().find(|b| product.contains(b.0)) else {
        return;
    };
    for addr in [0x2E, 0x4E] {
        let Ok(sio) = SuperIo::open(addr) else { continue };
        let id = sio.chip_id() & 0xFFF8;
        crate::log(&format!("Super I/O {addr:#x} : puce {id:#06x}"));
        if id == 0xD350 || id == 0xD450 {
            let d = MsiRgb::new(sio, invert, product);
            out.push(Box::new(d));
            return;
        }
    }
}

impl MsiRgb {
    fn new(sio: SuperIo, invert: bool, product: &str) -> Self {
        sio.outb(SIO_REG_LOGDEV, 0x09);
        let v = (sio.inb(0x2C) & 0b1111_0111) | 0b0001_0000;
        sio.outb(0x2C, v);
        sio.outb(SIO_REG_LOGDEV, LOGDEV_RGB);
        if sio.inb(0xE0) & 0xE0 != 0xE0 {
            sio.outb(0xE0, 0xE0);
        }
        sio.outb(0xE4, 0x00);
        let mut ff = 0b1110_0010;
        if invert {
            ff |= 0b0001_1100;
        }
        sio.outb(0xFF, ff);
        sio.outb(0xFD, 0x00);
        let addr = sio.addr;
        MsiRgb {
            info: DeviceInfo {
                id: format!("msi-rgb:{addr:x}"),
                name: format!("MSI {}", pretty_name(product)),
                vendor: "MSI".into(),
                kind: DeviceKind::Motherboard,
                location: format!("superio:{addr:x}"),
                zones: vec![Zone::single("Connecteurs RGB")],
                modes: Vec::new(),
                direct: true,
            },
            sio,
        }
    }
}

impl Device for MsiRgb {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, _mode: usize, _p: &HwParams) -> Result<()> {
        Ok(())
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        let c = colors.first().copied().unwrap_or(Color::BLACK);
        // 4 bits par canal, répétés sur les 4 phases du motif
        let (r, g, b) = (c.r >> 4, c.g >> 4, c.b >> 4);
        let s = &self.sio;
        s.outb(SIO_REG_LOGDEV, LOGDEV_RGB);
        for (base, v) in [(0xF0u8, r), (0xF4, g), (0xF8, b)] {
            for i in 0..4 {
                s.outb(base + i, v | (v << 4));
            }
        }
        Ok(())
    }

    fn max_fps(&self) -> u32 {
        30
    }
}
