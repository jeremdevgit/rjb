// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! MSI Mystic Light USB (cartes mères MSI 2017-2024, rapports 112/162/185 octets).
//! Protocole d'après OpenRGB.

use super::hid::{self, HidEntry};
use crate::device::{Device, Result};
use crate::model::*;
use hidapi::{DeviceInfo as HidInfo, HidApi, HidDevice};

const VID: u16 = 0x1462;
const VID_COMMON: u16 = 0x0DB0;
const PID_COMMON: u16 = 0x0076;

pub const HID: &[HidEntry] = &[
    HidEntry { vid: VID, pids: &[], usage_page: Some(0x0001), usage: Some(0x00), interface: None, open },
    HidEntry { vid: VID_COMMON, pids: &[PID_COMMON], usage_page: Some(0x0001), usage: Some(0x00), interface: None, open },
];

#[derive(Clone, Copy, PartialEq, Debug)]
enum Z {
    Jrgb1,
    Jrgb2,
    Jrainbow1,
    Jrainbow2,
    Jrainbow3,
    Jcorsair,
    Pipe1,
    Pipe2,
    Onboard,
}

#[derive(Clone, Copy, PartialEq)]
enum Direct {
    Off,
    PerLed,
    ZoneBased,
}

const SET0: &[Z] = &[Z::Jrgb1, Z::Jrgb2, Z::Jrainbow1, Z::Jrainbow2, Z::Onboard];
const SET1: &[Z] = &[Z::Jrgb1, Z::Jrainbow1, Z::Jrainbow2, Z::Jcorsair];
const SET2: &[Z] = &[Z::Jrgb1, Z::Jrainbow1, Z::Jrainbow2, Z::Onboard];
const SET3: &[Z] = &[Z::Jrgb1, Z::Jrainbow1, Z::Jrainbow2, Z::Jcorsair, Z::Pipe1, Z::Onboard];
const SET5: &[Z] = &[Z::Jrgb1, Z::Jrainbow1];
const SET6: &[Z] = &[Z::Jrainbow1];
const SET7: &[Z] = &[Z::Jrgb1, Z::Jrgb2, Z::Jrainbow1, Z::Jrainbow2, Z::Pipe1, Z::Onboard];
const SET8: &[Z] = &[Z::Jrgb1, Z::Jrainbow1, Z::Jrainbow2, Z::Jcorsair, Z::Pipe1, Z::Pipe2];
const SET9: &[Z] = &[Z::Jrgb1, Z::Jrainbow1, Z::Jrainbow2, Z::Jcorsair, Z::Pipe1];
const SET10: &[Z] = &[Z::Jrgb1, Z::Jrainbow1, Z::Jrainbow2, Z::Jrainbow3];
const SET11: &[Z] = &[Z::Jrgb1, Z::Jrainbow1, Z::Jrainbow2];
const SET12: &[Z] = &[Z::Jrgb1, Z::Jrainbow1, Z::Jrainbow2, Z::Jrainbow3, Z::Pipe1, Z::Onboard];
const SET13: &[Z] = &[Z::Jrgb1, Z::Jrgb2, Z::Jrainbow1, Z::Jrainbow2];
const SET14: &[Z] = &[Z::Jrainbow1, Z::Jrainbow2];
const SET15: &[Z] = &[Z::Jrgb1, Z::Jrainbow1, Z::Jrainbow2, Z::Jrainbow3, Z::Pipe1, Z::Pipe2, Z::Onboard];
const SET16: &[Z] = &[Z::Jrgb1, Z::Jrainbow1, Z::Jrainbow2, Z::Jrainbow3, Z::Pipe1, Z::Pipe2];
const SET17: &[Z] = &[Z::Jrgb1, Z::Jrgb2, Z::Jrainbow1, Z::Jrainbow2, Z::Pipe1];
const SET18: &[Z] = &[Z::Jrgb1, Z::Jrainbow1, Z::Jrainbow2, Z::Jrainbow3, Z::Pipe1];
const SET19: &[Z] = &[Z::Jrainbow1, Z::Jrainbow2, Z::Jrainbow3];
const SET20: &[Z] = &[Z::Jrgb1];
const ALL: &[Z] = &[Z::Jrgb1, Z::Jrgb2, Z::Jrainbow1, Z::Jrainbow2, Z::Jcorsair, Z::Pipe1, Z::Pipe2, Z::Onboard];

