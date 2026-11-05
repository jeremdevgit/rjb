// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Périphériques Corsair de première génération (protocole d'avant iCUE 4) :
//! claviers K55, K65, K68, K70 (RGB, LUX, RAPIDFIRE, MK.2), K95, Strafe,
//! souris Glaive, Harpoon, Ironclaw, M65, Nightsword, Sabre, Scimitar,
//! tapis MM800 Polaris et support de casque ST100. Protocole d'après OpenRGB.

use super::corsair_v1_table as table;
use super::hid::{self, HidEntry};
use crate::device::{Device, Result};
use crate::layout;
use crate::model::*;
use hidapi::{DeviceInfo as HidInfo, HidApi, HidDevice};

pub const HID: &[HidEntry] = &[HidEntry {
    vid: 0x1B1C,
    pids: &[
        0x1B3D, 0x1B17, 0x1B37, 0x1B39, 0x1B4F, 0x1B13, 0x1B33, 0x1B38, 0x1B49, 0x1B6B, 0x1B55, 0x1BC6, 0x1B11, 0x1B2D,
        0x1B82, 0x1B20, 0x1B44, 0x1B48, 0x1B36, 0x1B3F, 0x1BA5, 0x1B34, 0x1B74, 0x1B3C, 0x1B75, 0x1B5D, 0x1B12, 0x1B2E,
        0x1B5A, 0x1B5C, 0x1B1E, 0x1B3E, 0x1B8B, 0x1B2F, 0x1B32, 0x1B3B, 0x0A34,
    ],
    usage_page: Some(0xFFC2),
    usage: None,
    interface: None,
    open,
}];

const WRITE: u8 = 0x07;
const READ: u8 = 0x0E;
const STREAM: u8 = 0x7F;

#[derive(Clone, Copy, PartialEq)]
enum Layout {
    Normal,
    K95Plat,
    K95,
    K55,
    K70Mk2,
    K68,
    K70LuxRgb,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Keyboard(Layout),
    Mouse,
    Mousemat,
    Stand,
}

pub struct CorsairV1 {
    info: DeviceInfo,
    dev: HidDevice,
    kind: Kind,
    iso: bool,
    /// position sur le fil de chaque LED (clavier) ou identifiant (souris)
    wire: Vec<u8>,
}

/// Ordre des LED du support ST100 : bande de 8 LED puis logo.
const ST100: [usize; 9] = [0, 1, 2, 3, 5, 6, 7, 8, 4];

fn layout_of(pid: u16) -> Layout {
    match pid {
        0x1B2D => Layout::K95Plat,
        0x1B11 => Layout::K95,
        0x1B3D => Layout::K55,
        0x1B33 => Layout::K70LuxRgb,
        0x1B38 | 0x1B49 | 0x1B6B | 0x1B55 | 0x1B48 => Layout::K70Mk2,
        0x1B4F => Layout::K68,
        _ => Layout::Normal,
    }
}

impl CorsairV1 {
    fn send(&self, b: &[u8]) -> Result<()> {
        let mut p = [0u8; 65];
        p[1..1 + b.len()].copy_from_slice(b);
        self.dev.write(&p)?;
        Ok(())
    }

    fn special_function(&self, software: bool) -> Result<()> {
        self.send(&[WRITE, 0x04, if software { 0x02 } else { 0x01 }])
    }

    fn lighting_control(&self) -> Result<()> {
        let level = match self.kind {
            Kind::Mouse => 0x01,
            Kind::Mousemat => 0x04,
            _ => 0x03,
        };
        self.send(&[WRITE, 0x05, 0x02, 0x00, level])
    }

    fn needs_special(&self) -> bool {
        matches!(self.kind, Kind::Keyboard(Layout::K55 | Layout::K95Plat | Layout::K70Mk2 | Layout::K68))
    }

