// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Logitech G (HID++ 2.0) : pilote générique par découverte de fonctionnalités.
//! Couvre les souris, claviers, tapis et casques LIGHTSYNC filaires ou via
//! récepteur LIGHTSPEED (fonctions 0x8070, 0x8071, 0x8080, 0x8081).
//! Protocole d'après OpenRGB (GPL-2.0) et Solaar.

use super::hid::{self, entry, HidEntry};
use crate::device::{err, Device, Error, Result};
use crate::layout;
use crate::model::*;
use hidapi::{DeviceInfo as HidInfo, HidApi, HidDevice};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const MAX_PENDING_REPORTS: usize = 64;

/// Appareils Logitech qui répondent mais n'ont aucun éclairage RGB
/// (PRO X SUPERLIGHT 2, G305…) : emplacement → nom, pour les signaler.
static NO_LIGHT: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
/// Récepteurs dont aucun appareil n'a répondu (souris en veille) : à réessayer.
static ASLEEP: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Appareils sans éclairage encore branchés (emplacements présents).
pub fn without_lighting(present: &dyn Fn(&str) -> bool) -> Vec<String> {
    let mut l = NO_LIGHT.lock().unwrap();
    l.retain(|(loc, _)| present(loc));
    let mut names: Vec<String> = l.iter().map(|(_, n)| n.clone()).collect();
    names.dedup();
    names
}

/// Vrai si ce récepteur doit être sondé à nouveau (appareil peut-être en veille).
pub fn waiting(loc: &str) -> bool {
    ASLEEP.lock().unwrap().iter().any(|l| l == loc)
}

const VID: u16 = 0x046D;
const SWID: u8 = 0x0B;

const F_NAME: u16 = 0x0005;
const F_FWINFO: u16 = 0x0003;
const F_LED_8070: u16 = 0x8070;
const F_RGB_8071: u16 = 0x8071;
const F_PERKEY: u16 = 0x8081;
/// Éclairage par touche de 1re génération (G810, G910, G512, G513, G413…).
const F_PERKEY_V1: u16 = 0x8080;
const F_DISABLE_KEYS: u16 = 0x4522;
const F_ONBOARD: u16 = 0x8100;
const F_PROFILE_MGMT: u16 = 0x8101;

const RECEIVERS: &[u16] = &[
    0xC52B, 0xC532, 0xC534, 0xC539, 0xC53A, 0xC53D, 0xC53F, 0xC541, 0xC545, 0xC547, 0xC548, 0xC54D,
];

pub const HID: &[HidEntry] = &[
    entry(VID, &[], Some(0xFF00), Some(0x0002), None, open),
    entry(VID, &[], Some(0xFF43), Some(0x0602), None, open),
];

enum ReqErr {
    Timeout,
    Hidpp(u8),
    Io(String),
}

impl From<ReqErr> for Error {
    fn from(e: ReqErr) -> Self {
        match e {
            ReqErr::Timeout => Error("l'appareil ne répond pas".into()),
            ReqErr::Hidpp(c) => Error(format!("erreur HID++ {c:#04x}")),
            ReqErr::Io(s) => Error(s),
        }
    }
}

struct Link {
    dev: HidDevice,
    idx: u8,
}

impl Link {
    fn write(&self, feat: u8, func: u8, params: &[u8]) -> std::result::Result<(), ReqErr> {
        let mut b = [0u8; 20];
        b[0] = 0x11;
        b[1] = self.idx;
        b[2] = feat;
        b[3] = func | SWID;
        let n = params.len().min(16);
        b[4..4 + n].copy_from_slice(&params[..n]);
        self.dev.write(&b).map_err(|e| ReqErr::Io(e.to_string()))?;
        Ok(())
    }

