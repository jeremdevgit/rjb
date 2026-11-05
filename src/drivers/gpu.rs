// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Cartes graphiques NVIDIA (I2C via NVAPI) : détection par identifiants PCI
//! (table générée depuis OpenRGB) et pilotes par famille de contrôleur.

use super::gpu_table::GPUS;
use super::nvapi;
use super::smbus::I2c;
use crate::device::{Device, Result};
use crate::model::*;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GpuDriver {
    Ene,
    Fusion,
    Fusion2,
    Msi,
    MsiV2,
    AsusAura,
    EvgaAmpere,
    EvgaTuring,
    PalitV2,
    Colorful,
    ZotacV2,
    Galax,
    PnyLovelace,
}

pub fn detect(out: &mut Vec<Box<dyn Device>>) {
    for g in nvapi::gpus() {
        let (ven, dev, sv, sd) = g.pci;
        crate::log(&format!("GPU {} : {} PCI {ven:04x}:{dev:04x} {sv:04x}:{sd:04x}", g.index, g.name));
        let bus: Arc<dyn I2c> = g.bus.clone();
        for &(name, drv, v, d, s, sdv, addr) in GPUS {
            if (v, d, s, sdv) != (ven, dev, sv, sd) {
                continue;
            }
            let before = out.len();
            match drv {
                GpuDriver::Ene => super::ene::detect_gpu(&bus, name, out),
                GpuDriver::Fusion => {
                    if fusion_test(bus.as_ref(), addr) {
                        out.push(Box::new(FusionGpu::new(bus.clone(), addr, name)));
                    }
                }
                GpuDriver::Fusion2 => {
                    if fusion2_test(bus.as_ref(), addr) {
                        out.push(Box::new(Fusion2Gpu::new(bus.clone(), addr, name)));
                    }
                }
                GpuDriver::AsusAura => {
                    let hi = bus.read_byte_data(addr, 0x20).unwrap_or(0) as u16;
                    let lo = bus.read_byte_data(addr, 0x21).unwrap_or(0) as u16;
                    if (hi << 8) | lo == 0x1589 {
                        out.push(Box::new(AuraGpu::new(bus.clone(), addr, name)));
                    }
                }
                GpuDriver::Msi => out.push(Box::new(MsiGpu::new(bus.clone(), addr, name))),
                GpuDriver::EvgaAmpere => {
                    if let Some(e) = EvgaV3::new(bus.clone(), addr, name) {
                        out.push(Box::new(e));
                    }
                }
                other => crate::log(&format!("GPU {name} : contrôleur {other:?} pas encore pris en charge")),
            }
            if out.len() > before {
                crate::log(&format!("GPU reconnue : {name} ({drv:?} @ {addr:#04x})"));
                break;
            }
        }
    }
}

fn gpu_info(bus: &Arc<dyn I2c>, addr: u8, name: &str, vendor: &str, zones: Vec<Zone>, modes: Vec<ModeInfo>) -> DeviceInfo {
    DeviceInfo {
        id: format!("gpu:{}:{addr:02x}", bus.name()),
        name: name.to_string(),
        vendor: vendor.into(),
        kind: DeviceKind::Gpu,
        location: bus.location(addr),
        zones,
        modes,
        direct: true,
    }
}

// ── Gigabyte RGB Fusion (GTX 10 / RTX 20 / RTX 30 Eagle, Gaming OC…) ──────────

fn fusion_test(bus: &dyn I2c, addr: u8) -> bool {
    if bus.write_byte(addr, 0xAB).is_err() {
        return false;
    }
    let pad = if addr == 0x62 { 7 } else { 3 };
    for _ in 0..pad {
        let _ = bus.write_byte(addr, 0);
    }
    let a = bus.read_byte(addr).ok();
    let b = bus.read_byte(addr).ok();
    let _ = bus.read_byte(addr);
    let _ = bus.read_byte(addr);
    addr == 0x48 || (a == Some(0xAB) && matches!(b, Some(0x10 | 0x11 | 0x12 | 0x14)))
}

pub struct FusionGpu {
    info: DeviceInfo,
    bus: Arc<dyn I2c>,
    addr: u8,
    direct: bool,
}

impl FusionGpu {
    fn new(bus: Arc<dyn I2c>, addr: u8, name: &str) -> Self {
        let modes = vec![
            ModeInfo::new("Statique").colors(1, 1).brightness(),
            ModeInfo::new("Respiration").colors(1, 1).brightness().speed(),
            ModeInfo::new("Flash").colors(1, 1).brightness().speed(),
            ModeInfo::new("Double flash").colors(1, 1).brightness().speed(),
            ModeInfo::new("Cycle de couleurs").brightness().speed(),
            ModeInfo::new("Arc-en-ciel").brightness().speed(),
            ModeInfo::new("Éteint").off(),
        ];
        FusionGpu {
            info: gpu_info(&bus, addr, name, "Gigabyte", vec![Zone::single("Carte graphique")], modes),
            bus,
            addr,
            direct: false,
        }
    }

