# Security

**English** · [Français](#sécurité)

## Reporting a vulnerability

Please **do not** open a public issue for a security flaw.
Use GitHub's private reporting: the repository's **Security** tab → **Report a vulnerability**.
Describe the problem, the version affected, the steps to reproduce it and, if possible, a suggested fix.
You will get an answer as soon as possible; the fix will ship in a new release, crediting you if you agree.

## Supported versions

Only the latest release (*Releases* page) receives fixes.

## What the software guarantees

- No network connection, no telemetry.
- Automatic launch as administrator only from a folder that only administrators can modify (otherwise another program could replace rJb.exe to gain those rights).
- System libraries loaded from System32 only.

## Out of scope

- Hardware access (SMBus) goes through the PawnIO driver, maintained by its author: any flaws in it belong to that project.
- The executable isn't signed with a paid certificate (SmartScreen warning): check the SHA-256 hash published with each release.

---

# Sécurité

[English](#security) · **Français**

## Signaler une vulnérabilité

Merci de **ne pas** ouvrir de ticket public pour une faille de sécurité.
Utilise le signalement privé de GitHub : onglet **Security** du dépôt → **Report a vulnerability**.
Décris le problème, la version concernée, les étapes pour le reproduire et, si possible, une piste de correction.
Tu recevras une réponse dès que possible ; la correction sera publiée dans une nouvelle version, avec ton accord pour être cité.

## Versions suivies

Seule la dernière version publiée (page *Releases*) reçoit des correctifs.

## Ce que garantit le logiciel

- Aucune connexion réseau ni télémétrie.
- Lancement automatique en administrateur uniquement depuis un dossier modifiable par les seuls administrateurs (sinon un autre programme pourrait remplacer rJb.exe pour obtenir ces droits).
- Bibliothèques système chargées depuis System32 uniquement.

## Hors périmètre

- L'accès au matériel (SMBus) passe par le pilote PawnIO, maintenu par son auteur : ses failles éventuelles relèvent de son projet.
- L'exécutable n'est pas signé par un certificat payant (avertissement SmartScreen) : vérifie l'empreinte SHA-256 publiée avec chaque version.