    /// Lit jusqu'à recevoir la réponse à (feat, func) ou une erreur.
    fn wait(&self, feat: u8, func: u8, timeout: Duration) -> std::result::Result<[u8; 16], ReqErr> {
        let deadline = Instant::now() + timeout;
        let mut b = [0u8; 64];
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(ReqErr::Timeout);
            }
            let n = self
                .dev
                .read_timeout(&mut b, left.as_millis().max(1) as i32)
                .map_err(|e| ReqErr::Io(e.to_string()))?;
            if n < 7 || b[1] != self.idx {
                continue;
            }
            if b[2] == 0xFF && b[3] == feat && (b[4] & 0xF0) == (func & 0xF0) {
                return Err(ReqErr::Hidpp(b[5]));
            }
            if b[2] == feat && b[3] == (func | SWID) {
                let mut d = [0u8; 16];
                let m = (n - 4).min(16);
                d[..m].copy_from_slice(&b[4..4 + m]);
                return Ok(d);
            }
        }
    }

    fn request_t(&self, feat: u8, func: u8, params: &[u8], timeout: Duration, tries: u32) -> std::result::Result<[u8; 16], ReqErr> {
        let mut last = ReqErr::Timeout;
        for _ in 0..tries {
            self.write(feat, func, params)?;
            match self.wait(feat, func, timeout) {
                Ok(d) => return Ok(d),
                Err(ReqErr::Hidpp(8)) => {
                    // occupé : on réessaie un peu plus tard
                    std::thread::sleep(Duration::from_millis(10));
                    last = ReqErr::Hidpp(8);
                }
                Err(ReqErr::Timeout) => last = ReqErr::Timeout,
                Err(e) => return Err(e),
            }
        }
        Err(last)
    }

    fn request(&self, feat: u8, func: u8, params: &[u8]) -> std::result::Result<[u8; 16], ReqErr> {
        self.request_t(feat, func, params, Duration::from_millis(300), 4)
    }

    fn feature(&self, id: u16) -> u8 {
        self.request(0, 0x00, &[(id >> 8) as u8, id as u8])
            .map(|d| d[0])
            .unwrap_or(0)
    }

    fn ping(&self) -> bool {
        matches!(
            self.request_t(0, 0x10, &[0, 0, 0x5A], Duration::from_millis(150), 1),
            Ok(d) if d[2] == 0x5A
        )
    }

    fn drain(&self) {
        let mut b = [0u8; 64];
        for _ in 0..MAX_PENDING_REPORTS {
            if !matches!(self.dev.read_timeout(&mut b, 0), Ok(n) if n > 0) {
                break;
            }
        }
    }
}

#[derive(Clone)]
struct Effect {
    index: u8,
    id: u16,
}

#[derive(Clone)]
struct Cluster {
    index: u8,
    location: u16,
    effects: Vec<Effect>,
}

#[derive(PartialEq, Clone, Copy)]
enum Page {
    P8070,
    P8071,
}

pub struct Logitech {
    info: DeviceInfo,
    link: Link,
    page: Page,
    idx_rgb: u8,
    idx_perkey: u8,
    idx_disable_keys: u8,
    idx_onboard: u8,
    idx_profile: u8,
    clusters: Vec<Cluster>,
    /// identifiant sur le fil de chaque LED (zones par touche) ou index de cluster
    wire: Vec<u8>,
    perkey: bool,
    /// identifiants d'effet des modes matériels exposés (0xFFFF = Éteint)
    mode_ids: Vec<u16>,
    claimed: bool,
    upgraded: bool,
    prepped: bool,
    sent: Vec<Color>,
    wireless: bool,
    /// 0x8080 : les couleurs passent par le rapport « très long » (0x12),
    /// exposé par Windows comme une collection HID à part
    idx_v1: u8,
    vl: Option<HidDevice>,
    /// (type de touche << 8) | identifiant, pour chaque LED
    v1_wire: Vec<u16>,
}

fn open(api: &HidApi, i: &HidInfo) -> Result<Vec<Box<dyn Device>>> {
    let receiver = RECEIVERS.contains(&i.product_id())
        || i.product_string().is_some_and(|s| s.contains("Receiver"));
    let indices: Vec<u8> = if receiver { (1..=6).collect() } else { vec![0xFF] };
    let loc = hid::location(i);
    NO_LIGHT.lock().unwrap().retain(|(l, _)| *l != loc);
    let mut out: Vec<Box<dyn Device>> = Vec::new();
    let mut answered = 0;
    for idx in indices {
        let dev = api.open_path(i.path())?;
        let link = Link { dev, idx };
        link.drain();
        if !link.ping() {
            continue;
        }
        answered += 1;
        let vl = if receiver { None } else { very_long_sibling(api, i) };
        // une erreur sur un appareil du récepteur ne doit pas masquer les autres
        match Logitech::probe(link, i, receiver, vl) {
            Ok(Some(d)) => out.push(Box::new(d)),
            Ok(None) => {}
            Err(e) => crate::log(&format!("Logitech {loc}#{idx} : {e}")),
        }
    }
    // récepteur sans réponse : souris en veille ou éteinte, à réessayer plus tard
    let mut asleep = ASLEEP.lock().unwrap();
    asleep.retain(|l| *l != loc);
    if receiver && answered == 0 {
        asleep.push(loc);
    }
    Ok(out)
}

