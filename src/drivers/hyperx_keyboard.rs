// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Claviers HyperX Alloy Origins (pleine taille). Protocole d'après OpenRGB.

use super::hid::{self, entry, HidEntry};
use crate::device::{Device, Result};
use crate::layout;
use crate::model::*;
use hidapi::{DeviceInfo as HidInfo, HidApi, HidDevice};

pub const HID: &[HidEntry] = &[
    entry(0x0951, &[0x16E5], None, None, Some(3), open),
    entry(0x0951, &[0x16E5], None, None, Some(0), open),
    entry(0x03F0, &[0x0591], None, None, Some(3), open),
    entry(0x03F0, &[0x0591], None, None, Some(0), open),
];

/// Ordre des LED dans le clavier (colonne par colonne) ; `None` = emplacement vide.
const ORDER: &[Option<&str>] = &[
    Some("Échap"), Some("²"), Some("Tab"), Some("Verr. maj"), Some("Maj gauche"), Some("Ctrl gauche"), Some("<"),
    Some("1"), Some("Q"), Some("A"), Some("Z"), Some("Windows gauche"), Some("F1"), Some("2"), Some("W"), Some("S"),
    Some("X"), Some("Alt"), Some("F2"), Some("3"), Some("E"), Some("D"), Some("C"), None, Some("F3"), Some("4"),
    Some("R"), Some("F"), Some("V"), None, Some("F4"), Some("5"), Some("T"), Some("G"), Some("B"), Some("Espace"),
    Some("F5"), Some("6"), Some("Y"), Some("H"), Some("N"), None, Some("F6"), Some("7"), Some("U"), Some("J"),
    Some("M"), None, Some("F7"), Some("8"), Some("I"), Some("K"), Some(","), Some("Alt Gr"), Some("F8"), Some("9"),
    Some("O"), Some("L"), Some("."), None, Some("F9"), Some("0"), Some("P"), Some(";"), Some("/"), Some("Fn"),
    Some("F10"), Some("-"), Some("["), Some("'"), None, None, Some("F11"), Some("="), Some("]"), Some("#"), None,
    Some("Menu"), Some("F12"), Some("Retour arrière"), Some("\\"), Some("Entrée"), Some("Maj droite"),
    Some("Ctrl droite"), Some("Impr. écran"), Some("Inser"), Some("Suppr"), None, None, Some("Gauche"),
    Some("Arrêt défil."), Some("Origine"), Some("Fin"), None, Some("Haut"), Some("Bas"), Some("Pause"),
    Some("Page préc."), Some("Page suiv."), None, None, Some("Droite"), None, Some("Verr. num"), Some("Num 7"),
    Some("Num 4"), Some("Num 1"), Some("Num 0"), None, Some("Num /"), Some("Num 8"), Some("Num 5"), Some("Num 2"),
    None, None, Some("Num *"), Some("Num 9"), Some("Num 6"), Some("Num 3"), Some("Num ."), None, Some("Num -"),
    Some("Num +"), None, None, Some("Num Entrée"),
];

pub struct AlloyOrigins {
    info: DeviceInfo,
    dev: HidDevice,
    slots: Vec<usize>,
}

fn open(api: &HidApi, i: &HidInfo) -> Result<Vec<Box<dyn Device>>> {
    let dev = api.open_path(i.path())?;
    let keys: Vec<(&str, usize)> = ORDER.iter().enumerate().filter_map(|(k, n)| n.map(|n| (n, k))).collect();
    let (zone, slots) = layout::keyboard_zone("Touches", &keys, &[]);
    Ok(vec![Box::new(AlloyOrigins {
        info: DeviceInfo {
            id: hid::stable_id("hyperx-kb", i),
            name: "HyperX Alloy Origins".into(),
            vendor: "HyperX".into(),
            kind: DeviceKind::Keyboard,
            location: hid::location(i),
            zones: vec![zone],
            modes: Vec::new(),
            direct: true,
        },
        dev,
        slots,
    })])
}

impl Device for AlloyOrigins {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, _mode: usize, _p: &HwParams) -> Result<()> {
        Ok(())
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        let mut frame = vec![Color::BLACK; ORDER.len()];
        for (i, &s) in self.slots.iter().enumerate() {
            frame[s] = colors.get(i).copied().unwrap_or(Color::BLACK);
        }
        // initialisation du mode direct, puis 9 paquets de 16 LED
        let mut b = [0u8; 65];
        b[1] = 0x04;
        b[2] = 0xF2;
        b[9] = 0x09;
        self.dev.send_feature_report(&b)?;
        for pkt in 0..9 {
            let mut p = [0u8; 65];
            for k in 0..16 {
                let idx = pkt * 16 + k;
                if idx >= frame.len() {
                    break;
                }
                let c = frame[idx];
                p[k * 4 + 1..k * 4 + 5].copy_from_slice(&[0x81, c.r, c.g, c.b]);
            }
            self.dev.send_feature_report(&p)?;
        }
        Ok(())
    }

    fn max_fps(&self) -> u32 {
        30
    }
}
