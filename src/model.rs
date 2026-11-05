// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Types partagés : couleurs, description des appareils, zones, modes.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b }
    }

    pub fn scale(self, f: f32) -> Color {
        let f = f.clamp(0.0, 1.0);
        Color::rgb(
            (self.r as f32 * f + 0.5) as u8,
            (self.g as f32 * f + 0.5) as u8,
            (self.b as f32 * f + 0.5) as u8,
        )
    }

    pub fn lerp(a: Color, b: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t + 0.5) as u8;
        Color::rgb(m(a.r, b.r), m(a.g, b.g), m(a.b, b.b))
    }

    /// h en degrés (0..360), s et v dans 0..1
    pub fn from_hsv(h: f32, s: f32, v: f32) -> Color {
        let h = h.rem_euclid(360.0) / 60.0;
        let c = v * s;
        let x = c * (1.0 - (h % 2.0 - 1.0).abs());
        let (r, g, b) = match h as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = v - c;
        let q = |f: f32| ((f + m) * 255.0 + 0.5) as u8;
        Color::rgb(q(r), q(g), q(b))
    }

    pub fn max_channel(self) -> u8 {
        self.r.max(self.g).max(self.b)
    }

    pub fn hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

impl From<Color> for String {
    fn from(c: Color) -> String {
        c.hex()
    }
}

