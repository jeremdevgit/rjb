// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Installation, réparation et désinstallation de rJb, plus le lancement
//! automatique avec Windows.
//!
//! Sécurité : la tâche planifiée qui démarre rJb en administrateur à
//! l'ouverture de session n'est créée que si l'exécutable se trouve dans un
//! dossier modifiable uniquement par les administrateurs. Sinon n'importe quel
//! programme de l'utilisateur pourrait remplacer rJb.exe et obtenir les droits
//! administrateur sans invite UAC.

use crate::sys::{self, w};
use std::path::{Path, PathBuf};
use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use windows_sys::Win32::UI::Shell::{FOLDERID_CommonPrograms, FOLDERID_PublicDesktop};
use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW};

pub const UNINSTALL_KEY: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\rJb";
pub const APP_KEY: &str = "SOFTWARE\\rJb";
const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const TASK: &str = "rJb";
pub const APP_EXE: &str = "rJb.exe";
pub const MAINTENANCE_EXE: &str = "uninstall.exe";

#[derive(Clone, Debug)]
pub struct Options {
    pub dir: PathBuf,
    pub desktop: bool,
    pub start_menu: bool,
    pub autostart: bool,
    pub close_to_tray: bool,
    pub pawnio: bool,
}

impl Options {
    /// Choix par défaut, ou ceux de l'installation existante.
    pub fn current() -> Options {
        let get = |n: &str, d: bool| {
            sys::reg_get_dword(HKEY_LOCAL_MACHINE, APP_KEY, n)
                .map(|v| v != 0)
                .unwrap_or(d)
        };
        let existing = installed_dir();
        Options {
            // mise à jour / réparation : on reprend l'état réel, l'utilisateur
            // a pu changer ces options dans rJb depuis l'installation
            autostart: if existing.is_some() {
                autostart_enabled()
            } else {
                true
            },
            close_to_tray: crate::config::Config::load().close_to_tray,
            dir: crate::install_security::preferred_directory(
                existing,
                sys::program_files().join("rJb"),
            ),
            desktop: get("Desktop", true),
            start_menu: get("StartMenu", true),
            pawnio: !crate::drivers::pawnio::installed(),
        }
    }
}

/// Version déjà installée (affichée par l'installateur, qui la remplace).
pub fn installed_version() -> Option<String> {
    sys::reg_get_str(HKEY_LOCAL_MACHINE, UNINSTALL_KEY, "DisplayVersion").filter(|s| !s.is_empty())
}

pub fn installed_dir() -> Option<PathBuf> {
    sys::reg_get_str(HKEY_LOCAL_MACHINE, UNINSTALL_KEY, "InstallLocation")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

fn eq_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy()
        .trim_end_matches('\\')
        .eq_ignore_ascii_case(b.to_string_lossy().trim_end_matches('\\'))
}

fn under(p: &Path, root: &Path) -> bool {
    let p = p.to_string_lossy().to_lowercase();
    let r = root.to_string_lossy().to_lowercase();
    let r = r.trim_end_matches('\\');
    p.starts_with(&format!("{r}\\"))
}

/// Vérifie le propriétaire, les droits réels et tous les parents du dossier.
pub fn secure_dir(dir: &Path) -> bool {
    crate::install_security::trusted(dir)
}

/// rJb s'exécute-t-il depuis son installation officielle et protégée ?
pub fn running_from_trusted_install() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let Some(dir) = exe.parent() else {
        return false;
    };
    installed_dir().is_some_and(|d| {
        eq_path(&d, dir) && secure_dir(dir) && crate::install_security::trusted(&exe)
    })
}

