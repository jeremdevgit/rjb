// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Accès matériel bas niveau via PawnIO (pilote noyau signé, utilisé aussi par
//! OpenRGB, LibreHardwareMonitor, FanControl…). La bibliothèque PawnIOLib.dll
//! est chargée dynamiquement : sans PawnIO, rJb fonctionne sans les composants
//! internes.

use crate::device::{err, Result};
use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};
use windows_sys::Win32::System::LibraryLoader::{
    GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32,
};

fn dll_path() -> std::path::PathBuf {
    crate::sys::program_files().join("PawnIO").join("PawnIOLib.dll")
}

/// PawnIO est-il installé sur cette machine ?
pub fn installed() -> bool {
    dll_path().exists()
}

type Handle = *mut c_void;
type FnOpen = unsafe extern "system" fn(*mut Handle) -> i32;
type FnLoad = unsafe extern "system" fn(Handle, *const u8, usize) -> i32;
type FnExec = unsafe extern "system" fn(Handle, *const u8, *const u64, usize, *mut u64, usize, *mut usize) -> i32;
type FnClose = unsafe extern "system" fn(Handle) -> i32;

struct Lib {
    open: FnOpen,
    load: FnLoad,
    exec: FnExec,
    close: FnClose,
}

static LIB: OnceLock<std::result::Result<Lib, String>> = OnceLock::new();

/// Modules PawnIO signés intégrés à l'exécutable.
pub mod blobs {
    pub static SMBUS_I801: &[u8] = include_bytes!("../../assets/pawnio/SmbusI801.bin");
    pub static SMBUS_PIIX4: &[u8] = include_bytes!("../../assets/pawnio/SmbusPIIX4.bin");
    pub static SMBUS_NCT6793: &[u8] = include_bytes!("../../assets/pawnio/SmbusNCT6793.bin");
    pub static LPC_IO: &[u8] = include_bytes!("../../assets/pawnio/LpcIO.bin");
}

fn lib() -> std::result::Result<&'static Lib, String> {
    LIB.get_or_init(|| unsafe {
        // seulement l'emplacement officiel (protégé), jamais une DLL posée à
        // côté de rJb : elle serait chargée avec les droits administrateur
        for p in [dll_path()] {
            if !p.exists() {
                continue;
            }
            let w: Vec<u16> = p.as_os_str().to_string_lossy().encode_utf16().chain(Some(0)).collect();
            let h = LoadLibraryExW(w.as_ptr(), std::ptr::null_mut(), LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32);
            if h.is_null() {
                continue;
            }
            let get = |n: &[u8]| GetProcAddress(h, n.as_ptr());
            let (Some(o), Some(l), Some(e), Some(c)) = (
                get(b"pawnio_open\0"),
                get(b"pawnio_load\0"),
                get(b"pawnio_execute\0"),
                get(b"pawnio_close\0"),
            ) else {
                continue;
            };
            return Ok(Lib {
                open: std::mem::transmute::<_, FnOpen>(o),
                load: std::mem::transmute::<_, FnLoad>(l),
                exec: std::mem::transmute::<_, FnExec>(e),
                close: std::mem::transmute::<_, FnClose>(c),
            });
        }
        Err("PawnIO n'est pas installé".to_string())
    })
    .as_ref()
    .map_err(|e| e.clone())
}

pub fn available() -> std::result::Result<(), String> {
    lib().map(|_| ())
}

/// Un module PawnIO chargé (une instance de pilote).
pub struct Module {
    h: Mutex<usize>,
}

impl Module {
    pub fn load(blob: &[u8]) -> Result<Module> {
        let l = lib().map_err(crate::device::Error)?;
        unsafe {
            let mut h: Handle = std::ptr::null_mut();
            let r = (l.open)(&mut h);
            if r != 0 {
                return if r as u32 == 0x8007_0005 {
                    err("droits administrateur requis")
                } else {
                    err(format!("ouverture de PawnIO impossible ({r:#x})"))
                };
            }
            let r = (l.load)(h, blob.as_ptr(), blob.len());
            if r != 0 {
                (l.close)(h);
                return err(format!("module non pris en charge par ce matériel ({r:#x})"));
            }
            Ok(Module { h: Mutex::new(h as usize) })
        }
    }

    pub fn exec(&self, name: &str, input: &[u64], out_len: usize) -> Result<Vec<u64>> {
        let l = lib().map_err(crate::device::Error)?;
        let cname: Vec<u8> = name.bytes().chain(Some(0)).collect();
        let mut out = vec![0u64; out_len];
        let mut ret = 0usize;
        let h = self.h.lock().unwrap();
        let r = unsafe {
            (l.exec)(
                *h as Handle,
                cname.as_ptr(),
                input.as_ptr(),
                input.len(),
                if out_len > 0 { out.as_mut_ptr() } else { std::ptr::null_mut() },
                out_len,
                &mut ret,
            )
        };
        if r != 0 {
            return err(format!("{name} : erreur {r:#x}"));
        }
        Ok(out)
    }
}

impl Drop for Module {
    fn drop(&mut self) {
        if let Ok(l) = lib() {
            unsafe { (l.close)(*self.h.lock().unwrap() as Handle) };
        }
    }
}