/// Collection HID « très longue » (usage 0x0604) du même appareil.
fn very_long_sibling(api: &HidApi, i: &HidInfo) -> Option<HidDevice> {
    let strip = |p: &str| {
        let p = p.to_lowercase();
        match p.find("&col") {
            Some(s) => {
                let end = p[s + 4..].find(|c: char| !c.is_ascii_hexdigit()).map_or(p.len(), |e| s + 4 + e);
                format!("{}{}", &p[..s], &p[end..])
            }
            None => p,
        }
    };
    let me = strip(&i.path().to_string_lossy());
    api.device_list()
        .find(|o| {
            o.vendor_id() == i.vendor_id()
                && o.product_id() == i.product_id()
                && o.usage_page() == 0xFF43
                && o.usage() == 0x0604
                && strip(&o.path().to_string_lossy()) == me
        })
        .and_then(|o| api.open_path(o.path()).ok())
}

/// Nom (fonction 0x0005) et type d'appareil (0 clavier, 3/5 souris, 8 casque…).
fn device_name(link: &Link, i: &HidInfo) -> (String, u8) {
    let idx_name = link.feature(F_NAME);
    let mut name = String::new();
    let mut dtype = 0xFF;
    if idx_name != 0 {
        if let Ok(d) = link.request(idx_name, 0x00, &[]) {
            let len = d[0] as usize;
            while name.len() < len {
                match link.request(idx_name, 0x10, &[name.len() as u8]) {
                    Ok(c) => {
                        let s: Vec<u8> = c.iter().copied().take_while(|&b| b != 0).collect();
                        if s.is_empty() {
                            break;
                        }
                        name.push_str(&String::from_utf8_lossy(&s));
                    }
                    Err(_) => break,
                }
            }
            name.truncate(len);
        }
        if let Ok(d) = link.request(idx_name, 0x20, &[]) {
            dtype = d[0];
        }
    }
    if name.trim().is_empty() {
        name = hid::product(i, "Logitech G");
    }
    (pretty_name(&name), dtype)
}

fn effect_name(id: u16) -> Option<ModeInfo> {
    use Direction::*;
    Some(match id {
        0x0001 => ModeInfo::new("Statique").colors(1, 1),
        0x0002 => ModeInfo::new("Pulsation").colors(1, 1).speed(),
        0x0003 | 0x0015 => ModeInfo::new("Cycle de couleurs").speed().brightness(),
        0x0004 | 0x0016 => ModeInfo::new("Vague").speed().brightness().dirs(&[Right, Left, Down, Up, Out, In]),
        0x0007 => ModeInfo::new("Réactif au son").colors(0, 1).speed(),
        0x000A => ModeInfo::new("Respiration").colors(1, 1).speed().brightness(),
        0x000B | 0x0017 => ModeInfo::new("Ondulation").colors(1, 1).speed(),
        0x000E => ModeInfo::new("Décomposition").speed().brightness(),
        0x000F => ModeInfo::new("Signature 1").speed().brightness(),
        0x0010 => ModeInfo::new("Signature 2").speed().brightness(),
        _ => return None,
    })
}

fn location_name(loc: u16) -> String {
    match loc {
        1 => "Principale".into(),
        2 => "Logo".into(),
        3 => "Côté gauche".into(),
        4 => "Côté droit".into(),
        5 => "Ensemble".into(),
        6..=11 => format!("Principale {}", loc - 5),
        _ => format!("Zone {loc:#06x}"),
    }
}

/// Nom de touche (table `layout::KEYS`) pour un identifiant de zone 0x8081.
fn key_for_zone(z: u8) -> Option<&'static str> {
    const LETTERS: [&str; 26] = [
        "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z",
    ];
    const DIGITS: [&str; 10] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];
    Some(match z {
        1..=26 => LETTERS[z as usize - 1],
        27..=36 => DIGITS[z as usize - 27],
        37 => "Entrée",
        38 => "Échap",
        39 => "Retour arrière",
        40 => "Tab",
        41 => "Espace",
        42 => "-",
        43 => "=",
        44 => "[",
        45 => "]",
        46 => "\\",
        47 => "#",
        48 => ";",
        49 => "'",
        50 => "²",
        51 => ",",
        52 => ".",
        53 => "/",
        54 => "Verr. maj",
        55..=66 => ["F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12"][z as usize - 55],
        67 => "Impr. écran",
        68 => "Arrêt défil.",
        69 => "Pause",
        70 => "Inser",
        71 => "Origine",
        72 => "Page préc.",
        73 => "Suppr",
        74 => "Fin",
        75 => "Page suiv.",
        76 => "Droite",
        77 => "Gauche",
        78 => "Bas",
        79 => "Haut",
        80 => "Verr. num",
        81 => "Num /",
        82 => "Num *",
        83 => "Num -",
        84 => "Num +",
        85 => "Num Entrée",
        86..=95 => ["Num 1", "Num 2", "Num 3", "Num 4", "Num 5", "Num 6", "Num 7", "Num 8", "Num 9", "Num 0"][z as usize - 86],
        96 => "Num .",
        97 => "<",
        98 => "Menu",
        104 => "Ctrl gauche",
        105 => "Maj gauche",
        106 => "Alt",
        107 => "Windows gauche",
        108 => "Ctrl droite",
        109 => "Maj droite",
        110 => "Alt Gr",
        111 => "Windows droite",
        150 | 153 => "Luminosité",
        152 | 155 => "Lecture/Pause",
        156 => "Muet",
        154 | 157 => "Suivant",
        158 => "Précédent",
        180..=188 => ["G1", "G2", "G3", "G4", "G5", "G6", "G7", "G8", "G9"][z as usize - 180],
        210 => "Logo",
        _ => return None,
    })
}