use Direct::{PerLed as P, ZoneBased as ZB};

/// (PID, LED carte, LED pipe 1, LED pipe 2, nb JRGB, zones, mode direct, firmware min)
const BOARDS: &[(u16, usize, usize, usize, usize, &[Z], Direct, u8)] = &[
    (0x7B93, 6, 1, 0, 1, SET3, ZB, 0), (0x7C34, 0, 1, 1, 1, SET8, ZB, 0), (0x7C35, 0, 1, 0, 1, SET9, ZB, 0),
    (0x7C36, 6, 1, 0, 1, SET3, ZB, 0), (0x7C37, 6, 0, 0, 2, SET0, ZB, 0), (0x7C56, 6, 0, 0, 1, SET2, P, 6),
    (0x7C59, 0, 8, 0, 1, SET9, Direct::Off, 0), (0x7C60, 6, 0, 0, 1, SET1, P, 0), (0x7C67, 6, 0, 0, 1, SET5, P, 0),
    (0x7C71, 6, 6, 0, 1, SET3, P, 0), (0x7C73, 6, 4, 0, 1, SET3, P, 0), (0x7C75, 6, 0, 0, 1, SET2, P, 0),
    (0x7C76, 6, 0, 0, 2, SET0, Direct::Off, 0), (0x7C77, 0, 0, 0, 0, SET14, P, 0), (0x7C79, 6, 0, 0, 2, SET0, P, 0),
    (0x7C80, 6, 0, 0, 2, SET0, P, 0), (0x7C02, 6, 0, 0, 2, SET0, P, 0), (0x7C81, 6, 0, 0, 2, SET0, P, 0),
    (0x7C82, 6, 0, 0, 1, SET5, P, 0), (0x7C83, 6, 0, 0, 1, SET5, Direct::Off, 0), (0x7C84, 6, 0, 0, 2, SET0, ZB, 0),
    (0x7C86, 6, 0, 0, 1, SET5, Direct::Off, 0), (0x7C87, 6, 0, 0, 2, SET13, P, 0), (0x7C90, 6, 4, 0, 1, SET3, P, 0),
    (0x7C91, 6, 0, 0, 2, SET0, P, 0), (0x7C92, 6, 0, 0, 0, SET6, P, 0), (0x7C94, 6, 0, 0, 1, SET11, P, 0),
    (0x7C95, 6, 0, 0, 2, SET0, P, 0), (0x7C98, 6, 0, 0, 1, SET5, P, 0), (0x7D03, 0, 15, 18, 1, SET8, P, 0),
    (0x7D06, 4, 4, 0, 1, SET3, P, 0), (0x7D07, 4, 5, 0, 2, SET7, P, 0), (0x7D08, 6, 0, 0, 2, SET0, P, 0),
    (0x7D09, 6, 0, 0, 2, SET11, P, 0), (0x7D13, 6, 0, 0, 1, SET1, P, 0), (0x7D14, 6, 0, 0, 1, SET5, P, 0),
    (0x7D15, 6, 0, 0, 2, SET0, P, 0), (0x7D17, 6, 0, 0, 2, SET11, P, 0), (0x7D18, 6, 0, 0, 2, SET5, P, 0),
    (0x7D19, 6, 0, 0, 2, SET5, P, 0), (0x7D20, 6, 0, 0, 2, SET5, P, 0), (0x7D25, 6, 0, 0, 2, SET11, P, 0),
    (0x7D27, 6, 0, 0, 2, SET1, P, 0), (0x7D28, 6, 0, 0, 1, SET1, P, 0), (0x7D29, 6, 0, 0, 0, SET6, P, 0),
    (0x7D30, 6, 6, 0, 2, SET3, P, 0), (0x7D31, 4, 8, 0, 2, SET12, P, 0), (0x7D32, 1, 0, 0, 1, SET10, P, 0),
    (0x7D33, 6, 0, 0, 2, SET5, P, 0), (0x7D36, 6, 0, 0, 2, SET5, P, 0), (0x7D37, 6, 0, 0, 2, SET11, P, 0),
    (0x7D38, 0, 0, 0, 1, SET1, P, 0), (0x7D40, 0, 0, 0, 1, SET5, P, 0), (0x7D41, 6, 0, 0, 2, SET13, P, 0),
    (0x7D42, 6, 0, 0, 2, SET11, P, 0), (0x7D43, 0, 0, 0, 2, SET11, P, 0), (0x7D46, 0, 1, 1, 0, SET11, P, 0),
    (0x7D50, 6, 12, 0, 1, SET3, P, 0), (0x7D51, 6, 0, 0, 2, SET1, P, 0), (0x7D52, 6, 14, 0, 1, SET3, P, 0),
    (0x7D53, 6, 0, 0, 2, SET0, P, 0), (0x7D54, 6, 0, 0, 2, SET0, P, 0), (0x7D59, 0, 0, 0, 2, SET11, P, 0),
    (0x7D67, 0, 0, 0, 1, SET11, P, 0), (0x7D69, 9, 2, 4, 1, SET15, P, 0), (0x7D70, 0, 6, 0, 1, SET9, P, 0),
    (0x7D73, 1, 0, 0, 0, SET6, P, 0), (0x7D74, 0, 6, 0, 1, SET18, P, 0), (0x7D75, 0, 0, 0, 2, SET13, P, 0),
    (0x7D76, 0, 0, 0, 2, SET13, P, 0), (0x7D77, 0, 0, 0, 2, SET13, P, 0), (0x7D78, 0, 0, 0, 2, SET13, P, 0),
    (0x7D86, 0, 18, 4, 1, SET16, P, 0), (0x7D88, 0, 0, 0, 1, SET20, P, 0), (0x7D89, 0, 6, 0, 1, SET18, P, 0),
    (0x7D90, 0, 0, 0, 1, SET11, P, 0), (0x7D91, 1, 0, 0, 1, SET10, P, 0), (0x7D93, 6, 0, 0, 1, SET2, P, 0),
    (0x7D96, 0, 0, 0, 2, SET13, P, 0), (0x7D97, 6, 0, 0, 2, SET11, P, 0), (0x7D98, 0, 0, 0, 1, SET11, P, 0),
    (0x7D99, 6, 0, 0, 1, SET11, P, 0), (0x7E01, 0, 0, 0, 1, SET11, P, 0), (0x7E03, 6, 0, 0, 0, SET6, P, 0),
    (0x7E06, 0, 0, 0, 2, SET11, P, 0), (0x7E07, 0, 0, 0, 2, SET10, P, 0), (0x7E09, 0, 0, 0, 0, SET19, P, 0),
    (0x7E10, 0, 6, 0, 2, SET17, P, 0), (0x7E12, 0, 0, 0, 2, SET13, P, 0), (0x0076, 0, 0, 0, 2, SET13, P, 0),
    (0x7E16, 0, 0, 0, 2, SET13, P, 0), (0x7E24, 0, 0, 0, 2, SET13, P, 0), (0x7E26, 0, 0, 0, 2, SET13, P, 0),
    (0x7E27, 0, 0, 0, 2, SET13, P, 0), (0x7E28, 6, 0, 0, 2, ALL, ZB, 0), (0x7E30, 0, 0, 0, 2, SET13, P, 0),
];

