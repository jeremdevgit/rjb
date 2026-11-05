// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Moteur d'éclairage : possède les appareils, calcule les effets, écrit les couleurs.
//!
//! Il dort tant qu'aucun effet n'est animé (0 % CPU) et n'écrit sur un appareil
//! que lorsque ses couleurs changent.

use crate::config::{Config, Lighting};
use crate::device::Device;
use crate::drivers::Scanner;
use crate::effects::{EffectKind, EffectSpec, EffectState};
use crate::model::{Color, DeviceInfo, HwParams};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

pub enum Cmd {
    /// La configuration d'un appareil a changé.
    Changed(String),
    /// Tout réappliquer (profil, luminosité, extinction…).
    ReloadAll,
    /// Recherche demandée par l'utilisateur (complète).
    Rescan,
    /// Un appareil a été branché ou débranché.
    Hotplug,
    /// L'interface vient de s'ouvrir : republier l'état courant.
    RefreshPreview,
    Resize { id: String, zone: usize, leds: u16 },
    /// Mise en veille (true) / réveil (false) du PC.
    Suspend(bool),
    /// Arrêt : rend la main aux firmwares (ou éteint selon l'option).
    Quit,
}

#[derive(Clone)]
pub struct DeviceView {
    pub info: DeviceInfo,
    pub colors: Vec<Color>,
    pub error: Option<String>,
}

pub struct Shared {
    pub config: Mutex<Config>,
    pub devices: RwLock<Vec<DeviceView>>,
    pub ui_visible: AtomicBool,
    pub scanning: AtomicBool,
    /// Messages d'état (PawnIO absent, conflit avec un autre logiciel…)
    pub notices: Mutex<Vec<String>>,
    tx: Mutex<Option<Sender<Cmd>>>,
}

impl Shared {
    pub fn send(&self, c: Cmd) {
        if let Some(tx) = self.tx.lock().unwrap().as_ref() {
            let _ = tx.send(c);
        }
    }
    pub fn save_config(&self) {
        self.config.lock().unwrap().save();
    }
}

struct Slot {
    dev: Box<dyn Device>,
    effects: Vec<EffectState>,
    applied: Option<Lighting>,
    all_off_applied: bool,
    frame: Vec<Color>,
    sent: Vec<Color>,
    last_write: Instant,
    hw_due: Option<Instant>,
    failures: u32,
    error: Option<String>,
}

impl Slot {
    fn id(&self) -> &str {
        &self.dev.info().id
    }
}

pub fn spawn(config: Config) -> (Arc<Shared>, thread::JoinHandle<()>) {
    let (tx, rx) = mpsc::channel();
    let shared = Arc::new(Shared {
        config: Mutex::new(config),
        devices: RwLock::new(Vec::new()),
        ui_visible: AtomicBool::new(false),
        scanning: AtomicBool::new(true),
        notices: Mutex::new(Vec::new()),
        tx: Mutex::new(Some(tx)),
    });
    let s2 = shared.clone();
    let h = thread::Builder::new()
        .name("rjb-engine".into())
        .spawn(move || Engine::new(s2, rx).run())
        .expect("thread moteur");
    (shared, h)
}

struct Engine {
    shared: Arc<Shared>,
    rx: Receiver<Cmd>,
    slots: Vec<Slot>,
    scanner: Scanner,
    start: Instant,
    last_frame: Instant,
    suspended: bool,
    rescan_at: Option<Instant>,
    save_at: Option<Instant>,
    dead_backoff: Duration,
    /// souris sans fil en veille : nouvelle recherche de plus en plus espacée
    wake_backoff: Duration,
}

const HW_DEBOUNCE: Duration = Duration::from_millis(250);

impl Engine {
    fn new(shared: Arc<Shared>, rx: Receiver<Cmd>) -> Self {
        let internal = shared.config.lock().unwrap().internal_devices;
        let scanner = Scanner::new(internal);
        *shared.notices.lock().unwrap() = scanner.notices();
        Engine {
            shared,
            rx,
            slots: Vec::new(),
            scanner,
            start: Instant::now(),
            last_frame: Instant::now(),
            suspended: false,
            rescan_at: None,
            save_at: None,
            dead_backoff: Duration::from_secs(3),
            wake_backoff: Duration::from_secs(20),
        }
    }

