// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Effets calculés par rJb (mode direct), communs à tous les appareils.

use crate::model::{Color, Direction, Led};
use serde::{Deserialize, Serialize};
use std::f32::consts::TAU;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectKind {
    Static,
    Breathing,
    Spectrum,
    Rainbow,
    ColorWave,
    Gradient,
    Comet,
    Twinkle,
    Strobe,
    Fire,
    Aurora,
    Custom,
    Off,
}

impl EffectKind {
    pub const ALL: [EffectKind; 13] = [
        EffectKind::Static,
        EffectKind::Breathing,
        EffectKind::Spectrum,
        EffectKind::Rainbow,
        EffectKind::ColorWave,
        EffectKind::Gradient,
        EffectKind::Comet,
        EffectKind::Twinkle,
        EffectKind::Strobe,
        EffectKind::Fire,
        EffectKind::Aurora,
        EffectKind::Custom,
        EffectKind::Off,
    ];

    pub fn label(self) -> &'static str {
        match self {
            EffectKind::Static => crate::langue::tr("Couleur fixe"),
            EffectKind::Breathing => crate::langue::tr("Respiration"),
            EffectKind::Spectrum => crate::langue::tr("Cycle de couleurs"),
            EffectKind::Rainbow => crate::langue::tr("Vague arc-en-ciel"),
            EffectKind::ColorWave => crate::langue::tr("Vague de couleurs"),
            EffectKind::Gradient => crate::langue::tr("Dégradé"),
            EffectKind::Comet => crate::langue::tr("Comète"),
            EffectKind::Twinkle => crate::langue::tr("Scintillement"),
            EffectKind::Strobe => crate::langue::tr("Stroboscope"),
            EffectKind::Fire => crate::langue::tr("Feu"),
            EffectKind::Aurora => crate::langue::tr("Aurore"),
            EffectKind::Custom => crate::langue::tr("Personnalisé (LED par LED)"),
            EffectKind::Off => crate::langue::tr("Éteint"),
        }
    }

    pub fn animated(self) -> bool {
        !matches!(
            self,
            EffectKind::Static | EffectKind::Gradient | EffectKind::Custom | EffectKind::Off
        )
    }

    /// Réglages utiles pour cet effet (pour n'afficher que ce qui sert).
    pub fn uses(self) -> Uses {
        use EffectKind::*;
        let u = |speed, colors, direction, size, random| Uses {
            speed,
            colors,
            direction,
            size,
            random,
        };
        match self {
            Static => u(false, (1, 1), false, false, false),
            Breathing => u(true, (1, 8), false, false, true),
            Spectrum => u(true, (0, 0), false, false, false),
            Rainbow => u(true, (0, 0), true, true, false),
            ColorWave => u(true, (2, 8), true, true, false),
            Gradient => u(false, (2, 8), true, false, false),
            Comet => u(true, (1, 2), true, true, true),
            Twinkle => u(true, (1, 8), false, true, true),
            Strobe => u(true, (1, 8), false, false, true),
            Fire => u(true, (0, 0), true, true, false),
            Aurora => u(true, (2, 8), true, true, false),
            Custom => u(false, (1, 1), false, false, false),
            Off => u(false, (0, 0), false, false, false),
        }
    }
}

pub struct Uses {
    pub speed: bool,
    pub colors: (u8, u8),
    pub direction: bool,
    pub size: bool,
    pub random: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EffectSpec {
    pub kind: EffectKind,
    /// 0 = lent, 1 = rapide
    pub speed: f32,
    /// 0..1
    pub brightness: f32,
    pub colors: Vec<Color>,
    pub direction: Direction,
    /// Taille / densité / longueur selon l'effet, 0..1
    pub size: f32,
    pub random: bool,
    /// Couleurs LED par LED pour l'effet Personnalisé.
    pub custom: Vec<Color>,
}

impl Default for EffectSpec {
    fn default() -> Self {
        EffectSpec {
            kind: EffectKind::Rainbow,
            speed: 0.35,
            brightness: 1.0,
            colors: vec![
                Color::rgb(255, 0, 90),
                Color::rgb(0, 140, 255),
                Color::rgb(0, 255, 140),
            ],
            direction: Direction::Right,
            size: 0.4,
            random: false,
            custom: Vec::new(),
        }
    }
}

impl EffectSpec {
    pub fn solid(c: Color) -> Self {
        EffectSpec {
            kind: EffectKind::Static,
            colors: vec![c],
            ..Default::default()
        }
    }

