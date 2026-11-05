// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Configuration persistante : profils, éclairage par appareil, options.

use crate::effects::EffectSpec;
use crate::model::HwParams;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "source")]
pub enum Lighting {
    /// Effet calculé par rJb, un par zone.
    Software { zones: Vec<EffectSpec> },
    /// Effet intégré à l'appareil.
    Hardware { mode: String, params: HwParams },
}

impl Default for Lighting {
    fn default() -> Self {
        Lighting::Software {
            zones: vec![EffectSpec::default()],
        }
    }
}

impl Lighting {
    /// Effet de la zone `i` (la première sert de modèle si la liste est courte).
    pub fn zone_spec(zones: &[EffectSpec], i: usize) -> EffectSpec {
        zones
            .get(i)
            .or(zones.first())
            .cloned()
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub devices: BTreeMap<String, Lighting>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub fps: u32,
    pub brightness: f32,
    pub autostart: bool,
    pub start_minimized: bool,
    pub off_on_sleep: bool,
    pub off_on_shutdown: bool,
    pub all_off: bool,
    pub active_profile: String,
    pub profiles: BTreeMap<String, Profile>,
    /// Nombre de LED choisi pour les zones redimensionnables, par appareil.
    pub zone_sizes: BTreeMap<String, BTreeMap<usize, u16>>,
    /// Appareils ignorés par l'utilisateur.
    pub disabled: BTreeSet<String>,
    /// Accès aux composants internes (RAM, carte mère) via PawnIO.
    pub internal_devices: bool,
    /// Fermer la fenêtre la réduit dans la zone de notification (sinon rJb quitte).
    pub close_to_tray: bool,
}

impl Default for Config {
    fn default() -> Self {
        let mut profiles = BTreeMap::new();
        profiles.insert("Par défaut".to_string(), Profile::default());
        Config {
            fps: 30,
            brightness: 1.0,
            autostart: false,
            start_minimized: true,
            off_on_sleep: true,
            off_on_shutdown: false,
            all_off: false,
            active_profile: "Par défaut".to_string(),
            profiles,
            zone_sizes: BTreeMap::new(),
            disabled: BTreeSet::new(),
            internal_devices: true,
            // choix fait dans l'installateur, pour les nouveaux utilisateurs
            close_to_tray: crate::sys::reg_get_dword(
                windows_sys::Win32::System::Registry::HKEY_LOCAL_MACHINE,
                crate::install::APP_KEY,
                "CloseToTray",
            )
            .is_none_or(|v| v != 0),
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        // Mode portable : un config.json à côté de l'exécutable est prioritaire.
        if let Ok(exe) = std::env::current_exe() {
            let p = exe.with_file_name("config.json");
            if p.exists() {
                return p;
            }
        }
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("rJb").join("config.json")
    }

    pub fn load() -> Config {
        std::fs::read(Self::path())
            .ok()
            .and_then(|b| serde_json::from_slice::<Config>(&b).ok())
            .unwrap_or_default()
            .sanitized()
    }

    /// Remet dans les bornes les valeurs d'un fichier venu d'ailleurs.
    fn sanitized(mut self) -> Config {
        let c = &mut self;
        c.profiles.retain(|n, _| !n.trim().is_empty() && n.chars().count() <= 64);
        if c.profiles.is_empty() {
            c.profiles.insert("Par défaut".into(), Profile::default());
        }
        if !c.profiles.contains_key(&c.active_profile) {
            c.active_profile = c.profiles.keys().next().unwrap().clone();
        }
        c.fps = c.fps.clamp(5, 144);
        c.brightness = if c.brightness.is_finite() { c.brightness.clamp(0.0, 1.0) } else { 1.0 };
        self
    }

    /// Enregistre une copie de la configuration à l'emplacement choisi.
    pub fn export_to(&self, path: &std::path::Path) -> Result<(), String> {
        let json = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| format!("Écriture impossible : {e}"))
    }

    /// Lit une configuration exportée. Le fichier est validé avant d'être
    /// utilisé ; les options propres à cette machine (démarrage automatique)
    /// ne sont pas reprises.
    pub fn import_from(path: &std::path::Path, current: &Config) -> Result<Config, String> {
        let meta = std::fs::metadata(path).map_err(|e| format!("Lecture impossible : {e}"))?;
        if meta.len() > 8 * 1024 * 1024 {
            return Err("Ce fichier est trop volumineux pour être une configuration rJb.".into());
        }
        let bytes = std::fs::read(path).map_err(|e| format!("Lecture impossible : {e}"))?;
        let mut c: Config = serde_json::from_slice(&bytes).map_err(|e| format!("Fichier invalide : {e}"))?;
        c.autostart = current.autostart;
        Ok(c.sanitized())
    }

    pub fn save(&self) {
        let p = Self::path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_vec_pretty(self) {
            // écriture atomique : fichier temporaire puis renommage
            let tmp = p.with_extension("json.tmp");
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(&tmp, &p);
            }
        }
    }

    pub fn profile(&self) -> &Profile {
        &self.profiles[&self.active_profile]
    }

    pub fn profile_mut(&mut self) -> &mut Profile {
        self.profiles.get_mut(&self.active_profile).unwrap()
    }

    pub fn lighting(&self, id: &str) -> Option<&Lighting> {
        self.profile().devices.get(id)
    }
}

