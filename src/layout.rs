// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Disposition physique d'un clavier complet, partagée par les pilotes.
//! Chaque pilote associe un nom de touche à son identifiant sur le fil.

use crate::model::{Led, Zone};

/// (nom, x, y, largeur) en unités de touche. Les touches hors clavier
/// principal (G-keys, médias, logo) sont placées autour.
pub const KEYS: &[(&str, f32, f32, f32)] = &[
    // rangée F
    ("Échap", 0.0, 0.0, 1.0),
    ("F1", 2.0, 0.0, 1.0),
    ("F2", 3.0, 0.0, 1.0),
    ("F3", 4.0, 0.0, 1.0),
    ("F4", 5.0, 0.0, 1.0),
    ("F5", 6.5, 0.0, 1.0),
    ("F6", 7.5, 0.0, 1.0),
    ("F7", 8.5, 0.0, 1.0),
    ("F8", 9.5, 0.0, 1.0),
    ("F9", 11.0, 0.0, 1.0),
    ("F10", 12.0, 0.0, 1.0),
    ("F11", 13.0, 0.0, 1.0),
    ("F12", 14.0, 0.0, 1.0),
    ("Impr. écran", 15.25, 0.0, 1.0),
    ("Arrêt défil.", 16.25, 0.0, 1.0),
    ("Pause", 17.25, 0.0, 1.0),
    // chiffres
    ("²", 0.0, 1.25, 1.0),
    ("1", 1.0, 1.25, 1.0),
    ("2", 2.0, 1.25, 1.0),
    ("3", 3.0, 1.25, 1.0),
    ("4", 4.0, 1.25, 1.0),
    ("5", 5.0, 1.25, 1.0),
    ("6", 6.0, 1.25, 1.0),
    ("7", 7.0, 1.25, 1.0),
    ("8", 8.0, 1.25, 1.0),
    ("9", 9.0, 1.25, 1.0),
    ("0", 10.0, 1.25, 1.0),
    ("-", 11.0, 1.25, 1.0),
    ("=", 12.0, 1.25, 1.0),
    ("Retour arrière", 13.0, 1.25, 2.0),
    ("Inser", 15.25, 1.25, 1.0),
    ("Origine", 16.25, 1.25, 1.0),
    ("Page préc.", 17.25, 1.25, 1.0),
    ("Verr. num", 18.5, 1.25, 1.0),
    ("Num /", 19.5, 1.25, 1.0),
    ("Num *", 20.5, 1.25, 1.0),
    ("Num -", 21.5, 1.25, 1.0),
    // rangée Q
    ("Tab", 0.0, 2.25, 1.5),
    ("Q", 1.5, 2.25, 1.0),
    ("W", 2.5, 2.25, 1.0),
    ("E", 3.5, 2.25, 1.0),
    ("R", 4.5, 2.25, 1.0),
    ("T", 5.5, 2.25, 1.0),
    ("Y", 6.5, 2.25, 1.0),
    ("U", 7.5, 2.25, 1.0),
    ("I", 8.5, 2.25, 1.0),
    ("O", 9.5, 2.25, 1.0),
    ("P", 10.5, 2.25, 1.0),
    ("[", 11.5, 2.25, 1.0),
    ("]", 12.5, 2.25, 1.0),
    ("\\", 13.5, 2.25, 1.5),
    ("Suppr", 15.25, 2.25, 1.0),
    ("Fin", 16.25, 2.25, 1.0),
    ("Page suiv.", 17.25, 2.25, 1.0),
    ("Num 7", 18.5, 2.25, 1.0),
    ("Num 8", 19.5, 2.25, 1.0),
    ("Num 9", 20.5, 2.25, 1.0),
    ("Num +", 21.5, 2.75, 1.0),
    // rangée A
    ("Verr. maj", 0.0, 3.25, 1.75),
    ("A", 1.75, 3.25, 1.0),
    ("S", 2.75, 3.25, 1.0),
    ("D", 3.75, 3.25, 1.0),
    ("F", 4.75, 3.25, 1.0),
    ("G", 5.75, 3.25, 1.0),
    ("H", 6.75, 3.25, 1.0),
    ("J", 7.75, 3.25, 1.0),
    ("K", 8.75, 3.25, 1.0),
    ("L", 9.75, 3.25, 1.0),
    (";", 10.75, 3.25, 1.0),
    ("'", 11.75, 3.25, 1.0),
    ("#", 12.75, 3.25, 1.0),
    ("Entrée", 13.75, 3.25, 1.75),
    ("Num 4", 18.5, 3.25, 1.0),
    ("Num 5", 19.5, 3.25, 1.0),
    ("Num 6", 20.5, 3.25, 1.0),
    // rangée Z
    ("Maj gauche", 0.0, 4.25, 1.25),
    ("<", 1.25, 4.25, 1.0),
    ("Z", 2.25, 4.25, 1.0),
    ("X", 3.25, 4.25, 1.0),
    ("C", 4.25, 4.25, 1.0),
    ("V", 5.25, 4.25, 1.0),
    ("B", 6.25, 4.25, 1.0),
    ("N", 7.25, 4.25, 1.0),
    ("M", 8.25, 4.25, 1.0),
    (",", 9.25, 4.25, 1.0),
    (".", 10.25, 4.25, 1.0),
    ("/", 11.25, 4.25, 1.0),
    ("Maj droite", 12.25, 4.25, 2.75),
    ("Haut", 16.25, 4.25, 1.0),
    ("Num 1", 18.5, 4.25, 1.0),
    ("Num 2", 19.5, 4.25, 1.0),
    ("Num 3", 20.5, 4.25, 1.0),
    ("Num Entrée", 21.5, 4.75, 1.0),
    // rangée espace
    ("Ctrl gauche", 0.0, 5.25, 1.25),
    ("Windows gauche", 1.25, 5.25, 1.25),
    ("Alt", 2.5, 5.25, 1.25),
    ("Espace", 3.75, 5.25, 6.25),
    ("Alt Gr", 10.0, 5.25, 1.25),
    ("Windows droite", 11.25, 5.25, 1.25),
    ("Fn", 11.25, 5.25, 1.25),
    ("Menu", 12.5, 5.25, 1.25),
    ("Ctrl droite", 13.75, 5.25, 1.25),
    ("Gauche", 15.25, 5.25, 1.0),
    ("Bas", 16.25, 5.25, 1.0),
    ("Droite", 17.25, 5.25, 1.0),
    ("Num 0", 18.5, 5.25, 2.0),
    ("Num .", 20.5, 5.25, 1.0),
    // touches supplémentaires
    ("G1", -1.5, 1.25, 1.0),
    ("G2", -1.5, 2.25, 1.0),
    ("G3", -1.5, 3.25, 1.0),
    ("G4", -1.5, 4.25, 1.0),
    ("G5", -1.5, 5.25, 1.0),
    ("G6", -2.5, 1.25, 1.0),
    ("G7", -2.5, 2.25, 1.0),
    ("G8", -2.5, 3.25, 1.0),
    ("G9", -2.5, 4.25, 1.0),
    ("Logo", 0.0, -1.25, 1.0),
    ("Luminosité", 15.25, -1.25, 1.0),
    ("Jeu", 16.25, -1.25, 1.0),
    ("Précédent", 18.5, -1.25, 1.0),
    ("Lecture/Pause", 19.5, -1.25, 1.0),
    ("Suivant", 20.5, -1.25, 1.0),
    ("Muet", 21.5, -1.25, 1.0),
];

