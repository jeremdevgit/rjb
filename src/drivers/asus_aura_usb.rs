// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! ASUS Aura USB : contrôleur des cartes mères ASUS (2019+) et des
//! contrôleurs ARGB/terminal Aura. Protocole d'après OpenRGB.

use super::hid::{self, entry, HidEntry};
use crate::device::{err, Device, Result};
use crate::model::*;
use hidapi::{DeviceInfo as HidInfo, HidApi, HidDevice};

const VID: u16 = 0x0B05;
const MAINBOARDS: &[u16] = &[0x18F3, 0x1939, 0x19AF, 0x1AA6, 0x1BED];
const ADDRESSABLE: &[u16] = &[0x1867, 0x1872, 0x18A3, 0x18A5];
const MODE_DIRECT: u8 = 0xFF;
const DEFAULT_ARGB_LEDS: usize = 20;

pub const HID: &[HidEntry] = &[
    entry(VID, MAINBOARDS, Some(0xFF72), None, None, open),
    entry(VID, ADDRESSABLE, None, None, None, open),
];

#[derive(Clone, Copy)]
struct Channel {
    effect: u8,
    direct: u8,
    fixed: bool,
}

pub struct AsusAuraUsb {
    info: DeviceInfo,
    dev: HidDevice,
    channels: Vec<Channel>,
    mainboard: bool,
    in_direct: bool,
}

fn send(dev: &HidDevice, data: &[u8]) -> Result<()> {
    let mut b = [0u8; 65];
    b[0] = 0xEC;
    b[1..1 + data.len()].copy_from_slice(data);
    dev.write(&b)?;
    Ok(())
}

fn request(dev: &HidDevice, cmd: u8) -> Result<[u8; 65]> {
    send(dev, &[cmd])?;
    let mut b = [0u8; 65];
    dev.read_timeout(&mut b, 500)?;
    Ok(b)
}

fn open(api: &HidApi, i: &HidInfo) -> Result<Vec<Box<dyn Device>>> {
    let dev = api.open_path(i.path())?;
    let mainboard = MAINBOARDS.contains(&i.product_id());
    let _version = request(&dev, 0x82)?;
    let t = request(&dev, 0xB0)?;
    if t[1] != 0x30 {
        return err("table de configuration Aura illisible");
    }
    let cfg = &t[4..64];
    let mut channels = Vec::new();
    let mut zones = Vec::new();
    let argb = cfg[0x02] as usize;
    if mainboard {
        let leds = cfg[0x1B] as usize;
        if leds > 0 {
            channels.push(Channel { effect: 0, direct: 0x04, fixed: true });
            zones.push(Zone::linear("Carte mère", leds));
        }
        let first = channels.len() as u8;
        for h in 0..argb {
            channels.push(Channel { effect: first + h as u8, direct: h as u8, fixed: false });
            zones.push(Zone::resizable(format!("ARGB {}", h + 1), DEFAULT_ARGB_LEDS, 0, 500));
        }
        // passage en mode « génération 1 » (comme le logiciel ASUS)
        send(&dev, &[0x52, 0x53, 0x00, 0x01])?;
    } else {
        for h in 0..argb {
            channels.push(Channel { effect: h as u8, direct: h as u8, fixed: false });
            zones.push(Zone::resizable(format!("ARGB {}", h + 1), DEFAULT_ARGB_LEDS, 0, 500));
        }
    }
    if zones.is_empty() {
        return Ok(Vec::new());
    }
    let (board_vendor, board) = super::smbus::board();
    let name = if mainboard && board_vendor.to_uppercase().contains("ASUS") {
        format!("ASUS {}", pretty_name(&board))
    } else {
        hid::product(i, "ASUS Aura")
    };
    let mut modes = vec![
        ModeInfo::new("Statique").colors(1, 1),
        ModeInfo::new("Respiration").colors(1, 1),
        ModeInfo::new("Flash").colors(1, 1),
        ModeInfo::new("Cycle de couleurs"),
        ModeInfo::new("Arc-en-ciel"),
        ModeInfo::new("Poursuite en fondu").colors(1, 1),
        ModeInfo::new("Poursuite").colors(1, 1),
        ModeInfo::new("Respiration arc-en-ciel"),
        ModeInfo::new("Vague arc-en-ciel"),
        ModeInfo::new("Scintillement aléatoire"),
        ModeInfo::new("Transition douce"),
        ModeInfo::new("Propagation"),
        ModeInfo::new("Éteint").off(),
    ];
    modes.shrink_to_fit();
    Ok(vec![Box::new(AsusAuraUsb {
        info: DeviceInfo {
            id: hid::stable_id("aura", i),
            name,
            vendor: "ASUS".into(),
            kind: if mainboard { DeviceKind::Motherboard } else { DeviceKind::FanController },
            location: hid::location(i),
            zones,
            modes,
            direct: true,
        },
        dev,
        channels,
        mainboard,
        in_direct: false,
    })])
}

