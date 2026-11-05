// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Souris HyperX Pulsefire Surge, FPS Pro, Core et Haste. Ces souris
//! reprennent leur effet interne si elles ne reçoivent plus de trame, d'où un
//! renvoi périodique. Protocole d'après OpenRGB.

use super::hid::{self, entry, HidEntry};
use crate::device::{Device, Result};
use crate::model::*;
use hidapi::{DeviceInfo as HidInfo, HidApi, HidDevice};
use std::time::Duration;

const SURGE: &[u16] = &[0x16D3, 0x0490];
const FPS_PRO: &[u16] = &[0x16D7, 0x16DE, 0x0D8F];
const HASTE: &[u16] = &[0x1727, 0x0F8F];

pub const HID: &[HidEntry] = &[
    entry(0x0951, SURGE, Some(0xFF01), None, Some(1), open),
    entry(0x03F0, SURGE, Some(0xFF01), None, Some(1), open),
    entry(0x0951, FPS_PRO, Some(0xFF01), None, Some(1), open),
    entry(0x03F0, FPS_PRO, Some(0xFF01), None, Some(1), open),
    entry(0x0951, HASTE, Some(0xFF90), None, Some(3), open),
    entry(0x03F0, HASTE, Some(0xFF90), None, Some(3), open),
];

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Surge,
    FpsPro,
    Haste,
}

pub struct Pulsefire {
    info: DeviceInfo,
    dev: HidDevice,
    kind: Kind,
}

fn open(api: &HidApi, i: &HidInfo) -> Result<Vec<Box<dyn Device>>> {
    let dev = api.open_path(i.path())?;
    let pid = i.product_id();
    let kind = if SURGE.contains(&pid) {
        Kind::Surge
    } else if HASTE.contains(&pid) {
        Kind::Haste
    } else {
        Kind::FpsPro
    };
    let zones = match kind {
        Kind::Surge => vec![Zone::linear("Anneau", 32), Zone::single("Logo")],
        _ => vec![Zone::single("Logo")],
    };
    Ok(vec![Box::new(Pulsefire {
        info: DeviceInfo {
            id: hid::stable_id("hyperx-mouse", i),
            name: pretty_name(&hid::product(i, "HyperX Pulsefire")),
            vendor: "HyperX".into(),
            kind: DeviceKind::Mouse,
            location: hid::location(i),
            zones,
            modes: Vec::new(),
            direct: true,
        },
        dev,
        kind,
    })])
}

impl Device for Pulsefire {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, _mode: usize, _p: &HwParams) -> Result<()> {
        Ok(())
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        let c0 = colors.first().copied().unwrap_or(Color::BLACK);
        match self.kind {
            Kind::Surge => {
                // plans séparés : 32 rouges, 32 verts, 32 bleus, puis le logo
                let mut b = [0u8; 264];
                b[0] = 0x07;
                b[1] = 0x14;
                b[3] = 0xA0;
                for (k, c) in colors.iter().take(32).enumerate() {
                    b[0x08 + k] = c.r;
                    b[0x28 + k] = c.g;
                    b[0x48 + k] = c.b;
                }
                let logo = colors.get(32).copied().unwrap_or(Color::BLACK);
                b[0x6C..0x6F].copy_from_slice(&[logo.r, logo.g, logo.b]);
                self.dev.send_feature_report(&b)?;
            }
            Kind::FpsPro => {
                let mut b = [0u8; 264];
                b[0] = 0x07;
                b[1] = 0x0A;
                b[2..5].copy_from_slice(&[c0.r, c0.g, c0.b]);
                b[8] = 0xA0;
                self.dev.send_feature_report(&b)?;
            }
            Kind::Haste => {
                let mut b = [0u8; 65];
                b[1] = 0x04;
                b[2] = 0xF2;
                b[8] = 0x02;
                self.dev.send_feature_report(&b)?;
                let mut b = [0u8; 65];
                b[1] = 0x81;
                b[2..5].copy_from_slice(&[c0.r, c0.g, c0.b]);
                b[8] = 0x02;
                self.dev.send_feature_report(&b)?;
            }
        }
        Ok(())
    }

    fn keepalive(&self) -> Option<Duration> {
        Some(Duration::from_millis(50))
    }

    fn max_fps(&self) -> u32 {
        30
    }
}
