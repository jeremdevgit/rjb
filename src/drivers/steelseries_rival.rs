// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Souris SteelSeries Rival 100/105/106/110, 300, 310, 5, 600, 650, 700/710,
//! Sensei TEN / 310 et tapis QCK Prism. Protocole d'après OpenRGB.

use super::hid::{self, entry, HidEntry};
use crate::device::{Device, Result};
use crate::model::*;
use hidapi::{DeviceInfo as HidInfo, HidApi, HidDevice};

const VID: u16 = 0x1038;
const RIVAL_100: &[u16] = &[0x1702, 0x170C, 0x1814, 0x1816, 0x1729];
const RIVAL_300: &[u16] = &[0x1710, 0x1714, 0x1394, 0x1716, 0x171A, 0x1392, 0x1718];
const SENSEI: &[u16] = &[0x1720, 0x171E, 0x1736, 0x1832, 0x1834, 0x1722];
const RIVAL_600: &[u16] = &[0x1724, 0x172E];
const RIVAL_650: &[u16] = &[0x172B, 0x1726];
const RIVAL_700: &[u16] = &[0x1700, 0x1730];
const RIVAL_5: &[u16] = &[0x183C];
const QCK: &[u16] = &[0x150A, 0x150D, 0x151E, 0x1514, 0x151C, 0x1516, 0x1518, 0x151A, 0x1520];

pub const HID: &[HidEntry] = &[
    entry(VID, RIVAL_100, None, None, Some(0), open),
    entry(VID, RIVAL_300, None, None, Some(0), open),
    entry(VID, SENSEI, None, None, Some(0), open),
    entry(VID, RIVAL_600, None, None, Some(0), open),
    entry(VID, RIVAL_650, None, None, Some(0), open),
    entry(VID, RIVAL_700, None, None, Some(0), open),
    entry(VID, RIVAL_5, Some(0xFFC0), Some(1), Some(0), open),
    entry(VID, QCK, None, None, Some(0), open),
];

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Rival100,
    Rival300,
    Sensei,
    Rival600,
    Rival650,
    Rival700,
    Rival5,
    Qck,
}

pub struct Rival {
    info: DeviceInfo,
    dev: HidDevice,
    kind: Kind,
    /// identifiant matériel de chaque LED
    ids: Vec<u8>,
    sent: Vec<Color>,
    direct_set: bool,
}

fn open(api: &HidApi, i: &HidInfo) -> Result<Vec<Box<dyn Device>>> {
    let dev = api.open_path(i.path())?;
    let pid = i.product_id();
    let kind = if RIVAL_100.contains(&pid) {
        Kind::Rival100
    } else if RIVAL_300.contains(&pid) {
        Kind::Rival300
    } else if SENSEI.contains(&pid) {
        Kind::Sensei
    } else if RIVAL_600.contains(&pid) {
        Kind::Rival600
    } else if RIVAL_650.contains(&pid) {
        Kind::Rival650
    } else if RIVAL_700.contains(&pid) {
        Kind::Rival700
    } else if RIVAL_5.contains(&pid) {
        Kind::Rival5
    } else {
        Kind::Qck
    };
    const SIDES_600: [(u8, &str); 6] = [
        (2, "Gauche haut"), (4, "Gauche milieu"), (6, "Gauche bas"), (3, "Droite haut"), (5, "Droite milieu"), (7, "Droite bas"),
    ];
    const SIDES_650: [(u8, &str); 6] = [
        (0x12, "Gauche 1"), (0x14, "Gauche 2"), (0x16, "Gauche 3"), (0x13, "Droite 1"), (0x15, "Droite 2"), (0x17, "Droite 3"),
    ];
    let (zones, ids): (Vec<Zone>, Vec<u8>) = match kind {
        Kind::Rival100 => (vec![Zone::single("Logo")], vec![0]),
        Kind::Rival300 | Kind::Rival700 | Kind::Sensei => (vec![Zone::single("Logo"), Zone::single("Molette")], vec![0, 1]),
        Kind::Rival600 | Kind::Rival650 => {
            let (logo, wheel, sides) = if kind == Kind::Rival600 { (1, 0, SIDES_600) } else { (0x11, 0x10, SIDES_650) };
            let mut side = Zone::linear("Côtés", 6);
            for (l, s) in side.leds.iter_mut().zip(sides.iter()) {
                l.name = s.1.to_string();
            }
            let mut ids = vec![logo, wheel];
            ids.extend(sides.iter().map(|s| s.0));
            (vec![Zone::single("Logo"), Zone::single("Molette"), side], ids)
        }
        Kind::Rival5 => (vec![Zone::linear("Souris", 10)], (0..10).collect()),
        Kind::Qck => (vec![Zone::single("Bas"), Zone::single("Haut")], vec![0, 1]),
    };
    Ok(vec![Box::new(Rival {
        info: DeviceInfo {
            id: hid::stable_id("ss-rival", i),
            name: pretty_name(&hid::product(i, "SteelSeries")),
            vendor: "SteelSeries".into(),
            kind: if kind == Kind::Qck { DeviceKind::Mousemat } else { DeviceKind::Mouse },
            location: hid::location(i),
            zones,
            modes: Vec::new(),
            direct: true,
        },
        dev,
        kind,
        ids,
        sent: Vec::new(),
        direct_set: false,
    })])
}

