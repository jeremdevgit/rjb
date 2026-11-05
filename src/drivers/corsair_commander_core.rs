// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Corsair Commander Core / Core XT (watercoolings iCUE Elite Capellix, H100i…
//! et leurs ventilateurs). Protocole d'après OpenRGB.

use super::hid::{self, HidEntry};
use crate::device::{Device, Result};
use crate::model::*;
use hidapi::{DeviceInfo as HidInfo, HidApi, HidDevice};
use std::time::Duration;

const XT: u16 = 0x0C2A;

pub const HID: &[HidEntry] = &[HidEntry {
    vid: 0x1B1C,
    pids: &[0x0C1C, 0x0C32, 0x0C1D, 0x0C3C, 0x0C3D, 0x0C3E, XT],
    usage_page: Some(0xFF42),
    usage: Some(0x01),
    interface: Some(0),
    open,
}];

pub struct CommanderCore {
    info: DeviceInfo,
    dev: HidDevice,
    packet: usize,
    xt: bool,
}

impl CommanderCore {
    /// Commande (2 octets) + données ; retourne la réponse (après l'en-tête).
    fn command(&self, cmd: [u8; 2], data: &[u8], read: bool) -> Result<Vec<u8>> {
        let mut b = vec![0u8; self.packet];
        b[1] = 0x08;
        b[2..4].copy_from_slice(&cmd);
        let n = data.len().min(self.packet - 4);
        b[4..4 + n].copy_from_slice(&data[..n]);
        self.dev.write(&b)?;
        if read {
            for _ in 0..8 {
                let mut r = vec![0u8; self.packet];
                let got = self.dev.read_timeout(&mut r, 500)?;
                if got == 0 {
                    break;
                }
                if r[0] == 0x00 {
                    return Ok(r[3..].to_vec());
                }
            }
        }
        Ok(Vec::new())
    }

    fn write_data(&self, endpoint: [u8; 2], kind: [u8; 2], data: &[u8]) -> Result<()> {
        self.command([0x0D, 0x00], &endpoint, true)?;
        let first = data.len().min(self.packet - 10);
        let real = (data.len() + 2) as u16;
        let mut p = vec![real as u8, (real >> 8) as u8, 0, 0, kind[0], kind[1]];
        p.extend_from_slice(&data[..first]);
        self.command([0x06, 0x00], &p, true)?;
        let mut done = first;
        while done < data.len() {
            let n = (data.len() - done).min(self.packet - 4);
            self.command([0x07, 0x00], &data[done..done + n], true)?;
            done += n;
        }
        self.command([0x05, 0x01], &[], true).map(|_| ())
    }

    fn read_data(&self, endpoint: [u8; 2]) -> Result<Vec<u8>> {
        self.command([0x0D, 0x00], &endpoint, true)?;
        let r = self.command([0x08, 0x00], &[], true)?;
        self.command([0x05, 0x01], &[], true)?;
        Ok(r)
    }

    fn fan_mode(&self) -> Result<()> {
        let mut buf = [0u8; 15];
        buf[0] = 0x07;
        let (start, end) = if self.xt { (2, 14) } else { (3, 15) };
        if !self.xt {
            buf[1] = 0x01;
            buf[2] = 0x08;
        }
        let mut i = start;
        while i < end {
            buf[i] = 0x01;
            buf[i + 1] = 0x06;
            i += 2;
        }
        self.write_data([0x1E, 0x00], [0x0D, 0x00], &buf)?;
        self.command([0x01, 0x03], &[0x00, 0x02], true).map(|_| ())
    }
}

fn open(api: &HidApi, i: &HidInfo) -> Result<Vec<Box<dyn Device>>> {
    let dev = api.open_path(i.path())?;
    let pid = i.product_id();
    let packet = match pid {
        0x0C32 => 65,
        XT => 385,
        _ => 97,
    };
    let mut d = CommanderCore {
        info: DeviceInfo {
            id: hid::stable_id("corsair-cc", i),
            name: if pid == XT { "Corsair Commander Core XT".into() } else { "Corsair Commander Core".into() },
            vendor: "Corsair".into(),
            kind: DeviceKind::Cooler,
            location: hid::location(i),
            zones: Vec::new(),
            modes: vec![ModeInfo::new("Effet de l'appareil")],
            direct: true,
        },
        dev,
        packet,
        xt: pid == XT,
    };
    let v = d.command([0x02, 0x13], &[], true)?;
    if pid == 0x0C1C && v.first() == Some(&1) {
        d.packet = 1025;
    }
    d.fan_mode()?;
    let r = d.read_data([0x20, 0x00])?;
    let n = r.get(2).copied().unwrap_or(0) as usize;
    for k in 0..n.min(7) {
        let count = r.get(k * 4 + 5).copied().unwrap_or(0) as usize | (r.get(k * 4 + 6).copied().unwrap_or(0) as usize) << 8;
        if k == 0 && !d.xt {
            d.info.zones.push(Zone::linear("Pompe", count.max(1)));
        } else {
            let label = if d.xt { k + 1 } else { k };
            d.info.zones.push(Zone::resizable(format!("Ventilateur {label}"), count.min(34), 0, 34));
        }
    }
    if d.info.zones.is_empty() {
        d.info.zones.push(Zone::linear("Pompe", 29));
    }
    Ok(vec![Box::new(d)])
}

impl Device for CommanderCore {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, _mode: usize, _p: &HwParams) -> Result<()> {
        // retour à l'éclairage géré par l'appareil
        self.command([0x01, 0x03], &[0x00, 0x01], true).map(|_| ())
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        let mut data = Vec::with_capacity(1224);
        let mut off = 0;
        for (zi, z) in self.info.zones.iter().enumerate() {
            let n = z.leds.len();
            for c in &colors[off..off + n] {
                data.extend_from_slice(&[c.r, c.g, c.b]);
            }
            // chaque canal de ventilateur réserve 34 LED
            if zi != 0 || self.xt {
                data.extend(std::iter::repeat_n(0u8, 3 * 34usize.saturating_sub(n)));
            }
            off += n;
        }
        self.write_data([0x22, 0x00], [0x12, 0x00], &data)
    }

    fn resize_zone(&mut self, zone: usize, leds: usize) -> Result<()> {
        let z = &self.info.zones[zone];
        if z.resizable.is_none() {
            return crate::device::err("zone non redimensionnable");
        }
        let name = z.name.clone();
        self.info.zones[zone] = Zone::resizable(name, leds.min(34), 0, 34);
        Ok(())
    }

    fn keepalive(&self) -> Option<Duration> {
        Some(Duration::from_secs(9))
    }

    fn max_fps(&self) -> u32 {
        30
    }

    fn release(&mut self) {
        let _ = self.command([0x01, 0x03], &[0x00, 0x01], true);
    }
}
