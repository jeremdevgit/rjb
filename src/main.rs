#![windows_subsystem = "windows"]
// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! rJb — contrôle RGB unifié et léger.
//!
//! En arrière-plan, seuls tournent l'icône de la zone de notification et le
//! moteur d'éclairage. L'interface n'est créée qu'à l'ouverture et libérée à la
//! fermeture.

mod config;
mod device;
mod drivers;
mod effects;
mod engine;
mod icon;
mod install;
mod langue;
mod layout;
mod model;
mod setup;
mod sys;
mod textes;
mod tray;
mod ui;

use engine::{Cmd, Shared};
use std::sync::{Arc, Mutex};
use windows_sys::core::GUID;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

/// Réveil de la boucle principale ; wparam = 1 : ouvrir la fenêtre (envoyé
/// par une seconde instance, éventuellement sans droits administrateur).
pub const WM_APP_ACTION: u32 = WM_APP + 1;
/// Demande de fermeture (programme d'installation / désinstallation).
pub const WM_APP_QUIT: u32 = WM_APP + 2;
const RESCAN_TIMER: usize = 1;
const TRAY_TIMER: usize = 2;

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Open,
    Quit,
    ToggleAllOff,
    Profile(String),
    Rescan,
    RebuildTray,
}

static ACTIONS: Mutex<Vec<Action>> = Mutex::new(Vec::new());
static HIDDEN_HWND: Mutex<usize> = Mutex::new(0);
static UI_CTX: Mutex<Option<eframe::egui::Context>> = Mutex::new(None);
static SHARED: Mutex<Option<Arc<Shared>>> = Mutex::new(None);

/// Ajoute une action et réveille la boucle principale.
pub fn post(a: Action) {
    ACTIONS.lock().unwrap().push(a);
    let h = *HIDDEN_HWND.lock().unwrap();
    if h != 0 {
        unsafe { PostMessageW(h as HWND, WM_APP_ACTION, 0, 0) };
    }
    wake_ui();
}

pub fn take_actions() -> Vec<Action> {
    std::mem::take(&mut *ACTIONS.lock().unwrap())
}

/// Demande à l'interface (si ouverte) de se redessiner.
pub fn wake_ui() {
    if let Some(ctx) = UI_CTX.lock().unwrap().as_ref() {
        ctx.request_repaint();
    }
}

pub fn set_ui_ctx(ctx: Option<eframe::egui::Context>) {
    *UI_CTX.lock().unwrap() = ctx;
}

/// Journal de diagnostic, actif seulement si la variable RJB_LOG est définie
/// (écrit dans log.txt à côté de config.json).
pub fn log(msg: &str) {
    use std::io::Write;
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if !*ON.get_or_init(|| {
        // actif si demandé par la variable ou par le réglage (fichier présent) ;
        // un journal trop gros repart de zéro
        let p = log_path();
        if std::fs::metadata(&p).is_ok_and(|m| m.len() > 2 * 1024 * 1024) {
            let _ = std::fs::write(&p, b"");
        }
        std::env::var_os("RJB_LOG").is_some() || p.exists()
    }) {
        return;
    }
    let p = log_path();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
        let _ = writeln!(f, "{msg}");
    }
}

pub fn log_path() -> std::path::PathBuf {
    config::Config::path().with_file_name("log.txt")
}

pub fn is_elevated() -> bool {
    unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
}

/// Relance rJb en administrateur (invite UAC) puis quitte cette instance.
pub fn relaunch_elevated() {
    let exe = std::env::current_exe().unwrap_or_default();
    // libère l'instance unique avant que la nouvelle ne démarre
    if sys::run_elevated(&exe, "--elevated-restart", false).is_some() {
        post(Action::Quit);
    }
}

use sys::w;

