// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Bus I2C des cartes graphiques NVIDIA via NVAPI (pilote NVIDIA, sans PawnIO).

use super::smbus::I2c;
use crate::device::{err, Result};
use std::ffi::c_void;
use std::sync::{Arc, Mutex, OnceLock};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryExA, LOAD_LIBRARY_SEARCH_SYSTEM32};

type Handle = *mut c_void;

#[repr(C)]
struct I2cInfoV3 {
    version: u32,
    display_mask: u32,
    is_ddc_port: u8,
    dev_address: u8,
    reg_address: *const u8,
    reg_addr_size: u32,
    data: *mut u8,
    size: u32,
    speed: u32,
    speed_khz: u32,
    port_id: u8,
    is_port_id_set: u32,
}

type FnInit = unsafe extern "C" fn() -> i32;
type FnEnum = unsafe extern "C" fn(*mut Handle, *mut i32) -> i32;
type FnPci = unsafe extern "C" fn(Handle, *mut u32, *mut u32, *mut u32, *mut u32) -> i32;
type FnName = unsafe extern "C" fn(Handle, *mut u8) -> i32;
type FnI2c = unsafe extern "C" fn(Handle, *mut I2cInfoV3, *mut u32) -> i32;

struct Api {
    enum_gpus: FnEnum,
    pci: FnPci,
    name: FnName,
    write: FnI2c,
    read: FnI2c,
}

static API: OnceLock<Option<Api>> = OnceLock::new();

fn api() -> Option<&'static Api> {
    API.get_or_init(|| unsafe {
        let h = LoadLibraryExA(b"nvapi64.dll\0".as_ptr(), std::ptr::null_mut(), LOAD_LIBRARY_SEARCH_SYSTEM32);
        if h.is_null() {
            return None;
        }
        let qi = GetProcAddress(h, b"nvapi_QueryInterface\0".as_ptr())?;
        let qi: unsafe extern "C" fn(u32) -> *mut c_void = std::mem::transmute(qi);
        let get = |id: u32| {
            let p = qi(id);
            (!p.is_null()).then_some(p)
        };
        let init: FnInit = std::mem::transmute(get(0x0150_E828)?);
        if init() != 0 {
            return None;
        }
        Some(Api {
            enum_gpus: std::mem::transmute(get(0xE5AC_921F)?),
            pci: std::mem::transmute(get(0x2DDF_B66E)?),
            name: std::mem::transmute(get(0xCEEE_8E9F)?),
            write: std::mem::transmute(get(0x283A_C65A)?),
            read: std::mem::transmute(get(0x4D7B_0709)?),
        })
    })
    .as_ref()
}

pub struct Gpu {
    pub index: usize,
    pub name: String,
    /// (vendeur, périphérique, sous-vendeur, sous-périphérique)
    pub pci: (u16, u16, u16, u16),
    pub bus: Arc<NvBus>,
}

pub fn gpus() -> Vec<Gpu> {
    let Some(a) = api() else { return Vec::new() };
    let mut handles = [std::ptr::null_mut(); 64];
    let mut n = 0i32;
    unsafe {
        if (a.enum_gpus)(handles.as_mut_ptr(), &mut n) != 0 {
            return Vec::new();
        }
    }
    (0..n.max(0) as usize)
        .map(|i| {
            let h = handles[i];
            let (mut dev, mut sub, mut rev, mut ext) = (0u32, 0u32, 0u32, 0u32);
            let mut name = [0u8; 64];
            unsafe {
                (a.pci)(h, &mut dev, &mut sub, &mut rev, &mut ext);
                (a.name)(h, name.as_mut_ptr());
            }
            let len = name.iter().position(|&b| b == 0).unwrap_or(64);
            Gpu {
                index: i,
                name: String::from_utf8_lossy(&name[..len]).trim().to_string(),
                pci: (dev as u16, (dev >> 16) as u16, sub as u16, (sub >> 16) as u16),
                bus: Arc::new(NvBus {
                    handle: h as usize,
                    index: i,
                    lock: Mutex::new(()),
                }),
            }
        })
        .collect()
}

pub struct NvBus {
    handle: usize,
    index: usize,
    lock: Mutex<()>,
}