    /// K55, K68, K70 MK.2, K95 Platinum : déclaration des identifiants de touche.
    fn setup_identifiers(&self) -> Result<()> {
        self.send(&[WRITE, 0x05, 0x08, 0x00, 0x01])?;
        let skip: &[u8] = match (self.iso, self.kind) {
            (true, Kind::Keyboard(Layout::K70Mk2 | Layout::K70LuxRgb)) => {
                &[0x3f, 0x41, 0x42, 0x50, 0x53, 0x55, 0x6f, 0x78, 0x79, 0x7a, 0x7b, 0x7c, 0x7d, 0x7e, 0x7f, 0x80, 0x81]
            }
            (true, _) => &[0x3f, 0x41, 0x42, 0x50, 0x53, 0x55, 0x6f, 0x78, 0x7e, 0x7f, 0x80, 0x81],
            _ => &[0x31, 0x3f, 0x41, 0x42, 0x51, 0x53, 0x55, 0x6f, 0x7e, 0x7f, 0x80, 0x81],
        };
        let mut id: u8 = 0;
        for _ in 0..4 {
            let mut b = vec![WRITE, 0x40, 0x1E, 0x00];
            for _ in 0..30 {
                for &s in skip {
                    if id == s {
                        id = id.wrapping_add(1);
                    }
                }
                b.extend_from_slice(&[id, 0xC0]);
                id = id.wrapping_add(1);
            }
            self.send(&b)?;
        }
        Ok(())
    }

    fn software(&self) -> Result<()> {
        if self.needs_special() || !matches!(self.kind, Kind::Keyboard(_)) {
            self.special_function(true)?;
        }
        self.lighting_control()?;
        if self.needs_special() {
            self.setup_identifiers()?;
        }
        Ok(())
    }

    fn stream(&self, id: u8, data: &[u8]) -> Result<()> {
        let mut b = vec![STREAM, id, data.len() as u8, 0x00];
        b.extend_from_slice(data);
        self.send(&b)
    }

    fn keyboard_full(&self, colors: &[Color], wide: bool) -> Result<()> {
        let mut planes = [[0u8; 168]; 3];
        for (i, &pos) in self.wire.iter().enumerate() {
            let c = colors.get(i).copied().unwrap_or(Color::BLACK);
            planes[0][pos as usize] = c.r;
            planes[1][pos as usize] = c.g;
            planes[2][pos as usize] = c.b;
        }
        let last = if wide { 48 } else { 24 };
        for (ch, plane) in planes.iter().enumerate() {
            self.stream(1, &plane[0..60])?;
            self.stream(2, &plane[60..120])?;
            self.stream(3, &plane[120..120 + last])?;
            self.send(&[WRITE, 0x28, ch as u8 + 1, 3, if ch == 2 { 2 } else { 1 }])?;
        }
        Ok(())
    }
}

