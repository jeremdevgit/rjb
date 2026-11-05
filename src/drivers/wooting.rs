// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Claviers Wooting (One, Two, Two HE, 60HE, 80HE, UwU), firmware récents.
//! Les couleurs sont envoyées en RGB565 sur une matrice 6 × 21. Protocole
//! d'après OpenRGB.

use super::hid::{self, entry, HidEntry};
use crate::device::{Device, Result};
use crate::layout;
use crate::model::*;
use hidapi::{DeviceInfo as HidInfo, HidApi, HidDevice};

const VID: u16 = 0x31E3;
/// Chaque modèle existe en trois PID (Xinput, DirectInput, aucun).
const PIDS: &[u16] = &[
    0x1100, 0x1101, 0x1102, 0x1200, 0x1201, 0x1202, 0x1210, 0x1211, 0x1212, 0x1220, 0x1221, 0x1222, 0x1230, 0x1231,
    0x1232, 0x1300, 0x1301, 0x1302, 0x1310, 0x1311, 0x1312, 0x1320, 0x1321, 0x1322, 0x1340, 0x1341, 0x1342, 0x1400,
    0x1401, 0x1402, 0x1510, 0x1511, 0x1512,
];

pub const HID: &[HidEntry] = &[
    entry(VID, PIDS, Some(0x1337), None, None, open),
    entry(VID, PIDS, Some(0xFF55), None, None, open),
];

const COLS: usize = 21;

/// Touches de la matrice, ligne par ligne ; `""` = emplacement vide.
const MATRIX: [[&str; COLS]; 6] = [
    [
        "Échap", "", "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "Impr. écran", "Pause",
        "Arrêt défil.", "#A1", "#A2", "#A3", "#Mode",
    ],
    [
        "²", "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-", "=", "Retour arrière", "Inser", "Origine", "Page préc.",
        "Verr. num", "Num /", "Num *", "Num -",
    ],
    [
        "Tab", "Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P", "[", "]", "\\", "Suppr", "Fin", "Page suiv.", "Num 7",
        "Num 8", "Num 9", "Num +",
    ],
    [
        "Verr. maj", "A", "S", "D", "F", "G", "H", "J", "K", "L", ";", "'", "#", "Entrée", "", "", "", "Num 4", "Num 5",
        "Num 6", "",
    ],
    [
        "Maj gauche", "<", "Z", "X", "C", "V", "B", "N", "M", ",", ".", "/", "", "Maj droite", "", "Haut", "", "Num 1",
        "Num 2", "Num 3", "Num Entrée",
    ],
    [
        "Ctrl gauche", "Windows gauche", "Alt", "", "", "", "Espace", "", "", "", "Alt Gr", "Windows droite", "Fn",
        "Ctrl droite", "Gauche", "Bas", "Droite", "", "Num 0", "Num .", "",
    ],
];

pub struct Wooting {
    info: DeviceInfo,
    dev: HidDevice,
    v3: bool,
    slots: Vec<usize>,
}

impl Wooting {
    fn command(&self, cmd: u8) -> Result<()> {
        self.dev.send_feature_report(&[0, 0xD0, 0xDA, cmd, 0, 0, 0, 0])?;
        Ok(())
    }
}

fn open(api: &HidApi, i: &HidInfo) -> Result<Vec<Box<dyn Device>>> {
    let dev = api.open_path(i.path())?;
    let base = i.product_id() & 0xFFF0;
    // colonnes réellement présentes selon le format
    let cols = match base {
        0x1100 => 17,
        0x1300 | 0x1310 | 0x1320 | 0x1340 => 14,
        0x1400 => 17,
        0x1510 => 7,
        _ => COLS,
    };
    let mut keys: Vec<(&str, usize)> = Vec::new();
    let mut extra: Vec<(String, usize)> = Vec::new();
    for (r, row) in MATRIX.iter().enumerate() {
        for (c, &n) in row.iter().enumerate().take(cols) {
            match n {
                "" => {}
                _ if n.starts_with('#') => extra.push((n[1..].to_string(), r * COLS + c)),
                _ => keys.push((n, r * COLS + c)),
            }
        }
    }
    let (zone, slots) = layout::keyboard_zone("Touches", &keys, &extra);
    let w = Wooting {
        info: DeviceInfo {
            id: hid::stable_id("wooting", i),
            name: pretty_name(&hid::product(i, "Wooting")),
            vendor: "Wooting".into(),
            kind: DeviceKind::Keyboard,
            location: hid::location(i),
            zones: vec![zone],
            modes: vec![ModeInfo::new("Profil du clavier")],
            direct: true,
        },
        dev,
        v3: i.usage_page() == 0xFF55,
        slots,
    };
    w.command(33)?;
    Ok(vec![Box::new(w)])
}

impl Device for Wooting {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, _mode: usize, _p: &HwParams) -> Result<()> {
        // rend la main aux couleurs du profil enregistré dans le clavier
        self.command(32)
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        let mut b = vec![0u8; if self.v3 { 2046 } else { 257 }];
        b[..4].copy_from_slice(if self.v3 { &[4, 0xD1, 0xDA, 11] } else { &[0, 0xD0, 0xDA, 11] });
        for (k, &s) in self.slots.iter().enumerate() {
            let c = colors.get(k).copied().unwrap_or(Color::BLACK);
            let v = ((c.r as u16 & 0xF8) << 8) | ((c.g as u16 & 0xFC) << 3) | (c.b as u16 >> 3);
            b[4 + s * 2..6 + s * 2].copy_from_slice(&v.to_le_bytes());
        }
        self.dev.write(&b)?;
        Ok(())
    }

    fn max_fps(&self) -> u32 {
        60
    }

    fn release(&mut self) {
        let _ = self.command(32);
    }
}