/// Vérifie et complète le dossier choisi : on n'installe jamais en vrac dans
/// un dossier existant qui contient autre chose (la désinstallation n'efface
/// que nos propres fichiers, mais autant garder les choses nettes).
pub fn normalize_dir(input: &str) -> Result<PathBuf, String> {
    let p = crate::install_security::normalize(input)?;
    let s = p.to_string_lossy().to_string();
    if !(s.len() > 3 && s.as_bytes()[1] == b':' && s.as_bytes()[2] == b'\\') {
        return Err("Indiquez un chemin complet, par exemple C:\\Program Files\\rJb".into());
    }
    if s.contains(['<', '>', '"', '|', '?', '*']) {
        return Err("Le chemin contient des caractères interdits.".into());
    }
    let windir = sys::windows_directory()
        .ok_or_else(|| "Impossible de déterminer le dossier Windows.".to_string())?;
    if eq_path(&p, &windir) || under(&p, &windir) {
        return Err("Impossible d'installer dans le dossier de Windows.".into());
    }
    let ours = |d: &Path| d.join(APP_EXE).exists();
    let non_empty = std::fs::read_dir(&p).is_ok_and(|mut it| it.next().is_some());
    Ok(if non_empty && !ours(&p) {
        p.join("rJb")
    } else {
        p
    })
}

fn app_installer_version(name: &str) -> Option<Vec<u32>> {
    let remainder = name.strip_prefix("Microsoft.DesktopAppInstaller_")?;
    let (version, package) = remainder.split_once('_')?;
    if package != "x64__8wekyb3d8bbwe" {
        return None;
    }
    version.split('.').map(str::parse).collect()
}

/// Retourne uniquement le winget de l'App Installer installé sous WindowsApps
/// avec des ACL système protégées. Aucune recherche par PATH ou répertoire courant.
pub fn winget_executable() -> Option<PathBuf> {
    let root = sys::program_files().join("WindowsApps");
    let entries = std::fs::read_dir(root).ok()?;
    let mut candidates = Vec::new();
    for entry in entries.flatten() {
        let Some(version) = app_installer_version(&entry.file_name().to_string_lossy()) else {
            continue;
        };
        let exe = entry.path().join("winget.exe");
        if crate::install_security::trusted(&exe) {
            candidates.push((version, exe));
        }
    }
    candidates
        .into_iter()
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, p)| p)
}

// ── instance en cours ───────────────────────────────────────────────────

