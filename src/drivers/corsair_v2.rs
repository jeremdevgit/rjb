// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Périphériques Corsair récents (protocole « V2 » d'iCUE) : claviers K55 à K100,
//! souris Dark Core, Ironclaw, Katar, M55/M65/M75, Scimitar, tapis MM700,
//! récepteurs Slipstream. Protocole d'après OpenRGB.

use super::corsair_v2_table::DEVICES;
use super::hid::{self, HidEntry};
use crate::device::{Device, Result};
use crate::layout;
use crate::model::*;
use hidapi::{DeviceInfo as HidInfo, HidApi, HidDevice};
use std::time::Duration;

const VID: u16 = 0x1B1C;
const MAX_PENDING_REPORTS: usize = 64;
const PIDS: &[u16] = &[
    0x1BA4, 0x1B6E, 0x1BA0, 0x1B8D, 0x1BAD, 0x1BC7, 0x1BED, 0x1BFD, 0x2B01, 0x1BC4, 0x1BB3, 0x1B73, 0x1BB9, 0x1B89,
    0x1B7C, 0x1BC5, 0x1B7D, 0x1B4B, 0x1B7E, 0x1B5E, 0x1B4C, 0x2B32, 0x2B00, 0x1B93, 0x1BBA, 0x1BAC, 0x1B70, 0x1B9E,
    0x1BB5, 0x1BF0, 0x1BDC, 0x1BF1, 0x1BE3, 0x1BA6, 0x1B65, 0x1B66, 0x1B9B, 0x1BC9,
];
/// Modèles pilotés en « logiciel » (couleurs par plans R, G, B).
const SOFTWARE: &[u16] = &[
    0x1BA4, 0x1B6E, 0x1BA0, 0x1B8D, 0x1BAD, 0x1B5E, 0x1B4C, 0x2B32, 0x2B00, 0x1B70, 0x1B9E, 0x1B65, 0x1B66, 0x1B9B, 0x1BC9,
];
const RECEIVERS: &[u16] = &[0x1BA6, 0x1B66, 0x1B65, 0x1BDC, 0x2B00];

pub const HID: &[HidEntry] = &[
    HidEntry {
        vid: VID,
        pids: PIDS,
        usage_page: Some(0xFF42),
        usage: None,
        interface: Some(1),
        open,
    },
    // modèles récents encore inconnus (K55 Core, nouveaux K70…) : même
    // protocole, essayé avec une disposition de clavier standard
    HidEntry {
        vid: VID,
        pids: &[],
        usage_page: Some(0xFF42),
        usage: None,
        interface: Some(1),
        open: open_unknown,
    },
];

/// PID gérés par les autres pilotes Corsair (contrôleurs de ventilateurs).
const OTHER_DRIVERS: &[u16] = &[
    0x0C1A, 0x0C0B, 0x0C10, 0x0C1E, 0x1D00, 0x1D04, 0x0C23, 0x0C1C, 0x0C32, 0x0C1D, 0x0C3C, 0x0C3D, 0x0C3E, 0x0C2A,
];

fn open_unknown(api: &HidApi, i: &HidInfo) -> Result<Vec<Box<dyn Device>>> {
    let pid = i.product_id();
    if PIDS.contains(&pid) || OTHER_DRIVERS.contains(&pid) || (0x0C00..0x0D00).contains(&pid) {
        return Ok(Vec::new());
    }
    open_with(api, i, true)
}

fn open(api: &HidApi, i: &HidInfo) -> Result<Vec<Box<dyn Device>>> {
    open_with(api, i, false)
}

