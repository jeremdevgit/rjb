// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Watercoolings Corsair Hydro H60i/H100i/H115i/H150i Platinum, Pro XT et
//! Elite RGB : couronne de 16 LED sur la pompe et connecteur de ventilateurs
//! RGB. Protocole d'après OpenRGB.

use super::hid::{self, HidEntry};
use super::smbus::crc8;
use crate::device::{Device, Result};
use crate::model::*;
use hidapi::{DeviceInfo as HidInfo, HidApi, HidDevice};
use std::time::Duration;

/// Modèles Elite RGB : pas de connecteur de ventilateurs RGB.
const NO_FAN: &[u16] = &[0x0C35, 0x0C36, 0x0C37, 0x0C40, 0x0C41];

pub const HID: &[HidEntry] = &[HidEntry {
    vid: 0x1B1C,
    pids: &[
        0x0C18, 0x0C19, 0x0C17, 0x0C29, 0x0C20, 0x0C2D, 0x0C21, 0x0C22, 0x0C35, 0x0C36, 0x0C37, 0x0C40, 0x0C41,
    ],
    usage_page: None,
    usage: None,
    interface: None,
    open,
}];

const MAGIC_1: u8 = 0b001;
const MAGIC_2: u8 = 0b010;
const MAGIC_3: u8 = 0b011;
const LIGHTING: [u8; 3] = [0b100, 0b101, 0b110];

/// Prise de contrôle logicielle (séquence relevée par OpenRGB).
const SW: [[u8; 61]; 3] = {
    let mut m = [[0xFF; 61]; 3];
    let a: [u8; 26] = [
        0x01, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F, 0x7F, 0x7F, 0x7F, 0xFF,
        0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x00,
    ];
    let mut i = 0;
    while i < 26 {
        m[0][i] = a[i];
        i += 1;
    }
    m[0][30] = 0;
    m[0][35] = 0;
    m[0][40] = 0;
    let mut k = 0;
    while k < 40 {
        m[1][k] = k as u8;
        m[2][k] = 0x28 + k as u8;
        k += 1;
    }
    m
};

/// Retour au mode autonome du firmware à la fermeture.
const HW_1: [u8; 61] = [
    0x00, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F,
    0x09, 0x20, 0x07, 0x00, 0x0B, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0xFF, 0xFF, 0xFF, 0xFF,
    0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF,
];
const HW_2: [u8; 61] = [
    0x0A, 0x01, 0x04, 0x07, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x00, 0x01, 0x02, 0x03, 0x04, 0x01, 0x0A, 0x07, 0x04,
    0x0B, 0x0A, 0x09, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x00, 0x01, 0x0A, 0x07, 0x04, 0x01, 0x0A, 0x09, 0x08,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF,
];
const HW_3: [u8; 61] = {
    let mut m = [0xFF; 61];
    let mut i = 0;
    while i < 40 {
        m[i] = if i < 8 { 7 - i as u8 } else { 0x0C };
        i += 1;
    }
    m
};
const HW_4: [u8; 61] = [
    0x00, 0x00, 0xFF, 0x00, 0x4A, 0xFF, 0x00, 0x94, 0xFF, 0x00, 0xDF, 0xFF, 0x00, 0xFF, 0xAA, 0x00, 0xFF, 0x15, 0x7F, 0x7F,
    0x00, 0xFA, 0x00, 0x06, 0xDB, 0x00, 0x32, 0xD7, 0x00, 0x58, 0xF6, 0x00, 0x76, 0x94, 0x00, 0xB4, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

pub struct HydroPlatinum {
    info: DeviceInfo,
    dev: HidDevice,
    seq: u8,
}

fn open(api: &HidApi, i: &HidInfo) -> Result<Vec<Box<dyn Device>>> {
    let dev = api.open_path(i.path())?;
    let mut zones = vec![Zone::linear("Pompe", 16)];
    if !NO_FAN.contains(&i.product_id()) {
        zones.push(Zone::resizable("Ventilateurs RGB", 8, 1, 32));
    }
    let mut d = HydroPlatinum {
        info: DeviceInfo {
            id: hid::stable_id("corsair-hydro", i),
            name: pretty_name(&hid::product(i, "Corsair Hydro")),
            vendor: "Corsair".into(),
            kind: DeviceKind::Cooler,
            location: hid::location(i),
            zones,
            modes: vec![ModeInfo::new("Effet de l'appareil")],
            direct: true,
        },
        dev,
        seq: 0,
    };
    d.software()?;
    Ok(vec![Box::new(d)])
}

impl HydroPlatinum {
    fn send(&mut self, cmd: u8, payload: &[u8; 61], read: bool) -> Result<()> {
        self.seq = if self.seq < 31 { self.seq + 1 } else { 1 };
        let mut b = [0u8; 65];
        b[1] = 0x3F;
        b[2] = (self.seq << 3) | cmd;
        b[3..64].copy_from_slice(payload);
        b[64] = crc8(&b[2..64]);
        self.dev.write(&b)?;
        if read {
            let mut r = [0u8; 65];
            let _ = self.dev.read_timeout(&mut r, 100);
        }
        std::thread::sleep(Duration::from_millis(5));
        Ok(())
    }

    fn software(&mut self) -> Result<()> {
        self.send(MAGIC_1, &SW[0], true)?;
        self.send(MAGIC_2, &SW[1], true)?;
        self.send(MAGIC_3, &SW[2], true)
    }

    fn hardware(&mut self) -> Result<()> {
        self.send(MAGIC_2, &HW_2, true)?;
        self.send(MAGIC_3, &HW_3, true)?;
        self.send(LIGHTING[0], &HW_4, true)?;
        self.send(MAGIC_1, &HW_1, true)
    }
}

impl Device for HydroPlatinum {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, _mode: usize, _p: &HwParams) -> Result<()> {
        self.hardware()
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        // 20 LED par paquet (BGR), 48 au maximum
        for (k, chunk) in colors.chunks(20).take(3).enumerate() {
            let mut p = [0u8; 61];
            for (j, c) in chunk.iter().enumerate() {
                p[j * 3..j * 3 + 3].copy_from_slice(&[c.b, c.g, c.r]);
            }
            self.send(LIGHTING[k], &p, false)?;
        }
        Ok(())
    }

    fn resize_zone(&mut self, zone: usize, leds: usize) -> Result<()> {
        if zone == 1 && self.info.zones.len() > 1 {
            self.info.zones[1] = Zone::resizable("Ventilateurs RGB", leds.clamp(1, 32), 1, 32);
        }
        Ok(())
    }

    fn keepalive(&self) -> Option<Duration> {
        Some(Duration::from_secs(5))
    }

    fn max_fps(&self) -> u32 {
        30
    }

    fn release(&mut self) {
        let _ = self.hardware();
    }
}
