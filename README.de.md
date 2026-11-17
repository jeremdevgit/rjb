<p align="center"><img src="captures/icone.png" width="112" alt=""></p>

<h1 align="center">rJb</h1>

<p align="center"><b>RGB-Beleuchtung für alle Peripheriegeräte und Komponenten in einer einzigen schlanken App.</b></p>

<p align="center"><img src="https://img.shields.io/badge/%F0%9F%9B%A0%EF%B8%8F%20System%20%26%20hardware-Windows-2f6fde?style=flat-square" alt="System & Hardware"> <a href="https://github.com/jeremdevgit/rjb/releases"><img src="https://img.shields.io/github/v/release/jeremdevgit/rjb?include_prereleases&style=flat-square&label=version" alt="version"></a> <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPL--2.0--or--later-555?style=flat-square" alt="GPL-2.0-or-later"></a> <a href="https://github.com/jeremdevgit/rjb/actions/workflows/windows.yml"><img src="https://github.com/jeremdevgit/rjb/actions/workflows/windows.yml/badge.svg" alt="CI"></a></p>

<p align="center"><a href="README.md">English</a> · <a href="README.fr.md">Français</a> · <a href="README.es.md">Español</a> · <b>Deutsch</b> · <a href="README.it.md">Italiano</a> · <a href="README.pt.md">Português</a> · <a href="README.zh.md">简体中文</a> · <a href="README.ja.md">日本語</a></p>

<p align="center"><a href="https://github.com/jeremdevgit/rjb/releases/latest"><b>⬇ Herunterladen</b></a></p>

![Eine Tastatur mit Regenbogenwelle](captures/en/clavier.jpg)

Tastaturen, Mäuse, Headsets, Mauspads, RAM, Mainboard, Grafikkarte, Lüfter und LED-Streifen: **ein synchroner Effekt, ohne die schwere Software jedes Herstellers.**

<p><img src="captures/en/tous-les-appareils.jpg" alt="Alle Geräte synchron" width="49%"> <img src="captures/en/carte-mere.jpg" alt="Mainboard und ARGB-Anschluss" width="49%"></p>

*Screenshots mit Demo-Geräten, der echten Oberfläche und der echten Effekt-Engine.*

## Auf einen Blick

| | |
|---|---|
| **Kategorie** | 🛠️ System & Hardware |
| **Plattformen** | Windows |
| **Oberfläche** | 8 Sprachen (EN, FR, ES, DE, IT, PT, ZH, JA) |
| **Preis** | Kostenlos, quelloffen, ohne Werbung und Telemetrie |
| **Status** | Stabil |
| **Lizenz** | [GPL-2.0-or-later](LICENSE) |

## Was es kann

- **12 synchrone Effekte**: statisch, Atmen, Farbwechsel, Regenbogenwelle, Farbwelle, Verlauf, Komet, Funkeln, Stroboskop, Feuer, Polarlicht und LED-genaue Bearbeitung; globale Helligkeit, ignorierte Geräte.
- **Profile**, Start bei der Anmeldung (mit den nötigen Rechten für RAM und Mainboard), Symbol im Infobereich.
- **Konflikterkennung**: warnt, wenn G HUB, iCUE, Synapse, Armoury Crate, SignalRGB oder OpenRGB um dieselben Geräte konkurrieren.

## Unterstützte Hardware

| Gruppe | Beispiele |
|---|---|
| Tastaturen, Mäuse, Headsets, Mauspads | Logitech, Corsair, Razer, SteelSeries, HyperX, Wooting, Windows Dynamic Lighting (LampArray) |
| RAM | Corsair, Kingston Fury, HyperX, Crucial Ballistix, G.Skill und andere ENE-/SPD-Module |
| Mainboards & Controller | ASUS Aura (USB), MSI Mystic Light, Gigabyte RGB Fusion 2, ASRock Polychrome, Corsair Commander Core / Lighting Node / Hydro Platinum, NZXT Hue 2, Lian Li |
| Grafikkarten | NVIDIA (über den Treiber) und Controller der Kartenhersteller |

RAM- und Mainboard-Beleuchtung läuft über den SMBus mit dem signierten, quelloffenen Treiber **PawnIO** (auch von OpenRGB und FanControl genutzt), den der Installer anbietet. Ohne ihn werden nur USB-Geräte gesteuert.

## Installation

1. Lade `rJb-Setup.exe` aus der [neuesten Version](https://github.com/jeremdevgit/rjb/releases/latest) herunter und starte es.
2. Eine installierte Version wird automatisch ersetzt; Profile und Einstellungen bleiben erhalten.
3. rJb installiert sich nach *Program Files*, damit die erhöhte Anmeldeaufgabe auf einen Ordner zeigt, den nur Administratoren ändern können.

Kein kostenpflichtiges Zertifikat: Wenn SmartScreen erscheint, wähle **Weitere Informationen › Trotzdem ausführen**. Prüfen mit `Get-FileHash .\rJb-Setup.exe` und `SHA256SUMS.txt`.

## Sicherheit und Datenschutz

- **Keine Netzwerkverbindung** (außer der optionalen PawnIO-Installation per winget und Links, die du öffnest), keine Telemetrie.
- SMBus-Zugriffe teilen sich die übliche Systemsperre mit anderer Software; Schreibzugriffe sind auf erkannte Controller-Adressen beschränkt.
- Externe Links werden mit deinen Rechten geöffnet, nie als Administrator.

## Selbst bauen

```powershell
# Rust stable + Visual Studio Build Tools (C++)
cargo build --release        # target\release\rJb.exe
.\tools\release.ps1          # dist\rJb-Setup.exe + Prüfungen
```

Die Texte stehen in [`src/textes.rs`](src/textes.rs) (französischer Schlüssel + sieben Übersetzungen). Versionen werden mit GitHub Actions gebaut ([`windows.yml`](.github/workflows/windows.yml)).

## Lizenz

GNU GPL v2 oder später – siehe [LICENSE](LICENSE). Die Geräteprotokolle stammen aus der [OpenRGB](https://gitlab.com/CalcProgrammer1/OpenRGB)-Community (GPL-2.0-or-later); die enthaltenen [PawnIO](https://github.com/namazso/PawnIO.Modules)-Module stehen unter LGPL-2.1 ([`licences-tiers/`](licences-tiers/)). Marken gehören ihren Inhabern; rJb ist mit keinem Hersteller verbunden.

---

Eine Sicherheitslücke gefunden? Bitte privat melden (siehe [SECURITY.md](SECURITY.md)), nicht in einem öffentlichen Issue.

## Weitere freie Software von jeremdevgit

Alle kostenlos, quelloffen und werbefrei.

**🛠️ System & Hardware** — [Boîte à outils](https://github.com/jeremdevgit/boite-a-outils) · [LJ Hub](https://github.com/jeremdevgit/lj-hub) · [Decklight](https://github.com/jeremdevgit/decklight)

**📒 Produktivität & Daten** — [Carnet de notes](https://github.com/jeremdevgit/carnet-rust) · [JMeca](https://github.com/jeremdevgit/jmeca) · [Suite Libre](https://jeremdevgit.github.io/suite-libre/)

**🎮 Musik & Spiele** — [Pomme](https://github.com/jeremdevgit/pomme) · [Ludothèque](https://github.com/jeremdevgit/ludotheque) · [AutoDex](https://github.com/jeremdevgit/autodex) · [Batailles de France](https://github.com/jeremdevgit/batailles-de-france)
