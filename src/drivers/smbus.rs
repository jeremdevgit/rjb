// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Bus SMBus (RAM, contrôleurs de carte mère) et puces Super I/O via PawnIO.

use super::pawnio::{blobs, Module};
use crate::device::{err, Result};
use std::sync::Arc;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject};

const READ: u64 = 1;
const WRITE: u64 = 0;
const QUICK: u64 = 0;
const BYTE: u64 = 1;
const BYTE_DATA: u64 = 2;
const WORD_DATA: u64 = 3;
const BLOCK_DATA: u64 = 5;

/// Verrou partagé avec les autres logiciels (HWiNFO, OpenRGB, outils constructeurs…).
struct GlobalLock(usize);

impl GlobalLock {
    fn new(name: &str) -> GlobalLock {
        let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        GlobalLock(unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) } as usize)
    }
    fn hold(&self) -> Option<LockGuard<'_>> {
        if self.0 == 0 {
            return None;
        }
        let r = unsafe { WaitForSingleObject(self.0 as HANDLE, 2000) };
        // 0 = acquis, 0x80 = abandonné par un autre processus (acquis aussi)
        (r == 0 || r == 0x80).then_some(LockGuard(self))
    }
}

struct LockGuard<'a>(&'a GlobalLock);
impl Drop for LockGuard<'_> {
    fn drop(&mut self) {
        unsafe { ReleaseMutex(self.0 .0 as HANDLE) };
    }
}

impl Drop for GlobalLock {
    fn drop(&mut self) {
        if self.0 != 0 {
            unsafe { CloseHandle(self.0 as HANDLE) };
        }
    }
}

/// Opérations I2C/SMBus communes aux bus de la carte mère (PawnIO) et des
/// cartes graphiques (NVAPI) : les mêmes pilotes servent sur les deux.
pub trait I2c: Send + Sync {
    fn read_byte(&self, addr: u8) -> Result<u8>;
    fn write_byte(&self, addr: u8, v: u8) -> Result<()>;
    fn read_byte_data(&self, addr: u8, cmd: u8) -> Result<u8>;
    fn write_byte_data(&self, addr: u8, cmd: u8, v: u8) -> Result<()>;
    #[allow(dead_code)]
    fn read_word_data(&self, addr: u8, cmd: u8) -> Result<u16>;
    fn write_word_data(&self, addr: u8, cmd: u8, v: u16) -> Result<()>;
    fn write_block_data(&self, addr: u8, cmd: u8, bytes: &[u8]) -> Result<()>;
    /// Écriture I2C brute (sans registre).
    fn i2c_write(&self, _addr: u8, _bytes: &[u8]) -> Result<()> {
        err("écriture I2C brute non prise en charge")
    }
    /// Lecture I2C brute.
    fn i2c_read(&self, _addr: u8, _len: usize) -> Result<Vec<u8>> {
        err("lecture I2C brute non prise en charge")
    }
    /// Écriture d'un bloc I2C à partir d'un registre.
    fn write_i2c_block(&self, _addr: u8, _reg: u8, _bytes: &[u8]) -> Result<()> {
        err("bloc I2C non pris en charge")
    }
    /// Lecture d'un bloc I2C à partir d'un registre.
    fn read_i2c_block(&self, _addr: u8, _reg: u8, _len: usize) -> Result<Vec<u8>> {
        err("bloc I2C non pris en charge")
    }
    fn location(&self, addr: u8) -> String;
    fn name(&self) -> String;
}

impl I2c for Bus {
    fn read_byte(&self, addr: u8) -> Result<u8> {
        Bus::read_byte(self, addr)
    }
    fn write_byte(&self, addr: u8, v: u8) -> Result<()> {
        Bus::write_byte(self, addr, v)
    }
    fn read_byte_data(&self, addr: u8, cmd: u8) -> Result<u8> {
        Bus::read_byte_data(self, addr, cmd)
    }
    fn write_byte_data(&self, addr: u8, cmd: u8, v: u8) -> Result<()> {
        Bus::write_byte_data(self, addr, cmd, v)
    }
    fn read_word_data(&self, addr: u8, cmd: u8) -> Result<u16> {
        Bus::read_word_data(self, addr, cmd)
    }
    fn write_word_data(&self, addr: u8, cmd: u8, v: u16) -> Result<()> {
        Bus::write_word_data(self, addr, cmd, v)
    }
    fn write_block_data(&self, addr: u8, cmd: u8, bytes: &[u8]) -> Result<()> {
        Bus::write_block_data(self, addr, cmd, bytes)
    }
    fn location(&self, addr: u8) -> String {
        Bus::location(self, addr)
    }
    fn name(&self) -> String {
        self.name.clone()
    }
}

