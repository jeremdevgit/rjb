// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Razer Chroma (claviers, souris, tapis, casques USB, accessoires…) : protocole
//! à rapports de 90 octets. Base d'appareils générée depuis OpenRGB.

use super::hid::{self, HidEntry};
use super::razer_table::{DETECT, DEVICES};
use crate::device::{Device, Result};
use crate::model::*;
use hidapi::{DeviceInfo as HidInfo, HidApi, HidDevice};

pub const HID: &[HidEntry] = &[HidEntry {
    vid: 0x1532,
    pids: &[],
    usage_page: None,
    usage: None,
    interface: None,
    open,
}];

/// Zone : (nom, lignes, colonnes, type 0 = unique, 1 = linéaire, 2 = matrice)
pub struct RazerZone(pub &'static str, pub u8, pub u8, pub u8);

/// Appareil : (nom, PID, type, matrice, transaction, lignes, colonnes, zones)
#[allow(clippy::type_complexity)]
pub struct RazerDevice(
    pub &'static str,
    pub u16,
    pub DeviceKind,
    pub u8,
    pub u8,
    pub u8,
    pub u8,
    pub &'static [RazerZone],
);

const STANDARD: u8 = 1;
const EXTENDED: u8 = 2;
const LINEAR: u8 = 3;
const EXTENDED_ARGB: u8 = 4;

/// Appareils dont les effets visent l'identifiant de LED 0 (et non le rétroéclairage).
const LED_ID_ZERO: &[u16] = &[
    0x0086, 0x0088, 0x0099, 0x00CB, 0x00B9, 0x00AA, 0x00AB, 0x00CC, 0x00CD, 0x00AC, 0x00D6, 0x00D7, 0x00D8, 0x0F08,
    0x0F20, 0x028C, 0x029D, 0x028A, 0x0F26, 0x0F09, 0x00AF, 0x00B0, 0x0F1A, 0x005C, 0x0084, 0x008C, 0x0098, 0x0298,
    0x0296, 0x0292, 0x0290, 0x0C04, 0x0C08, 0x0068, 0x0C02, 0x0C01, 0x0C06, 0x0F0D, 0x0F2B, 0x0532, 0x054A, 0x006C,
    0x0069, 0x0F1D, 0x00A4, 0x0093, 0x008D, 0x00A7, 0x00A8, 0x00E7, 0x00E8, 0x0F13, 0x0C05, 0x0244, 0x022B,
];

fn zone_name(n: &str) -> String {
    match n {
        "ZONE_EN_KEYBOARD" => "Touches".into(),
        "Scroll Wheel" => "Molette".into(),
        "Underglow" => "Éclairage inférieur".into(),
        "Left Side" => "Côté gauche".into(),
        "Right Side" => "Côté droit".into(),
        "Keypad" | "Numpad" => "Pavé".into(),
        "Thumb Buttons" | "Numpad Buttons" => "Boutons latéraux".into(),
        "Lightbar" | "Light Bar" => "Barre lumineuse".into(),
        "Headset Stand" => "Support".into(),
        "Base" => "Socle".into(),
        other if other.starts_with("ZONE_EN_") => other["ZONE_EN_".len()..].to_lowercase(),
        other => other.to_string(),
    }
}

fn open(api: &HidApi, i: &HidInfo) -> Result<Vec<Box<dyn Device>>> {
    let pid = i.product_id();
    let Some(d) = DEVICES.iter().find(|d| d.1 == pid) else {
        return Ok(Vec::new());
    };
    // ne garder que l'interface de commande prévue pour ce modèle
    let rules: Vec<_> = DETECT.iter().filter(|r| r.0 == pid).collect();
    let ok = rules.is_empty()
        || rules.iter().any(|&&(_, iface, page, usage)| {
            (iface < 0 || iface == i.interface_number())
                && (page == 0 || page == i.usage_page())
                && (usage == 0 || usage == i.usage())
        });
    if !ok {
        return Ok(Vec::new());
    }
    let dev = api.open_path(i.path())?;

    // zones et positions des LED (ordre ligne par ligne de la matrice)
    let mut zones = Vec::new();
    for z in d.7 {
        let (rows, cols) = (z.1.max(1) as usize, z.2.max(1) as usize);
        let mut leds = Vec::with_capacity(rows * cols);
        for r in 0..rows {
            for c in 0..cols {
                leds.push(Led {
                    name: if rows * cols == 1 { zone_name(z.0) } else { format!("LED {}", r * cols + c + 1) },
                    x: if cols > 1 { c as f32 / (cols - 1) as f32 } else { 0.5 },
                    y: if rows > 1 { r as f32 / (rows - 1) as f32 } else { 0.5 },
                    w: 1.0,
                });
            }
        }
        zones.push(Zone {
            name: zone_name(z.0),
            leds,
            resizable: None,
            grid: (z.3 == 2 && rows > 1).then_some((cols as u16, rows as u16)),
        });
    }
    if zones.is_empty() {
        return Ok(Vec::new());
    }
    let extended = matches!(d.3, EXTENDED | EXTENDED_ARGB);
    let mut modes = vec![
        ModeInfo::new("Statique").colors(1, 1),
        ModeInfo::new("Respiration").colors(1, 2).random(),
        ModeInfo::new("Cycle de couleurs"),
        ModeInfo::new("Vague").dirs(&[Direction::Right, Direction::Left]),
    ];
    if extended {
        modes.push(ModeInfo::new("Scintillement").colors(1, 2).random().speed());
    }
    modes.push(ModeInfo::new("Éteint").off());
    let mut r = Razer {
        info: DeviceInfo {
            id: hid::stable_id("razer", i),
            name: d.0.to_string(),
            vendor: "Razer".into(),
            kind: d.2,
            location: hid::location(i),
            zones,
            modes,
            direct: matches!(d.3, STANDARD | EXTENDED | LINEAR),
        },
        dev,
        tid: d.4,
        matrix: d.3,
        rows: d.5 as usize,
        cols: d.6 as usize,
        led_id: if LED_ID_ZERO.contains(&pid) { 0x00 } else { 0x05 },
        report_id: if pid == 0x0532 || pid == 0x054A { 0x07 } else { 0x00 },
        in_custom: false,
    };
    // quelques accessoires doivent passer en mode logiciel
    if matches!(pid, 0x0F26 | 0x0F07 | 0x0068) {
        let _ = r.send(0x00, 0x04, &[0x03, 0x00]);
    }
    r.in_custom = false;
    Ok(vec![Box::new(r)])
}

pub struct Razer {
    info: DeviceInfo,
    dev: HidDevice,
    tid: u8,
    matrix: u8,
    rows: usize,
    cols: usize,
    led_id: u8,
    report_id: u8,
    in_custom: bool,
}

impl Razer {
    /// Envoie un rapport (classe, commande, arguments) avec sa somme de contrôle.
    fn send(&self, class: u8, cmd: u8, args: &[u8]) -> Result<()> {
        let mut r = [0u8; 91];
        r[0] = self.report_id;
        r[2] = self.tid;
        r[6] = args.len() as u8;
        r[7] = class;
        r[8] = cmd;
        let n = args.len().min(80);
        r[9..9 + n].copy_from_slice(&args[..n]);
        r[89] = r[3..89].iter().fold(0u8, |a, b| a ^ b);
        self.dev.send_feature_report(&r)?;
        Ok(())
    }

    /// Envoie un rapport dont la taille déclarée diffère de la longueur réelle.
    fn send_sized(&self, class: u8, cmd: u8, size: u8, args: &[u8]) -> Result<()> {
        let mut r = [0u8; 91];
        r[0] = self.report_id;
        r[2] = self.tid;
        r[6] = size;
        r[7] = class;
        r[8] = cmd;
        let n = args.len().min(80);
        r[9..9 + n].copy_from_slice(&args[..n]);
        r[89] = r[3..89].iter().fold(0u8, |a, b| a ^ b);
        self.dev.send_feature_report(&r)?;
        Ok(())
    }

    fn effect_ext(&self, effect: u8, args: &[u8]) -> Result<()> {
        let mut a = vec![0x00, self.led_id, effect];
        a.extend_from_slice(args);
        let size = a.len().max(6) as u8;
        self.send_sized(0x0F, 0x02, size, &a)
    }

    fn row(&self, row: usize, rgb: &[u8]) -> Result<()> {
        let stop = (self.cols - 1) as u8;
        match self.matrix {
            STANDARD => {
                let mut a = vec![0xFF, row as u8, 0, stop];
                a.extend_from_slice(rgb);
                self.send(0x03, 0x0B, &a)
            }
            LINEAR => {
                let mut a = vec![0, stop];
                a.extend_from_slice(rgb);
                self.send_sized(0x03, 0x0C, 0x32, &a)
            }
            _ => {
                let mut a = vec![0, 0, row as u8, 0, stop];
                a.extend_from_slice(rgb);
                self.send(0x0F, 0x03, &a)
            }
        }
    }

    fn custom_mode(&self) -> Result<()> {
        match self.matrix {
            STANDARD | LINEAR => self.send_sized(0x03, 0x0A, 0x02, &[0x05, 0x00]),
            EXTENDED => self.send_sized(0x0F, 0x02, 0x0C, &[0x00, 0x00, 0x08]),
            _ => Ok(()),
        }
    }
}

impl Device for Razer {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, mode: usize, p: &HwParams) -> Result<()> {
        self.in_custom = false;
        let m = self.info.modes[mode].clone();
        let (c1, c2) = (p.color(0), p.color(1));
        let ext = matches!(self.matrix, EXTENDED | EXTENDED_ARGB);
        let two = p.colors.len() >= 2;
        let random = m.random && p.random;
        match (m.name.as_str(), ext) {
            ("Éteint", true) => self.effect_ext(0x00, &[]),
            ("Éteint", false) => self.send(0x03, 0x0A, &[0x00]),
            ("Statique", true) => self.send_sized(0x0F, 0x02, 0x09, &[0, self.led_id, 0x01, 0, 0, 0x01, c1.r, c1.g, c1.b]),
            ("Statique", false) => self.send(0x03, 0x0A, &[0x06, c1.r, c1.g, c1.b]),
            ("Respiration", true) if random => self.effect_ext(0x02, &[]),
            ("Respiration", true) if two => self.send_sized(
                0x0F,
                0x02,
                0x0C,
                &[0, self.led_id, 0x02, 0x02, 0, 0x02, c1.r, c1.g, c1.b, c2.r, c2.g, c2.b],
            ),
            ("Respiration", true) => self.send_sized(0x0F, 0x02, 0x09, &[0, self.led_id, 0x02, 0x01, 0, 0x01, c1.r, c1.g, c1.b]),
            ("Respiration", false) if random => self.send_sized(0x03, 0x0A, 0x08, &[0x03, 0x03]),
            ("Respiration", false) if two => self.send_sized(0x03, 0x0A, 0x08, &[0x03, 0x02, c1.r, c1.g, c1.b, c2.r, c2.g, c2.b]),
            ("Respiration", false) => self.send_sized(0x03, 0x0A, 0x08, &[0x03, 0x01, c1.r, c1.g, c1.b]),
            ("Cycle de couleurs", true) => self.effect_ext(0x03, &[]),
            ("Cycle de couleurs", false) => self.send(0x03, 0x0A, &[0x04]),
            ("Vague", ext) => {
                let dir = if p.direction == Direction::Left { 2 } else { 1 };
                if ext {
                    self.effect_ext(0x04, &[dir, 0x28])
                } else {
                    self.send(0x03, 0x0A, &[0x01, dir])
                }
            }
            ("Scintillement", _) => {
                let speed = p.speed_in(3, 1) as u8;
                if random {
                    self.send_sized(0x0F, 0x02, 0x06, &[0, self.led_id, 0x07, 0x00, speed, 0])
                } else if two {
                    self.send_sized(0x0F, 0x02, 0x0C, &[0, self.led_id, 0x07, 0x02, speed, 2, c1.r, c1.g, c1.b, c2.r, c2.g, c2.b])
                } else {
                    self.send_sized(0x0F, 0x02, 0x09, &[0, self.led_id, 0x07, 0x01, speed, 1, c1.r, c1.g, c1.b])
                }
            }
            _ => Ok(()),
        }
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        let total = self.rows * self.cols;
        for r in 0..self.rows {
            let mut rgb = Vec::with_capacity(self.cols * 3);
            for c in 0..self.cols {
                let k = r * self.cols + c;
                let col = if k < total { colors.get(k).copied().unwrap_or(Color::BLACK) } else { Color::BLACK };
                rgb.extend_from_slice(&[col.r, col.g, col.b]);
            }
            self.row(r, &rgb)?;
        }
        // affiche l'image envoyée (mode « image personnalisée »)
        self.in_custom = true;
        self.custom_mode()
    }

    fn max_fps(&self) -> u32 {
        30
    }
}