/// Nom de touche (table `KEYS`) pour un code d'usage HID clavier (page 0x07).
pub fn hid_usage_key(u: u16) -> Option<&'static str> {
    const LETTERS: [&str; 26] = [
        "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z",
    ];
    const DIGITS: [&str; 10] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];
    const F: [&str; 12] = ["F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12"];
    const NUM: [&str; 10] = ["Num 1", "Num 2", "Num 3", "Num 4", "Num 5", "Num 6", "Num 7", "Num 8", "Num 9", "Num 0"];
    Some(match u {
        0x04..=0x1D => LETTERS[(u - 0x04) as usize],
        0x1E..=0x27 => DIGITS[(u - 0x1E) as usize],
        0x28 => "Entrée",
        0x29 => "Échap",
        0x2A => "Retour arrière",
        0x2B => "Tab",
        0x2C => "Espace",
        0x2D => "-",
        0x2E => "=",
        0x2F => "[",
        0x30 => "]",
        0x31 => "\\",
        0x32 => "#",
        0x33 => ";",
        0x34 => "'",
        0x35 => "²",
        0x36 => ",",
        0x37 => ".",
        0x38 => "/",
        0x39 => "Verr. maj",
        0x3A..=0x45 => F[(u - 0x3A) as usize],
        0x46 => "Impr. écran",
        0x47 => "Arrêt défil.",
        0x48 => "Pause",
        0x49 => "Inser",
        0x4A => "Origine",
        0x4B => "Page préc.",
        0x4C => "Suppr",
        0x4D => "Fin",
        0x4E => "Page suiv.",
        0x4F => "Droite",
        0x50 => "Gauche",
        0x51 => "Bas",
        0x52 => "Haut",
        0x53 => "Verr. num",
        0x54 => "Num /",
        0x55 => "Num *",
        0x56 => "Num -",
        0x57 => "Num +",
        0x58 => "Num Entrée",
        0x59..=0x62 => NUM[(u - 0x59) as usize],
        0x63 => "Num .",
        0x64 => "<",
        0x65 => "Menu",
        0xE0 => "Ctrl gauche",
        0xE1 => "Maj gauche",
        0xE2 => "Alt",
        0xE3 => "Windows gauche",
        0xE4 => "Ctrl droite",
        0xE5 => "Maj droite",
        0xE6 => "Alt Gr",
        0xE7 => "Windows droite",
        _ => return None,
    })
}