fn open(api: &HidApi, i: &HidInfo) -> Result<Vec<Box<dyn Device>>> {
    let dev = api.open_path(i.path())?;
    // type d'appareil et disposition (ANSI / ISO) lus dans le firmware
    let mut r = [0u8; 65];
    let mut off = 0;
    for _ in 0..3 {
        let mut q = [0u8; 65];
        q[1] = READ;
        q[2] = 0x01;
        dev.write(&q)?;
        let n = dev.read_timeout(&mut r, 1000)?;
        off = 0;
        if n == 0 {
            dev.send_feature_report(&q)?;
            r = [0u8; 65];
            r[0] = 0;
            let _ = dev.get_feature_report(&mut r);
            off = 1;
        }
        if r[0x14 + off] == 0xC0 {
            break;
        }
    }
    let pid = i.product_id();
    let kind = match r[0x14 + off] {
        0xC0 => Kind::Keyboard(layout_of(pid)),
        0xC1 => Kind::Mouse,
        0xC2 if pid == 0x0A34 => Kind::Stand,
        0xC2 => Kind::Mousemat,
        _ => {
            crate::log(&format!("Corsair V1 {pid:#06x} : type inconnu {:#04x}", r[0x14 + off]));
            return Ok(Vec::new());
        }
    };
    let iso = r[0x17 + off] == 0x01;

    let mut wire = Vec::new();
    let zones = match kind {
        Kind::Keyboard(Layout::K55) => {
            wire = vec![0, 1, 2];
            vec![Zone::single("Gauche"), Zone::single("Centre"), Zone::single("Droite")]
        }
        Kind::Keyboard(l) => {
            let t = match l {
                Layout::K95Plat => table::K95_PLAT,
                Layout::K95 => table::K95,
                Layout::K70Mk2 => table::K70_MK2,
                Layout::K70LuxRgb => table::K70_LUX_RGB,
                _ => table::NORMAL,
            };
            let mut keys: Vec<(&str, u8)> = Vec::new();
            let mut extra: Vec<(String, u8)> = Vec::new();
            for &(pos, name) in t {
                if layout::is_key(name) {
                    keys.push((name, pos));
                } else {
                    extra.push((name.to_string(), pos));
                }
            }
            let (zone, ids) = layout::keyboard_zone("Touches", &keys, &extra);
            wire = ids;
            vec![zone]
        }
        Kind::Mouse => {
            let leds: &[(u8, &str)] = match pid {
                0x1B5A | 0x1B5D => &[(2, "Logo"), (3, "DPI"), (4, "Molette")],
                0x1B2F | 0x1B32 => &[(1, "Dessous"), (2, "Logo"), (3, "DPI"), (4, "Molette")],
                0x1B75 => &[(3, "Logo")],
                // zones non documentées : une couleur envoyée à toutes
                _ => &[(0xFF, "Éclairage")],
            };
            wire = leds.iter().map(|l| l.0).collect();
            leds.iter().map(|l| Zone::single(l.1)).collect()
        }
        Kind::Mousemat => vec![Zone::linear("Bordure", 15)],
        Kind::Stand => vec![Zone::linear("Base", 8), Zone::single("Logo")],
    };
    let d = CorsairV1 {
        info: DeviceInfo {
            id: hid::stable_id("corsair-v1", i),
            name: pretty_name(&hid::product(i, "Corsair")),
            vendor: "Corsair".into(),
            kind: match kind {
                Kind::Keyboard(_) => DeviceKind::Keyboard,
                Kind::Mouse => DeviceKind::Mouse,
                Kind::Mousemat => DeviceKind::Mousemat,
                Kind::Stand => DeviceKind::HeadsetStand,
            },
            location: hid::location(i),
            zones,
            modes: vec![ModeInfo::new("Effet de l'appareil")],
            direct: true,
        },
        dev,
        kind,
        iso,
        wire,
    };
    d.software()?;
    Ok(vec![Box::new(d)])
}

impl Device for CorsairV1 {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, _mode: usize, _p: &HwParams) -> Result<()> {
        // retour à l'éclairage enregistré dans l'appareil
        self.special_function(false)?;
        self.send(&[WRITE, 0x05, 0x01])
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        let c = |i: usize| colors.get(i).copied().unwrap_or(Color::BLACK);
        match self.kind {
            Kind::Keyboard(Layout::K55) => {
                let mut b = vec![WRITE, 0x25, 0, 0];
                for k in 0..3 {
                    b.extend_from_slice(&[c(k).r, c(k).g, c(k).b]);
                }
                self.send(&b)
            }
            Kind::Keyboard(l) => self.keyboard_full(colors, matches!(l, Layout::K95 | Layout::K95Plat)),
            Kind::Mouse => {
                let mut ids: Vec<(u8, Color)> = Vec::new();
                for (k, &w) in self.wire.iter().enumerate() {
                    if w == 0xFF {
                        ids.extend((0..15).map(|id| (id, c(k))));
                    } else {
                        ids.push((w, c(k)));
                    }
                }
                let mut b = vec![WRITE, 0x22, ids.len() as u8, 0];
                for (id, col) in ids.iter().take(15) {
                    b.extend_from_slice(&[*id, col.r, col.g, col.b]);
                }
                self.send(&b)
            }
            Kind::Mousemat | Kind::Stand => {
                let n = if self.kind == Kind::Stand { 9 } else { 15 };
                let mut pos = vec![Color::BLACK; n];
                for k in 0..n {
                    let at = if self.kind == Kind::Stand { ST100[k] } else { k };
                    pos[at] = c(k);
                }
                let mut b = vec![WRITE, 0x22, n as u8, 0];
                for p in pos {
                    b.extend_from_slice(&[p.r, p.g, p.b]);
                }
                self.send(&b)
            }
        }
    }

    fn max_fps(&self) -> u32 {
        30
    }

    fn release(&mut self) {
        let _ = self.set_mode(0, &HwParams::default());
    }
}