impl Logitech {
    fn probe(link: Link, i: &HidInfo, wireless: bool, vl: Option<HidDevice>) -> Result<Option<Logitech>> {
        let idx_rgb8071 = link.feature(F_RGB_8071);
        let (page, idx_rgb) = if idx_rgb8071 != 0 {
            (Page::P8071, idx_rgb8071)
        } else {
            (Page::P8070, link.feature(F_LED_8070))
        };
        let idx_perkey = link.feature(F_PERKEY);
        let idx_v1 = if idx_perkey == 0 && vl.is_some() { link.feature(F_PERKEY_V1) } else { 0 };
        let (name, dtype) = device_name(&link, i);
        if idx_rgb == 0 && idx_perkey == 0 && idx_v1 == 0 {
            // reconnu, mais rien à éclairer (PRO X SUPERLIGHT 2, G305, G502 X…)
            crate::log(&format!("Logitech {name} idx={:#04x} : aucun éclairage RGB (ni 0x8070, ni 0x8071, ni 0x8080/0x8081)", link.idx));
            NO_LIGHT.lock().unwrap().push((hid::location(i), name));
            return Ok(None);
        }

        // identifiant matériel stable
        let idx_fw = link.feature(F_FWINFO);
        let unit = if idx_fw != 0 {
            link.request(idx_fw, 0x00, &[])
                .ok()
                .map(|d| format!("{:02x}{:02x}{:02x}{:02x}", d[1], d[2], d[3], d[4]))
                .filter(|u| u != "00000000")
        } else {
            None
        };
        let id = match unit {
            Some(u) => format!("logi:{u}"),
            None => format!("{}:{}", hid::stable_id("logi", i), link.idx),
        };

        let mut d = Logitech {
            info: DeviceInfo {
                id,
                name,
                vendor: "Logitech".into(),
                kind: DeviceKind::Other,
                location: format!("{}#{}", hid::location(i), link.idx),
                zones: Vec::new(),
                modes: Vec::new(),
                direct: true,
            },
            idx_disable_keys: link.feature(F_DISABLE_KEYS),
            idx_onboard: link.feature(F_ONBOARD),
            idx_profile: link.feature(F_PROFILE_MGMT),
            link,
            page,
            idx_rgb,
            idx_perkey,
            clusters: Vec::new(),
            wire: Vec::new(),
            perkey: false,
            mode_ids: Vec::new(),
            claimed: false,
            upgraded: false,
            prepped: false,
            sent: Vec::new(),
            wireless,
            idx_v1,
            vl: if idx_v1 != 0 { vl } else { None },
            v1_wire: Vec::new(),
        };
        d.discover_clusters();
        d.discover_perkey();
        d.discover_perkey_v1();

        d.info.kind = match dtype {
            0 => DeviceKind::Keyboard,
            3 | 5 => DeviceKind::Mouse,
            8 => DeviceKind::Headset,
            14 => DeviceKind::Speaker,
            15 => DeviceKind::Microphone,
            _ if d.perkey => DeviceKind::Keyboard,
            _ if i.product_id() == 0xC53A || d.info.name.contains("POWERPLAY") => DeviceKind::Mousemat,
            _ => DeviceKind::Other,
        };

        if !d.perkey {
            for c in &d.clusters {
                d.info.zones.push(Zone::single(location_name(c.location)));
                d.wire.push(c.index);
            }
        }
        if d.info.zones.is_empty() {
            return Ok(None);
        }

        // modes matériels : union des effets des clusters
        let mut seen = Vec::new();
        for c in &d.clusters {
            for e in &c.effects {
                if !seen.contains(&e.id) {
                    if let Some(m) = effect_name(e.id) {
                        if !d.info.modes.iter().any(|x| x.name == m.name) {
                            d.info.modes.push(m);
                            d.mode_ids.push(e.id);
                        }
                    }
                    seen.push(e.id);
                }
            }
        }
        d.info.modes.push(ModeInfo::new("Éteint").off());
        d.mode_ids.push(0xFFFF);
        crate::log(&format!(
            "Logitech {} idx={:#04x} page={} rgb={} perkey={} v1={} keys={} clusters={:?} 4522={} onboard={}/{}",
            d.info.name,
            d.link.idx,
            if d.page == Page::P8071 { "8071" } else { "8070" },
            d.idx_rgb,
            d.idx_perkey,
            d.idx_v1,
            d.wire.len(),
            d.clusters.iter().map(|c| (c.location, c.effects.iter().map(|e| e.id).collect::<Vec<_>>())).collect::<Vec<_>>(),
            d.idx_disable_keys,
            d.idx_onboard,
            d.idx_profile
        ));
        Ok(Some(d))
    }