/// Effets matériels : (nom, code, couleur réglable)
const MODES: &[(&str, u8, bool)] = &[
    ("Statique", 1, true),
    ("Respiration", 2, true),
    ("Flash", 3, true),
    ("Double flash", 4, true),
    ("Éclair", 5, true),
    ("Météore", 7, true),
    ("Anneau de couleurs", 15, false),
    ("Planétaire", 16, false),
    ("Double météore", 17, false),
    ("Énergie", 18, false),
    ("Clignotement", 19, true),
    ("Horloge", 20, false),
    ("Pulsation", 21, true),
    ("Changement de couleur", 22, false),
    ("Vague de couleurs", 23, true),
    ("Défilement", 24, true),
    ("Vague arc-en-ciel", 26, false),
    ("Visière", 27, true),
    ("Flash arc-en-ciel", 29, false),
    ("Empilement", 36, true),
    ("Feu", 38, false),
];

const SYNC_RAINBOW: usize = 40;
const SYNC_CORSAIR: usize = 120;

#[derive(Clone, Copy, PartialEq)]
enum Variant {
    V112,
    V162,
    V185,
}

pub struct MysticLight {
    info: DeviceInfo,
    dev: HidDevice,
    variant: Variant,
    zones: Vec<Z>,
    direct_kind: Direct,
    onboard: usize,
    pipe1: usize,
    pipe2: usize,
    jrgbs: usize,
    mixed: bool,
    in_direct: bool,
}

