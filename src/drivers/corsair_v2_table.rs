// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
// Fichier généré à partir de la base Corsair V2 d'OpenRGB (GPL-2.0) — ne pas éditer.

use crate::model::DeviceKind as K;

/// Disposition : (valeurs des touches de base, touches ajoutées (valeur, nom))
pub type Layout = (&'static [u16], &'static [(u16, &'static str)]);

pub const TKL_KEYS: &[u16] = CORSAIR_TKL_VALUES;
pub const FULL_KEYS: &[u16] = CORSAIR_FULL_SIZE_VALUES;
const CORSAIR_TKL_VALUES: &[u16] = &[41, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 53, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 45, 46, 42, 73, 74, 75, 43, 20, 26, 8, 21, 23, 28, 24, 12, 18, 19, 47, 48, 49, 76, 77, 78, 57, 4, 22, 7, 9, 10, 11, 13, 14, 15, 51, 52, 50, 40, 106, 100, 29, 27, 6, 25, 5, 17, 16, 54, 55, 56, 110, 82, 105, 108, 107, 44, 111, 122, 101, 109, 80, 81, 79];
const CORSAIR_FULL_SIZE_VALUES: &[u16] = &[41, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 53, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 45, 46, 42, 73, 74, 75, 83, 84, 85, 86, 43, 20, 26, 8, 21, 23, 28, 24, 12, 18, 19, 47, 48, 49, 76, 77, 78, 95, 96, 97, 87, 57, 4, 22, 7, 9, 10, 11, 13, 14, 15, 51, 52, 50, 40, 92, 93, 94, 106, 100, 29, 27, 6, 25, 5, 17, 16, 54, 55, 56, 110, 82, 89, 90, 91, 88, 105, 108, 107, 44, 111, 122, 101, 109, 80, 81, 79, 98, 99];
const L_CORSAIR_K100_LAYOUT: Layout = (CORSAIR_FULL_SIZE_VALUES, &[(123, "KEY_EN_MEDIA_STOP"), (126, "KEY_EN_MEDIA_PREVIOUS"), (124, "KEY_EN_MEDIA_PLAY_PAUSE"), (125, "KEY_EN_MEDIA_NEXT"), (128, "Profile"), (137, "iCue"), (114, "Lock"), (190, "Logo L"), (191, "Logo M"), (192, "Logo R"), (102, "KEY_EN_MEDIA_MUTE"), (131, "G1"), (132, "G2"), (133, "G3"), (134, "G4"), (135, "G5"), (136, "G6"), (138, "Underglow 1"), (139, "Underglow 2"), (140, "Underglow 3"), (141, "Underglow 4"), (142, "Underglow 5"), (143, "Underglow 6"), (144, "Underglow 7"), (145, "Underglow 8"), (146, "Underglow 9"), (147, "Underglow 10"), (148, "Underglow 11"), (149, "Underglow 12"), (150, "Underglow 13"), (151, "Underglow 14"), (152, "Underglow 15"), (153, "Underglow 16"), (154, "Underglow 17"), (155, "Underglow 18"), (156, "Underglow 19"), (157, "Underglow 20"), (158, "Underglow 21"), (159, "Underglow 22"), (160, "Underglow 23"), (161, "Underglow 24"), (162, "Underglow 25"), (163, "Underglow 26"), (164, "Underglow 27"), (165, "Underglow 28"), (166, "Underglow 29"), (167, "Underglow 30"), (168, "Underglow 31"), (169, "Underglow 32"), (170, "Underglow 33"), (171, "Underglow 34"), (172, "Underglow 35"), (173, "Underglow 36"), (174, "Underglow 37"), (175, "Underglow 38"), (176, "Underglow 39"), (177, "Underglow 40"), (178, "Underglow 41"), (179, "Underglow 42"), (180, "Underglow 43"), (181, "Underglow 44"), (182, "Wheel 1"), (183, "Wheel 2"), (184, "Wheel 3"), (185, "Wheel 4"), (186, "Wheel 5"), (187, "Wheel 6"), (188, "Wheel 7"), (189, "Wheel 8")]);
const L_CORSAIR_K70_CORE_TKL_LAYOUT: Layout = (CORSAIR_TKL_VALUES, &[(70, "KEY_EN_MEDIA_MUTE"), (71, "KEY_EN_UNUSED"), (72, "KEY_EN_MEDIA_VOLUME_UP")]);
const L_CORSAIR_K60_LAYOUT: Layout = (CORSAIR_FULL_SIZE_VALUES, &[]);
const L_CORSAIR_K70_LAYOUT: Layout = (CORSAIR_FULL_SIZE_VALUES, &[]);
const L_CORSAIR_K70_TKL_CS_LAYOUT: Layout = (CORSAIR_TKL_VALUES, &[(123, "KEY_EN_MEDIA_STOP"), (126, "KEY_EN_MEDIA_PREVIOUS"), (124, "KEY_EN_MEDIA_PLAY_PAUSE"), (125, "KEY_EN_MEDIA_NEXT"), (1, "Logo L"), (3, "Logo R"), (128, "Profile"), (113, "Light"), (114, "Lock"), (102, "KEY_EN_MEDIA_MUTE")]);
const L_CORSAIR_K57_LAYOUT: Layout = (CORSAIR_FULL_SIZE_VALUES, &[(1, "Lock/Macro Indicator"), (131, "Key: G1"), (132, "Key: G2"), (133, "Key: G3"), (134, "Key: G4"), (135, "Key: G5"), (136, "Key: G6")]);
const L_CORSAIR_K60_TKL_LAYOUT: Layout = (CORSAIR_TKL_VALUES, &[]);
const L_CORSAIR_K70_PRO_LAYOUT: Layout = (CORSAIR_FULL_SIZE_VALUES, &[(128, "Profile"), (113, "Light"), (114, "Lock"), (138, "Logo"), (102, "KEY_EN_MEDIA_MUTE"), (123, "KEY_EN_MEDIA_STOP"), (126, "KEY_EN_MEDIA_PREVIOUS"), (124, "KEY_EN_MEDIA_PLAY_PAUSE"), (125, "KEY_EN_MEDIA_NEXT"), (140, "KEY_EN_SPACE"), (141, "KEY_EN_SPACE")]);
const L_CORSAIR_K95_LAYOUT: Layout = (CORSAIR_FULL_SIZE_VALUES, &[(112, "KEY_EN_RIGHT_WINDOWS"), (123, "KEY_EN_MEDIA_STOP"), (126, "KEY_EN_MEDIA_PREVIOUS"), (124, "KEY_EN_MEDIA_PLAY_PAUSE"), (125, "KEY_EN_MEDIA_NEXT"), (128, "Profile"), (113, "Light"), (114, "Lock"), (102, "KEY_EN_MEDIA_MUTE"), (131, "G1"), (132, "G2"), (133, "G3"), (134, "G4"), (135, "G5"), (136, "G6"), (137, "Top Bar 1"), (138, "Top Bar 2"), (139, "Top Bar 3"), (140, "Top Bar 4"), (141, "Top Bar 5"), (142, "Top Bar 6"), (143, "Top Bar 7"), (144, "Top Bar 8"), (145, "Top Bar 9"), (146, "Top Bar 10"), (147, "Top Bar 11"), (148, "Top Bar 12"), (149, "Top Bar 13"), (150, "Top Bar 14"), (151, "Top Bar 15"), (152, "Top Bar 16"), (153, "Top Bar 17"), (154, "Top Bar 18"), (155, "Top Bar 19")]);

/// (PID, type, zones (nom, type, lignes, colonnes), disposition clavier)
#[allow(clippy::type_complexity)]
pub const DEVICES: &[(u16, K, &[(&str, u8, u8, u8)], Option<Layout>)] = &[
    (0x1B4B, K::Mouse, &[("Scroll Wheel", 0, 1, 1), ("Side Buttons", 1, 1, 4), ("Rear Left", 0, 1, 1), ("Logo", 0, 1, 1), ("Rear Right", 0, 1, 1), ("DPI & Indicator Zone", 1, 1, 4)], None),
    (0x1B7E, K::Mouse, &[("Scroll Wheel", 0, 1, 1), ("Side Buttons", 1, 1, 4), ("Rear Left", 0, 1, 1), ("Logo", 0, 1, 1), ("Rear Right", 0, 1, 1), ("DPI & Indicator Zone", 1, 1, 4)], None),
    (0x1B5E, K::Mouse, &[("Indicator", 0, 1, 1), ("Logo", 0, 1, 1)], None),
    (0x1B4C, K::Mouse, &[("Logo", 0, 1, 1), ("Scroll Wheel", 0, 1, 1), ("Buttons", 0, 1, 1), ("Side Zone", 1, 1, 3)], None),
    (0x2B32, K::Mouse, &[("Logo", 0, 1, 1), ("Scroll Wheel", 0, 1, 1), ("Buttons", 0, 1, 1), ("Side Zone", 1, 1, 3)], None),
    (0x1B93, K::Mouse, &[("Scroll Wheel", 0, 1, 1), ("DPI", 0, 1, 1)], None),
    (0x1BBA, K::Mouse, &[("Scroll Wheel", 0, 1, 1), ("DPI", 0, 1, 1)], None),
    (0x1BAC, K::Mouse, &[("Scroll Wheel", 0, 1, 1), ("DPI", 0, 1, 1)], None),
    (0x1BA4, K::Keyboard, &[("ZONE_EN_KEYBOARD", 1, 1, 6)], None),
    (0x1B6E, K::Keyboard, &[("ZONE_EN_KEYBOARD", 2, 7, 22)], Some(L_CORSAIR_K57_LAYOUT)),
    (0x1BA0, K::Keyboard, &[("ZONE_EN_KEYBOARD", 2, 6, 21)], Some(L_CORSAIR_K60_LAYOUT)),
    (0x1B8D, K::Keyboard, &[("ZONE_EN_KEYBOARD", 2, 6, 21)], Some(L_CORSAIR_K60_LAYOUT)),
    (0x1BAD, K::Keyboard, &[("ZONE_EN_KEYBOARD", 2, 6, 21)], Some(L_CORSAIR_K60_LAYOUT)),
    (0x1BC7, K::Keyboard, &[("ZONE_EN_KEYBOARD", 2, 6, 21)], Some(L_CORSAIR_K60_TKL_LAYOUT)),
    (0x1BED, K::Keyboard, &[("ZONE_EN_KEYBOARD", 2, 6, 21)], Some(L_CORSAIR_K60_TKL_LAYOUT)),
    (0x1BFD, K::Keyboard, &[("ZONE_EN_KEYBOARD", 2, 6, 21)], Some(L_CORSAIR_K70_LAYOUT)),
    (0x1B73, K::Keyboard, &[("ZONE_EN_KEYBOARD", 2, 7, 17)], Some(L_CORSAIR_K70_TKL_CS_LAYOUT)),
    (0x2B01, K::Keyboard, &[("ZONE_EN_KEYBOARD", 2, 6, 17)], Some(L_CORSAIR_K70_CORE_TKL_LAYOUT)),
    (0x1BB9, K::Keyboard, &[("ZONE_EN_KEYBOARD", 2, 7, 17)], Some(L_CORSAIR_K70_TKL_CS_LAYOUT)),
    (0x1BC4, K::Keyboard, &[("ZONE_EN_KEYBOARD", 2, 7, 21)], Some(L_CORSAIR_K70_PRO_LAYOUT)),
    (0x1BB3, K::Keyboard, &[("ZONE_EN_KEYBOARD", 2, 7, 21)], Some(L_CORSAIR_K70_PRO_LAYOUT)),
    (0x1B89, K::Keyboard, &[("ZONE_EN_KEYBOARD", 2, 8, 22)], Some(L_CORSAIR_K95_LAYOUT)),
    (0x1B7D, K::Keyboard, &[("ZONE_EN_KEYBOARD", 2, 12, 24)], Some(L_CORSAIR_K100_LAYOUT)),
    (0x1B7C, K::Keyboard, &[("ZONE_EN_KEYBOARD", 2, 12, 24)], Some(L_CORSAIR_K100_LAYOUT)),
    (0x1BC5, K::Keyboard, &[("ZONE_EN_KEYBOARD", 2, 12, 24)], Some(L_CORSAIR_K100_LAYOUT)),
    (0x1B70, K::Mouse, &[("Middle Button", 0, 1, 1), ("Logo", 0, 1, 1)], None),
    (0x1B9E, K::Mouse, &[("Logo", 0, 1, 1), ("Scroll Wheel", 0, 1, 1), ("Indicator", 0, 1, 1)], None),
    (0x1BB5, K::Mouse, &[("Logo", 0, 1, 1), ("DPI", 0, 1, 1)], None),
    (0x1BF0, K::Mouse, &[("Logo", 0, 1, 1), ("Scroll Wheel", 0, 1, 1)], None),
    (0x1BF1, K::Mouse, &[("Logo", 0, 1, 1), ("Bottom", 0, 1, 1)], None),
    (0x1BE3, K::Mouse, &[("Logo", 0, 1, 1), ("Scroll Wheel", 0, 1, 1), ("Front Right", 0, 1, 1), ("Side Buttons", 0, 1, 1), ("Front Left", 0, 1, 1)], None),
    (0x1B9B, K::Mousemat, &[("Left", 0, 1, 1), ("Right", 0, 1, 1), ("Logo", 0, 1, 1)], None),
    (0x1BC9, K::Mousemat, &[("Left", 0, 1, 1), ("Right", 0, 1, 1), ("Logo", 0, 1, 1)], None),
];