    fn discover_clusters(&mut self) {
        if self.idx_rgb == 0 {
            return;
        }
        let l = &self.link;
        let count = match self.page {
            Page::P8071 => l.request(self.idx_rgb, 0x00, &[0xFF, 0xFF, 0x00]).map(|d| d[2]),
            Page::P8070 => l.request(self.idx_rgb, 0x00, &[]).map(|d| d[0]),
        }
        .unwrap_or(0);
        for ci in 0..count.min(16) {
            let r = match self.page {
                Page::P8071 => l.request(self.idx_rgb, 0x00, &[ci, 0xFF]).map(|d| (((d[2] as u16) << 8) | d[3] as u16, d[4])),
                Page::P8070 => l.request(self.idx_rgb, 0x10, &[ci, 0x00]).map(|d| (((d[1] as u16) << 8) | d[2] as u16, d[3])),
            };
            let Ok((location, n)) = r else { continue };
            let mut effects = Vec::new();
            for ei in 0..n.min(32) {
                let func = if self.page == Page::P8071 { 0x00 } else { 0x20 };
                if let Ok(d) = l.request(self.idx_rgb, func, &[ci, ei, 0, 0]) {
                    effects.push(Effect {
                        index: ei,
                        id: ((d[2] as u16) << 8) | d[3] as u16,
                    });
                }
            }
            self.clusters.push(Cluster {
                index: ci,
                location,
                effects,
            });
        }
    }

    fn discover_perkey(&mut self) {
        if self.idx_perkey == 0 {
            return;
        }
        let mut ids = Vec::new();
        for page in 0..3u8 {
            let Ok(d) = self.link.request(self.idx_perkey, 0x00, &[0x00, page, 0x00]) else {
                continue;
            };
            for bit in 0..112u16 {
                if page == 0 && bit == 0 {
                    continue;
                }
                if d[2 + (bit / 8) as usize] & (1 << (bit % 8)) != 0 {
                    let z = page as u16 * 112 + bit;
                    if z < 0xFE {
                        ids.push(z as u8);
                    }
                }
            }
        }
        if ids.is_empty() {
            return;
        }
        let mut keys = Vec::new();
        let mut extra = Vec::new();
        for z in ids {
            match key_for_zone(z) {
                Some(k) => keys.push((k, z)),
                None => extra.push((format!("Zone {z}"), z)),
            }
        }
        let (zone, wire) = layout::keyboard_zone("Touches", &keys, &extra);
        self.info.zones.push(zone);
        self.wire = wire;
        self.perkey = true;
    }