    /// Fréquence en cycles par seconde (0,05 à 2 Hz).
    fn rate(&self) -> f32 {
        0.05 * 40f32.powf(self.speed.clamp(0.0, 1.0))
    }
}

/// Instance d'effet avec son état interne (scintillement, feu…).
pub struct EffectState {
    pub spec: EffectSpec,
    rng: u32,
    levels: Vec<f32>,
    hues: Vec<Color>,
    last_cycle: i64,
    cycle_color: Color,
}

impl EffectState {
    pub fn new(spec: EffectSpec) -> Self {
        EffectState {
            spec,
            rng: 0x9E37_79B9,
            levels: Vec::new(),
            hues: Vec::new(),
            last_cycle: -1,
            cycle_color: Color::WHITE,
        }
    }

    /// Met à jour les réglages en gardant l'état si l'effet ne change pas de type.
    pub fn update(&mut self, spec: &EffectSpec) {
        if spec.kind != self.spec.kind {
            *self = EffectState::new(spec.clone());
        } else if *spec != self.spec {
            self.spec = spec.clone();
        }
    }

    fn rand(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    fn palette_color(&self, i: usize) -> Color {
        let c = &self.spec.colors;
        if c.is_empty() {
            Color::WHITE
        } else {
            c[i % c.len()]
        }
    }

    pub fn render(&mut self, t: f64, dt: f32, leds: &[Led], out: &mut [Color]) {
        let n = leds.len().min(out.len());
        let s = self.spec.clone();
        let rate = s.rate() as f64;
        let phase = (t * rate) as f32;
        let fphase = phase.fract();

        match s.kind {
            EffectKind::Off => out[..n].fill(Color::BLACK),
            EffectKind::Static => out[..n].fill(self.palette_color(0)),
            EffectKind::Custom => {
                for i in 0..n {
                    out[i] = s.custom.get(i).copied().unwrap_or(self.palette_color(0));
                }
            }
            EffectKind::Breathing => {
                let cycle = phase.floor() as i64;
                if cycle != self.last_cycle {
                    self.last_cycle = cycle;
                    self.cycle_color = if s.random {
                        Color::from_hsv(self.rand() * 360.0, 1.0, 1.0)
                    } else {
                        self.palette_color(cycle.rem_euclid(s.colors.len().max(1) as i64) as usize)
                    };
                }
                let v = 0.5 - 0.5 * (fphase * TAU).cos();
                out[..n].fill(self.cycle_color.scale(v * v));
            }
            EffectKind::Spectrum => {
                out[..n].fill(Color::from_hsv(fphase * 360.0, 1.0, 1.0));
            }
            EffectKind::Rainbow => {
                let reps = 0.25 + s.size * 3.0;
                for i in 0..n {
                    let u = coord(&leds[i], s.direction);
                    out[i] = Color::from_hsv((u * reps - fphase) * 360.0, 1.0, 1.0);
                }
            }
            EffectKind::ColorWave => {
                let reps = 0.25 + s.size * 3.0;
                for i in 0..n {
                    let u = coord(&leds[i], s.direction);
                    out[i] = palette(&s.colors, (u * reps - fphase).rem_euclid(1.0), true);
                }
            }
            EffectKind::Gradient => {
                for i in 0..n {
                    out[i] = palette(&s.colors, coord(&leds[i], s.direction), false);
                }
            }
            EffectKind::Comet => {
                let len = 0.05 + s.size * 0.6;
                let head = fphase * (1.0 + len);
                let cycle = phase.floor() as i64;
                if cycle != self.last_cycle {
                    self.last_cycle = cycle;
                    self.cycle_color = if s.random {
                        Color::from_hsv(self.rand() * 360.0, 1.0, 1.0)
                    } else {
                        self.palette_color(0)
                    };
                }
                let bg = if s.colors.len() > 1 && !s.random {
                    s.colors[1]
                } else {
                    Color::BLACK
                };
                for i in 0..n {
                    let u = coord(&leds[i], s.direction);
                    let d = head - u;
                    let k = if (0.0..=len).contains(&d) {
                        let k = 1.0 - d / len;
                        k * k
                    } else {
                        0.0
                    };
                    out[i] = Color::lerp(bg, self.cycle_color, k);
                }
            }
            EffectKind::Twinkle => {
                if self.levels.len() != n {
                    self.levels = vec![0.0; n];
                    self.hues = vec![Color::BLACK; n];
                }
                let density = 0.2 + s.size * 3.0;
                let decay = (s.rate() * 3.0 * dt).min(1.0);
                for i in 0..n {
                    self.levels[i] = (self.levels[i] - decay).max(0.0);
                    if self.levels[i] <= 0.0 && self.rand() < density * s.rate() * dt {
                        self.levels[i] = 1.0;
                        let k = (self.rand() * 8.0) as usize;
                        self.hues[i] = if s.random {
                            Color::from_hsv(self.rand() * 360.0, 1.0, 1.0)
                        } else {
                            self.palette_color(k)
                        };
                    }
                    let l = self.levels[i];
                    out[i] = self.hues[i].scale(l * l);
                }
            }
            EffectKind::Strobe => {
                let cycle = phase.floor() as i64;
                if cycle != self.last_cycle {
                    self.last_cycle = cycle;
                    self.cycle_color = if s.random {
                        Color::from_hsv(self.rand() * 360.0, 1.0, 1.0)
                    } else {
                        self.palette_color(cycle.rem_euclid(s.colors.len().max(1) as i64) as usize)
                    };
                }
                let on = fphase < 0.12;
                out[..n].fill(if on { self.cycle_color } else { Color::BLACK });
            }
            EffectKind::Fire => {
                let scale = 2.0 + s.size * 8.0;
                let tt = (t * rate * 4.0) as f32;
                for i in 0..n {
                    let u = coord(&leds[i], s.direction);
                    let w = coord(&leds[i], perpendicular(s.direction));
                    let heat = noise(w * scale, u * scale - tt) * (1.15 - u);
                    out[i] = fire_color(heat.clamp(0.0, 1.0));
                }
            }
            EffectKind::Aurora => {
                let scale = 1.0 + s.size * 4.0;
                let tt = (t * rate) as f32;
                for i in 0..n {
                    let u = coord(&leds[i], s.direction);
                    let w = coord(&leds[i], perpendicular(s.direction));
                    let a = noise(u * scale + tt * 0.7, w * 2.0 - tt * 0.3);
                    let b = noise(u * scale * 0.5 - tt * 0.4, w + 7.0);
                    let c = palette(&s.colors, (a + 0.5 * b).rem_euclid(1.0), true);
                    out[i] = c.scale(0.25 + 0.75 * b);
                }
            }
        }

        if s.brightness < 0.999 {
            for c in &mut out[..n] {
                *c = c.scale(s.brightness);
            }
        }
    }
}

fn coord(l: &Led, d: Direction) -> f32 {
    match d {
        Direction::Right => l.x,
        Direction::Left => 1.0 - l.x,
        Direction::Down => l.y,
        Direction::Up => 1.0 - l.y,
        Direction::Out | Direction::In => {
            let (dx, dy) = (l.x - 0.5, l.y - 0.5);
            let r = ((dx * dx + dy * dy).sqrt() * std::f32::consts::SQRT_2).min(1.0);
            if d == Direction::Out {
                r
            } else {
                1.0 - r
            }
        }
    }
}

fn perpendicular(d: Direction) -> Direction {
    match d {
        Direction::Right | Direction::Left => Direction::Down,
        Direction::Down | Direction::Up => Direction::Right,
        Direction::Out | Direction::In => Direction::Right,
    }
}

/// Interpolation dans une palette, cyclique ou non.
pub fn palette(colors: &[Color], u: f32, wrap: bool) -> Color {
    match colors.len() {
        0 => Color::WHITE,
        1 => colors[0],
        n => {
            let segs = if wrap { n } else { n - 1 } as f32;
            let p = u.clamp(0.0, 1.0) * segs;
            let i = (p.floor() as usize).min(segs as usize - 1);
            let a = colors[i % n];
            let b = colors[(i + 1) % n];
            Color::lerp(a, b, p - i as f32)
        }
    }
}

fn hash(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    (h ^ (h >> 16)) as f32 / u32::MAX as f32
}

/// Bruit de valeur 2D lissé, 0..1
fn noise(x: f32, y: f32) -> f32 {
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (xf, yf) = (x - xi as f32, y - yi as f32);
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (u, v) = (s(xf), s(yf));
    let a = hash(xi, yi);
    let b = hash(xi + 1, yi);
    let c = hash(xi, yi + 1);
    let d = hash(xi + 1, yi + 1);
    let top = a + (b - a) * u;
    let bot = c + (d - c) * u;
    top + (bot - top) * v
}

fn fire_color(h: f32) -> Color {
    // noir -> rouge -> orange -> jaune
    if h < 0.35 {
        Color::lerp(Color::BLACK, Color::rgb(200, 10, 0), h / 0.35)
    } else if h < 0.7 {
        Color::lerp(Color::rgb(200, 10, 0), Color::rgb(255, 110, 0), (h - 0.35) / 0.35)
    } else {
        Color::lerp(Color::rgb(255, 110, 0), Color::rgb(255, 220, 60), (h - 0.7) / 0.3)
    }
}