pub struct Bus {
    pub name: String,
    /// (vendeur PCI, périphérique PCI, sous-vendeur, sous-périphérique)
    pub pci: (u16, u16, u16, u16),
    module: Module,
    lock: GlobalLock,
}

impl Bus {
    fn open(blob: &[u8], name: &str, port: Option<u64>) -> Result<Bus> {
        let module = Module::load(blob)?;
        let lock = GlobalLock::new("Global\\Access_SMBUS.HTP.Method");
        if let Some(p) = port {
            let _g = lock.hold();
            module.exec("ioctl_piix4_port_sel", &[p], 1)?;
        }
        let _ = module.exec("ioctl_set_sleep_mode", &[2], 0);
        let id = module.exec("ioctl_identity", &[], 3).unwrap_or(vec![0; 3]);
        let p = id[2];
        Ok(Bus {
            name: name.to_string(),
            pci: (p as u16, (p >> 16) as u16, (p >> 32) as u16, (p >> 48) as u16),
            module,
            lock,
        })
    }

    /// Bus sur lequel se trouvent les barrettes de RAM.
    pub fn is_dram(&self) -> bool {
        self.name == "i801" || self.name == "piix4_0"
    }

    fn xfer(&self, addr: u8, rw: u64, cmd: u8, size: u64, data: &mut [u8; 40]) -> Result<()> {
        let mut input = [0u64; 9];
        input[0] = addr as u64;
        input[1] = rw;
        input[2] = cmd as u64;
        input[3] = size;
        for i in 0..5 {
            input[4 + i] = u64::from_le_bytes(data[i * 8..i * 8 + 8].try_into().unwrap());
        }
        let _g = self.lock.hold();
        let out = self.module.exec("ioctl_smbus_xfer", &input, 5)?;
        for i in 0..5 {
            data[i * 8..i * 8 + 8].copy_from_slice(&out[i].to_le_bytes());
        }
        Ok(())
    }

    pub fn quick(&self, addr: u8) -> Result<()> {
        self.xfer(addr, WRITE, 0, QUICK, &mut [0; 40])
    }

    pub fn read_byte(&self, addr: u8) -> Result<u8> {
        let mut d = [0u8; 40];
        self.xfer(addr, READ, 0, BYTE, &mut d)?;
        Ok(d[0])
    }

    pub fn write_byte(&self, addr: u8, v: u8) -> Result<()> {
        self.xfer(addr, WRITE, v, BYTE, &mut [0; 40])
    }

    pub fn read_byte_data(&self, addr: u8, cmd: u8) -> Result<u8> {
        let mut d = [0u8; 40];
        self.xfer(addr, READ, cmd, BYTE_DATA, &mut d)?;
        Ok(d[0])
    }

    pub fn write_byte_data(&self, addr: u8, cmd: u8, v: u8) -> Result<()> {
        let mut d = [0u8; 40];
        d[0] = v;
        self.xfer(addr, WRITE, cmd, BYTE_DATA, &mut d)
    }

    pub fn read_word_data(&self, addr: u8, cmd: u8) -> Result<u16> {
        let mut d = [0u8; 40];
        self.xfer(addr, READ, cmd, WORD_DATA, &mut d)?;
        Ok(u16::from_le_bytes([d[0], d[1]]))
    }

    pub fn write_word_data(&self, addr: u8, cmd: u8, v: u16) -> Result<()> {
        let mut d = [0u8; 40];
        d[..2].copy_from_slice(&v.to_le_bytes());
        self.xfer(addr, WRITE, cmd, WORD_DATA, &mut d)
    }

    pub fn write_block_data(&self, addr: u8, cmd: u8, bytes: &[u8]) -> Result<()> {
        if bytes.len() > 32 {
            return err("bloc SMBus trop long");
        }
        let mut d = [0u8; 40];
        d[0] = bytes.len() as u8;
        d[1..1 + bytes.len()].copy_from_slice(bytes);
        self.xfer(addr, WRITE, cmd, BLOCK_DATA, &mut d)
    }

