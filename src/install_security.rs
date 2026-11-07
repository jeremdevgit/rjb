// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit
//! Fail-closed installation policy shared by the Windows installers.
use std::path::{Path, PathBuf};

pub const INVALID_PATH: &str = "Le chemin contient des caractères interdits.";
pub const UNSAFE_DIRECTORY: &str = "Le dossier d'installation doit être protégé en écriture.";
pub const PERMISSIONS_FAILED: &str = "Impossible de protéger le dossier d'installation.";

/// Validate the original spelling, before Path::components can hide aliases.
pub fn checked_spelling(input: &str) -> Result<String, String> {
    let raw = input;
    let raw = if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
        &raw[1..raw.len() - 1]
    } else {
        raw
    };
    let s = raw.replace('/', "\\");
    let b = s.as_bytes();
    if b.len() <= 3
        || !b[0].is_ascii_alphabetic()
        || b[1] != b':'
        || b[2] != b'\\'
        || s.chars().any(|c| c.is_control() || "<>\"|?*".contains(c))
        || s[2..].contains(':')
    {
        return Err(INVALID_PATH.into());
    }
    let s = s.trim_end_matches('\\');
    for part in s[3..].split('\\') {
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        let device = matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        ) || ["COM", "LPT"].iter().any(|p| {
            stem.strip_prefix(p).is_some_and(|n| {
                matches!(
                    n,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        });
        if part.is_empty() || part == "." || part == ".." || part.ends_with(['.', ' ']) || device {
            return Err(INVALID_PATH.into());
        }
    }
    Ok(s.to_owned())
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::*;
    use windows_sys::Win32::Security::*;

    const REPARSE_POINT: u32 = 0x400;
    const PARENT_DANGER: u32 = 0x40 | 0x10000 | 0x40000 | 0x80000 | 0x10000000;
    const OBJECT_DANGER: u32 = PARENT_DANGER | 0x2 | 0x4 | 0x10 | 0x100 | 0x40000000;
    const TRUSTED: &[&str] = &[
        "S-1-5-18",
        "S-1-5-32-544",
        "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464",
    ];
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    struct OwnedDescriptor(PSECURITY_DESCRIPTOR);
    impl Drop for OwnedDescriptor {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    LocalFree(self.0);
                }
            }
        }
    }

    unsafe fn trusted_sid(sid: PSID) -> bool {
        if sid.is_null() || IsValidSid(sid) == 0 {
            return false;
        }
        let mut text = std::ptr::null_mut();
        if ConvertSidToStringSidW(sid, &mut text) == 0 {
            return false;
        }
        let mut len = 0;
        while *text.add(len) != 0 {
            len += 1;
        }
        let value = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
        LocalFree(text.cast());
        TRUSTED.contains(&value.as_str())
    }
    /// Conservatively reject unknown allow ACEs and untrusted owners. Deny ACEs
    /// never make this check more permissive; complex DACLs therefore fail closed.
    unsafe fn descriptor_is_safe(owner: PSID, acl: *const ACL, danger: u32) -> bool {
        if !trusted_sid(owner) || acl.is_null() || IsValidAcl(acl) == 0 {
            return false;
        }
        for i in 0..(*acl).AceCount as u32 {
            let mut ptr = std::ptr::null_mut();
            if GetAce(acl, i, &mut ptr) == 0 || ptr.is_null() {
                return false;
            }
            let header = &*ptr.cast::<ACE_HEADER>();
            if header.AceFlags & INHERIT_ONLY_ACE as u8 != 0 {
                continue;
            }
            match header.AceType {
                0 => {
                    // ACCESS_ALLOWED_ACE
                    if (header.AceSize as usize) < std::mem::size_of::<ACCESS_ALLOWED_ACE>() {
                        return false;
                    }
                    let ace = &*ptr.cast::<ACCESS_ALLOWED_ACE>();
                    let sid = std::ptr::addr_of!(ace.SidStart).cast_mut().cast();
                    if ace.Mask & danger != 0 && !trusted_sid(sid) {
                        return false;
                    }
                }
                1 | 6 => {} // ACCESS_DENIED[_OBJECT]_ACE
                _ => return false,
            }
        }
        true
    }
    fn safe_acl(path: &Path, danger: u32) -> bool {
        let mut owner = std::ptr::null_mut();
        let mut acl = std::ptr::null_mut();
        let mut sd = std::ptr::null_mut();
        let name = wide(&path.to_string_lossy());
        let status = unsafe {
            GetNamedSecurityInfoW(
                name.as_ptr(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &mut owner,
                std::ptr::null_mut(),
                &mut acl,
                std::ptr::null_mut(),
                &mut sd,
            )
        };
        let _guard = OwnedDescriptor(sd);
        status == 0 && unsafe { descriptor_is_safe(owner, acl, danger) }
    }
    fn not_reparse(path: &Path) -> bool {
        std::fs::symlink_metadata(path).is_ok_and(|m| m.file_attributes() & REPARSE_POINT == 0)
    }
    fn safe_parents(path: &Path) -> bool {
        path.ancestors()
            .all(|p| not_reparse(p) && safe_acl(p, PARENT_DANGER))
    }
    pub fn normalize(input: &str) -> Result<PathBuf, String> {
        let path = PathBuf::from(checked_spelling(input)?);
        let mut suffix = Vec::new();
        let mut parent = path.as_path();
        loop {
            match std::fs::symlink_metadata(parent) {
                Ok(m) => {
                    if !m.is_dir() || m.file_attributes() & REPARSE_POINT != 0 {
                        return Err(UNSAFE_DIRECTORY.into());
                    }
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    suffix.push(parent.file_name().ok_or(INVALID_PATH)?.to_os_string());
                    parent = parent.parent().ok_or(INVALID_PATH)?;
                }
                Err(_) => return Err(UNSAFE_DIRECTORY.into()),
            }
        }
        // Reject every reparse ancestor before canonicalization follows anything.
        if !parent.ancestors().all(not_reparse) {
            return Err(UNSAFE_DIRECTORY.into());
        }
        let canonical = std::fs::canonicalize(parent).map_err(|_| UNSAFE_DIRECTORY)?;
        let text = canonical.to_string_lossy();
        let text = text.strip_prefix(r"\\?\").unwrap_or(&text);
        let mut result = PathBuf::from(text);
        for part in suffix.into_iter().rev() {
            result.push(part);
        }
        // Root itself is never an install destination; spelling also rejects UNC/device namespaces.
        checked_spelling(&result.to_string_lossy())?;
        Ok(result)
    }
    pub fn trusted(path: &Path) -> bool {
        let Ok(spelling) = checked_spelling(&path.to_string_lossy()) else {
            return false;
        };
        let path = PathBuf::from(spelling);
        let normalized = if path.is_file() {
            let Some(parent) = path.parent() else {
                return false;
            };
            let Ok(parent) = normalize(&parent.to_string_lossy()) else {
                return false;
            };
            parent.join(path.file_name().unwrap())
        } else {
            let Ok(path) = normalize(&path.to_string_lossy()) else {
                return false;
            };
            path
        };
        not_reparse(&normalized)
            && safe_acl(&normalized, OBJECT_DANGER)
            && normalized.parent().is_some_and(safe_parents)
    }
    pub fn seal_file(path: &Path) -> Result<(), String> {
        if !path.is_file() || !not_reparse(path) || !path.parent().is_some_and(trusted) {
            return Err(UNSAFE_DIRECTORY.into());
        }
        set_acl(path)?;
        if !trusted(path) {
            return Err(PERMISSIONS_FAILED.into());
        }
        Ok(())
    }

    fn set_acl(path: &Path) -> Result<(), String> {
        // Replace, rather than add grants to, the DACL. Set owner to Administrators
        // so an unprivileged former owner cannot restore their own write access.
        let sddl = wide("O:BAG:BAD:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FRFX;;;BU)");
        let mut sd = std::ptr::null_mut();
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut sd,
                std::ptr::null_mut(),
            )
        } == 0
        {
            return Err(PERMISSIONS_FAILED.into());
        }
        let _guard = OwnedDescriptor(sd);
        let mut acl = std::ptr::null_mut();
        let mut owner = std::ptr::null_mut();
        let mut present = 0;
        let mut defaulted = 0;
        if unsafe { GetSecurityDescriptorDacl(sd, &mut present, &mut acl, &mut defaulted) } == 0
            || present == 0
            || unsafe { GetSecurityDescriptorOwner(sd, &mut owner, &mut defaulted) } == 0
        {
            return Err(PERMISSIONS_FAILED.into());
        }
        let name = wide(&path.to_string_lossy());
        let status = unsafe {
            SetNamedSecurityInfoW(
                name.as_ptr(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION
                    | DACL_SECURITY_INFORMATION
                    | PROTECTED_DACL_SECURITY_INFORMATION,
                owner,
                std::ptr::null_mut(),
                acl,
                std::ptr::null(),
            )
        };
        if status != 0 {
            return Err(PERMISSIONS_FAILED.into());
        }
        Ok(())
    }
    pub fn prepare(path: &Path) -> Result<(), String> {
        let normalized = normalize(&path.to_string_lossy())?;
        // Verify existing ancestors, before creating anything or replacing files.
        let mut parent = normalized.parent().ok_or(UNSAFE_DIRECTORY)?;
        while !parent.exists() {
            parent = parent.parent().ok_or(UNSAFE_DIRECTORY)?;
        }
        if !safe_parents(parent) {
            return Err(UNSAFE_DIRECTORY.into());
        }
        // Do not take over an existing writable directory: outstanding handles
        // from its former users could otherwise retain mutation rights.
        if normalized.exists() && !trusted(&normalized) {
            return Err(UNSAFE_DIRECTORY.into());
        }
        std::fs::create_dir_all(&normalized).map_err(|_| PERMISSIONS_FAILED)?;
        if !normalized.parent().is_some_and(safe_parents) {
            return Err(UNSAFE_DIRECTORY.into());
        }
        set_acl(&normalized)?;
        if !trusted(&normalized) {
            return Err(PERMISSIONS_FAILED.into());
        }
        Ok(())
    }
    #[cfg(test)]
    mod windows_tests {
        use super::*;
        fn root() -> PathBuf {
            use windows_sys::Win32::System::Com::CoTaskMemFree;
            use windows_sys::Win32::UI::Shell::{FOLDERID_ProgramFiles, SHGetKnownFolderPath};
            let mut p = std::ptr::null_mut();
            assert_eq!(
                unsafe {
                    SHGetKnownFolderPath(&FOLDERID_ProgramFiles, 0, std::ptr::null_mut(), &mut p)
                },
                0
            );
            let mut n = 0;
            while unsafe { *p.add(n) } != 0 {
                n += 1;
            }
            let path = PathBuf::from(String::from_utf16_lossy(unsafe {
                std::slice::from_raw_parts(p, n)
            }));
            unsafe { CoTaskMemFree(p.cast()) };
            path.join(format!(
                "InstallPolicyTest-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ))
        }
        #[test]
        fn protected_install_and_writable_acl_rejected() {
            let p = root();
            prepare(&p).expect("CI runner must support a protected installation fixture");
            assert!(trusted(&p));
            let file = p.join("test.exe");
            std::fs::write(&file, b"demo").unwrap();
            seal_file(&file).unwrap();
            assert!(trusted(&file));
            let command = std::env::var_os("SystemRoot")
                .map(PathBuf::from)
                .unwrap()
                .join("System32/icacls.exe");
            assert!(std::process::Command::new(command)
                .arg(&p)
                .args(["/grant", "*S-1-1-0:(OI)(CI)M", "/Q"])
                .status()
                .unwrap()
                .success());
            assert!(!trusted(&p));
            assert!(prepare(&p).is_err());
            std::fs::remove_dir_all(p).unwrap();
        }
        #[test]
        fn junction_ancestor_rejected() {
            let p = root();
            prepare(&p).unwrap();
            let target = p.join("target");
            std::fs::create_dir(&target).unwrap();
            let junction = p.join("link");
            let command = std::env::var_os("SystemRoot")
                .map(PathBuf::from)
                .unwrap()
                .join("System32/WindowsPowerShell/v1.0/powershell.exe");
            // Paths are fresh test fixtures, not external input. Use PowerShell's
            // native junction creation rather than cmd.exe's special quoting.
            let script = format!("$ErrorActionPreference='Stop'; New-Item -ItemType Junction -Path '{}' -Target '{}' | Out-Null", junction.display(), target.display());
            assert!(std::process::Command::new(command)
                .args(["-NoProfile", "-NonInteractive", "-Command"])
                .arg(script)
                .status()
                .unwrap()
                .success());
            assert!(normalize(&junction.join("app").to_string_lossy()).is_err());
            assert!(!trusted(&junction));
            std::fs::remove_dir(&junction).unwrap();
            std::fs::remove_dir_all(p).unwrap();
        }
    }
}
#[cfg(windows)]
pub use platform::{normalize, prepare, seal_file, trusted};
#[cfg(not(windows))]
pub fn normalize(input: &str) -> Result<PathBuf, String> {
    checked_spelling(input).map(PathBuf::from)
}
#[cfg(not(windows))]
pub fn prepare(_: &Path) -> Result<(), String> {
    Err(UNSAFE_DIRECTORY.into())
}
#[cfg(not(windows))]
pub fn seal_file(_: &Path) -> Result<(), String> {
    Err(UNSAFE_DIRECTORY.into())
}
#[cfg(not(windows))]
pub fn trusted(_: &Path) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aliases_rejected_before_path_parsing() {
        for value in [
            r"C:\Program Files\..\Users\AuditUser\App",
            r"C:\Program Files\.\App",
            r"C:\Program Files\App.",
            r"C:\Program Files\App ",
            r"C:\Program Files\NUL.txt",
            r"C:\Program Files\App:stream",
            r"C:relative",
            r"\\?\C:\App",
            r"\\server\share\App",
            r"C:\\App",
            "C:\\App\0bad",
            r"C:\",
        ] {
            assert!(checked_spelling(value).is_err(), "{value:?}");
        }
    }
    #[test]
    fn ordinary_absolute_paths_allowed() {
        assert_eq!(
            checked_spelling(r"C:\Program Files\Application").unwrap(),
            r"C:\Program Files\Application"
        );
        assert_eq!(
            checked_spelling(r#""C:\Program Files\Application""#).unwrap(),
            r"C:\Program Files\Application"
        );
        assert_eq!(
            checked_spelling("C:/Program Files/Application").unwrap(),
            r"C:\Program Files\Application"
        );
    }
}
