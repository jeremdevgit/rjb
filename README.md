<p align="center"><img src="captures/icone.png" width="112" alt=""></p>

<h1 align="center">rJb</h1>

<p align="center"><b>RGB lighting for all your peripherals and components in one lightweight app.</b></p>

<p align="center"><img src="https://img.shields.io/badge/%F0%9F%9B%A0%EF%B8%8F%20System%20%26%20hardware-Windows-2f6fde?style=flat-square" alt="System & hardware"> <a href="https://github.com/jeremdevgit/rjb/releases"><img src="https://img.shields.io/github/v/release/jeremdevgit/rjb?include_prereleases&style=flat-square&label=version" alt="version"></a> <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPL--2.0--or--later-555?style=flat-square" alt="GPL-2.0-or-later"></a> <a href="https://github.com/jeremdevgit/rjb/actions/workflows/windows.yml"><img src="https://github.com/jeremdevgit/rjb/actions/workflows/windows.yml/badge.svg" alt="CI"></a></p>

<p align="center"><b>English</b> · <a href="README.fr.md">Français</a> · <a href="README.es.md">Español</a> · <a href="README.de.md">Deutsch</a> · <a href="README.it.md">Italiano</a> · <a href="README.pt.md">Português</a> · <a href="README.zh.md">简体中文</a> · <a href="README.ja.md">日本語</a></p>

<p align="center"><a href="https://github.com/jeremdevgit/rjb/releases/latest"><b>⬇ Download</b></a></p>

![A keyboard with a rainbow wave](captures/en/clavier.jpg)

Keyboards, mice, headsets, mousepads, RAM, motherboard, graphics card, fans and LED strips: **one synchronized effect, without each manufacturer's heavy software.**

<p><img src="captures/en/tous-les-appareils.jpg" alt="All devices in sync" width="49%"> <img src="captures/en/carte-mere.jpg" alt="Motherboard and ARGB header" width="49%"></p>

*Screenshots use demo devices with the real interface and effects engine.*

## At a glance

| | |
|---|---|
| **Category** | 🛠️ System & hardware |
| **Platforms** | Windows |
| **Interface** | 8 languages (EN, FR, ES, DE, IT, PT, ZH, JA) |
| **Price** | Free, open source, no ads, no telemetry |
| **Status** | Stable |
| **License** | [GPL-2.0-or-later](LICENSE) |

## What it does

- **12 synchronized effects**: static, breathing, color cycle, rainbow wave, color wave, gradient, comet, twinkle, strobe, fire, aurora and LED-by-LED editing; global brightness, ignored devices.
- **Profiles**, launch at sign-in (with the rights needed for RAM and motherboard), notification-area icon.
- **Conflict detection**: warns when G HUB, iCUE, Synapse, Armoury Crate, SignalRGB or OpenRGB fight over the same devices.

## Supported hardware

| Family | Examples |
|---|---|
| Keyboards, mice, headsets, mousepads | Logitech, Corsair, Razer, SteelSeries, HyperX, Wooting, Windows Dynamic Lighting (LampArray) |
| RAM | Corsair, Kingston Fury, HyperX, Crucial Ballistix, G.Skill and other ENE / SPD modules |
| Motherboards & controllers | ASUS Aura (USB), MSI Mystic Light, Gigabyte RGB Fusion 2, ASRock Polychrome, Corsair Commander Core / Lighting Node / Hydro Platinum, NZXT Hue 2, Lian Li |
| Graphics cards | NVIDIA (through the driver) and board partners' controllers |

RAM and motherboard lighting goes through the SMBus using the signed open-source **PawnIO** driver (also used by OpenRGB and FanControl), offered by the installer. Without it, only USB devices are controlled.

## Install

1. Download `rJb-Setup.exe` from the [latest release](https://github.com/jeremdevgit/rjb/releases/latest) and run it.
2. An installed version is replaced automatically; profiles and settings are kept.
3. rJb installs into *Program Files*, so the elevated sign-in task points to a folder only administrators can modify.

Not signed with a paid certificate: if SmartScreen appears, choose **More info › Run anyway**. Verify with `Get-FileHash .\rJb-Setup.exe` against `SHA256SUMS.txt`.

## Safety and privacy

- **No network connection** (apart from the optional PawnIO install through winget and links you open), no telemetry.
- SMBus access shares the usual system lock with other software; writes are limited to recognized controller addresses.
- External links open with your rights, never as administrator.

## Build from source

```powershell
# Rust stable + Visual Studio Build Tools (C++)
cargo build --release        # target\release\rJb.exe
.\tools\release.ps1          # dist\rJb-Setup.exe + checks
```

Texts live in [`src/textes.rs`](src/textes.rs) (French key + seven translations). Releases are built by GitHub Actions ([`windows.yml`](.github/workflows/windows.yml)).

## License

GNU GPL v2 or later — see [LICENSE](LICENSE). Device protocols come from the [OpenRGB](https://gitlab.com/CalcProgrammer1/OpenRGB) community (GPL-2.0-or-later); bundled [PawnIO](https://github.com/namazso/PawnIO.Modules) modules are LGPL-2.1 ([`licences-tiers/`](licences-tiers/)). Brand names belong to their owners; rJb is not affiliated with any manufacturer.

---

Found a vulnerability? Please report it privately (see [SECURITY.md](SECURITY.md)) rather than in a public issue.

## More free software by jeremdevgit

All free, open source and ad-free.

**🛠️ System & hardware** — [Boîte à outils](https://github.com/jeremdevgit/boite-a-outils) · [LJ Hub](https://github.com/jeremdevgit/lj-hub) · [Decklight](https://github.com/jeremdevgit/decklight)

**📒 Productivity & data** — [Carnet de notes](https://github.com/jeremdevgit/carnet-rust) · [JMeca](https://github.com/jeremdevgit/jmeca) · [Suite Libre](https://jeremdevgit.github.io/suite-libre/)

**🎮 Music & games** — [Pomme](https://github.com/jeremdevgit/pomme) · [Ludothèque](https://github.com/jeremdevgit/ludotheque) · [AutoDex](https://github.com/jeremdevgit/autodex) · [Batailles de France](https://github.com/jeremdevgit/batailles-de-france)