impl Rival {
    /// Rapport de sortie précédé de l'identifiant 0.
    fn out(&self, data: &[u8], size: usize) -> Result<()> {
        let mut b = vec![0u8; size + 1];
        b[1..1 + data.len()].copy_from_slice(data);
        self.dev.write(&b)?;
        Ok(())
    }

    fn set_one(&self, id: u8, c: Color) -> Result<()> {
        match self.kind {
            Kind::Rival100 => self.out(&[0x05, 0x00, c.r, c.g, c.b], 9),
            Kind::Rival300 => self.out(&[0x08, id + 1, c.r, c.g, c.b], 9),
            Kind::Sensei => {
                let mut b = [0u8; 65];
                b[0] = 0x5B;
                b[2] = id;
                b[0x13] = 0x01;
                b[0x1B] = 0x01;
                b[0x1C..0x1F].copy_from_slice(&[c.r, c.g, c.b]);
                b[0x1F..0x22].copy_from_slice(&[c.r, c.g, c.b]);
                self.out(&b, 65)
            }
            Kind::Rival600 => {
                // ce modèle attend le rapport tel quel (0x1C fait office d'identifiant)
                self.dev.write(&[0x1C, 0x27, 0x00, 1 << id, c.r, c.g, c.b])?;
                Ok(())
            }
            Kind::Rival650 => {
                let mut b = [0u8; 60];
                b[0x00] = 0x03;
                b[0x04] = 0x30;
                b[0x06] = 0x10;
                b[0x07] = 0x27;
                b[0x16] = 0x01;
                b[0x1E] = 0x04;
                b[0x1F..0x22].copy_from_slice(&[c.r, c.g, c.b]);
                b[0x22] = 0xFF;
                b[0x27] = 0xFF;
                b[0x29] = 0x54;
                b[0x2C] = 0xFF;
                b[0x2D] = 0x54;
                b[0x2E..0x31].copy_from_slice(&[c.r, c.g, c.b]);
                b[0x31] = 0x56;
                self.out(&b, 60)?;
                let mut b = [0u8; 60];
                b[0] = 0x03;
                b[2] = 0x30;
                b[4] = 0x2C;
                self.out(&b, 60)?;
                let mut b = [0u8; 60];
                b[0] = 0x05;
                b[2] = id;
                b[3] = 0xFF;
                b[8] = 0x5C;
                self.out(&b, 60)?;
                let mut b = [0u8; 60];
                b[0] = 0x1C;
                b[2] = 0x55;
                b[4] = 0x46;
                self.out(&b, 60)
            }
            Kind::Rival700 => {
                let mut b = vec![0u8; 579];
                b[1] = 0x05;
                b[3] = id;
                b[4..7].copy_from_slice(&[c.r, c.g, c.b]);
                b[0x0C] = id;
                b[0x0D] = 0x01;
                self.dev.send_feature_report(&b)?;
                Ok(())
            }
            Kind::Rival5 | Kind::Qck => Ok(()),
        }
    }

    /// Effet « fixe » nécessaire avant les couleurs sur les anciens modèles.
    fn direct_effect(&mut self) -> Result<()> {
        if self.direct_set {
            return Ok(());
        }
        match self.kind {
            Kind::Rival100 => self.out(&[0x07, 0x00, 0x01], 9)?,
            Kind::Rival300 => {
                self.out(&[0x07, 0x01, 0x01], 9)?;
                self.out(&[0x07, 0x02, 0x01], 9)?;
            }
            _ => {}
        }
        self.direct_set = true;
        Ok(())
    }
}

impl Device for Rival {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, _mode: usize, _p: &HwParams) -> Result<()> {
        Ok(())
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        let c = |i: usize| colors.get(i).copied().unwrap_or(Color::BLACK);
        match self.kind {
            Kind::Rival5 => {
                let mut b = [0u8; 64];
                b[1] = 0x21;
                b[2] = 0xFF;
                b[3] = 0x03;
                for k in 0..10 {
                    b[4 + k * 3..7 + k * 3].copy_from_slice(&[c(k).r, c(k).g, c(k).b]);
                }
                self.dev.send_feature_report(&b)?;
            }
            Kind::Qck => {
                let mut b = vec![0u8; 525];
                b[0x01] = 0x0E;
                b[0x03] = 0x02;
                b[0x05..0x08].copy_from_slice(&[c(0).r, c(0).g, c(0).b]);
                b[0x08] = 0xFF;
                b[0x09] = 0x32;
                b[0x0A] = 0xC8;
                b[0x0E] = 0x01;
                b[0x11..0x14].copy_from_slice(&[c(1).r, c(1).g, c(1).b]);
                b[0x14] = 0xFF;
                b[0x15] = 0x32;
                b[0x16] = 0xC8;
                b[0x19] = 0x01;
                b[0x1A] = 0x01;
                b[0x1C] = 0x01;
                self.dev.send_feature_report(&b)?;
                let mut a = [0u8; 65];
                a[1] = 0x0D;
                self.dev.write(&a)?;
            }
            _ => {
                self.direct_effect()?;
                // une commande par LED : seulement celles qui changent
                for (k, &id) in self.ids.clone().iter().enumerate() {
                    if self.sent.get(k) != Some(&c(k)) {
                        self.set_one(id, c(k))?;
                    }
                }
            }
        }
        self.sent = colors.to_vec();
        Ok(())
    }

    fn max_fps(&self) -> u32 {
        match self.kind {
            Kind::Rival650 | Kind::Rival700 => 10,
            _ => 25,
        }
    }
}
