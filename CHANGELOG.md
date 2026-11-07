# Historique des versions

## 1.2.1 — 2026-11-07 — Sécurité / Security

- Création atomique des dossiers avec propriétaire/DACL protégés dès l'origine ; refus d'une création concurrente non fiable. Migration par défaut vers un emplacement protégé, sans suivre les anciens dossiers non fiables ; rJb désactive l'ancien démarrage élevé avant migration.
- Create directories atomically with protected ownership/DACLs; reject an untrusted concurrent creator. Default upgrades choose a protected location and do not traverse unsafe old folders; rJb disables unsafe previous elevated startup before migration.
- Vérification des pointeurs COM avant utilisation ; Tokio 1.54.1 dans le verrou de dépendances. Éditeur documenté dans EDITEUR.md.
- Check COM output pointers before use; update Tokio lock entry to 1.54.1.
**Français**

- Installation : chemins ambigus et jonctions refusés ; résolution des chemins, contrôle des propriétaires et droits réels des dossiers et de leurs parents.
- Échec explicite si la protection ne peut pas être établie. Exécutables protégés et vérifiés avant remplacement.
- Le démarrage à privilèges élevés exige une installation dont les droits réels sont vérifiés.
- CI : Actions épinglées, publication isolée et tests Windows des ACL/jonctions.

**English**

- Reject ambiguous paths and reparse points, resolve paths, and verify actual owners and directory/parent permissions.
- Fail closed on protection errors; protect staged executables before replacement.
- Elevated autostart requires a verified protected installation.
- Pin CI Actions, isolate release permissions and add native ACL/junction tests.


## 1.2.0 — 2026-11-06

**English**

- **8 languages**: English, French, Spanish, German, Italian, Portuguese, Chinese and Japanese. Your Windows language is picked automatically; *Settings › Language* lets you switch, instantly (no restart, notification area menu included).
- The installer also uses the chosen language.
- Chinese and Japanese: the matching Windows font (Microsoft YaHei, Yu Gothic…) is loaded only for those languages.

**Français**

- **8 langues** : français, anglais, espagnol, allemand, italien, portugais, chinois et japonais. La langue de Windows est choisie automatiquement ; *Réglages › Langue* permet d'en changer, immédiatement (sans relancer, menu de la zone de notification compris).
- L'installateur utilise lui aussi la langue choisie.
- Chinois et japonais : la police de Windows correspondante (Microsoft YaHei, Yu Gothic…) est chargée seulement pour ces langues.

## 1.1.0 — 2026-11-05 — Publication open source

- Code publié sous licence GNU GPL v2 ou ultérieure (protocoles issus d'OpenRGB).
- **Installateur** : la version déjà installée est indiquée et remplacée proprement (fichiers de l'ancienne version retirés, inscription Windows réécrite) ; profils et réglages conservés.
- Crédit « jeremdevgit » dans l'application, l'installateur, *Paramètres › Applications* et les propriétés du fichier ; mention des licences OpenRGB et PawnIO dans « À propos ».
- Réglages : nouvelle section **Mes autres logiciels** (liens ouverts sans droits administrateur).
- Versions construites par GitHub Actions, avec contrôle automatique du binaire.

## 1.0.1

- Première version publique : effets synchronisés sur les périphériques USB, la mémoire, la carte mère et la carte graphique ; installateur intégré.
