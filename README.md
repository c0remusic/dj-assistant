# Sift

Préparation de musique pour DJ, sur Windows et macOS. Sift vérifie les fichiers lossless, repère
les doublons, écrit les tags, range la bibliothèque et l'exporte vers Rekordbox et la clé USB.

**[sift-music.vercel.app](https://sift-music.vercel.app)** — présentation et téléchargement.

## Télécharger

Les installeurs sont dans les [versions publiées](https://github.com/c0remusic/sift/releases/latest).
Un seul fichier à prendre, selon la machine :

| machine | fichier |
|---|---|
| Windows | `Sift_<version>_x64-setup.exe` |
| Mac Apple Silicon | `Sift_<version>_aarch64.dmg` |
| Mac Intel | `Sift_<version>_x64.dmg` |

Le reste de la liste sert à la mise à jour automatique et ne s'installe pas.

Ces builds ne sont pas signés, faute de certificat Apple ou Microsoft : le système affiche un
avertissement au premier lancement, une seule fois. Le
[manuel](https://sift-music.vercel.app/manuel.html#installer) dit quoi cliquer.

## Se servir de Sift

- [Manuel](https://sift-music.vercel.app/manuel.html), ou [en PDF](https://sift-music.vercel.app/manuel.pdf).
- [Nouveautés par version](CHANGELOG.md).

## Un problème, une idée

Les [issues](https://github.com/c0remusic/sift/issues) sont ouvertes. Indiquez la version, votre
machine, et ce que vous attendiez. Pour une faille de sécurité, ne passez pas par une issue
publique : voir [SECURITY.md](SECURITY.md).

## Le code

Sift est un logiciel propriétaire. Son code source est développé dans un dépôt privé et n'est plus
publié ici : ce dépôt ne sert plus qu'à la distribution, aux versions et aux issues.

L'historique de ce dépôt s'arrête à la version 0.1.2. Ce qui y a été publié l'a été sous licence
MIT, et le reste pour ces versions-là. Les versions suivantes sont couvertes par la
[licence](LICENSE) du produit.