/// Demande à rJb de se fermer proprement et attend qu'il soit parti.
pub fn stop_running_app() -> bool {
    let class = w("rJbHidden");
    for i in 0..100 {
        let h = unsafe { FindWindowW(class.as_ptr(), std::ptr::null()) };
        if h.is_null() {
            return true;
        }
        if i % 20 == 0 {
            unsafe { PostMessageW(h, crate::WM_APP_QUIT, 0, 0) };
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    false
}

// ── lancement automatique ───────────────────────────────────────────────

fn task_exists() -> bool {
    sys::run_hidden("schtasks.exe", &["/Query", "/TN", TASK]) == Some(0)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Tâche « à l'ouverture de session, privilèges les plus élevés », sans les
/// réglages par défaut de schtasks qui arrêteraient rJb au bout de 72 h ou
/// sur batterie.
fn create_task(exe: &Path) -> bool {
    if !crate::install_security::trusted(exe) {
        return false;
    }
    let user = format!(
        "{}\\{}",
        std::env::var("USERDOMAIN").unwrap_or_default(),
        std::env::var("USERNAME").unwrap_or_default()
    );
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>Démarre rJb à l'ouverture de session.</Description></RegistrationInfo>
  <Triggers><LogonTrigger><Enabled>true</Enabled><UserId>{u}</UserId></LogonTrigger></Triggers>
  <Principals><Principal id="Author"><UserId>{u}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>HighestAvailable</RunLevel></Principal></Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <IdleSettings><StopOnIdleEnd>false</StopOnIdleEnd><RestartOnIdle>false</RestartOnIdle></IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>5</Priority>
  </Settings>
  <Actions Context="Author"><Exec><Command>"{e}"</Command><Arguments>--minimized</Arguments></Exec></Actions>
</Task>"#,
        u = xml_escape(&user),
        e = xml_escape(&exe.to_string_lossy())
    );
    // le fichier est écrit dans le dossier d'installation, protégé : personne
    // ne peut le modifier entre son écriture et sa lecture par schtasks
    let Some(dir) = exe.parent() else {
        return false;
    };
    let file = dir.join("autostart.xml");
    let mut bytes = vec![0xFF, 0xFE];
    for u in xml.encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    if std::fs::write(&file, bytes).is_err() {
        return false;
    }
    let ok = sys::run_hidden(
        "schtasks.exe",
        &[
            "/Create",
            "/F",
            "/TN",
            TASK,
            "/XML",
            &file.to_string_lossy(),
        ],
    ) == Some(0);
    let _ = std::fs::remove_file(&file);
    ok
}

fn set_run_key(exe: Option<&Path>) {
    match exe {
        Some(e) => {
            sys::reg_set_str(
                HKEY_CURRENT_USER,
                RUN_KEY,
                "rJb",
                &format!("\"{}\" --minimized", e.display()),
            );
        }
        None => sys::reg_delete_value(HKEY_CURRENT_USER, RUN_KEY, "rJb"),
    }
}

pub fn autostart_enabled() -> bool {
    sys::reg_get_str(HKEY_CURRENT_USER, RUN_KEY, "rJb").is_some() || task_exists()
}

/// Active ou désactive le lancement automatique. Depuis l'installation
/// protégée, rJb démarre en administrateur (RAM et carte mère disponibles
/// d'emblée) ; une invite UAC est affichée si besoin pour créer la tâche.
pub fn set_autostart(enable: bool) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let elevated = crate::is_elevated();
    if enable && running_from_trusted_install() {
        set_run_key(None);
        if elevated {
            create_task(&exe);
        } else {
            sys::run_elevated(&exe, "--autostart on", true);
        }
        return;
    }
    set_run_key(enable.then_some(exe.as_path()));
    if task_exists() {
        if elevated {
            sys::run_hidden("schtasks.exe", &["/Delete", "/F", "/TN", TASK]);
        } else {
            sys::run_elevated(&exe, "--autostart off", true);
        }
    }
}

/// `--autostart on|off`, appelé en administrateur par `set_autostart`.
pub fn autostart_helper(on: bool) -> i32 {
    let Ok(exe) = std::env::current_exe() else {
        return 1;
    };
    if on {
        if !running_from_trusted_install() {
            return 2;
        }
        return if create_task(&exe) { 0 } else { 1 };
    }
    sys::run_hidden("schtasks.exe", &["/Delete", "/F", "/TN", TASK]);
    0
}

// ── installation ────────────────────────────────────────────────────────

fn shortcut_paths() -> (Option<PathBuf>, Option<PathBuf>) {
    (
        sys::known_folder(&FOLDERID_PublicDesktop).map(|d| d.join("rJb.lnk")),
        sys::known_folder(&FOLDERID_CommonPrograms).map(|d| d.join("rJb.lnk")),
    )
}

/// Seuls les administrateurs et le système peuvent modifier le dossier.
fn lock_down(dir: &Path) -> Result<(), String> {
    crate::install_security::prepare(dir)
}

fn dir_size_kb(dir: &Path) -> u32 {
    std::fs::read_dir(dir)
        .map(|it| {
            it.flatten()
                .filter_map(|e| e.metadata().ok())
                .map(|m| m.len())
                .sum::<u64>()
        })
        .unwrap_or(0) as u32
        / 1024
}

/// Copie en passant par un fichier temporaire : un fichier à moitié écrit ne
/// remplace jamais une version qui fonctionne.
fn copy_atomic(src: &Path, dst: &Path) -> std::io::Result<()> {
    let policy_error = |e: String| std::io::Error::new(std::io::ErrorKind::PermissionDenied, e);
    if eq_path(src, dst) {
        return crate::install_security::seal_file(dst).map_err(policy_error);
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let tmp = dst.with_extension(format!("install-{}-{nonce}.tmp", std::process::id()));
    let result = (|| {
        let mut input = std::fs::File::open(src)?;
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        std::io::copy(&mut input, &mut output)?;
        output.sync_all()?;
        drop(output);
        crate::install_security::seal_file(&tmp).map_err(policy_error)?;
        std::fs::rename(&tmp, dst)?;
        if !crate::install_security::trusted(dst) {
            return Err(policy_error(
                crate::install_security::PERMISSIONS_FAILED.into(),
            ));
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Installe (ou met à jour / répare) rJb. `step` reçoit l'avancement.
pub fn install(o: &Options, step: &dyn Fn(&str)) -> Result<Vec<String>, String> {
    let previous = installed_dir();
    if previous
        .as_ref()
        .is_some_and(|p| !crate::install_security::trusted(&p.join(APP_EXE)))
    {
        // Disable the old elevated entry before validating/copying a replacement.
        // A failed migration must not leave an unsafe scheduled task active.
        if task_exists()
            && sys::run_hidden("schtasks.exe", &["/Delete", "/F", "/TN", TASK]) != Some(0)
        {
            return Err("Impossible de désactiver l'ancien démarrage non sécurisé.".into());
        }
        set_run_key(None);
    }
    let mut checked = o.clone();
    checked.dir = normalize_dir(&o.dir.to_string_lossy())?;
    let o = &checked;
    let mut notes = Vec::new();
    let src = std::env::current_exe().map_err(|e| e.to_string())?;
    step("Fermeture de rJb s'il est ouvert…");
    if !stop_running_app() {
        return Err(
            "rJb est toujours ouvert. Quittez-le depuis la zone de notification puis réessayez."
                .into(),
        );
    }
    step("Copie des fichiers…");
    lock_down(&o.dir)?;
    let app = o.dir.join(APP_EXE);
    let maint = o.dir.join(MAINTENANCE_EXE);
    copy_atomic(&src, &app).map_err(|e| format!("Copie de rJb.exe impossible : {e}"))?;
    copy_atomic(&src, &maint)
        .map_err(|e| format!("Copie du programme de désinstallation impossible : {e}"))?;
    // ancienne version : ses fichiers sont retirés une fois la nouvelle copiée
    // (profils et réglages conservés) ; installée ailleurs, elle ne reste pas orpheline
    if let Some(old) = previous.filter(|p| crate::install_security::trusted(p)) {
        step("Retrait de l'ancienne version…");
        let moved = !eq_path(&old, &o.dir);
        for f in [APP_EXE, MAINTENANCE_EXE, "rJb.new", "uninstall.new"] {
            let file = old.join(f);
            if (moved || f.ends_with(".new")) && !eq_path(&file, &src) {
                let _ = std::fs::remove_file(file);
            }
        }
        if moved {
            let _ = std::fs::remove_dir(&old); // seulement s'il est vide
        }
    }

    step("Raccourcis…");
    let (desk, menu) = shortcut_paths();
    for (want, path) in [(o.desktop, desk), (o.start_menu, menu)] {
        let Some(path) = path else { continue };
        if want {
            if !sys::create_shortcut(&path, &app, "", crate::langue::tr("Contrôle RGB unifié")) {
                notes.push(format!("Raccourci non créé : {}", path.display()));
            }
        } else {
            let _ = std::fs::remove_file(&path);
        }
    }

    step("Inscription dans Windows…");
    let m = format!("\"{}\" --maintenance", maint.display());
    let k = UNINSTALL_KEY;
    let hk = HKEY_LOCAL_MACHINE;
    sys::reg_delete_tree(hk, k); // inscription de l'ancienne version remplacée en entier
    sys::reg_set_str(hk, k, "DisplayName", "rJb");
    sys::reg_set_str(hk, k, "DisplayVersion", env!("CARGO_PKG_VERSION"));
    sys::reg_set_str(hk, k, "Publisher", "jeremdevgit");
    sys::reg_set_str(hk, k, "URLInfoAbout", "https://github.com/jeremdevgit/rjb");
    sys::reg_set_str(
        hk,
        k,
        "URLUpdateInfo",
        "https://github.com/jeremdevgit/rjb/releases/latest",
    );
    sys::reg_set_str(hk, k, "DisplayIcon", &format!("{},0", app.display()));
    sys::reg_set_str(hk, k, "InstallLocation", &o.dir.to_string_lossy());
    sys::reg_set_str(hk, k, "UninstallString", &m);
    sys::reg_set_str(hk, k, "ModifyPath", &m);
    sys::reg_set_str(
        hk,
        k,
        "QuietUninstallString",
        &format!("\"{}\" --uninstall --quiet", maint.display()),
    );
    sys::reg_set_str(
        hk,
        k,
        "Comments",
        crate::langue::tr("Contrôle RGB unifié et léger"),
    );
    sys::reg_set_dword(hk, k, "EstimatedSize", dir_size_kb(&o.dir));
    sys::reg_set_dword(hk, k, "NoRepair", 0);
    for (n, v) in [
        ("Desktop", o.desktop),
        ("StartMenu", o.start_menu),
        ("Autostart", o.autostart),
        ("CloseToTray", o.close_to_tray),
    ] {
        sys::reg_set_dword(hk, APP_KEY, n, v as u32);
    }

    step("Préférences…");
    let mut cfg = crate::config::Config::load();
    cfg.close_to_tray = o.close_to_tray;
    cfg.autostart = o.autostart;
    cfg.save();
    set_run_key(None);
    if o.autostart {
        if !secure_dir(&o.dir) {
            set_run_key(Some(&app));
            notes.push("Dossier hors de Program Files : rJb démarrera avec Windows sans droits administrateur.".into());
        } else if !create_task(&app) {
            set_run_key(Some(&app));
            notes.push("La tâche de démarrage n'a pas pu être créée, rJb utilisera le démarrage classique.".into());
        }
    } else {
        sys::run_hidden("schtasks.exe", &["/Delete", "/F", "/TN", TASK]);
    }

    if o.pawnio && !crate::drivers::pawnio::installed() {
        step("Installation du pilote PawnIO (RAM et carte mère)…");
        let Some(winget) = winget_executable() else {
            notes.push(
                "winget est introuvable dans un dossier protégé. Installez PawnIO depuis pawnio.eu."
                    .into(),
            );
            return Ok(notes);
        };
        let code = std::process::Command::new(winget)
            .args([
                "install",
                "--id",
                "namazso.PawnIO",
                "-e",
                "--silent",
                "--accept-package-agreements",
                "--accept-source-agreements",
            ])
            .creation_flags_hidden()
            .status()
            .ok()
            .and_then(|s| s.code());
        if !crate::drivers::pawnio::installed() {
            notes.push(crate::langue::trf(
                "PawnIO n'a pas pu être installé automatiquement (code {code}). Téléchargez-le sur pawnio.eu.",
                &[("code", &code.map(|c| c.to_string()).unwrap_or("?".into()))],
            ));
        }
    }
    Ok(notes)
}

trait HiddenCommand {
    fn creation_flags_hidden(&mut self) -> &mut Self;
}

impl HiddenCommand for std::process::Command {
    fn creation_flags_hidden(&mut self) -> &mut Self {
        use std::os::windows::process::CommandExt;
        self.creation_flags(0x0800_0000)
    }
}

/// Réparation : recopie les fichiers et recrée raccourcis, inscription et
/// démarrage automatique avec les choix d'origine. Les réglages sont gardés.
pub fn repair(step: &dyn Fn(&str)) -> Result<Vec<String>, String> {
    let mut o = Options::current();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            o.dir = d.to_path_buf();
        }
    }
    o.pawnio = false;
    install(&o, step)
}

/// Désinstalle. `everything` supprime aussi les réglages et journaux.
pub fn uninstall(everything: bool, step: &dyn Fn(&str)) -> Result<(), String> {
    let dir = installed_dir()
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|e| e.parent().map(Path::to_path_buf))
        })
        .ok_or("Installation introuvable")?;
    step("Fermeture de rJb…");
    if !stop_running_app() {
        return Err(
            "rJb est toujours ouvert. Quittez-le depuis la zone de notification puis réessayez."
                .into(),
        );
    }
    step("Démarrage automatique et raccourcis…");
    sys::run_hidden("schtasks.exe", &["/Delete", "/F", "/TN", TASK]);
    set_run_key(None);
    let (desk, menu) = shortcut_paths();
    for p in [desk, menu].into_iter().flatten() {
        let _ = std::fs::remove_file(p);
    }
    step("Suppression des fichiers…");
    // uniquement nos fichiers, jamais le contenu complet du dossier
    let mut files = vec![APP_EXE, "rJb.new", "uninstall.new", "autostart.xml"];
    if everything {
        files.extend(["config.json", "config.json.tmp", "log.txt"]);
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let d = PathBuf::from(appdata).join("rJb");
            for f in ["config.json", "config.json.tmp", "log.txt"] {
                let _ = std::fs::remove_file(d.join(f));
            }
            let _ = std::fs::remove_dir(&d);
        }
    }
    for f in files {
        let _ = std::fs::remove_file(dir.join(f));
    }
    step("Retrait de l'inscription Windows…");
    sys::reg_delete_tree(HKEY_LOCAL_MACHINE, UNINSTALL_KEY);
    sys::reg_delete_tree(HKEY_LOCAL_MACHINE, APP_KEY);
    Ok(())
}