fn open(api: &HidApi, i: &HidInfo) -> Result<Vec<Box<dyn Device>>> {
    let dev = api.open_path(i.path())?;
    let mut buf = [0u8; 200];
    buf[0] = 0x52;
    let len = dev.get_feature_report(&mut buf).unwrap_or(0);
    let variant = match len {
        185 | 186 => Variant::V185,
        162 | 163 => Variant::V162,
        112 | 113 => Variant::V112,
        _ => {
            crate::log(&format!("MSI Mystic Light {:04x} : rapport de {len} octets non pris en charge", i.product_id()));
            return Ok(Vec::new());
        }
    };
    let mut pid = i.product_id();
    if pid == PID_COMMON {
        if let Some(p) = i.serial_number().and_then(|s| u16::from_str_radix(s.get(..4)?, 16).ok()) {
            pid = p;
        }
    }
    // version du firmware (une seule carte exige un minimum)
    let mut fw = 0xFFu8;
    let mut req = [0xCCu8; 64];
    req[0] = 0x01;
    req[1] = 0xB0;
    if dev.write(&req).is_ok() {
        let mut r = [0u8; 64];
        if dev.read_timeout(&mut r, 300).unwrap_or(0) > 2 {
            fw = r[2];
        }
    }
    let cfg = BOARDS.iter().find(|b| b.0 == pid);
    let (onboard, pipe1, pipe2, jrgbs, zones, mut direct_kind) = match cfg {
        Some(&(_, on, p1, p2, jr, z, d, min)) => {
            let d = if fw < min { Direct::Off } else { d };
            (on, p1, p2, jr, z.to_vec(), d)
        }
        None => (6, 1, 1, 2, ALL.to_vec(), Direct::Off),
    };
    if variant != Variant::V185 {
        // anciennes cartes : une couleur par zone
        direct_kind = Direct::ZoneBased;
    }
    let zones: Vec<Z> = zones
        .into_iter()
        .filter(|z| match variant {
            Variant::V185 => true,
            Variant::V162 => matches!(z, Z::Jrgb1 | Z::Jrgb2 | Z::Jrainbow1 | Z::Jcorsair | Z::Onboard),
            Variant::V112 => matches!(z, Z::Jrgb1 | Z::Jrainbow1 | Z::Jcorsair | Z::Onboard),
        })
        .collect();

    let mut info_zones = Vec::new();
    for z in &zones {
        let per_led = direct_kind == Direct::PerLed;
        info_zones.push(match z {
            Z::Jrgb1 => Zone::single("JRGB 1"),
            Z::Jrgb2 => Zone::single("JRGB 2"),
            Z::Pipe1 => Zone::linear("Tuyau 1", pipe1.max(1)),
            Z::Pipe2 => Zone::linear("Tuyau 2", pipe2.max(1)),
            Z::Onboard => Zone::linear("Carte mère", onboard.max(1)),
            Z::Jrainbow1 | Z::Jrainbow2 if per_led => {
                let n = if *z == Z::Jrainbow1 { "JRAINBOW 1" } else { "JRAINBOW 2" };
                Zone::resizable(n, 20, 1, SYNC_RAINBOW as u16)
            }
            Z::Jrainbow3 | Z::Jcorsair if per_led => {
                Zone::resizable(if *z == Z::Jcorsair { "JCORSAIR" } else { "JRAINBOW 3" }, 20, 1, SYNC_CORSAIR as u16)
            }
            Z::Jrainbow1 => Zone::single("JRAINBOW 1"),
            Z::Jrainbow2 => Zone::single("JRAINBOW 2"),
            Z::Jrainbow3 => Zone::single("JRAINBOW 3"),
            Z::Jcorsair => Zone::single("JCORSAIR"),
        });
    }
    let mut modes: Vec<ModeInfo> = MODES
        .iter()
        .map(|m| {
            let mut i = ModeInfo::new(m.0).brightness();
            if m.1 != 1 {
                i = i.speed();
            }
            if m.2 {
                i = i.colors(1, 2);
                if m.1 != 1 {
                    i = i.random();
                }
            }
            i
        })
        .collect();
    modes.push(ModeInfo::new("Éteint").off());
    let (_, board) = super::smbus::board();
    Ok(vec![Box::new(MysticLight {
        info: DeviceInfo {
            id: hid::stable_id("msi-ml", i),
            name: if board.is_empty() { "MSI Mystic Light".into() } else { format!("MSI {}", pretty_name(&board)) },
            vendor: "MSI".into(),
            kind: DeviceKind::Motherboard,
            location: hid::location(i),
            zones: info_zones,
            modes,
            direct: direct_kind != Direct::Off,
        },
        dev,
        variant,
        zones,
        direct_kind,
        onboard,
        pipe1,
        pipe2,
        jrgbs,
        mixed: pid >= 0x7D03,
        in_direct: false,
    })])
}

