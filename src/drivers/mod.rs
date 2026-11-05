// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Détection des appareils et registre des pilotes.

pub mod asrock_polychrome;
pub mod asus_aura_usb;
pub mod corsair_commander_core;
pub mod corsair_dram;
pub mod corsair_hydro_platinum;
pub mod corsair_v1;
pub mod corsair_v1_table;
pub mod corsair_lighting_node;
pub mod corsair_v2;
pub mod corsair_v2_table;
pub mod crucial;
pub mod ene;
pub mod gigabyte_fusion2_smbus;
pub mod gigabyte_fusion2_usb;
pub mod gpu;
pub mod gpu_table;
pub mod hid;
pub mod hyperx_dram;
pub mod hyperx_keyboard;
pub mod hyperx_mouse;
pub mod kingston_fury;
pub mod lamparray;
pub mod lianli;
pub mod logitech;
pub mod msi_mystic_light;
pub mod msi_rgb;
pub mod nvapi;
pub mod nzxt_hue2;
pub mod pawnio;
pub mod razer;
pub mod razer_table;
pub mod smbus;
pub mod spd;
pub mod steelseries_apex;
pub mod steelseries_mouse;
pub mod steelseries_rival;
pub mod wooting;

use crate::device::Device;
use hid::HidEntry;
use hidapi::HidApi;
use std::collections::HashSet;

pub struct ScanResult {
    pub new_devices: Vec<Box<dyn Device>>,
    /// Emplacements encore présents (pour retirer les appareils débranchés).
    pub present: HashSet<String>,
}

fn hid_drivers() -> impl Iterator<Item = &'static HidEntry> {
    [
        logitech::HID,
        asus_aura_usb::HID,
        msi_mystic_light::HID,
        gigabyte_fusion2_usb::HID,
        razer::HID,
        corsair_v2::HID,
        corsair_hydro_platinum::HID,
        corsair_v1::HID,
        corsair_lighting_node::HID,
        corsair_commander_core::HID,
        nzxt_hue2::HID,
        lianli::HID,
        steelseries_apex::HID,
        steelseries_mouse::HID,
        steelseries_rival::HID,
        hyperx_keyboard::HID,
        hyperx_mouse::HID,
        wooting::HID,
        lamparray::HID,
    ]
    .into_iter()
    .flatten()
}

pub struct Scanner {
    hid: Option<HidApi>,
    /// Interfaces déjà sondées sans résultat (évite de les réinterroger).
    negative: HashSet<String>,
    internal: bool,
    internal_done: bool,
    internal_locs: HashSet<String>,
    notices: Vec<String>,
    /// Interfaces HID présentes lors de la dernière recherche.
    seen: HashSet<String>,
}

impl Scanner {
    pub fn new(internal: bool) -> Self {
        let mut notices = Vec::new();
        let hid = match HidApi::new() {
            Ok(h) => Some(h),
            Err(e) => {
                notices.push(format!("USB HID indisponible : {e}"));
                None
            }
        };
        Scanner {
            hid,
            negative: HashSet::new(),
            internal,
            internal_done: false,
            internal_locs: HashSet::new(),
            notices,
            seen: HashSet::new(),
        }
    }

    pub fn notices(&self) -> Vec<String> {
        let mut n = self.notices.clone();
        let seen = &self.seen;
        for name in logitech::without_lighting(&|loc| seen.contains(loc)) {
            n.push(format!("{INFO_PREFIX}{name} : reconnu, mais sans éclairage RGB à piloter"));
        }
        n
    }

    /// Un récepteur sans fil n'a eu aucune réponse (souris en veille) : il faut
    /// refaire une recherche plus tard pour la trouver à son réveil.
    pub fn retry_soon(&self) -> bool {
        self.seen.iter().any(|l| logitech::waiting(l))
    }

    /// Recherche manuelle : on réinterroge aussi les interfaces déjà écartées.
    pub fn forget_negatives(&mut self) {
        self.negative.clear();
    }

