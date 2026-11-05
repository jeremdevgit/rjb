// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Lian Li Uni Hub SL V2 / AL V2 (ventilateurs UNI FAN, 4 canaux jusqu'à 6
//! ventilateurs de 16 LED). Protocole d'après OpenRGB.

use super::hid::{self, HidEntry};
use crate::device::{Device, Result};
use crate::model::*;
use hidapi::{DeviceInfo as HidInfo, HidApi, HidDevice};
use std::time::Duration;

const PER_FAN: usize = 16;
const MAX_FANS: usize = 6;

pub const HID: &[HidEntry] = &[HidEntry {
    vid: 0x0CF2,
    pids: &[0xA103, 0xA104, 0xA105],
    usage_page: Some(0xFF72),
    usage: Some(0xA1),
    interface: Some(1),
    open,
}];

pub struct LianLi {
    info: DeviceInfo,
    dev: HidDevice,
}

fn open(api: &HidApi, i: &HidInfo) -> Result<Vec<Box<dyn Device>>> {
    let dev = api.open_path(i.path())?;
    let zones = (0..4)
        .map(|c| Zone::resizable(format!("Canal {}", c + 1), 3 * PER_FAN, PER_FAN as u16, (MAX_FANS * PER_FAN) as u16))
        .collect();
    let modes = vec![
        ModeInfo::new("Statique").colors(1, 1).brightness(),
        ModeInfo::new("Respiration").colors(1, 1).brightness().speed(),
        ModeInfo::new("Arc-en-ciel").brightness().speed().dirs(&[Direction::Right, Direction::Left]),
        ModeInfo::new("Morphing arc-en-ciel").brightness().speed(),
        ModeInfo::new("Cycle de couleurs").colors(1, 3).brightness().speed().dirs(&[Direction::Right, Direction::Left]),
        ModeInfo::new("Météore").colors(1, 2).brightness().speed().dirs(&[Direction::Right, Direction::Left]),
        ModeInfo::new("Marée").colors(1, 2).brightness().speed().dirs(&[Direction::Right, Direction::Left]),
        ModeInfo::new("Empilement").colors(1, 1).brightness().speed().dirs(&[Direction::Right, Direction::Left]),
        ModeInfo::new("Néon").brightness().speed(),
        ModeInfo::new("Éteint").off(),
    ];
    Ok(vec![Box::new(LianLi {
        info: DeviceInfo {
            id: hid::stable_id("lianli", i),
            name: "Lian Li Uni Hub".into(),
            vendor: "Lian Li".into(),
            kind: DeviceKind::FanController,
            location: hid::location(i),
            zones,
            modes,
            direct: true,
        },
        dev,
    })])
}

/// Le fabricant limite le blanc pur pour protéger les LED.
fn limit(c: Color) -> Color {
    let sum = c.r as u32 + c.g as u32 + c.b as u32;
    if sum > 460 {
        c.scale(460.0 / sum as f32)
    } else {
        c
    }
}

impl LianLi {
    fn send(&self, p: &[u8]) -> Result<()> {
        let mut b = [0u8; 353];
        b[..p.len()].copy_from_slice(p);
        self.dev.write(&b)?;
        std::thread::sleep(Duration::from_millis(5));
        Ok(())
    }

    fn channel(&self, ch: u8, fans: usize, data: &[u8], effect: u8, speed: u8, dir: u8, bright: u8) -> Result<()> {
        self.send(&[0xE0, 0x10, 0x60, (ch << 4) + fans as u8])?;
        let mut p = vec![0xE0, 0x30 + ch];
        p.extend_from_slice(data);
        self.send(&p)?;
        self.send(&[0xE0, 0x10 + ch, effect, speed, dir, bright])
    }
}

impl Device for LianLi {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, mode: usize, p: &HwParams) -> Result<()> {
        let m = self.info.modes[mode].clone();
        let code = match m.name.as_str() {
            "Respiration" => 0x02,
            "Arc-en-ciel" => 0x05,
            "Morphing arc-en-ciel" => 0x04,
            "Cycle de couleurs" => 0x23,
            "Météore" => 0x24,
            "Marée" => 0x1A,
            "Empilement" => 0x20,
            "Néon" => 0x22,
            _ => 0x01,
        };
        let speed = [0x02, 0x01, 0x00, 0xFF, 0xFE][p.speed_in(0, 4) as usize];
        let bright = if m.is_off { 0x08 } else { [0x08, 0x03, 0x02, 0x01, 0x00][p.brightness_in(0, 4) as usize] };
        let dir = (p.direction == Direction::Left) as u8;
        for ch in 0..4u8 {
            let fans = self.info.zones[ch as usize].leds.len().div_ceil(PER_FAN);
            let mut data = vec![0u8; MAX_FANS * PER_FAN * 3];
            if code == 0x01 || code == 0x02 {
                let c = limit(p.color(0));
                for led in 0..fans * PER_FAN {
                    data[led * 3..led * 3 + 3].copy_from_slice(&[c.r, c.b, c.g]);
                }
            } else if m.colors.1 > 0 {
                // une couleur par motif, répétée pour chaque ventilateur
                for k in 0..4usize {
                    let c = if k < p.colors.len() { limit(p.color(k)) } else { Color::BLACK };
                    for f in 0..MAX_FANS {
                        let o = (f * 12) + k * 3;
                        data[o..o + 3].copy_from_slice(&[c.r, c.b, c.g]);
                    }
                }
            }
            self.channel(ch, fans.max(1), &data[..fans.max(1) * PER_FAN * 3], code, speed, dir, bright)?;
        }
        Ok(())
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        let mut off = 0;
        for ch in 0..4u8 {
            let n = self.info.zones[ch as usize].leds.len();
            let fans = n.div_ceil(PER_FAN).max(1);
            let mut data = vec![0u8; fans * PER_FAN * 3];
            for (k, &c) in colors[off..off + n].iter().enumerate() {
                let c = limit(c);
                data[k * 3..k * 3 + 3].copy_from_slice(&[c.r, c.b, c.g]);
            }
            self.channel(ch, fans, &data, 0x01, 0x02, 0x00, 0x00)?;
            off += n;
        }
        Ok(())
    }

    fn resize_zone(&mut self, zone: usize, leds: usize) -> Result<()> {
        let name = self.info.zones[zone].name.clone();
        let n = leds.clamp(PER_FAN, MAX_FANS * PER_FAN);
        self.info.zones[zone] = Zone::resizable(name, n, PER_FAN as u16, (MAX_FANS * PER_FAN) as u16);
        Ok(())
    }

    fn max_fps(&self) -> u32 {
        15
    }
}