/// Clavier inconnu : les touches sont adressées par leur code d'usage HID,
/// une disposition standard (complète ou TKL) convient donc à la plupart.
fn guess_keyboard(product: &str) -> Option<&'static [u16]> {
    let p = product.to_lowercase();
    let model = p.split(|c: char| !c.is_ascii_alphanumeric()).any(|w| {
        w.len() >= 3 && w.starts_with('k') && w[1..].chars().all(|c| c.is_ascii_digit())
    });
    if !(model || p.contains("keyboard")) {
        return None;
    }
    let compact = ["tkl", "k65", "k60 pro mini", "k70 pro mini", "mini"].iter().any(|k| p.contains(k));
    Some(if compact { super::corsair_v2_table::TKL_KEYS } else { super::corsair_v2_table::FULL_KEYS })
}

pub struct CorsairV2 {
    info: DeviceInfo,
    dev: HidDevice,
    write_id: u8,
    light_ctrl: u8,
    software: bool,
    skip_reads: bool,
    /// index dans le tampon de l'appareil pour chaque LED
    index: Vec<u16>,
    buffer_len: usize,
    sw: bool,
}

fn key_name(n: &str) -> String {
    match n {
        "KEY_EN_MEDIA_STOP" => "Stop".into(),
        "KEY_EN_MEDIA_PREVIOUS" => "Précédent".into(),
        "KEY_EN_MEDIA_PLAY_PAUSE" => "Lecture/Pause".into(),
        "KEY_EN_MEDIA_NEXT" => "Suivant".into(),
        "KEY_EN_MEDIA_MUTE" => "Muet".into(),
        "KEY_EN_MEDIA_VOLUME_UP" => "Volume +".into(),
        "KEY_EN_MEDIA_VOLUME_DOWN" => "Volume -".into(),
        "KEY_EN_RIGHT_WINDOWS" => "Windows droite".into(),
        "KEY_EN_SPACE" => "Espace".into(),
        "Profile" => "Profil".into(),
        "Light" => "Luminosité".into(),
        "Lock" => "Verrou Windows".into(),
        other => other.trim_start_matches("Key: ").to_string(),
    }
}

/// Nom de touche pour une valeur Corsair (= code d'usage HID, sauf modificateurs).
fn corsair_key(v: u16) -> Option<&'static str> {
    Some(match v {
        100 => "<",
        101 => "Menu",
        105 => "Ctrl gauche",
        106 => "Maj gauche",
        107 => "Alt",
        108 => "Windows gauche",
        109 => "Ctrl droite",
        110 => "Maj droite",
        111 => "Alt Gr",
        122 => "Fn",
        _ => return layout::hid_usage_key(v),
    })
}

fn zone_name(n: &str) -> String {
    match n {
        "ZONE_EN_KEYBOARD" => "Touches".into(),
        "Scroll Wheel" => "Molette".into(),
        "Side Buttons" => "Boutons latéraux".into(),
        "Rear Left" => "Arrière gauche".into(),
        "Rear Right" => "Arrière droit".into(),
        "DPI & Indicator Zone" | "DPI" => "DPI".into(),
        "Indicator" => "Témoin".into(),
        "Buttons" => "Boutons".into(),
        "Side Zone" => "Côté".into(),
        other => other.to_string(),
    }
}

impl CorsairV2 {
    fn packet(&self, cmd: u8, args: &[u8]) -> [u8; 65] {
        let mut b = [0u8; 65];
        b[1] = self.write_id;
        b[2] = cmd;
        b[3..3 + args.len()].copy_from_slice(args);
        b
    }

    fn write(&self, b: &[u8; 65], wait: bool) -> Result<[u8; 65]> {
        self.dev.write(b)?;
        let mut r = [0u8; 65];
        if wait && !self.skip_reads {
            let _ = self.dev.read_timeout(&mut r, 50);
        }
        Ok(r)
    }

    fn get(&self, addr: u8) -> Option<u32> {
        let r = self.write(&self.packet(0x02, &[addr]), true).ok()?;
        (r[2] == 0).then(|| u32::from_le_bytes([r[3], r[4], r[5], r[6]]))
    }

    fn get_retry(&self, addr: u8) -> Option<u32> {
        for _ in 0..8 {
            if let Some(v) = self.get(addr).filter(|&v| v != 0) {
                return Some(v);
            }
            std::thread::sleep(Duration::from_millis(120));
        }
        None
    }