fn powershell_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// Le programme de désinstallation ne peut pas s'effacer lui-même pendant
/// qu'il tourne : PowerShell supprime ses fichiers après sa fermeture. Les
/// chemins sont des littéraux PowerShell échappés, jamais du code cmd.exe.
/// À appeler au moment de quitter.
pub fn finish_uninstall() {
    let Some(maint) = std::env::current_exe().ok() else {
        return;
    };
    let Some(dir) = maint.parent() else { return };
    if !maint
        .file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case(MAINTENANCE_EXE))
    {
        return; // seulement depuis le programme de désinstallation installé
    }
    let Some(powershell) = sys::system_tool("WindowsPowerShell\\v1.0\\powershell.exe") else {
        return;
    };
    let Some(modules_dir) = powershell.parent().map(|p| p.join("Modules")) else {
        return;
    };
    let script = format!(
        "Microsoft.PowerShell.Utility\\Start-Sleep -Milliseconds 1500; Microsoft.PowerShell.Management\\Remove-Item -LiteralPath {} -Force -ErrorAction SilentlyContinue; Microsoft.PowerShell.Management\\Remove-Item -LiteralPath {} -Force -ErrorAction SilentlyContinue",
        powershell_literal(&maint.to_string_lossy()),
        powershell_literal(&dir.to_string_lossy())
    );
    let _ = std::process::Command::new(powershell)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &script,
        ])
        .env("PSModulePath", modules_dir)
        .spawn();
}

#[cfg(test)]
mod uninstall_tests {
    use super::{app_installer_version, powershell_literal};

    #[test]
    fn only_the_x64_desktop_app_installer_package_name_is_accepted() {
        assert_eq!(
            app_installer_version("Microsoft.DesktopAppInstaller_1.2.3.4_x64__8wekyb3d8bbwe"),
            Some(vec![1, 2, 3, 4])
        );
        assert_eq!(
            app_installer_version("Microsoft.DesktopAppInstaller_1.2.3.4_x86__8wekyb3d8bbwe"),
            None
        );
    }

    #[test]
    fn uninstall_path_is_a_literal_even_with_shell_metacharacters() {
        assert_eq!(
            powershell_literal(r"C:\rJb\&^!'$(whoami)%PATH%"),
            r"'C:\rJb\&^!''$(whoami)%PATH%'"
        );
    }
}