    /// `open` : emplacements déjà ouverts ; `open_ids` : identifiants déjà ouverts.
    pub fn scan(&mut self, open: &HashSet<String>, open_ids: &HashSet<String>) -> ScanResult {
        let mut out = ScanResult {
            new_devices: Vec::new(),
            present: HashSet::new(),
        };
        let mut ids: HashSet<String> = open_ids.clone();
        self.scan_internal(&mut out, &mut ids);
        if let Some(api) = self.hid.as_mut() {
            let _ = api.reset_devices();
            let _ = api.add_devices(0, 0);
            let api = &*api;
            let mut seen_paths = HashSet::new();
            // appareils déjà pris par un pilote dédié (VID, PID) : les pilotes
            // génériques (LampArray…) ne passent qu'après eux
            let mut claimed: HashSet<(u16, u16)> = open.iter().filter_map(|o| hid::vid_pid_of(o)).collect();
            for generic in [false, true] {
                for info in api.device_list() {
                    let loc = hid::location(info);
                    seen_paths.insert(loc.clone());
                    // un même appareil peut exposer plusieurs adresses (récepteur : #index)
                    let mut already = false;
                    for o in open {
                        if o.starts_with(&loc) {
                            out.present.insert(o.clone());
                            already = true;
                        }
                    }
                    if already || self.negative.contains(&loc) {
                        continue;
                    }
                    if generic && claimed.contains(&(info.vendor_id(), info.product_id())) {
                        continue;
                    }
                    let mut found = false;
                    let mut tried = false;
                    for d in hid_drivers().filter(|d| (d.vid == 0) == generic && d.matches(info)) {
                        tried = true;
                        match (d.open)(api, info) {
                            Ok(devs) => {
                                for dev in devs {
                                    let i = dev.info();
                                    if ids.insert(i.id.clone()) {
                                        found = true;
                                        out.present.insert(i.location.clone());
                                        out.new_devices.push(dev);
                                    }
                                }
                            }
                            Err(e) => crate::log(&format!("{loc} : {e}")),
                        }
                        if found {
                            break;
                        }
                    }
                    if found {
                        claimed.insert((info.vendor_id(), info.product_id()));
                    } else if tried && generic && !logitech::waiting(&loc) {
                        self.negative.insert(loc);
                    }
                }
            }
            // une interface sans pilote ne sera plus sondée tant qu'elle reste branchée
            for info in api.device_list() {
                let loc = hid::location(info);
                if !out.present.iter().any(|p| p.starts_with(&loc)) && !logitech::waiting(&loc) {
                    self.negative.insert(loc);
                }
            }
            self.negative.retain(|l| seen_paths.contains(l));
            self.seen = seen_paths;
        }
        out
    }

    /// RAM, carte mère : détectés une seule fois (pas de branchement à chaud).
    fn scan_internal(&mut self, out: &mut ScanResult, ids: &mut HashSet<String>) {
        if !self.internal {
            return;
        }
        if self.internal_done {
            out.present.extend(self.internal_locs.iter().cloned());
            return;
        }
        self.internal_done = true;
        let mut devs: Vec<Box<dyn Device>> = Vec::new();
        // cartes graphiques NVIDIA : via le pilote NVIDIA, sans PawnIO
        gpu::detect(&mut devs);
        let pawn = pawnio::available();
        if let Err(e) = &pawn {
            self.notices.push(format!("{e} : RAM et carte mère indisponibles (Réglages)"));
        }
        let (buses, errors) = if pawn.is_ok() { smbus::open_buses() } else { (Vec::new(), Vec::new()) };
        if errors.iter().any(|e| e.contains("administrateur")) {
            self.notices.push(ADMIN_NOTICE.to_string());
        }
        let (manufacturer, product) = smbus::board();
        crate::log(&format!("Carte mère : {manufacturer} / {product}"));
        for bus in buses.iter().filter(|b| b.is_dram()) {
            // RAM : l'ordre évite qu'un pilote n'écrive sur la barrette d'un autre
            corsair_dram::detect(bus, &mut devs);
            let spd = spd::read_slots(bus);
            let taken: Vec<String> = devs.iter().map(|d| d.info().location.clone()).collect();
            let fury = kingston_fury::detect(bus, &spd, &taken, &mut devs);
            let before = devs.len();
            if !fury {
                hyperx_dram::detect(bus, &spd, &mut devs);
            }
            let hyperx = devs.len() > before;
            if !hyperx && (spd.is_empty() || spd.iter().any(|s| s.jedec == 0x002B)) {
                crucial::detect(bus, &mut devs);
            }
            ene::detect_ram(bus, &mut devs);
            ene::detect_motherboard(bus, &product, &mut devs);
            gigabyte_fusion2_smbus::detect(bus, &manufacturer, &product, &mut devs);
            asrock_polychrome::detect(bus, &product, &mut devs);
        }
        if pawn.is_ok() && !errors.iter().any(|e| e.contains("administrateur")) {
            msi_rgb::detect(&manufacturer, &product, &mut devs);
        }

        for d in devs {
            let i = d.info();
            if ids.insert(i.id.clone()) {
                self.internal_locs.insert(i.location.clone());
                out.present.insert(i.location.clone());
                out.new_devices.push(d);
            }
        }
    }
}

/// Message d'information (et non d'avertissement) dans la liste des appareils.
pub const INFO_PREFIX: &str = "ℹ ";

pub const ADMIN_NOTICE: &str ="Droits administrateur requis pour la RAM et la carte mère";


#[cfg(test)]
mod tests {
    /// Détection réelle, en lecture seule (aucune couleur envoyée) :
    /// `cargo test --release -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn detection_reelle() {
        let mut s = super::Scanner::new(false);
        let r = s.scan(&Default::default(), &Default::default());
        for d in &r.new_devices {
            let i = d.info();
            println!("appareil : {} ({:?}, {} zones)", i.name, i.kind, i.zones.len());
        }
        for n in s.notices() {
            println!("message : {n}");
        }
        println!("nouvelle recherche prévue : {}", s.retry_soon());
    }
}