impl MysticLight {
    fn size(&self) -> usize {
        match self.variant {
            Variant::V112 => 112,
            Variant::V162 => 162,
            Variant::V185 => 185,
        }
    }

    /// Position du bloc d'une zone dans le rapport de configuration.
    fn offset(&self, z: Z) -> Option<usize> {
        match self.variant {
            Variant::V185 => Some(match z {
                Z::Jrgb1 => 1,
                Z::Pipe1 => 11,
                Z::Pipe2 => 21,
                Z::Jrainbow1 => 31,
                Z::Jrainbow2 => 42,
                Z::Jrainbow3 | Z::Jcorsair => 53,
                Z::Onboard => 74,
                Z::Jrgb2 if self.mixed => 134,
                Z::Jrgb2 => 174,
            }),
            Variant::V162 => match z {
                Z::Jrgb1 => Some(1),
                Z::Jrainbow1 => Some(11),
                Z::Jcorsair => Some(21),
                Z::Onboard => Some(41),
                Z::Jrgb2 => Some(151),
                _ => None,
            },
            Variant::V112 => match z {
                Z::Jrgb1 => Some(1),
                Z::Jrainbow1 => Some(11),
                Z::Jcorsair => Some(21),
                Z::Onboard => Some(41),
                _ => None,
            },
        }
    }