    fn render_mode(&self, software: bool) -> Result<()> {
        let mut b = self.packet(0x01, &[0x03]);
        b[5] = if software { 0x02 } else { 0x01 };
        self.write(&b, true).map(|_| ())
    }

    fn start(&self) -> Result<u8> {
        Ok(self.write(&self.packet(0x0D, &[0x00, self.light_ctrl]), true)?[2])
    }

    fn stop(&self) -> Result<()> {
        self.write(&self.packet(0x05, &[0x01, 0x00]), true).map(|_| ())
    }

    fn send_block(&self, data: &[u8]) -> Result<()> {
        // purge des réponses en attente
        let mut tmp = [0u8; 65];
        for _ in 0..MAX_PENDING_REPORTS {
            if self.skip_reads || !matches!(self.dev.read_timeout(&mut tmp, 3), Ok(n) if n > 0) {
                break;
            }
        }
        self.start()?;
        let mut b = self.packet(0x06, &[]);
        b[4] = data.len() as u8;
        b[5] = (data.len() >> 8) as u8;
        let first = data.len().min(65 - 8);
        b[8..8 + first].copy_from_slice(&data[..first]);
        self.write(&b, true)?;
        let mut done = first;
        while done < data.len() {
            let mut b = self.packet(0x07, &[]);
            let n = (data.len() - done).min(65 - 4);
            b[4..4 + n].copy_from_slice(&data[done..done + n]);
            self.dev.write(&b)?;
            std::thread::sleep(Duration::from_millis(2));
            done += n;
        }
        self.stop()
    }
}