impl NvBus {
    /// Transfert brut. `reg` : registre éventuel ; `data` lu ou écrit.
    fn xfer(&self, addr: u8, reg: Option<u8>, data: &mut [u8], write: bool) -> Result<usize> {
        let Some(a) = api() else { return err("NVAPI indisponible") };
        let r = reg.unwrap_or(0);
        let mut info = I2cInfoV3 {
            version: (3 << 16) | std::mem::size_of::<I2cInfoV3>() as u32,
            display_mask: 0,
            is_ddc_port: 0,
            dev_address: addr << 1,
            reg_address: if reg.is_some() { &r } else { std::ptr::null() },
            reg_addr_size: reg.is_some() as u32,
            data: data.as_mut_ptr(),
            size: data.len() as u32,
            speed: 0xFFFF,
            speed_khz: 0,
            port_id: 1,
            is_port_id_set: 1,
        };
        let mut unknown = 0u32;
        let _g = self.lock.lock().unwrap();
        let s = unsafe {
            if write {
                (a.write)(self.handle as Handle, &mut info, &mut unknown)
            } else {
                (a.read)(self.handle as Handle, &mut info, &mut unknown)
            }
        };
        if s != 0 {
            return err(format!("I2C NVIDIA {addr:#04x} : {s}"));
        }
        Ok(info.size as usize)
    }
}

impl I2c for NvBus {
    fn read_byte(&self, addr: u8) -> Result<u8> {
        let mut d = [0u8; 1];
        self.xfer(addr, None, &mut d, false)?;
        Ok(d[0])
    }
    fn write_byte(&self, addr: u8, v: u8) -> Result<()> {
        self.xfer(addr, None, &mut [v], true).map(|_| ())
    }
    fn read_byte_data(&self, addr: u8, cmd: u8) -> Result<u8> {
        let mut d = [0u8; 1];
        self.xfer(addr, Some(cmd), &mut d, false)?;
        Ok(d[0])
    }
    fn write_byte_data(&self, addr: u8, cmd: u8, v: u8) -> Result<()> {
        self.xfer(addr, Some(cmd), &mut [v], true).map(|_| ())
    }
    fn read_word_data(&self, addr: u8, cmd: u8) -> Result<u16> {
        let mut d = [0u8; 2];
        self.xfer(addr, Some(cmd), &mut d, false)?;
        Ok(u16::from_le_bytes(d))
    }
    fn write_word_data(&self, addr: u8, cmd: u8, v: u16) -> Result<()> {
        self.xfer(addr, Some(cmd), &mut v.to_le_bytes(), true).map(|_| ())
    }
    fn write_block_data(&self, addr: u8, cmd: u8, bytes: &[u8]) -> Result<()> {
        let mut d = Vec::with_capacity(bytes.len() + 1);
        d.push(bytes.len() as u8);
        d.extend_from_slice(bytes);
        self.xfer(addr, Some(cmd), &mut d, true).map(|_| ())
    }
    fn i2c_write(&self, addr: u8, bytes: &[u8]) -> Result<()> {
        let mut d = bytes.to_vec();
        self.xfer(addr, None, &mut d, true).map(|_| ())
    }
    fn i2c_read(&self, addr: u8, len: usize) -> Result<Vec<u8>> {
        let mut d = vec![0u8; len];
        let n = self.xfer(addr, None, &mut d, false)?;
        d.truncate(n.min(len));
        Ok(d)
    }
    fn write_i2c_block(&self, addr: u8, reg: u8, bytes: &[u8]) -> Result<()> {
        let mut d = bytes.to_vec();
        self.xfer(addr, Some(reg), &mut d, true).map(|_| ())
    }
    fn read_i2c_block(&self, addr: u8, reg: u8, len: usize) -> Result<Vec<u8>> {
        let mut d = vec![0u8; len];
        let n = self.xfer(addr, Some(reg), &mut d, false)?;
        d.truncate(n.min(len));
        Ok(d)
    }
    fn location(&self, addr: u8) -> String {
        format!("nvapi:{}:{addr:02x}", self.index)
    }
    fn name(&self) -> String {
        format!("gpu{}", self.index)
    }
}