    /// 0x8080 : liste des touches par type (clavier, touches G, logo), lue
    /// page par page (14 touches) avec GetKeyColors.
    fn discover_perkey_v1(&mut self) {
        if self.idx_v1 == 0 || self.perkey {
            return;
        }
        let Some(vl) = &self.vl else { return };
        let mut found: Vec<(u16, u8)> = Vec::new();
        for kt in [0x0001u16, 0x0004, 0x0010, 0x0040] {
            for page in 0..12u16 {
                let start = page * 14;
                let q = [(kt >> 8) as u8, kt as u8, (start >> 8) as u8, start as u8, 0];
                if self.link.write(self.idx_v1, 0x20, &q).is_err() {
                    break;
                }
                let mut b = [0u8; 64];
                let deadline = Instant::now() + Duration::from_millis(150);
                let mut got = 0;
                loop {
                    let left = deadline.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        break;
                    }
                    match vl.read_timeout(&mut b, left.as_millis().max(1) as i32) {
                        Ok(n) if n >= 12 && b[0] == 0x12 && b[2] == self.idx_v1 && b[3] == (0x20 | SWID) => {
                            for e in 0..14 {
                                let id = b[8 + e * 4];
                                if id != 0 {
                                    found.push((kt, id));
                                    got += 1;
                                }
                            }
                            break;
                        }
                        Ok(0) | Err(_) => break,
                        _ => {}
                    }
                }
                self.link.drain();
                if got == 0 {
                    break;
                }
            }
        }
        if !found.iter().any(|k| k.0 == 1) {
            // lecture impossible : touches standard (codes d'usage HID)
            found.extend((0x04u8..=0x65).chain(0xE0..=0xE7).map(|u| (1, u)));
        }
        let mut keys = Vec::new();
        let mut extra = Vec::new();
        for (kt, id) in found {
            let wire = (kt << 8) | id as u16;
            match kt {
                1 => match layout::hid_usage_key(id as u16) {
                    Some(k) => keys.push((k, wire)),
                    None => extra.push((format!("Touche {id:#04x}"), wire)),
                },
                4 => extra.push((format!("G{id}"), wire)),
                0x10 => extra.push(("Logo".to_string(), wire)),
                _ => extra.push((format!("Témoin {id}"), wire)),
            }
        }
        let (zone, wire) = layout::keyboard_zone("Touches", &keys, &extra);
        self.info.zones.push(zone);
        self.v1_wire = wire;
        self.wire = vec![0; self.v1_wire.len()];
        self.perkey = true;
    }

    fn perkey_v1_frame(&mut self, colors: &[Color]) -> Result<()> {
        self.claim();
        self.perkey_prep();
        let full = self.sent.len() != colors.len();
        let Some(vl) = &self.vl else { return Ok(()) };
        let mut by_type: Vec<(u16, Vec<[u8; 4]>)> = Vec::new();
        for (i, &c) in colors.iter().enumerate() {
            if !full && self.sent[i] == c {
                continue;
            }
            let w = self.v1_wire[i];
            let kt = w >> 8;
            let e = [w as u8, c.r, c.g, c.b];
            match by_type.iter_mut().find(|x| x.0 == kt) {
                Some(x) => x.1.push(e),
                None => by_type.push((kt, vec![e])),
            }
        }
        if by_type.is_empty() {
            return Ok(());
        }
        for (kt, keys) in &by_type {
            for chunk in keys.chunks(14) {
                let mut b = [0u8; 64];
                b[0] = 0x12;
                b[1] = self.link.idx;
                b[2] = self.idx_v1;
                b[3] = 0x30 | SWID;
                b[4] = (kt >> 8) as u8;
                b[5] = *kt as u8;
                b[7] = chunk.len() as u8;
                for (k, e) in chunk.iter().enumerate() {
                    b[8 + k * 4..12 + k * 4].copy_from_slice(e);
                }
                vl.write(&b)?;
            }
        }
        // les couleurs n'apparaissent qu'après FlushLEDS
        let _ = self.link.request_t(self.idx_v1, 0x50, &[], Duration::from_millis(120), 2);
        self.link.drain();
        self.sent = colors.to_vec();
        Ok(())
    }

    fn sw_control(&self, mode: u8, flags: u8) {
        if self.idx_rgb == 0 {
            return;
        }
        let _ = match self.page {
            Page::P8070 => {
                let en = (mode > 0) as u8;
                self.link.request(self.idx_rgb, 0x80, &[en, en])
            }
            Page::P8071 => self.link.request(self.idx_rgb, 0x50, &[0x01, mode, flags]),
        };
    }

    fn host_mode(&self, host: bool) {
        if self.idx_profile != 0 {
            let _ = self.link.request(self.idx_profile, 0x60, &[if host { 0x05 } else { 0x03 }]);
        } else if self.idx_onboard != 0 {
            let _ = self.link.request(self.idx_onboard, 0x10, &[if host { 0x02 } else { 0x01 }]);
        }
    }

    fn claim(&mut self) {
        if self.claimed {
            return;
        }
        match self.page {
            Page::P8070 => self.sw_control(1, 1),
            Page::P8071 => {
                self.sw_control(3, 6);
                if self.idx_disable_keys != 0 {
                    let _ = self.link.request_t(self.idx_disable_keys, 0x30, &[], Duration::from_millis(150), 1);
                    let _ = self.link.request_t(self.idx_disable_keys, 0x10, &[], Duration::from_millis(150), 1);
                }
                if self.idx_rgb != 0 {
                    let _ = self.link.request(self.idx_rgb, 0x80, &[0x01, 0x01]); // RGB power mode actif
                }
                self.host_mode(true);
            }
        }
        self.claimed = true;
        self.upgraded = false;
        self.prepped = false;
    }

    fn upgrade(&mut self) {
        if !self.upgraded && self.page == Page::P8071 {
            self.sw_control(3, 5);
        }
        self.upgraded = true;
    }

    fn set_zone_effect(&self, cluster: u8, eff: &Effect, c: Color, period: u16, bright: u8, dir: u8, persist: bool) -> Result<()> {
        let mut d = [0u8; 16];
        d[0] = cluster;
        d[1] = eff.index;
        let (ph, pl) = ((period >> 8) as u8, period as u8);
        match eff.id {
            0x0001 => {
                d[2..5].copy_from_slice(&[c.r, c.g, c.b]);
                if c != Color::BLACK {
                    d[5] = 0x02;
                }
            }
            0x000A => {
                d[2..5].copy_from_slice(&[c.r, c.g, c.b]);
                d[5] = ph;
                d[6] = pl;
                d[8] = bright;
            }
            0x0003 | 0x000F | 0x0010 => {
                d[7] = ph;
                d[8] = pl;
                d[9] = bright;
            }
            0x0007 => {
                if c != Color::BLACK {
                    d[3..6].copy_from_slice(&[c.r, c.g, c.b]);
                } else {
                    d[2] = 1;
                }
                d[7] = ph;
                d[8] = pl;
            }
            0x0004 => {
                d[8] = pl;
                d[9] = dir;
                d[10] = bright.max(1);
                d[11] = ph;
            }
            0x000B => {
                d[2..5].copy_from_slice(&[c.r, c.g, c.b]);
                d[6] = ph;
                d[7] = pl;
            }
            0x000E => {
                d[8] = ph;
                d[9] = pl;
                d[10] = bright;
            }
            0x0015 => {
                d[3] = 0xFF;
                d[8] = ph;
                d[9] = pl;
                d[10] = bright;
            }
            0x0016 => {
                d[3] = 0xFF;
                d[8] = ph;
                d[9] = pl;
                d[10] = bright;
                d[11] = dir;
            }
            0x0017 => {
                d[2..5].copy_from_slice(&[c.r, c.g, c.b]);
                d[5] = 0xFF;
                d[8] = ph;
                d[9] = pl;
            }
            _ => {
                d[2..5].copy_from_slice(&[c.r, c.g, c.b]);
                d[5] = ph;
                d[6] = pl;
            }
        }
        d[12] = persist as u8;
        let func = if self.page == Page::P8071 { 0x10 } else { 0x30 };
        self.link.request(self.idx_rgb, func, &d)?;
        Ok(())
    }

    fn static_effect(&self, c: &Cluster) -> Option<Effect> {
        c.effects.iter().find(|e| e.id == 0x0001).cloned()
    }

    /// Prépare le passage en mode touche par touche (une fois par prise de contrôle).
    fn perkey_prep(&mut self) {
        if self.prepped {
            return;
        }
        self.prepped = true;
        if self.page == Page::P8071 && self.idx_disable_keys != 0 {
            // famille G815 / G915 / G PRO : clusters éteints + touche d'amorçage
            for c in &self.clusters {
                let mut d = [0u8; 16];
                d[0] = c.index;
                d[12] = 1;
                let _ = self.link.request(self.idx_rgb, 0x10, &d);
            }
            if let Some(&z) = self.wire.first() {
                let _ = self.link.request(self.idx_perkey, 0x10, &[z, 0, 0, 0]);
                let _ = self.frame_end();
            }
        } else {
            for c in &self.clusters {
                if let Some(e) = self.static_effect(c) {
                    let _ = self.set_zone_effect(c.index, &e, Color::BLACK, 0, 100, 0, true);
                }
            }
        }
    }

    fn frame_end(&self) -> Result<()> {
        for _ in 0..30 {
            self.link.write(self.idx_perkey, 0x70, &[0u8; 16]).map_err(Error::from)?;
            match self.link.wait(self.idx_perkey, 0x70, Duration::from_millis(100)) {
                Ok(_) => return Ok(()),
                Err(ReqErr::Hidpp(8)) => std::thread::sleep(Duration::from_millis(5)),
                Err(ReqErr::Timeout) => return Ok(()),
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }

    fn perkey_frame(&mut self, colors: &[Color]) -> Result<()> {
        self.claim();
        self.perkey_prep();
        let full = self.sent.len() != colors.len();
        // regroupe les touches modifiées par couleur
        let mut groups: HashMap<Color, Vec<u8>> = HashMap::new();
        for (i, &c) in colors.iter().enumerate() {
            if full || self.sent[i] != c {
                groups.entry(c).or_default().push(self.wire[i]);
            }
        }
        if groups.is_empty() {
            return Ok(());
        }
        let mut packets: Vec<(u8, Vec<u8>)> = Vec::new();
        let mut singles: Vec<u8> = Vec::new();
        for (c, ids) in groups {
            if ids.len() >= 5 {
                for chunk in ids.chunks(13) {
                    let mut p = vec![c.r, c.g, c.b];
                    p.extend_from_slice(chunk);
                    packets.push((0x60, p));
                }
            } else {
                for z in ids {
                    singles.extend_from_slice(&[z, c.r, c.g, c.b]);
                }
            }
        }
        for chunk in singles.chunks(16) {
            packets.push((0x10, chunk.to_vec()));
        }
        // envoi en pipeline, fenêtre de 8 paquets non acquittés
        let mut inflight = 0;
        for (func, p) in &packets {
            self.link.write(self.idx_perkey, *func, p).map_err(Error::from)?;
            inflight += 1;
            if inflight >= 8 {
                self.read_ack(Duration::from_millis(60));
                inflight -= 1;
            }
        }
        while inflight > 0 {
            if !self.read_ack(Duration::from_millis(40)) {
                break;
            }
            inflight -= 1;
        }
        self.frame_end()?;
        self.sent = colors.to_vec();
        self.upgrade();
        Ok(())
    }

    fn read_ack(&self, t: Duration) -> bool {
        let mut b = [0u8; 64];
        let deadline = Instant::now() + t;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            match self.link.dev.read_timeout(&mut b, left.as_millis().max(1) as i32) {
                Ok(n) if n >= 4 && b[1] == self.link.idx => {
                    if (b[2] == self.idx_perkey && b[3] & 0x0F == SWID) || (b[2] == 0xFF && b[3] == self.idx_perkey) {
                        return true;
                    }
                }
                Ok(0) | Err(_) => return false,
                _ => {}
            }
        }
    }

    fn zone_frame(&mut self, colors: &[Color]) -> Result<()> {
        self.claim();
        let full = self.sent.len() != colors.len();
        for (i, &c) in colors.iter().enumerate() {
            if !full && self.sent[i] == c {
                continue;
            }
            let ci = self.wire[i];
            if let Some(cl) = self.clusters.iter().find(|x| x.index == ci) {
                if let Some(e) = self.static_effect(cl) {
                    self.set_zone_effect(ci, &e, c, 0, 100, 0, false)?;
                }
            }
        }
        self.sent = colors.to_vec();
        self.upgrade();
        Ok(())
    }
}

