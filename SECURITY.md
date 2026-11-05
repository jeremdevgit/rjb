# Sécurité

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
