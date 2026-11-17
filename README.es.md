<p align="center"><img src="captures/icone.png" width="112" alt=""></p>

<h1 align="center">rJb</h1>

<p align="center"><b>La iluminación RGB de todos tus periféricos y componentes en una sola aplicación ligera.</b></p>

<p align="center"><img src="https://img.shields.io/badge/%F0%9F%9B%A0%EF%B8%8F%20System%20%26%20hardware-Windows-2f6fde?style=flat-square" alt="Sistema y hardware"> <a href="https://github.com/jeremdevgit/rjb/releases"><img src="https://img.shields.io/github/v/release/jeremdevgit/rjb?include_prereleases&style=flat-square&label=version" alt="version"></a> <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPL--2.0--or--later-555?style=flat-square" alt="GPL-2.0-or-later"></a> <a href="https://github.com/jeremdevgit/rjb/actions/workflows/windows.yml"><img src="https://github.com/jeremdevgit/rjb/actions/workflows/windows.yml/badge.svg" alt="CI"></a></p>

<p align="center"><a href="README.md">English</a> · <a href="README.fr.md">Français</a> · <b>Español</b> · <a href="README.de.md">Deutsch</a> · <a href="README.it.md">Italiano</a> · <a href="README.pt.md">Português</a> · <a href="README.zh.md">简体中文</a> · <a href="README.ja.md">日本語</a></p>

<p align="center"><a href="https://github.com/jeremdevgit/rjb/releases/latest"><b>⬇ Descargar</b></a></p>

![Un teclado con una ola arcoíris](captures/en/clavier.jpg)

Teclados, ratones, auriculares, alfombrillas, RAM, placa base, tarjeta gráfica, ventiladores y tiras LED: **un solo efecto sincronizado, sin el pesado software de cada fabricante.**

<p><img src="captures/en/tous-les-appareils.jpg" alt="Todos los dispositivos sincronizados" width="49%"> <img src="captures/en/carte-mere.jpg" alt="Placa base y conector ARGB" width="49%"></p>

*Capturas con dispositivos de demostración, la interfaz real y el motor de efectos real.*

## De un vistazo

| | |
|---|---|
| **Categoría** | 🛠️ Sistema y hardware |
| **Plataformas** | Windows |
| **Interfaz** | 8 idiomas (EN, FR, ES, DE, IT, PT, ZH, JA) |
| **Precio** | Gratis, de código abierto, sin anuncios ni telemetría |
| **Estado** | Estable |
| **Licencia** | [GPL-2.0-or-later](LICENSE) |

## Qué hace

- **12 efectos sincronizados**: color fijo, respiración, ciclo de color, ola arcoíris, ola de color, degradado, cometa, destello, estroboscopio, fuego, aurora y edición LED a LED; brillo global, dispositivos ignorados.
- **Perfiles**, inicio con la sesión (con los permisos necesarios para RAM y placa base), icono en el área de notificación.
- **Detección de conflictos**: avisa cuando G HUB, iCUE, Synapse, Armoury Crate, SignalRGB u OpenRGB se disputan los mismos dispositivos.

## Hardware compatible

| Familia | Ejemplos |
|---|---|
| Teclados, ratones, auriculares, alfombrillas | Logitech, Corsair, Razer, SteelSeries, HyperX, Wooting, Iluminación dinámica de Windows (LampArray) |
| RAM | Corsair, Kingston Fury, HyperX, Crucial Ballistix, G.Skill y otros módulos ENE / SPD |
| Placas base y controladores | ASUS Aura (USB), MSI Mystic Light, Gigabyte RGB Fusion 2, ASRock Polychrome, Corsair Commander Core / Lighting Node / Hydro Platinum, NZXT Hue 2, Lian Li |
| Tarjetas gráficas | NVIDIA (mediante el controlador) y controladores de los fabricantes de tarjetas |

La RAM y la placa base usan el bus SMBus con el controlador libre y firmado **PawnIO** (el mismo de OpenRGB y FanControl), que ofrece el instalador. Sin él, solo se controlan dispositivos USB.

## Instalar

1. Descarga `rJb-Setup.exe` de la [última versión](https://github.com/jeremdevgit/rjb/releases/latest) y ejecútalo.
2. Una versión instalada se reemplaza automáticamente; se conservan perfiles y ajustes.
3. rJb se instala en *Program Files*, así que la tarea elevada de inicio de sesión apunta a una carpeta que solo pueden modificar los administradores.

Sin certificado de pago: si aparece SmartScreen, elige **Más información › Ejecutar de todas formas**. Comprueba con `Get-FileHash .\rJb-Setup.exe` y `SHA256SUMS.txt`.

## Seguridad y privacidad

- **Sin conexión de red** (salvo la instalación opcional de PawnIO con winget y los enlaces que abras), sin telemetría.
- El acceso SMBus comparte el bloqueo habitual del sistema con otros programas; las escrituras se limitan a direcciones de controladores reconocidos.
- Los enlaces externos se abren con tus permisos, nunca como administrador.

## Compilar

```powershell
# Rust estable + Visual Studio Build Tools (C++)
cargo build --release        # target\release\rJb.exe
.\tools\release.ps1          # dist\rJb-Setup.exe + comprobaciones
```

Los textos están en [`src/textes.rs`](src/textes.rs) (clave en francés + siete traducciones). Las versiones se compilan con GitHub Actions ([`windows.yml`](.github/workflows/windows.yml)).

## Licencia

GNU GPL v2 o posterior — ver [LICENSE](LICENSE). Los protocolos de los dispositivos proceden de la comunidad [OpenRGB](https://gitlab.com/CalcProgrammer1/OpenRGB) (GPL-2.0-or-later); los módulos [PawnIO](https://github.com/namazso/PawnIO.Modules) incluidos son LGPL-2.1 ([`licences-tiers/`](licences-tiers/)). Las marcas pertenecen a sus propietarios; rJb no está afiliado a ningún fabricante.

---

¿Has encontrado una vulnerabilidad? Infórmala en privado (consulta [SECURITY.md](SECURITY.md)) y no en una incidencia pública.

## Más software libre de jeremdevgit

Todos gratuitos, de código abierto y sin anuncios.

**🛠️ Sistema y hardware** — [Boîte à outils](https://github.com/jeremdevgit/boite-a-outils) · [LJ Hub](https://github.com/jeremdevgit/lj-hub) · [Decklight](https://github.com/jeremdevgit/decklight)

**📒 Productividad y datos** — [Carnet de notes](https://github.com/jeremdevgit/carnet-rust) · [JMeca](https://github.com/jeremdevgit/jmeca) · [Suite Libre](https://jeremdevgit.github.io/suite-libre/)

**🎮 Música y juegos** — [Pomme](https://github.com/jeremdevgit/pomme) · [Ludothèque](https://github.com/jeremdevgit/ludotheque) · [AutoDex](https://github.com/jeremdevgit/autodex) · [Batailles de France](https://github.com/jeremdevgit/batailles-de-france)
