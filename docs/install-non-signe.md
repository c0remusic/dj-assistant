# Installer Sift (build non signé)

Sift n'a pas encore de certificat de signature de code Windows ni de compte
Apple Developer (notarization macOS) — voir
`docs/superpowers/changes/archive/2026-07-24-auto-update/design.md` pour le contexte.
Le premier lancement d'un installeur téléchargé déclenche donc un
avertissement de l'OS. Ces étapes ne sont nécessaires qu'à la **première**
installation manuelle — les mises à jour suivantes passent par l'auto-update
intégré à l'app.

## Quel fichier prendre

La page de release expose huit fichiers. Un seul s'installe, selon la machine :

| machine | fichier |
|---|---|
| Windows | `Sift_<version>_x64-setup.exe` |
| Mac Apple Silicon | `Sift_<version>_aarch64.dmg` |
| Mac Intel | `Sift_<version>_x64.dmg` |

Tout le reste sert à la mise à jour automatique et ne s'installe pas : `.app.tar.gz`, `.msi`,
les `.sig`, `latest.json`.

⚠️ Ce tableau vient d'un échec réel. Le 2026-08-16, la première installation sans
accompagnement s'est arrêtée sur « is damaged » parce que la personne avait téléchargé
`Sift_0.0.3_aarch64.app.tar.gz` — **34 198 810 octets contre 33 204 781 pour le `.dmg`** : le
fichier de mise à jour pèse plus lourd que l'installeur, donc il a l'air d'être le bon. Un `.app`
sorti d'une archive téléchargée porte la quarantaine et n'est pas signé.

Le `.msi` s'installe aussi sur Windows, mais un seul fichier nommé vaut mieux que deux
équivalents à départager : l'`.exe` est celui que l'auto-update utilise ensuite.

## Windows

1. Double-cliquer `Sift_<version>_x64-setup.exe`.
2. Windows SmartScreen affiche « Windows a protégé votre ordinateur ».
3. Cliquer **Informations complémentaires**, puis **Exécuter quand même**.

## macOS

Deux builds depuis la v0.1.1 : **Apple Silicon** (`aarch64`) et **Intel** (`x64`). Un
binaire arm64 ne démarre pas du tout sur un Mac Intel — prendre celui de sa machine
(menu Pomme › À propos de ce Mac : « Puce Apple M… » ou « Processeur Intel »).

1. Ouvrir le `.dmg` de sa machine (§ Quel fichier prendre), glisser Sift dans Applications.
2. Au double-clic, macOS affiche **l'un des deux messages suivants**. Ils n'ont pas le
   même contournement, et c'est le point qui a fait échouer une installation réelle le
   2026-08-16 :

### Cas A — « développeur non identifié »

3. Ouvrir **Réglages Système → Confidentialité et sécurité**, descendre jusqu'au
   message concernant Sift, et cliquer **Ouvrir quand même**. Nécessaire une seule
   fois.

### Cas B — « "Sift" is damaged and can't be opened. You should move it to the Trash. »

**Le bouton « Ouvrir quand même » n'existe pas pour ce message** : la boîte ne propose
que *Cancel* et *Move to Trash*. Le cas A ne s'applique pas, et suivre ses étapes ne mène
nulle part. L'app n'est pas endommagée — elle est mise en quarantaine par le navigateur et
son bundle n'a pas de signature (voir la note en bas). Dans le Terminal :

3. `xattr -dr com.apple.quarantine /Applications/Sift.app`

   Le `-r` n'est pas optionnel : un `.app` est un **dossier**, l'attribut de quarantaine
   est posé fichier par fichier, et sans `-r` les binaires imbriqués — dont le sidecar
   ffmpeg — restent marqués.

4. Si le message persiste, resigner le bundle en ad-hoc :
   `codesign --force --deep --sign - /Applications/Sift.app`

> ⚠️ L'étape 3 disait « clic droit → Ouvrir » jusqu'au 2026-08-02. Ce contournement
> a été **retiré à partir de macOS 15 Sequoia** : le menu contextuel n'offre plus de
> dérogation pour une app non signée, et suivre l'ancienne instruction sur un Mac
> récent mène à une impasse sans message utile. Le passage par les Réglages Système
> fonctionne sur les deux générations.

> ⚠️ **Pourquoi deux messages.** Aucun bloc `bundle.macOS` n'existe dans
> `src-tauri/tauri.conf.json` ni dans `tauri.release.conf.json` : `signingIdentity` n'est
> déclaré nulle part, donc `tauri build` ne code-signe pas le bundle (l'étape de
> `build.yml` s'appelle d'ailleurs « Build Tauri app (**unsigned**) »). Un `.app` sans
> signature de bundle, mis en quarantaine par un navigateur, produit le cas B plutôt que
> le cas A. Si une signature ad-hoc au build suffirait à ramener tout le monde au cas A
> reste **à mesurer sur un vrai Mac** — c'est l'objet de l'issue #36, et tant que ce n'est
> pas mesuré, ne pas activer l'option en croyant que ça corrige quelque chose.

Ces mêmes étapes sont reprises dans le pied de page des notes de version, ajouté par
`scripts/changelog-section.mjs` — c'est ce que lit quelqu'un à qui on envoie le lien
d'une release. Les deux se corrigent ensemble.
