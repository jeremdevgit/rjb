// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Outils communs aux pilotes USB HID.

use crate::device::{Device, Result};
use hidapi::{DeviceInfo as HidInfo, HidApi};

/// Décrit quelles interfaces HID un pilote sait ouvrir.
pub struct HidEntry {
    pub vid: u16,
    /// PID acceptés (vide = tous)
    pub pids: &'static [u16],
    pub usage_page: Option<u16>,
    pub usage: Option<u16>,
    pub interface: Option<i32>,
    pub open: fn(&HidApi, &HidInfo) -> Result<Vec<Box<dyn Device>>>,
}

impl HidEntry {
    pub fn matches(&self, i: &HidInfo) -> bool {
        (self.vid == 0 || i.vendor_id() == self.vid)
            && (self.pids.is_empty() || self.pids.contains(&i.product_id()))
            && self.usage_page.is_none_or(|u| u == i.usage_page())
            && self.usage.is_none_or(|u| u == i.usage())
            && self.interface.is_none_or(|n| n == i.interface_number())
    }
}

/// Raccourci pour déclarer une entrée.
pub const fn entry(
    vid: u16,
    pids: &'static [u16],
    usage_page: Option<u16>,
    usage: Option<u16>,
    interface: Option<i32>,
    open: fn(&HidApi, &HidInfo) -> Result<Vec<Box<dyn Device>>>,
) -> HidEntry {
    HidEntry {
        vid,
        pids,
        usage_page,
        usage,
        interface,
        open,
    }
}

/// Identifiant « vid:pid » extrait d'un emplacement HID (pour éviter les doublons).
pub fn vid_pid_of(location: &str) -> Option<(u16, u16)> {
    let l = location.to_ascii_lowercase();
    let v = l.find("vid_")?;
    let p = l.find("pid_")?;
    let vid = u16::from_str_radix(l.get(v + 4..v + 8)?, 16).ok()?;
    let pid = u16::from_str_radix(l.get(p + 4..p + 8)?, 16).ok()?;
    Some((vid, pid))
}

pub fn location(i: &HidInfo) -> String {
    format!("hid:{}", i.path().to_string_lossy())
}

/// Identifiant stable : VID:PID + numéro de série s'il existe,
/// sinon le chemin USB (stable tant que l'appareil reste sur le même port).
pub fn stable_id(prefix: &str, i: &HidInfo) -> String {
    let serial = i.serial_number().unwrap_or("").trim();
    if !serial.is_empty() && serial.chars().any(|c| c != '0') {
        format!("{prefix}:{:04x}:{:04x}:{serial}", i.vendor_id(), i.product_id())
    } else {
        let path = i.path().to_string_lossy();
        // retire la partie collection/interface pour regrouper un même appareil
        let key: String = path
            .split('#')
            .nth(2)
            .unwrap_or(&path)
            .chars()
            .take(24)
            .collect();
        format!("{prefix}:{:04x}:{:04x}:{key}", i.vendor_id(), i.product_id())
    }
}

pub fn product(i: &HidInfo, fallback: &str) -> String {
    i.product_string()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}