    pub fn read_block_data(&self, addr: u8, cmd: u8) -> Result<Vec<u8>> {
        let mut d = [0u8; 40];
        self.xfer(addr, READ, cmd, BLOCK_DATA, &mut d)?;
        let n = (d[0] as usize).min(32);
        Ok(d[1..1 + n].to_vec())
    }

    pub fn location(&self, addr: u8) -> String {
        format!("smbus:{}:{addr:02x}", self.name)
    }
}

/// Ouvre tous les bus SMBus disponibles sur cette machine.
pub fn open_buses() -> (Vec<Arc<Bus>>, Vec<String>) {
    let mut buses = Vec::new();
    let mut errors = Vec::new();
    let tries: [(&[u8], &str, Option<u64>); 4] = [
        (blobs::SMBUS_I801, "i801", None),
        (blobs::SMBUS_PIIX4, "piix4_0", Some(0)),
        (blobs::SMBUS_PIIX4, "piix4_1", Some(1)),
        (blobs::SMBUS_NCT6793, "nct6793", None),
    ];
    for (blob, name, port) in tries {
        match Bus::open(blob, name, port) {
            Ok(b) => {
                crate::log(&format!("SMBus {name} ouvert, PCI {:04x?}", b.pci));
                buses.push(Arc::new(b));
            }
            Err(e) => {
                crate::log(&format!("SMBus {name} : {e}"));
                if e.0.contains("administrateur") && !errors.contains(&e.0) {
                    errors.push(e.0);
                }
            }
        }
    }
    (buses, errors)
}

// ── Super I/O ────────────────────────────────────────────────────────────

pub const SIO_REG_LOGDEV: u8 = 0x07;
pub const SIO_REG_DEVID: u8 = 0x20;

/// Accès aux registres de configuration d'une puce Super I/O (Nuvoton, ITE).
pub struct SuperIo {
    module: Module,
    lock: GlobalLock,
    pub addr: u16,
}

impl SuperIo {
    pub fn open(addr: u16) -> Result<SuperIo> {
        let module = Module::load(blobs::LPC_IO)?;
        let slot = match addr {
            0x2E => 0,
            0x4E => 1,
            _ => return err("adresse Super I/O invalide"),
        };
        module.exec("ioctl_select_slot", &[slot], 0)?;
        let s = SuperIo {
            module,
            lock: GlobalLock::new("Global\\Access_ISABUS.HTP.Method"),
            addr,
        };
        // séquence d'entrée en mode configuration (comme OpenRGB)
        s.inb(0x87);
        s.inb(0x87);
        Ok(s)
    }

    pub fn inb(&self, reg: u8) -> u8 {
        let _g = self.lock.hold();
        self.module
            .exec("ioctl_superio_inb", &[reg as u64], 1)
            .map(|o| o[0] as u8)
            .unwrap_or(0xFF)
    }

    pub fn outb(&self, reg: u8, val: u8) {
        let _g = self.lock.hold();
        let _ = self.module.exec("ioctl_superio_outb", &[reg as u64, val as u64], 0);
    }

    pub fn chip_id(&self) -> u16 {
        ((self.inb(SIO_REG_DEVID) as u16) << 8) | self.inb(SIO_REG_DEVID + 1) as u16
    }
}

/// Fabricant et modèle de la carte mère (SMBIOS, via le registre).
pub fn board() -> (String, String) {
    use windows_sys::Win32::System::Registry::*;
    let get = |name: &str| -> String {
        let key: Vec<u16> = "HARDWARE\\DESCRIPTION\\System\\BIOS\0".encode_utf16().collect();
        let val: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let mut buf = [0u16; 256];
        let mut len = (buf.len() * 2) as u32;
        let r = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                val.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buf.as_mut_ptr() as _,
                &mut len,
            )
        };
        if r != 0 {
            return String::new();
        }
        let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..n]).trim().to_string()
    };
    (get("BaseBoardManufacturer"), get("BaseBoardProduct"))
}

/// CRC-8 (polynôme 0x07), utilisé par plusieurs contrôleurs de RAM.
pub fn crc8(data: &[u8]) -> u8 {
    let mut crc = 0u8;
    for &b in data {
        crc ^= b;
        for _ in 0..8 {
            crc = if crc & 0x80 != 0 { (crc << 1) ^ 0x07 } else { crc << 1 };
        }
    }
    crc
}