fn main() {
    // DLL chargées à la demande : uniquement depuis System32, jamais depuis le
    // dossier de l'exécutable (protège un rJb lancé depuis Téléchargements)
    unsafe {
        windows_sys::Win32::System::LibraryLoader::SetDefaultDllDirectories(
            windows_sys::Win32::System::LibraryLoader::LOAD_LIBRARY_SEARCH_SYSTEM32,
        );
    }
    let args: Vec<String> = std::env::args().collect();
    let has = |a: &str| args.iter().skip(1).any(|x| x.eq_ignore_ascii_case(a));
    let stem = std::env::current_exe()
        .ok()
        .and_then(|e| e.file_stem().map(|s| s.to_string_lossy().to_lowercase()))
        .unwrap_or_default();

    // langue choisie dans les réglages, sinon celle de Windows (installateur compris)
    langue::init(&config::Config::load().language);

    if let Some(i) = args.iter().position(|a| a == "--autostart") {
        let on = args.get(i + 1).is_some_and(|v| v == "on");
        std::process::exit(install::autostart_helper(on));
    }
    if has("--uninstall") {
        std::process::exit(setup::quiet_uninstall(has("--all")));
    }
    if has("--maintenance") || stem == "uninstall" {
        setup::run_maintenance();
        return;
    }
    if has("--setup") || stem.contains("setup") || stem.contains("install") {
        setup::run_setup(&args);
        return;
    }
    let minimized = has("--minimized");

    // instance unique : la seconde demande à la première d'ouvrir sa fenêtre
    // (après une relance en administrateur, on attend que l'ancienne se ferme)
    let restart = args.iter().any(|a| a == "--elevated-restart");
    unsafe {
        let name = w("Local\\rJb.instance");
        let mut tries = if restart { 50 } else { 1 };
        loop {
            let h = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
            if GetLastError() != ERROR_ALREADY_EXISTS {
                break;
            }
            tries -= 1;
            if tries == 0 {
                break;
            }
            CloseHandle(h);
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let class = w("rJbHidden");
            let h = FindWindowW(class.as_ptr(), std::ptr::null());
            if !h.is_null() {
                PostMessageW(h, WM_APP_ACTION, 1, 0);
            }
            return;
        }
    }

    // --minimized : lancement automatique avec Windows, sans ouvrir la fenêtre
    let cfg = config::Config::load();
    let (shared, engine_thread) = engine::spawn(cfg);
    *SHARED.lock().unwrap() = Some(shared.clone());

    let hwnd = create_hidden_window();
    *HIDDEN_HWND.lock().unwrap() = hwnd as usize;
    let mut tray = tray::Tray::new(&shared);
    if tray.missing() {
        unsafe { SetTimer(hwnd, TRAY_TIMER, 3000, None) };
    }

    if !minimized {
        post(Action::Open);
    }

    // boucle de messages Win32 : ne consomme rien tant qu'aucun événement n'arrive
    let mut msg: MSG = unsafe { std::mem::zeroed() };
    'main: loop {
        let r = unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) };
        if r <= 0 {
            break;
        }
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        for a in take_actions() {
            match a {
                Action::Open => {
                    ui::run(shared.clone());
                    // les actions reçues pendant que la fenêtre était ouverte
                    if ACTIONS.lock().unwrap().contains(&Action::Quit) {
                        break 'main;
                    }
                    // bouton « fermer » réglé sur « quitter rJb »
                    if !shared.config.lock().unwrap().close_to_tray {
                        break 'main;
                    }
                    tray.rebuild(&shared);
                }
                Action::Quit => break 'main,
                Action::RebuildTray => {
                    tray.rebuild(&shared);
                    if !tray.missing() {
                        unsafe { KillTimer(hwnd, TRAY_TIMER) };
                    }
                }
                other => handle_common(&shared, other),
            }
        }
        tray.refresh_checks(&shared);
    }

    shared.send(Cmd::Quit);
    let _ = engine_thread.join();
}

/// Actions utilisables aussi bien depuis la boucle principale que depuis l'interface.
pub fn handle_common(shared: &Shared, a: Action) {
    match a {
        Action::ToggleAllOff => {
            {
                let mut c = shared.config.lock().unwrap();
                c.all_off = !c.all_off;
            }
            shared.send(Cmd::ReloadAll);
        }
        Action::Profile(p) => {
            {
                let mut c = shared.config.lock().unwrap();
                if c.profiles.contains_key(&p) {
                    c.active_profile = p;
                }
            }
            shared.send(Cmd::ReloadAll);
        }
        Action::Rescan => shared.send(Cmd::Rescan),
        _ => {}
    }
}

fn create_hidden_window() -> HWND {
    unsafe {
        let class = w("rJbHidden");
        let hinst = GetModuleHandleW(std::ptr::null());
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class.as_ptr(),
        };
        RegisterClassW(&wc);
        let title = w("rJb");
        // fenêtre de premier niveau invisible : reçoit veille, arrêt et branchements
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinst,
            std::ptr::null(),
        );
        // notifications de branchement des interfaces HID
        let mut filter: DEV_BROADCAST_DEVICEINTERFACE_W = std::mem::zeroed();
        filter.dbcc_size = std::mem::size_of::<DEV_BROADCAST_DEVICEINTERFACE_W>() as u32;
        filter.dbcc_devicetype = DBT_DEVTYP_DEVICEINTERFACE;
        filter.dbcc_classguid = GUID::from_u128(0x4d1e55b2_f16f_11cf_88cb_001111000030);
        RegisterDeviceNotificationW(hwnd as _, &filter as *const _ as _, DEVICE_NOTIFY_WINDOW_HANDLE);
        // rJb tourne souvent en administrateur : on accepte le seul message
        // « ouvrir la fenêtre » venant d'un lancement normal (menu Démarrer)
        ChangeWindowMessageFilterEx(hwnd, WM_APP_ACTION, MSGFLT_ALLOW, std::ptr::null_mut());
        hwnd
    }
}

fn send_engine(c: Cmd) {
    if let Some(s) = SHARED.lock().unwrap().as_ref() {
        s.send(c);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_APP_ACTION => {
            // les actions internes sont déjà dans la file ; seule la demande
            // d'ouverture peut venir d'un autre processus
            if wp == 1 {
                post(Action::Open);
            }
            wake_ui();
            0
        }
        WM_APP_QUIT => {
            post(Action::Quit);
            0
        }
        WM_DEVICECHANGE => {
            if wp as u32 == DBT_DEVICEARRIVAL || wp as u32 == DBT_DEVICEREMOVECOMPLETE {
                // regroupe les rafales de notifications (un appareil = plusieurs interfaces)
                SetTimer(hwnd, RESCAN_TIMER, 800, None);
            }
            1
        }
        WM_TIMER if wp == TRAY_TIMER => {
            post(Action::RebuildTray);
            0
        }
        WM_TIMER if wp == RESCAN_TIMER => {
            KillTimer(hwnd, RESCAN_TIMER);
            send_engine(Cmd::Hotplug);
            0
        }
        WM_POWERBROADCAST => {
            match wp as u32 {
                PBT_APMSUSPEND => send_engine(Cmd::Suspend(true)),
                PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => send_engine(Cmd::Suspend(false)),
                _ => {}
            }
            1
        }
        WM_ENDSESSION if wp != 0 => {
            // arrêt de Windows : on rend la main proprement avant de partir
            let s = SHARED.lock().unwrap().clone();
            if let Some(s) = s {
                s.send(Cmd::Quit);
                std::thread::sleep(std::time::Duration::from_millis(400));
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