    /// Envoie un registre suivi de ses octets (4 octets ou 8 pour 0x62).
    fn send(&self, bytes: &[u8]) -> Result<()> {
        let n = if self.addr == 0x62 { 8 } else { 4 };
        for i in 0..n {
            self.bus.write_byte(self.addr, bytes.get(i).copied().unwrap_or(0))?;
        }
        Ok(())
    }
}

impl Device for FusionGpu {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, mode: usize, p: &HwParams) -> Result<()> {
        self.direct = false;
        let m = &self.info.modes[mode];
        let code = [0x01, 0x02, 0x04, 0x08, 0x10, 0x11, 0x01][mode.min(6)];
        let c = if m.is_off { Color::BLACK } else { p.color(0) };
        let bright = if m.is_off { 0 } else { p.brightness_in(0, 0x63) as u8 };
        self.send(&[0x88, code, p.speed_in(0, 9) as u8, bright])?;
        self.send(&[0x40, c.r, c.g, c.b])?;
        self.send(&[0xAA]) // enregistrer
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        if !self.direct {
            self.send(&[0x88, 0x01, 0x05, 0x63])?;
            self.direct = true;
        }
        let c = colors[0];
        self.send(&[0x40, c.r, c.g, c.b])
    }

    fn max_fps(&self) -> u32 {
        25
    }
}

// ── Gigabyte RGB Fusion 2 (AORUS / Gaming OC récentes) ───────────────────────

fn fusion2_test(bus: &dyn I2c, addr: u8) -> bool {
    if bus.i2c_write(addr, &[0xAB, 0, 0, 0, 0, 0, 0, 0]).is_err() {
        return false;
    }
    match bus.i2c_read(addr, 4) {
        Ok(r) if r.len() == 4 => r[0] == 0xAB && !(r[1] == 0xAB && r[2] == 0xAB && r[3] == 0xAB),
        _ => false,
    }
}

pub struct Fusion2Gpu {
    info: DeviceInfo,
    bus: Arc<dyn I2c>,
    addr: u8,
}

impl Fusion2Gpu {
    fn new(bus: Arc<dyn I2c>, addr: u8, name: &str) -> Self {
        let modes = vec![
            ModeInfo::new("Statique").colors(1, 1).brightness(),
            ModeInfo::new("Respiration").colors(1, 1).speed(),
            ModeInfo::new("Flash").colors(1, 1).brightness().speed(),
            ModeInfo::new("Double flash").colors(1, 1).brightness().speed(),
            ModeInfo::new("Cycle de couleurs").brightness().speed(),
            ModeInfo::new("Dégradé").colors(1, 1).brightness().speed(),
            ModeInfo::new("Vague").colors(1, 1).brightness().speed(),
            ModeInfo::new("Éteint").off(),
        ];
        Fusion2Gpu {
            info: gpu_info(&bus, addr, name, "Gigabyte", vec![Zone::single("Carte graphique")], modes),
            bus,
            addr,
        }
    }

    /// Applique un effet à toutes les zones de la carte (jusqu'à 4).
    fn zones(&self, mode: u8, speed: u8, bright: u8, flag: u8, c: Color) -> Result<()> {
        for zone in 0..4u8 {
            self.bus.i2c_write(self.addr, &[0x88, mode, speed, bright, flag, zone + 1, 0, 0])?;
            let p2 = match zone {
                0 | 1 => [0xB0, mode, c.r, c.g, c.b, c.r, c.g, c.b],
                2 => [0xB1, mode, c.r, c.g, c.b, 0, 0, 0],
                _ => [0x40, c.r, c.g, c.b, zone + 1, 0, 0, 0],
            };
            if mode != 0x03 {
                self.bus.i2c_write(self.addr, &p2)?;
            }
        }
        Ok(())
    }
}

impl Device for Fusion2Gpu {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, mode: usize, p: &HwParams) -> Result<()> {
        let m = &self.info.modes[mode];
        let (code, flag) = match m.name.as_str() {
            "Respiration" => (0x02, 0),
            "Flash" => (0x04, 0),
            "Double flash" => (0x08, 0),
            "Cycle de couleurs" => (0x03, 0),
            "Dégradé" => (0x05, 0x08),
            "Vague" => (0x07, 0),
            _ => (0x01, 0),
        };
        let c = if m.is_off { Color::BLACK } else { p.color(0) };
        let bright = if m.is_off { 0 } else if code == 0x02 { 0x63 } else { p.brightness_in(0, 0x63) as u8 };
        self.zones(code, p.speed_in(0, 5) as u8, bright, flag, c)?;
        self.bus.i2c_write(self.addr, &[0xAA, 0, 0, 0, 0, 0, 0, 0])
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        self.zones(0x01, 0, 0x63, 0, colors[0])
    }

    fn max_fps(&self) -> u32 {
        20
    }
}