impl TryFrom<String> for Color {
    type Error = String;
    fn try_from(s: String) -> Result<Self, String> {
        let h = s.trim_start_matches('#');
        if h.len() != 6 {
            return Err(format!("couleur invalide : {s}"));
        }
        let v = u32::from_str_radix(h, 16).map_err(|e| e.to_string())?;
        Ok(Color::rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceKind {
    Keyboard,
    Mouse,
    Mousemat,
    Headset,
    HeadsetStand,
    Speaker,
    Microphone,
    Motherboard,
    Ram,
    Gpu,
    Cooler,
    FanController,
    LedStrip,
    Case,
    Gamepad,
    Light,
    Other,
}

impl DeviceKind {
    pub fn label(self) -> &'static str {
        match self {
            DeviceKind::Keyboard => "Clavier",
            DeviceKind::Mouse => "Souris",
            DeviceKind::Mousemat => "Tapis",
            DeviceKind::Headset => "Casque",
            DeviceKind::HeadsetStand => "Support de casque",
            DeviceKind::Speaker => "Enceinte",
            DeviceKind::Microphone => "Micro",
            DeviceKind::Motherboard => "Carte mère",
            DeviceKind::Ram => "Mémoire",
            DeviceKind::Gpu => "Carte graphique",
            DeviceKind::Cooler => "Refroidissement",
            DeviceKind::FanController => "Contrôleur de ventilateurs",
            DeviceKind::LedStrip => "Bande LED",
            DeviceKind::Case => "Boîtier",
            DeviceKind::Gamepad => "Manette",
            DeviceKind::Light => "Lampe",
            DeviceKind::Other => "Appareil",
        }
    }
}

/// Une LED, avec sa position normalisée (0..1) dans sa zone.
#[derive(Clone, Debug)]
pub struct Led {
    pub name: String,
    pub x: f32,
    pub y: f32,
    /// Largeur relative (touches larges d'un clavier), 1 = standard.
    pub w: f32,
}

#[derive(Clone, Debug)]
pub struct Zone {
    pub name: String,
    pub leds: Vec<Led>,
    /// Pour les connecteurs ARGB : nombre de LED réglable par l'utilisateur.
    pub resizable: Option<(u16, u16)>,
    /// Grille (largeur, hauteur) si la zone est une matrice (clavier) : sert à l'affichage.
    pub grid: Option<(u16, u16)>,
}

impl Zone {
    /// Zone linéaire de `n` LED alignées de gauche à droite.
    pub fn linear(name: impl Into<String>, n: usize) -> Zone {
        Zone {
            name: name.into(),
            leds: linear_leds(n),
            resizable: None,
            grid: None,
        }
    }

    pub fn single(name: impl Into<String>) -> Zone {
        Zone::linear(name, 1)
    }

    pub fn resizable(name: impl Into<String>, n: usize, min: u16, max: u16) -> Zone {
        let mut z = Zone::linear(name, n);
        z.resizable = Some((min, max));
        z
    }
}

/// « G815 RGB MECHANICAL GAMING KEYBOARD » → « G815 RGB Mechanical Gaming Keyboard ».
/// Garde les sigles courts et les mots contenant des chiffres.
pub fn pretty_name(s: &str) -> String {
    let letters: Vec<char> = s.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.is_empty() || letters.iter().any(|c| c.is_lowercase()) {
        return s.trim().to_string();
    }
    s.split_whitespace()
        .map(|w| {
            if w.len() <= 3 || w.chars().any(|c| c.is_ascii_digit()) {
                w.to_string()
            } else {
                let mut c = w.chars();
                let first = c.next().unwrap();
                first.to_string() + &c.as_str().to_lowercase()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn linear_leds(n: usize) -> Vec<Led> {
    (0..n)
        .map(|i| Led {
            name: format!("LED {}", i + 1),
            x: if n <= 1 { 0.5 } else { i as f32 / (n - 1) as f32 },
            y: 0.5,
            w: 1.0,
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    #[default]
    Right,
    Left,
    Down,
    Up,
    Out,
    In,
}

impl Direction {
    pub const ALL: [Direction; 6] = [
        Direction::Right,
        Direction::Left,
        Direction::Down,
        Direction::Up,
        Direction::Out,
        Direction::In,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Direction::Right => "→ Vers la droite",
            Direction::Left => "← Vers la gauche",
            Direction::Down => "↓ Vers le bas",
            Direction::Up => "↑ Vers le haut",
            Direction::Out => "◎ Du centre vers l'extérieur",
            Direction::In => "◉ De l'extérieur vers le centre",
        }
    }
}

/// Un effet géré par le firmware de l'appareil (survit sans l'application).
#[derive(Clone, Debug)]
pub struct ModeInfo {
    pub name: String,
    pub speed: bool,
    pub brightness: bool,
    /// nombre de couleurs (min, max) ; (0,0) = aucune couleur réglable
    pub colors: (u8, u8),
    pub random: bool,
    pub directions: Vec<Direction>,
    /// Ce mode éteint l'appareil.
    pub is_off: bool,
}

impl ModeInfo {
    pub fn new(name: impl Into<String>) -> Self {
        ModeInfo {
            name: name.into(),
            speed: false,
            brightness: false,
            colors: (0, 0),
            random: false,
            directions: Vec::new(),
            is_off: false,
        }
    }
    pub fn speed(mut self) -> Self {
        self.speed = true;
        self
    }
    pub fn brightness(mut self) -> Self {
        self.brightness = true;
        self
    }
    pub fn colors(mut self, min: u8, max: u8) -> Self {
        self.colors = (min, max);
        self
    }
    pub fn random(mut self) -> Self {
        self.random = true;
        self
    }
    pub fn dirs(mut self, d: &[Direction]) -> Self {
        self.directions = d.to_vec();
        self
    }
    pub fn off(mut self) -> Self {
        self.is_off = true;
        self
    }
}

/// Paramètres d'un mode matériel, indépendants de l'appareil
/// (le pilote les convertit dans ses propres unités).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HwParams {
    /// 0 = lent, 1 = rapide
    pub speed: f32,
    /// 0..1
    pub brightness: f32,
    pub colors: Vec<Color>,
    pub random: bool,
    pub direction: Direction,
}

impl Default for HwParams {
    fn default() -> Self {
        HwParams {
            speed: 0.5,
            brightness: 1.0,
            colors: vec![Color::rgb(0, 140, 255)],
            random: false,
            direction: Direction::Right,
        }
    }
}

impl HwParams {
    pub fn color(&self, i: usize) -> Color {
        self.colors
            .get(i)
            .or(self.colors.first())
            .copied()
            .unwrap_or(Color::WHITE)
    }
    /// Convertit la vitesse normalisée vers un intervalle d'entiers du pilote.
    pub fn speed_in(&self, slow: u32, fast: u32) -> u32 {
        let s = self.speed.clamp(0.0, 1.0);
        (slow as f32 + (fast as f32 - slow as f32) * s).round() as u32
    }
    pub fn brightness_in(&self, min: u32, max: u32) -> u32 {
        let b = self.brightness.clamp(0.0, 1.0);
        (min as f32 + (max as f32 - min as f32) * b).round() as u32
    }
}

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    /// Identifiant stable (sert de clé dans la configuration).
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub kind: DeviceKind,
    pub location: String,
    pub zones: Vec<Zone>,
    pub modes: Vec<ModeInfo>,
    /// L'appareil accepte des couleurs LED par LED envoyées par rJb.
    pub direct: bool,
}

impl DeviceInfo {
    pub fn led_count(&self) -> usize {
        self.zones.iter().map(|z| z.leds.len()).sum()
    }
}
