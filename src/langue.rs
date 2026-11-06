// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Langue de l'interface : choix enregistré dans la configuration, sinon langue
//! de Windows, sinon anglais. Le texte français sert de clé (table dans textes.rs).

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

pub const CODES: [&str; 8] = ["fr", "en", "es", "de", "it", "pt", "zh", "ja"];
/// Nom de chaque langue dans sa propre écriture.
pub const NOMS: [&str; 8] = ["Français", "English", "Español", "Deutsch", "Italiano", "Português (Brasil)", "简体中文", "日本語"];

static LANGUE: AtomicUsize = AtomicUsize::new(1);

/// Langue de l'interface de Windows, anglais si elle n'est pas proposée.
pub fn systeme() -> usize {
    let id = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() } & 0x3ff;
    match id {
        0x0c => 0,
        0x0a => 2,
        0x07 => 3,
        0x10 => 4,
        0x16 => 5,
        0x04 => 6,
        0x11 => 7,
        _ => 1,
    }
}

/// `choix` : code enregistré ("" = langue de Windows).
pub fn init(choix: &str) {
    let i = CODES.iter().position(|c| *c == choix).unwrap_or_else(systeme);
    LANGUE.store(i, Ordering::Relaxed);
}

pub fn actuelle() -> usize {
    LANGUE.load(Ordering::Relaxed)
}

pub fn code() -> &'static str {
    CODES[actuelle()]
}

fn table() -> &'static HashMap<&'static str, &'static [&'static str; 8]> {
    static T: OnceLock<HashMap<&'static str, &'static [&'static str; 8]>> = OnceLock::new();
    T.get_or_init(|| crate::textes::TEXTES.iter().map(|t| (t[0], t)).collect())
}

/// Texte connu (littéral du code) dans la langue choisie.
pub fn tr(fr: &str) -> &str {
    let i = actuelle();
    if i == 0 {
        return fr;
    }
    table().get(fr).map_or(fr, |t| t[i])
}

/// Texte avec des valeurs : `trf("Version {v}", &[("v", "1.2")])`.
pub fn trf(fr: &str, vals: &[(&str, &str)]) -> String {
    let mut s = tr(fr).to_string();
    for (k, v) in vals {
        s = s.replace(&format!("{{{k}}}"), v);
    }
    s
}

fn sep() -> &'static str {
    match actuelle() {
        0 => " : ",
        6 | 7 => "：",
        _ => ": ",
    }
}

/// Texte venu des pilotes ou composé à l'exécution (« Ventilateur 2 »,
/// « Mémoire RGB · barrette 1 », « Copie impossible : détail ») : chaque
/// morceau connu est traduit, le reste (noms de produits, détails) est gardé.
pub fn trd(s: &str) -> Cow<'_, str> {
    let i = actuelle();
    if i == 0 || s.is_empty() {
        return Cow::Borrowed(s);
    }
    let t = table();
    if let Some(x) = t.get(s) {
        return Cow::Borrowed(x[i]);
    }
    if s.contains(" · ") {
        return Cow::Owned(s.split(" · ").map(trd).collect::<Vec<_>>().join(" · "));
    }
    if let Some((a, b)) = s.split_once(" : ") {
        return Cow::Owned(format!("{}{}{}", trd(a), sep(), trd(b)));
    }
    if let Some(a) = s.strip_suffix(" (expérimental)") {
        return Cow::Owned(format!("{} {}", trd(a), tr("(expérimental)")));
    }
    // numéro final : « Canal 2 », « Touche 0x1f », « version 1.2.0 »
    if let Some((a, n)) = s.rsplit_once(' ') {
        if n.len() <= 8 && n.chars().any(|c| c.is_ascii_digit()) && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '.') {
            if let Some(x) = t.get(a) {
                return Cow::Owned(format!("{} {n}", x[i]));
            }
        }
    }
    Cow::Borrowed(s)
}