// ── ASUS Aura GPU (GTX 10 / RX 400-500 / Vega) ──────────────────────────────

pub struct AuraGpu {
    info: DeviceInfo,
    bus: Arc<dyn I2c>,
    addr: u8,
    direct: bool,
}

impl AuraGpu {
    fn new(bus: Arc<dyn I2c>, addr: u8, name: &str) -> Self {
        let modes = vec![
            ModeInfo::new("Statique").colors(1, 1),
            ModeInfo::new("Respiration").colors(1, 1),
            ModeInfo::new("Flash").colors(1, 1),
            ModeInfo::new("Cycle de couleurs"),
            ModeInfo::new("Éteint").off(),
        ];
        AuraGpu {
            info: gpu_info(&bus, addr, name, "ASUS", vec![Zone::single("Carte graphique")], modes),
            bus,
            addr,
            direct: false,
        }
    }

    fn color(&self, c: Color) -> Result<()> {
        self.bus.write_byte_data(self.addr, 0x04, c.r)?;
        self.bus.write_byte_data(self.addr, 0x05, c.g)?;
        self.bus.write_byte_data(self.addr, 0x06, c.b)
    }
}

impl Device for AuraGpu {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, mode: usize, p: &HwParams) -> Result<()> {
        self.direct = false;
        let m = &self.info.modes[mode];
        let (code, c) = if m.is_off { (1, Color::BLACK) } else { ([1, 2, 3, 4][mode.min(3)], p.color(0)) };
        self.bus.write_byte_data(self.addr, 0x07, code)?;
        self.color(c)?;
        self.bus.write_byte_data(self.addr, 0x0E, 0x01)
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        if !self.direct {
            self.bus.write_byte_data(self.addr, 0x07, 1)?;
            self.direct = true;
        }
        self.color(colors[0])
    }

    fn max_fps(&self) -> u32 {
        25
    }
}

// ── MSI (Gaming X / Suprim, génération 1) ───────────────────────────────────

const MSI_MODES: &[(&str, u8, bool)] = &[
    ("Statique", 0x13, true),
    ("Arc-en-ciel", 0x08, false),
    ("Goutte de pluie", 0x1A, true),
    ("Magie", 0x07, false),
    ("Patrouille", 0x05, true),
    ("Flux", 0x06, false),
    ("Éclair", 0x15, false),
    ("Vague", 0x1F, true),
    ("Météore", 0x16, true),
    ("Empilement", 0x0D, true),
    ("Rythme", 0x0B, true),
    ("Écoulement", 0x09, true),
    ("Tourbillon", 0x0F, true),
    ("Torsion", 0x11, true),
    ("Laminage", 0x1D, true),
    ("Respiration", 0x04, true),
    ("Flash", 0x02, true),
    ("Double flash", 0x03, true),
];

pub struct MsiGpu {
    info: DeviceInfo,
    bus: Arc<dyn I2c>,
    addr: u8,
    direct: bool,
}

impl MsiGpu {
    fn new(bus: Arc<dyn I2c>, addr: u8, name: &str) -> Self {
        let mut modes: Vec<ModeInfo> = MSI_MODES
            .iter()
            .map(|m| {
                let mut i = ModeInfo::new(m.0).brightness();
                if m.1 != 0x13 {
                    i = i.speed();
                }
                if m.2 {
                    i = i.colors(1, 1);
                }
                i
            })
            .collect();
        modes.push(ModeInfo::new("Éteint").off());
        MsiGpu {
            info: gpu_info(&bus, addr, name, "MSI", vec![Zone::single("Carte graphique")], modes),
            bus,
            addr,
            direct: false,
        }
    }

    fn w(&self, r: u8, v: u8) -> Result<()> {
        self.bus.write_byte_data(self.addr, r, v)
    }

    fn rgb1(&self, c: Color) -> Result<()> {
        self.w(0x26, 0)?;
        self.w(0x30, c.r)?;
        self.w(0x31, c.g)?;
        self.w(0x32, c.b)
    }
}

