// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Claviers SteelSeries Apex 5 / 7 / 9 / Pro (filaires et sans fil).
//! Chaque touche est adressée par son code d'usage HID. Protocole d'après OpenRGB.

use super::hid::{self, entry, HidEntry};
use crate::device::{Device, Result};
use crate::layout;
use crate::model::*;
use hidapi::{DeviceInfo as HidInfo, HidApi, HidDevice};

const VID: u16 = 0x1038;
const WIRED: &[u16] = &[0x161C, 0x1612, 0x1618, 0x1634, 0x1620, 0x1610, 0x1614, 0x1628, 0x1642, 0x1640];
const WIRELESS: &[u16] = &[0x1630, 0x1632, 0x1644, 0x1646];
const GEN2: &[u16] = &[0x1634, 0x1620, 0x1628];

pub const HID: &[HidEntry] = &[
    entry(VID, WIRED, None, None, Some(1), open),
    entry(VID, WIRELESS, Some(0xFFC0), Some(1), Some(3), open),
];

/// Codes des touches dans l'ordre attendu par le clavier.
const KEYS: &[u8] = &[
    0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17,
    0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x2B,
    0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x3F, 0x40,
    0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F, 0x50, 0x51, 0x52, 0x64, 0xE0,
    0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xF0, 0x31, 0x87, 0x88, 0x89, 0x8A, 0x8B, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58,
    0x59, 0x5A, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F, 0x60, 0x61, 0x62, 0x63, 0xFB,
];

pub struct Apex {
    info: DeviceInfo,
    dev: HidDevice,
    direct_id: u8,
    onboard_id: u8,
    packet_len: usize,
    codes: Vec<u8>,
    onboard: bool,
}

fn open(api: &HidApi, i: &HidInfo) -> Result<Vec<Box<dyn Device>>> {
    let dev = api.open_path(i.path())?;
    let pid = i.product_id();
    let wireless = WIRELESS.contains(&pid);
    let gen3 = wireless || matches!(pid, 0x1642 | 0x1640);
    let modern = gen3 || GEN2.contains(&pid);
    if gen3 {
        let mut b = [0u8; 65];
        b[1] = 0x4B;
        dev.send_feature_report(&b)?;
    }
    let mut keys: Vec<(&str, u8)> = Vec::new();
    let mut extra: Vec<(String, u8)> = Vec::new();
    for &k in KEYS {
        match layout::hid_usage_key(k as u16) {
            Some(n) => keys.push((n, k)),
            None => extra.push((format!("Touche {k:#04x}"), k)),
        }
    }
    let (zone, codes) = layout::keyboard_zone("Touches", &keys, &extra);
    Ok(vec![Box::new(Apex {
        info: DeviceInfo {
            id: hid::stable_id("ss-apex", i),
            name: pretty_name(&hid::product(i, "SteelSeries Apex")),
            vendor: "SteelSeries".into(),
            kind: DeviceKind::Keyboard,
            location: hid::location(i),
            zones: vec![zone],
            modes: vec![ModeInfo::new("Effet de l'appareil")],
            direct: true,
        },
        dev,
        direct_id: if wireless { 0x61 } else if modern { 0x40 } else { 0x3A },
        onboard_id: if gen3 { 0x41 } else { 0x3B },
        packet_len: if matches!(pid, 0x1634 | 0x1620) { 513 } else { 643 },
        codes,
        onboard: false,
    })])
}

impl Device for Apex {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, _mode: usize, _p: &HwParams) -> Result<()> {
        let mut b = [0u8; 65];
        b[1] = self.onboard_id;
        self.dev.write(&b)?;
        self.onboard = true;
        Ok(())
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        self.onboard = false;
        let max = (self.packet_len - 3) / 4;
        let n = self.codes.len().min(colors.len()).min(max);
        let mut b = vec![0u8; self.packet_len];
        b[1] = self.direct_id;
        b[2] = n as u8;
        for k in 0..n {
            let c = colors[k];
            b[3 + k * 4..7 + k * 4].copy_from_slice(&[self.codes[k], c.r, c.g, c.b]);
        }
        self.dev.send_feature_report(&b)?;
        Ok(())
    }

    fn max_fps(&self) -> u32 {
        30
    }

    fn release(&mut self) {
        if !self.onboard {
            let _ = self.set_mode(0, &HwParams::default());
        }
    }
}