    /// Bloc de la LED n de la carte (on_board_led_1..).
    fn onboard_led_offset(&self, n: usize) -> Option<usize> {
        let base = match self.variant {
            Variant::V185 => 84,
            _ => 51,
        };
        let max = match self.variant {
            Variant::V185 => 9,
            Variant::V162 => 10,
            Variant::V112 => 6,
        };
        if n >= max {
            return None;
        }
        if self.variant == Variant::V185 && self.mixed && n == 5 {
            return Some(64); // on_board_led_6 -> j_corsair_outerll120
        }
        Some(base + n * 10)
    }

    fn read_config(&self) -> Vec<u8> {
        let mut buf = vec![0u8; self.size()];
        buf[0] = 0x52;
        let _ = self.dev.get_feature_report(&mut buf);
        buf[0] = 0x52;
        buf
    }

    fn write_zone(p: &mut [u8], o: usize, effect: u8, c1: Color, c2: Color, speed_bright: u8, custom: bool) {
        p[o] = effect;
        p[o + 1..o + 4].copy_from_slice(&[c1.r, c1.g, c1.b]);
        p[o + 4] = speed_bright;
        p[o + 5..o + 8].copy_from_slice(&[c2.r, c2.g, c2.b]);
        p[o + 8] = if custom { p[o + 8] | 0x80 } else { p[o + 8] & !0x80 };
    }

    fn enable_per_led(&self) -> Result<()> {
        let mut p = vec![0u8; 185];
        p[0] = 0x52;
        // blocs par défaut : effet statique, nombre de LED ARGB à 100
        for o in [1usize, 11, 21, 64, 74, 84, 94, 104, 114, 124, 134, 144, 154, 164, 174] {
            p[o] = 1;
        }
        for o in [31usize, 42] {
            p[o] = 1;
            p[o + 10] = 100;
        }
        p[53] = 1;
        p[57] = 40;
        let mut set = |o: usize, sb: u8, cf: u8| {
            p[o + 4] = sb;
            p[o + 8] = cf;
        };
        set(1, 0x08, 0x80);
        set(11, 0x2A, 0x80);
        set(21, 0x2A, 0x80);
        set(31, 0x29, 0x80);
        set(42, 0x29, 0x80);
        set(64, 0x28, 0x80);
        for o in (84..=164).step_by(10) {
            set(o, 0x28, 0x80);
        }
        set(174, 0x2A, 0x80);
        // JRAINBOW 1/2 synchronisés (40 LED), JCORSAIR 120 LED
        p[31] = 1;
        p[41] = SYNC_RAINBOW as u8;
        p[42] = 1;
        p[52] = SYNC_RAINBOW as u8;
        p[53] = 1;
        p[57] = 0x29;
        p[58] = 0;
        p[61] = 0x82;
        p[63] = SYNC_CORSAIR as u8;
        // carte : mode direct (0x25) synchronisé avec tous les connecteurs
        p[74] = 0x25;
        p[78] = 0x29 | 0x80;
        p[82] = 0xBF;
        self.dev.send_feature_report(&p)?;
        Ok(())
    }
}