impl Device for MsiGpu {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, mode: usize, p: &HwParams) -> Result<()> {
        self.direct = false;
        if self.info.modes[mode].is_off {
            self.w(0x22, 0x01)?;
            return self.w(0x3F, 0x01);
        }
        let (_, code, colored) = MSI_MODES[mode];
        self.w(0x36, p.brightness_in(0, 5) as u8 * 20)?;
        self.w(0x38, [0x04, 0x02, 0x01][p.speed_in(0, 2) as usize])?;
        self.w(0x22, code)?;
        if colored {
            self.rgb1(p.color(0))?;
        }
        self.w(0x3F, 0x01)
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        if !self.direct {
            self.w(0x36, 100)?;
            self.w(0x22, 0x13)?;
            self.direct = true;
        }
        self.rgb1(colors[0])
    }

    fn max_fps(&self) -> u32 {
        25
    }
}

// ── EVGA RTX 30 (iCX3, « v3 ») ──────────────────────────────────────────────

pub struct EvgaV3 {
    info: DeviceInfo,
    bus: Arc<dyn I2c>,
    addr: u8,
    zone_ids: Vec<u8>,
    direct: bool,
}

impl EvgaV3 {
    fn new(bus: Arc<dyn I2c>, addr: u8, name: &str) -> Option<Self> {
        let d = bus.read_i2c_block(addr, 0xC0, 10).ok()?;
        if d.len() < 10 {
            return None;
        }
        let mut zones = Vec::new();
        let mut zone_ids = Vec::new();
        for z in 0..4u8 {
            if d[z as usize + 5] > 0 {
                zones.push(Zone::single(format!("Zone {}", z + 1)));
                zone_ids.push(z);
            }
        }
        if zones.is_empty() {
            return None;
        }
        let modes = vec![
            ModeInfo::new("Statique").colors(1, 1).brightness(),
            ModeInfo::new("Respiration").colors(1, 2).brightness().speed(),
            ModeInfo::new("Arc-en-ciel").brightness().speed(),
            ModeInfo::new("Vague arc-en-ciel").brightness().speed(),
            ModeInfo::new("Étoiles").brightness().speed(),
            ModeInfo::new("Éteint").off(),
        ];
        Some(EvgaV3 {
            info: gpu_info(&bus, addr, name, "EVGA", zones, modes),
            bus,
            addr,
            zone_ids,
            direct: false,
        })
    }

    fn init(&self) -> Result<()> {
        self.bus.write_i2c_block(self.addr, 0xB2, &[0x04, 0xC6, 0xEB, 0xEA, 0x15])
    }

    fn all_modes(&self, mode: u8) -> Result<()> {
        self.init()?;
        let mut p = [0x09, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00];
        for &z in &self.zone_ids {
            p[z as usize + 1] = mode;
        }
        self.bus.write_i2c_block(self.addr, 0xC0, &p)
    }

    fn static_zone(&self, z: u8, bright: u8, c: Color) -> Result<()> {
        self.bus.write_i2c_block(self.addr, 0xC1 + z, &[0x04, bright, c.r, c.g, c.b])
    }
}

impl Device for EvgaV3 {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn set_mode(&mut self, mode: usize, p: &HwParams) -> Result<()> {
        self.direct = false;
        let m = self.info.modes[mode].clone();
        let bright = p.brightness_in(0, 255) as u8;
        let code = match m.name.as_str() {
            "Statique" => 1,
            "Respiration" => 2,
            "Arc-en-ciel" => 3,
            "Vague arc-en-ciel" => 5,
            "Étoiles" => 7,
            _ => 0,
        };
        for &z in &self.zone_ids {
            match code {
                1 => self.static_zone(z, bright, p.color(0))?,
                2 => {
                    let (a, b) = (p.color(0), p.color(1));
                    let s = p.speed_in(20000, 1000) as u16;
                    let pkt = [0x09, bright, a.r, a.g, a.b, b.r, b.g, b.b, s as u8, (s >> 8) as u8];
                    self.bus.write_i2c_block(self.addr, 0xC5 + z, &pkt)?;
                }
                3 | 5 | 7 => {
                    let s = if code == 7 { p.speed_in(10000, 500) } else { p.speed_in(20000, 1000) } as u16;
                    let reg = 0xC1 + (code - 1) * 4 + z;
                    self.bus.write_i2c_block(self.addr, reg, &[0x03, bright, s as u8, (s >> 8) as u8])?;
                }
                _ => {}
            }
        }
        self.all_modes(code)?;
        self.bus.write_i2c_block(self.addr, 0x90, &[0x04, 0x9E, 0xEB, 0x00, 0x90])
    }

    fn set_leds(&mut self, colors: &[Color]) -> Result<()> {
        if !self.direct {
            self.all_modes(1)?;
            self.direct = true;
        }
        for (i, &z) in self.zone_ids.iter().enumerate() {
            self.static_zone(z, 255, colors[i])?;
        }
        Ok(())
    }

    fn max_fps(&self) -> u32 {
        20
    }
}