fn mode_code(name: &str) -> u8 {
    match name {
        "Statique" => 1,
        "Respiration" => 2,
        "Flash" => 3,
        "Cycle de couleurs" => 4,
        "Arc-en-ciel" => 5,
        "Respiration arc-en-ciel" => 6,
        "Poursuite en fondu" => 7,
        "Poursuite" => 9,
        "Vague arc-en-ciel" => 11,
        "Scintillement aléatoire" => 13,
        "Transition douce" => 16,
        "Propagation" => 17,
        _ => 0,
    }
}

impl AsusAuraUsb {
    fn set_channel_mode(&self, ch: usize, mode: u8, c: Color) -> Result<()> {
        let chan = self.channels[ch];
        if !self.mainboard {
            return send(&self.dev, &[0x3B, chan.effect, 0x00, mode, c.r, c.g, c.b]);
        }
        send(&self.dev, &[0x35, chan.effect, 0x00, 0x00, mode])?;
        if mode == MODE_DIRECT {
            return Ok(());
        }
        // couleur de l'effet : masque des LED concernées
        let start: usize = self.info.zones[..ch].iter().map(|z| z.leds.len()).sum::<usize>().min(15);
        let count = self.info.zones[ch].leds.len().min(16 - start);
        let mask: u16 = (((1u32 << count) - 1) << start) as u16;
        let mut p = vec![0u8; 64];
        p[0] = 0x36;
        p[1] = (mask >> 8) as u8;
        p[2] = mask as u8;
        for i in 0..count {
            let o = 4 + 3 * (start + i);
            if o + 2 < p.len() {
                p[o..o + 3].copy_from_slice(&[c.r, c.g, c.b]);
            }
        }
        send(&self.dev, &p)
    }

    fn send_direct(&self, channel: u8, colors: &[Color]) -> Result<()> {
        let n = colors.len();
        let mut done = 0usize;
        let mut cmd = channel;
        let mut offset: u8 = 0;
        while done < n || n == 0 {
            let count = (n - done).min(20);
            if done + count >= n {
                cmd |= 0x80; // appliquer
            }
            let mut p = vec![0x40, cmd, offset, count as u8];
            for c in &colors[done..done + count] {
                p.extend_from_slice(&[c.r, c.g, c.b]);
            }
            send(&self.dev, &p)?;
            done += count;
            offset = offset.wrapping_add(count as u8);
            if done > 255 {
                cmd |= 0x10;
                offset = (done - 256) as u8;
            }
            if n == 0 {
                break;
            }
        }
        Ok(())
    }
}

impl Device for AsusAuraUsb {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, mode: usize, p: &HwParams) -> Result<()> {
        self.in_direct = false;
        let m = &self.info.modes[mode];
        let code = mode_code(&m.name);
        let c = if m.colors.1 > 0 { p.color(0) } else { Color::BLACK };
        for ch in 0..self.channels.len() {
            if !self.info.zones[ch].leds.is_empty() {
                self.set_channel_mode(ch, code, c)?;
            }
        }
        if self.mainboard {
            send(&self.dev, &[0x3F, 0x55])?; // enregistrer
        }
        Ok(())
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        if !self.in_direct {
            for ch in 0..self.channels.len() {
                self.set_channel_mode(ch, MODE_DIRECT, Color::BLACK)?;
            }
            self.in_direct = true;
        }
        let mut off = 0;
        for (ch, z) in self.info.zones.iter().enumerate() {
            let n = z.leds.len();
            if n > 0 {
                self.send_direct(self.channels[ch].direct, &colors[off..off + n])?;
            }
            off += n;
        }
        Ok(())
    }

    fn resize_zone(&mut self, zone: usize, leds: usize) -> Result<()> {
        if zone < self.channels.len() && !self.channels[zone].fixed {
            let name = self.info.zones[zone].name.clone();
            self.info.zones[zone] = Zone::resizable(name, leds, 0, 500);
            self.in_direct = false;
            Ok(())
        } else {
            err("zone non redimensionnable")
        }
    }
}