impl Device for MysticLight {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, mode: usize, p: &HwParams) -> Result<()> {
        self.in_direct = false;
        let m = &self.info.modes[mode];
        let (effect, c1, c2) = if m.is_off {
            (1, Color::BLACK, Color::BLACK)
        } else {
            (MODES[mode].1, p.color(0), p.color(1))
        };
        let speed = if m.speed { p.speed_in(0, 2) as u8 } else { 0 };
        let bright = if m.is_off { 0 } else { p.brightness_in(1, 10) as u8 };
        let sb = (bright << 2) | speed;
        let custom = !(m.random && p.random) && m.colors.1 > 0;
        let mut pkt = self.read_config();
        for z in self.zones.clone() {
            if let Some(o) = self.offset(z) {
                Self::write_zone(&mut pkt, o, effect, c1, c2, sb, custom);
            }
            if z == Z::Onboard && effect <= 5 {
                for n in 0..self.onboard {
                    if let Some(o) = self.onboard_led_offset(n) {
                        Self::write_zone(&mut pkt, o, effect, c1, c2, sb, custom);
                    }
                }
            }
        }
        // nombre de LED des connecteurs ARGB pour les effets matériels
        if self.variant == Variant::V185 {
            for (zi, z) in self.zones.iter().enumerate() {
                let n = self.info.zones[zi].leds.len() as u8;
                match z {
                    Z::Jrainbow1 => pkt[41] = n.max(1),
                    Z::Jrainbow2 => pkt[52] = n.max(1),
                    _ => {}
                }
            }
        }
        let last = pkt.len() - 1;
        pkt[last] = 1; // enregistrer
        self.dev.send_feature_report(&pkt)?;
        Ok(())
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        match self.direct_kind {
            Direct::Off => Ok(()),
            Direct::PerLed => {
                if !self.in_direct {
                    self.enable_per_led()?;
                    self.in_direct = true;
                }
                let mut buf = vec![Color::BLACK; 240];
                // disposition du tampon synchronisé
                let p1 = self.onboard;
                let p2 = p1 + self.pipe1;
                let jrgb = p2 + self.pipe2;
                let r1 = jrgb + self.jrgbs;
                let r2 = r1 + SYNC_RAINBOW;
                let cors = r2 + SYNC_RAINBOW;
                let mut off = 0;
                for (zi, z) in self.zones.iter().enumerate() {
                    let n = self.info.zones[zi].leds.len();
                    let src = &colors[off..off + n];
                    let (start, max) = match z {
                        Z::Onboard => (0, self.onboard),
                        Z::Pipe1 => (p1, self.pipe1),
                        Z::Pipe2 => (p2, self.pipe2),
                        Z::Jrgb1 => (jrgb, 1),
                        Z::Jrgb2 => (jrgb + 1, 1),
                        Z::Jrainbow1 => (r1, SYNC_RAINBOW),
                        Z::Jrainbow2 => (r2, SYNC_RAINBOW),
                        Z::Jrainbow3 | Z::Jcorsair => (cors, SYNC_CORSAIR),
                    };
                    for (k, &c) in src.iter().take(max).enumerate() {
                        if start + k < 240 {
                            buf[start + k] = c;
                        }
                    }
                    off += n;
                }
                let mut pkt = vec![0x53, 0x25, 0x06, 0x00, 0x00];
                for c in buf {
                    pkt.extend_from_slice(&[c.r, c.g, c.b]);
                }
                self.dev.send_feature_report(&pkt)?;
                Ok(())
            }
            Direct::ZoneBased => {
                let mut pkt = vec![0u8; self.size()];
                pkt[0] = 0x52;
                let mut off = 0;
                for (zi, z) in self.zones.clone().into_iter().enumerate() {
                    let n = self.info.zones[zi].leds.len();
                    let c = colors[off];
                    if let Some(o) = self.offset(z) {
                        Self::write_zone(&mut pkt, o, 1, c, c, 10 << 2, true);
                    }
                    if z == Z::Onboard {
                        for k in 0..n {
                            if let Some(o) = self.onboard_led_offset(k) {
                                Self::write_zone(&mut pkt, o, 1, colors[off + k], colors[off + k], 10 << 2, true);
                            }
                        }
                    }
                    off += n;
                }
                self.dev.send_feature_report(&pkt)?;
                Ok(())
            }
        }
    }

    fn resize_zone(&mut self, zone: usize, leds: usize) -> Result<()> {
        let z = &mut self.info.zones[zone];
        match z.resizable {
            Some((min, max)) => {
                let name = z.name.clone();
                *z = Zone::resizable(name, leds.clamp(min as usize, max as usize), min, max);
                Ok(())
            }
            None => crate::device::err("zone non redimensionnable"),
        }
    }

    fn max_fps(&self) -> u32 {
        30
    }
}