/// Libellé affiché pour une touche (AZERTY si Windows est en français).
pub fn display_key(name: &str) -> String {
    if french_layout() {
        azerty(name).to_string()
    } else {
        name.to_string()
    }
}

/// Libellés AZERTY pour l'affichage quand Windows est en clavier français.
fn azerty(name: &str) -> &str {
    match name {
        "Q" => "A",
        "W" => "Z",
        "A" => "Q",
        "Z" => "W",
        ";" => "M",
        "M" => ",",
        "," => ";",
        "." => ":",
        "/" => "!",
        "[" => "^",
        "]" => "$",
        "'" => "ù",
        "#" => "*",
        "-" => ")",
        "1" => "&",
        "2" => "é",
        "3" => "\"",
        "4" => "'",
        "5" => "(",
        "6" => "-",
        "7" => "è",
        "8" => "_",
        "9" => "ç",
        "0" => "à",
        other => other,
    }
}

fn french_layout() -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayout;
    let hkl = unsafe { GetKeyboardLayout(0) } as usize;
    (hkl & 0x3FF) == 0x0C // LANG_FRENCH
}

/// Construit une zone clavier à partir des touches présentes.
/// `keys` : (nom de touche dans `KEYS`, identifiant sur le fil).
/// Retourne la zone et les identifiants dans l'ordre des LED.
/// Les identifiants inconnus de la table sont ajoutés sur une ligne à part.
/// La touche existe-t-elle dans la disposition de référence ?
pub fn is_key(name: &str) -> bool {
    KEYS.iter().any(|e| e.0 == name)
}

pub fn keyboard_zone<T: Copy>(name: &str, keys: &[(&str, T)], extra: &[(String, T)]) -> (Zone, Vec<T>) {
    let fr = french_layout();
    let mut placed: Vec<(String, f32, f32, f32, T)> = Vec::new();
    for &(k, id) in keys {
        if let Some(&(n, x, y, w)) = KEYS.iter().find(|e| e.0 == k) {
            let label = if fr { azerty(n) } else { n };
            placed.push((label.to_string(), x, y, w, id));
        }
    }
    let mut x = 0.0;
    for (n, id) in extra {
        placed.push((n.clone(), x, 6.5, 1.0, *id));
        x += 1.0;
    }
    let (minx, maxx) = placed
        .iter()
        .fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.1), b.max(p.1 + p.3)));
    let (miny, maxy) = placed
        .iter()
        .fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.2), b.max(p.2 + 1.0)));
    let (sw, sh) = ((maxx - minx).max(1.0), (maxy - miny).max(1.0));
    let mut ids = Vec::with_capacity(placed.len());
    let leds = placed
        .into_iter()
        .map(|(n, x, y, w, id)| {
            ids.push(id);
            Led {
                name: n,
                x: (x - minx + w / 2.0) / sw,
                y: (y - miny + 0.5) / sh,
                w,
            }
        })
        .collect();
    (
        Zone {
            name: name.to_string(),
            leds,
            resizable: None,
            grid: Some((sw.ceil() as u16, sh.ceil() as u16)),
        },
        ids,
    )
}