impl Device for Logitech {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, mode: usize, p: &HwParams) -> Result<()> {
        let Some(&eid) = self.mode_ids.get(mode) else {
            return err("mode inconnu");
        };
        self.claim();
        self.sent.clear();
        if eid == 0xFFFF {
            if self.perkey {
                let n = self.wire.len();
                return self.set_leds(&vec![Color::BLACK; n]);
            }
            for c in self.clusters.clone() {
                if let Some(e) = self.static_effect(&c) {
                    self.set_zone_effect(c.index, &e, Color::BLACK, 0, 100, 0, false)?;
                }
            }
            return Ok(());
        }
        if self.page == Page::P8071 {
            // le moteur d'effets du firmware doit rester actif
            self.sw_control(3, 6);
            self.upgraded = true;
            self.prepped = false;
        }
        let ripple = matches!(eid, 0x000B | 0x0017);
        let period = if ripple {
            p.speed_in(200, 2) as u16
        } else {
            p.speed_in(20000, 1000) as u16
        };
        let bright = p.brightness_in(1, 100) as u8;
        let dir = match p.direction {
            Direction::Right => 1,
            Direction::Left => 6,
            Direction::Down => 2,
            Direction::Up => 7,
            Direction::Out => 3,
            Direction::In => 8,
        };
        let persist = self.page == Page::P8071;
        for c in self.clusters.clone() {
            // même type d'effet, ou variante équivalente, dans ce cluster
            let eq = |a: u16| match a {
                0x0015 => 0x0003,
                0x0016 => 0x0004,
                0x0017 => 0x000B,
                x => x,
            };
            if let Some(e) = c.effects.iter().find(|e| eq(e.id) == eq(eid)).cloned() {
                self.set_zone_effect(c.index, &e, p.color(0), period, bright, dir, persist)?;
            }
        }
        Ok(())
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        if !self.v1_wire.is_empty() {
            self.perkey_v1_frame(colors)
        } else if self.perkey {
            self.perkey_frame(colors)
        } else {
            self.zone_frame(colors)
        }
    }

    fn max_fps(&self) -> u32 {
        if self.wireless {
            15
        } else {
            40
        }
    }

    fn release(&mut self) {
        if self.claimed {
            self.sw_control(0, 0);
            self.host_mode(false);
            self.claimed = false;
        }
    }
}