    fn run(mut self) {
        self.rescan();
        loop {
            let timeout = self.next_wake();
            match self.rx.recv_timeout(timeout) {
                Ok(cmd) => {
                    if !self.handle(cmd) {
                        return;
                    }
                    // traite les commandes en rafale avant de redessiner
                    while let Ok(cmd) = self.rx.try_recv() {
                        if !self.handle(cmd) {
                            return;
                        }
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            let now = Instant::now();
            if self.rescan_at.is_some_and(|t| now >= t) {
                self.rescan_at = None;
                self.rescan();
            }
            if self.save_at.is_some_and(|t| now >= t) {
                self.save_at = None;
                self.shared.save_config();
            }
            self.frame();
        }
    }

    fn next_wake(&self) -> Duration {
        let now = Instant::now();
        let mut wake = now + Duration::from_secs(3600);
        if !self.suspended {
            let fps = self.shared.config.lock().unwrap().fps.max(1);
            let dt = Duration::from_secs_f32(1.0 / fps as f32);
            for s in &self.slots {
                if s.effects.iter().any(|e| e.spec.kind.animated()) && !self.all_off() {
                    wake = wake.min(self.last_frame + dt);
                }
                if let Some(t) = s.hw_due {
                    wake = wake.min(t);
                }
                // renvoi périodique seulement si rJb pilote les LED (effet ou extinction)
                if let Some(k) = s.dev.keepalive().filter(|_| !s.effects.is_empty() || s.all_off_applied) {
                    wake = wake.min(s.last_write + k);
                }
            }
        }
        for t in [self.rescan_at, self.save_at].into_iter().flatten() {
            wake = wake.min(t);
        }
        wake.saturating_duration_since(now)
    }

    fn all_off(&self) -> bool {
        self.shared.config.lock().unwrap().all_off
    }

    /// Retourne false pour quitter.
    fn handle(&mut self, cmd: Cmd) -> bool {
        match cmd {
            Cmd::Changed(id) => {
                if let Some(i) = self.slots.iter().position(|s| s.id() == id) {
                    self.sync_slot(i);
                }
                self.save_at = Some(Instant::now() + Duration::from_secs(2));
            }
            Cmd::ReloadAll => {
                for i in 0..self.slots.len() {
                    self.slots[i].all_off_applied = false;
                    self.slots[i].applied = None;
                    self.sync_slot(i);
                }
                self.save_at = Some(Instant::now() + Duration::from_secs(2));
            }
            Cmd::Rescan => {
                self.scanner.forget_negatives();
                self.dead_backoff = Duration::from_secs(3);
                self.wake_backoff = Duration::from_secs(20);
                self.rescan();
            }
            Cmd::Hotplug => {
                self.dead_backoff = Duration::from_secs(3);
                self.rescan();
            }
            Cmd::RefreshPreview => self.publish(),
            Cmd::Resize { id, zone, leds } => {
                if let Some(i) = self.slots.iter().position(|s| s.id() == id) {
                    match self.slots[i].dev.resize_zone(zone, leds as usize) {
                        Ok(()) => {
                            self.shared
                                .config
                                .lock()
                                .unwrap()
                                .zone_sizes
                                .entry(id)
                                .or_default()
                                .insert(zone, leds);
                            self.slots[i].applied = None;
                            self.sync_slot(i);
                            self.publish();
                            self.save_at = Some(Instant::now() + Duration::from_secs(2));
                        }
                        Err(e) => self.slots[i].error = Some(e.0),
                    }
                }
            }
            Cmd::Suspend(on) => {
                self.suspended = on;
                let off = self.shared.config.lock().unwrap().off_on_sleep;
                if on && off {
                    for s in &mut self.slots {
                        blackout(s);
                    }
                } else if !on {
                    // au réveil les appareils ont souvent perdu leur état
                    for s in &mut self.slots {
                        s.applied = None;
                        s.sent.clear();
                    }
                    for i in 0..self.slots.len() {
                        self.sync_slot(i);
                    }
                    self.rescan_at = Some(Instant::now() + Duration::from_secs(3));
                }
            }
            Cmd::Quit => {
                self.shared.save_config();
                let off = self.shared.config.lock().unwrap().off_on_shutdown;
                for s in &mut self.slots {
                    if off {
                        blackout(s);
                    }
                    s.dev.release();
                }
                return false;
            }
        }
        true
    }

    /// Relit la configuration d'un appareil et prépare ses effets.
    fn sync_slot(&mut self, i: usize) {
        let cfg = self.shared.config.lock().unwrap();
        let id = self.slots[i].id().to_string();
        let disabled = cfg.disabled.contains(&id);
        let lighting = cfg
            .lighting(&id)
            .cloned()
            .unwrap_or_else(|| default_lighting(self.slots[i].dev.info()));
        let all_off = cfg.all_off;
        let gbright = cfg.brightness;
        drop(cfg);

        let slot = &mut self.slots[i];
        if disabled {
            slot.effects.clear();
            return;
        }
        if all_off {
            if !slot.all_off_applied {
                blackout(slot);
                slot.all_off_applied = true;
            }
            slot.effects.clear();
            slot.applied = None;
            return;
        }
        slot.all_off_applied = false;

        match &lighting {
            Lighting::Software { zones } if slot.dev.info().direct => {
                let nz = slot.dev.info().zones.len();
                slot.effects.truncate(nz);
                for z in 0..nz {
                    let mut spec = Lighting::zone_spec(zones, z);
                    spec.brightness *= gbright;
                    if z < slot.effects.len() {
                        slot.effects[z].update(&spec);
                    } else {
                        slot.effects.push(EffectState::new(spec));
                    }
                }
                if !matches!(slot.applied, Some(Lighting::Software { .. })) {
                    slot.sent.clear(); // force une écriture complète
                }
            }
            Lighting::Software { .. } => {
                // appareil sans mode direct : on tente l'équivalent matériel
                slot.effects.clear();
                if slot.applied.as_ref() != Some(&lighting) {
                    slot.hw_due = Some(Instant::now());
                }
            }
            Lighting::Hardware { .. } => {
                slot.effects.clear();
                if slot.applied.as_ref() != Some(&lighting) {
                    slot.hw_due = Some(Instant::now() + HW_DEBOUNCE);
                }
            }
        }
        slot.applied = Some(lighting);
    }

    fn frame(&mut self) {
        if self.suspended {
            return;
        }
        let now = Instant::now();
        let t = (now - self.start).as_secs_f64();
        let dt = (now - self.last_frame).as_secs_f32().min(0.25);
        self.last_frame = now;
        let gbright = self.shared.config.lock().unwrap().brightness;
        let mut failed = false;
        let mut preview_dirty = false;

        for slot in &mut self.slots {
            // modes matériels (avec anti-rebond pour ménager l'EEPROM)
            if let Some(due) = slot.hw_due {
                if now >= due {
                    slot.hw_due = None;
                    if let Some(l) = slot.applied.clone() {
                        let r = apply_hardware(slot.dev.as_mut(), &l, gbright);
                        record(slot, r, &mut failed);
                        preview_dirty = true;
                    }
                }
                continue;
            }
            if slot.effects.is_empty() {
                // tout éteint : certains appareils reprennent leur effet interne
                // s'ils ne reçoivent plus rien, on leur renvoie du noir
                if slot.all_off_applied && slot.dev.info().direct {
                    if let Some(k) = slot.dev.keepalive() {
                        if now - slot.last_write >= k {
                            let n = slot.dev.info().led_count();
                            let r = slot.dev.set_leds(&vec![Color::BLACK; n]);
                            slot.last_write = now;
                            record(slot, r, &mut failed);
                        }
                    }
                }
                continue;
            }
            let info = slot.dev.info();
            let n = info.led_count();
            slot.frame.resize(n, Color::BLACK);
            let mut off = 0;
            for (z, zone) in info.zones.iter().enumerate() {
                let len = zone.leds.len();
                if let Some(e) = slot.effects.get_mut(z) {
                    e.render(t, dt, &zone.leds, &mut slot.frame[off..off + len]);
                }
                off += len;
            }
            let min_dt = Duration::from_secs_f32(1.0 / slot.dev.max_fps().max(1) as f32);
            let keepalive_due = slot
                .dev
                .keepalive()
                .is_some_and(|k| now - slot.last_write >= k);
            let changed = slot.frame != slot.sent;
            if (changed && now - slot.last_write >= min_dt) || keepalive_due {
                let r = slot.dev.set_leds(&slot.frame);
                if r.is_ok() {
                    slot.sent.clone_from(&slot.frame);
                }
                slot.last_write = now;
                record(slot, r, &mut failed);
                preview_dirty = true;
            }
        }

        if failed {
            self.drop_dead();
        }
        if preview_dirty && self.shared.ui_visible.load(Ordering::Relaxed) {
            self.publish_colors();
        }
    }

    fn drop_dead(&mut self) {
        let before = self.slots.len();
        self.slots.retain(|s| s.failures < 3);
        if self.slots.len() != before {
            self.publish();
            // un appareil qui échoue en boucle (tenu par un autre logiciel) ne
            // doit pas provoquer une recherche toutes les 3 s : délai croissant
            self.rescan_at = Some(Instant::now() + self.dead_backoff);
            self.dead_backoff = (self.dead_backoff * 2).min(Duration::from_secs(300));
        }
    }

    fn rescan(&mut self) {
        self.shared.scanning.store(true, Ordering::Relaxed);
        let open: HashSet<String> = self
            .slots
            .iter()
            .map(|s| s.dev.info().location.clone())
            .collect();
        let open_ids: HashSet<String> = self.slots.iter().map(|s| s.id().to_string()).collect();
        let found = self.scanner.scan(&open, &open_ids);
        // retire les appareils débranchés
        self.slots
            .retain(|s| found.present.contains(&s.dev.info().location));
        let sizes = self.shared.config.lock().unwrap().zone_sizes.clone();
        for mut dev in found.new_devices {
            if let Some(z) = sizes.get(&dev.info().id) {
                for (&zone, &n) in z {
                    let _ = dev.resize_zone(zone, n as usize);
                }
            }
            self.slots.push(Slot {
                dev,
                effects: Vec::new(),
                applied: None,
                all_off_applied: false,
                frame: Vec::new(),
                sent: Vec::new(),
                last_write: Instant::now() - Duration::from_secs(1),
                hw_due: None,
                failures: 0,
                error: None,
            });
            let i = self.slots.len() - 1;
            self.sync_slot(i);
        }
        self.slots.sort_by(|a, b| {
            let (a, b) = (a.dev.info(), b.dev.info());
            (a.kind as u8, &a.name).cmp(&(b.kind as u8, &b.name))
        });
        *self.shared.notices.lock().unwrap() = self.scanner.notices();
        // récepteur Logitech muet : la souris dort, on la cherchera à son réveil
        if self.scanner.retry_soon() {
            if self.rescan_at.is_none() {
                self.rescan_at = Some(Instant::now() + self.wake_backoff);
                self.wake_backoff = (self.wake_backoff * 2).min(Duration::from_secs(600));
            }
        } else {
            self.wake_backoff = Duration::from_secs(20);
        }
        self.publish();
        self.shared.scanning.store(false, Ordering::Relaxed);
    }

    fn publish(&self) {
        let views = self
            .slots
            .iter()
            .map(|s| DeviceView {
                info: s.dev.info().clone(),
                colors: preview_colors(s),
                error: s.error.clone(),
            })
            .collect();
        *self.shared.devices.write().unwrap() = views;
        crate::wake_ui();
    }

    fn publish_colors(&self) {
        let mut v = self.shared.devices.write().unwrap();
        for (view, s) in v.iter_mut().zip(&self.slots) {
            view.colors = preview_colors(s);
            view.error.clone_from(&s.error);
        }
    }
}

fn record(slot: &mut Slot, r: crate::device::Result<()>, failed: &mut bool) {
    match r {
        Ok(()) => {
            slot.failures = 0;
            slot.error = None;
        }
        Err(e) => {
            slot.failures += 1;
            slot.error = Some(e.0);
            if slot.failures >= 3 {
                *failed = true;
            }
        }
    }
}

fn preview_colors(s: &Slot) -> Vec<Color> {
    let n = s.dev.info().led_count();
    if !s.effects.is_empty() && s.sent.len() == n {
        return s.sent.clone();
    }
    let c = match &s.applied {
        Some(Lighting::Hardware { params, mode }) => {
            let off = s
                .dev
                .info()
                .modes
                .iter()
                .find(|m| &m.name == mode)
                .is_some_and(|m| m.is_off);
            if off {
                Color::BLACK
            } else {
                params.color(0)
            }
        }
        Some(Lighting::Software { zones }) => zones.first().map(|z| z.colors.first().copied().unwrap_or(Color::WHITE)).unwrap_or(Color::WHITE),
        None => Color::BLACK,
    };
    vec![c; n]
}

/// Éclairage proposé à la découverte d'un appareil.
pub fn default_lighting(info: &DeviceInfo) -> Lighting {
    if info.direct {
        Lighting::default()
    } else {
        let m = info
            .modes
            .iter()
            .find(|m| !m.is_off)
            .map(|m| m.name.clone())
            .unwrap_or_default();
        Lighting::Hardware {
            mode: m,
            params: HwParams::default(),
        }
    }
}

fn apply_hardware(dev: &mut dyn Device, l: &Lighting, gbright: f32) -> crate::device::Result<()> {
    let info = dev.info();
    let (name, mut params) = match l {
        Lighting::Hardware { mode, params } => (mode.clone(), params.clone()),
        Lighting::Software { zones } => {
            // équivalent matériel le plus proche d'un effet logiciel
            let s = Lighting::zone_spec(zones, 0);
            let wanted = match s.kind {
                EffectKind::Static | EffectKind::Custom | EffectKind::Gradient => &["Statique", "Fixe", "Static"][..],
                EffectKind::Breathing => &["Respiration", "Breathing"][..],
                EffectKind::Off => &["Éteint", "Off"][..],
                EffectKind::Strobe => &["Flash", "Stroboscope"][..],
                _ => &["Arc-en-ciel", "Cycle de couleurs", "Vague", "Spectrum"][..],
            };
            let name = info
                .modes
                .iter()
                .find(|m| wanted.iter().any(|w| m.name.eq_ignore_ascii_case(w)))
                .or(info.modes.first())
                .map(|m| m.name.clone())
                .unwrap_or_default();
            (name, spec_to_params(&s))
        }
    };
    let Some(idx) = info.modes.iter().position(|m| m.name == name) else {
        return Ok(());
    };
    params.brightness *= gbright;
    if !info.modes[idx].brightness && gbright < 0.999 {
        // pas de luminosité matérielle : on l'applique aux couleurs
        for c in &mut params.colors {
            *c = c.scale(gbright);
        }
    }
    dev.set_mode(idx, &params)
}

fn spec_to_params(s: &EffectSpec) -> HwParams {
    HwParams {
        speed: s.speed,
        brightness: s.brightness,
        colors: if s.colors.is_empty() { vec![Color::WHITE] } else { s.colors.clone() },
        random: s.random,
        direction: s.direction,
    }
}

/// Éteint un appareil : noir en direct, sinon son mode « Éteint ».
fn blackout(s: &mut Slot) {
    let info = s.dev.info();
    if info.direct {
        let n = info.led_count();
        let _ = s.dev.set_leds(&vec![Color::BLACK; n]);
        s.sent = vec![Color::BLACK; n];
    } else if let Some(i) = info.modes.iter().position(|m| m.is_off) {
        let _ = s.dev.set_mode(i, &HwParams::default());
    } else if let Some(i) = info.modes.iter().position(|m| m.colors.0 >= 1) {
        let p = HwParams {
            colors: vec![Color::BLACK],
            ..Default::default()
        };
        let _ = s.dev.set_mode(i, &p);
    }
}
