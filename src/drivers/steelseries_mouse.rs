// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Souris SteelSeries Rival 3 et Aerox 3 / 5 (filaires). Protocole d'après OpenRGB.

use super::hid::{self, entry, HidEntry};
use crate::device::{Device, Result};
use crate::model::*;
use hidapi::{DeviceInfo as HidInfo, HidApi, HidDevice};

const VID: u16 = 0x1038;
const RIVAL3: &[u16] = &[0x184C, 0x1824];
const AEROX3: u16 = 0x1836;
const AEROX5: u16 = 0x1850;

pub const HID: &[HidEntry] = &[
    entry(VID, RIVAL3, None, None, Some(3), open),
    entry(VID, &[AEROX3, AEROX5], Some(0xFFC0), Some(1), Some(3), open),
];

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Rival3,
    Aerox3,
    Aerox5,
}

pub struct SsMouse {
    info: DeviceInfo,
    dev: HidDevice,
    kind: Kind,
    brightness: Option<u8>,
}

fn open(api: &HidApi, i: &HidInfo) -> Result<Vec<Box<dyn Device>>> {
    let dev = api.open_path(i.path())?;
    let pid = i.product_id();
    let kind = match pid {
        AEROX3 => Kind::Aerox3,
        AEROX5 => Kind::Aerox5,
        _ => Kind::Rival3,
    };
    let names: &[&str] = match kind {
        Kind::Rival3 => &["Avant", "Milieu", "Arrière", "Logo"],
        Kind::Aerox3 => &["Avant", "Milieu", "Arrière"],
        Kind::Aerox5 => &["Avant", "Milieu", "Arrière", "Réactif"],
    };
    if kind != Kind::Rival3 {
        let mut b = [0u8; 65];
        b[1] = 0x2D;
        dev.send_feature_report(&b)?;
    }
    let mut modes = Vec::new();
    if kind == Kind::Rival3 {
        modes.push(ModeInfo::new("Cycle de couleurs").speed());
        modes.push(ModeInfo::new("Respiration").speed());
        modes.push(ModeInfo::new("Respiration arc-en-ciel").speed());
    }
    Ok(vec![Box::new(SsMouse {
        info: DeviceInfo {
            id: hid::stable_id("ss-mouse", i),
            name: pretty_name(&hid::product(i, "SteelSeries")),
            vendor: "SteelSeries".into(),
            kind: DeviceKind::Mouse,
            location: hid::location(i),
            zones: names.iter().map(|n| Zone::single(*n)).collect(),
            modes,
            direct: true,
        },
        dev,
        kind,
        brightness: None,
    })])
}

impl SsMouse {
    fn write(&self, p: &[u8], size: usize) -> Result<()> {
        let mut b = vec![0u8; size];
        b[..p.len()].copy_from_slice(p);
        self.dev.write(&b)?;
        Ok(())
    }
}

impl Device for SsMouse {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, mode: usize, p: &HwParams) -> Result<()> {
        if self.kind != Kind::Rival3 {
            return Ok(());
        }
        // effets : cycle 0x04, respiration 0x03, respiration arc-en-ciel 0x05 (moins la vitesse)
        let base = [0x04u8, 0x03, 0x05][mode.min(2)];
        let effect = base.saturating_sub(p.speed_in(0, 2) as u8);
        self.write(&[0x00, 0x06, 0x00, effect], 4)
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        match self.kind {
            Kind::Rival3 => {
                for (z, c) in colors.iter().enumerate().take(4) {
                    self.write(&[0x00, 0x05, 0x00, z as u8 + 1, c.r, c.g, c.b, 100], 8)?;
                }
                self.write(&[0x00, 0x06, 0x00, 0x00], 4)
            }
            Kind::Aerox3 => {
                for (z, c) in colors.iter().enumerate().take(3) {
                    let mut b = vec![0u8; 3 + (z + 1) * 3];
                    b[1] = 0x21;
                    b[2] = 1 << z;
                    b[3 + z * 3..6 + z * 3].copy_from_slice(&[c.r, c.g, c.b]);
                    self.write(&b, 65)?;
                }
                self.brightness(100)
            }
            Kind::Aerox5 => {
                for (z, c) in colors.iter().enumerate().take(4) {
                    let (cmd, mask, off) = match z {
                        0 => (0x21, 0x01, 3),
                        1 => (0x21, 0x02, 6),
                        2 => (0x21, 0x04, 9),
                        _ => (0x26, 0x01, 4),
                    };
                    let mut b = vec![0u8; off + 3];
                    b[1] = cmd;
                    b[2] = mask;
                    b[off..off + 3].copy_from_slice(&[c.r, c.g, c.b]);
                    self.write(&b, 65)?;
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
                self.brightness(100)
            }
        }
    }

    fn max_fps(&self) -> u32 {
        30
    }
}

impl SsMouse {
    fn brightness(&mut self, b: u8) -> Result<()> {
        if self.brightness != Some(b) {
            self.write(&[0x00, 0x23, b], 65)?;
            self.brightness = Some(b);
        }
        Ok(())
    }
}