fn open_with(api: &HidApi, i: &HidInfo, unknown: bool) -> Result<Vec<Box<dyn Device>>> {
    let dev = api.open_path(i.path())?;
    let mut d = CorsairV2 {
        info: DeviceInfo {
            id: String::new(),
            name: String::new(),
            vendor: "Corsair".into(),
            kind: DeviceKind::Other,
            location: hid::location(i),
            zones: Vec::new(),
            modes: Vec::new(),
            direct: true,
        },
        dev,
        write_id: 0x08,
        light_ctrl: 34,
        software: SOFTWARE.contains(&i.product_id()),
        skip_reads: false,
        index: Vec::new(),
        buffer_len: 0,
        sw: false,
    };
    if i.product_id() == 0x1B6E {
        d.write_id = 0x80;
        d.light_ctrl = 1;
        d.skip_reads = true;
    }
    // le PID réel est demandé à l'appareil (utile derrière un récepteur sans fil)
    let mut pid = d.get_retry(0x12).unwrap_or(i.product_id() as u32) as u16;
    if RECEIVERS.contains(&pid) || RECEIVERS.contains(&i.product_id()) {
        d.write_id = 0x09;
        pid = d.get_retry(0x12).unwrap_or(pid as u32) as u16;
    }
    let product = hid::product(i, "");
    let generic_kb: (&[u16], &[(u16, &str)]);
    const KB_ZONE: &[(&str, u8, u8, u8)] = &[("ZONE_EN_KEYBOARD", 2, 6, 21)];
    let (kind, zones, kb, experimental) = match DEVICES.iter().find(|x| x.0 == pid) {
        Some(&(_, kind, zones, kb)) => (kind, zones, kb, false),
        // le premier pilote laisse passer un appareil sans modèle connu : il
        // sera repris ici seulement s'il ressemble à un clavier
        None if !unknown => return Ok(Vec::new()),
        None => match guess_keyboard(&product) {
            Some(keys) => {
                generic_kb = (keys, &[]);
                (DeviceKind::Keyboard, KB_ZONE, Some(generic_kb), true)
            }
            None => {
                crate::log(&format!("Corsair V2 : modèle {pid:#06x} « {product} » inconnu"));
                return Ok(Vec::new());
            }
        },
    };
    if unknown && !experimental {
        return Ok(Vec::new()); // déjà pris en charge par l'entrée principale
    }
    if d.light_ctrl == 34 && !matches!(pid, 0x1B7C | 0x1BC5 | 0x1B7D) {
        if d.start().unwrap_or(0) > 0 {
            d.light_ctrl = 1;
            let _ = d.start();
        }
        let _ = d.stop();
    }

    // zones et index dans le tampon
    let mut next = 0u16;
    for &(zname, ztype, rows, cols) in zones {
        if ztype == 2 {
            let Some((base, extras)) = kb else { continue };
            let mut keys: Vec<(&str, u16)> = Vec::new();
            for &v in base {
                if let Some(k) = corsair_key(v) {
                    keys.push((k, v));
                }
            }
            let mut extra: Vec<(String, u16)> = Vec::new();
            for &(v, n) in extras {
                if n == "KEY_EN_UNUSED" {
                    keys.retain(|k| k.1 != v);
                } else if keys.iter().any(|k| k.1 == v) || extra.iter().any(|e| e.1 == v) {
                    // la touche existe déjà (renommage dans la disposition d'origine)
                } else {
                    extra.push((key_name(n), v));
                }
            }
            let (zone, ids) = layout::keyboard_zone("Touches", &keys, &extra);
            d.buffer_len = d.buffer_len.max(ids.iter().map(|&v| v as usize + 1).max().unwrap_or(0));
            d.index.extend(ids);
            d.info.zones.push(zone);
        } else {
            let n = (rows as usize * cols as usize).max(1);
            d.info.zones.push(if n == 1 { Zone::single(zone_name(zname)) } else { Zone::linear(zone_name(zname), n) });
            for _ in 0..n {
                d.index.push(next);
                next += 1;
            }
            d.buffer_len = d.buffer_len.max(next as usize);
        }
    }
    if d.info.zones.is_empty() {
        return Ok(Vec::new());
    }
    d.info.kind = kind;
    d.info.name = format!("Corsair {}", pretty_name(&hid::product(i, "périphérique")).trim_start_matches("Corsair ").trim());
    if experimental {
        d.info.name.push_str(" (expérimental)");
        crate::log(&format!("Corsair V2 : {pid:#06x} « {product} » piloté avec une disposition standard"));
    }
    d.info.id = format!("corsair-v2:{pid:04x}:{}", hid::stable_id("", i));
    d.info.modes = vec![ModeInfo::new("Effet de l'appareil")];
    d.render_mode(true)?;
    d.sw = true;
    let _ = d.write(&d.packet(0x02, &[0x5F]), true);
    Ok(vec![Box::new(d)])
}

impl Device for CorsairV2 {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, _mode: usize, _p: &HwParams) -> Result<()> {
        // retour à l'effet enregistré dans l'appareil
        self.sw = false;
        self.render_mode(false)
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        if !self.sw {
            self.render_mode(true)?;
            self.sw = true;
        }
        let n = self.buffer_len;
        let mut buf = vec![Color::BLACK; n];
        for (i, &ix) in self.index.iter().enumerate() {
            if (ix as usize) < n {
                buf[ix as usize] = colors.get(i).copied().unwrap_or(Color::BLACK);
            }
        }
        let data: Vec<u8> = if self.software || self.light_ctrl == 1 {
            // plans de couleur : toutes les composantes rouges, puis vertes, puis bleues
            buf.iter().map(|c| c.r).chain(buf.iter().map(|c| c.g)).chain(buf.iter().map(|c| c.b)).collect()
        } else {
            let mut v = vec![0x12, 0x00];
            for c in &buf {
                v.extend_from_slice(&[c.r, c.g, c.b]);
            }
            v
        };
        self.send_block(&data)
    }

    fn keepalive(&self) -> Option<Duration> {
        Some(Duration::from_secs(25))
    }

    fn max_fps(&self) -> u32 {
        30
    }

    fn release(&mut self) {
        let _ = self.render_mode(false);
    }
}
