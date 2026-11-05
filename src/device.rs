// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Interface commune à tous les pilotes.

use crate::model::{Color, DeviceInfo, HwParams};
use std::fmt;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<hidapi::HidError> for Error {
    fn from(e: hidapi::HidError) -> Self {
        Error(e.to_string())
    }
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error(s)
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Error(s.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn err<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error(msg.into()))
}

pub trait Device: Send {
    fn info(&self) -> &DeviceInfo;

    /// Active un effet matériel (index dans `info().modes`).
    fn set_mode(&mut self, mode: usize, p: &HwParams) -> Result<()>;

    /// Écrit la couleur de toutes les LED, dans l'ordre des zones.
    /// Le pilote passe lui-même en mode direct si nécessaire.
    fn set_leds(&mut self, colors: &[Color]) -> Result<()>;

    /// Change le nombre de LED d'une zone redimensionnable (connecteurs ARGB).
    fn resize_zone(&mut self, _zone: usize, _leds: usize) -> Result<()> {
        err("zone non redimensionnable")
    }

    /// Certains contrôleurs reprennent leur effet interne si on n'écrit plus :
    /// le moteur renvoie alors la dernière image à cet intervalle.
    fn keepalive(&self) -> Option<Duration> {
        None
    }

    /// Cadence maximale supportée en mode direct.
    fn max_fps(&self) -> u32 {
        60
    }

    /// Appelé à la fermeture : rend la main au firmware si besoin.
    fn release(&mut self) {}
}
