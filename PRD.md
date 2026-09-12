# Sift — PRD (rétro-PRD, centré chemin utilisateur)

> Reconstruit depuis l'usage réel d'une app feature-complète (M0→M8), le
> 2026-07-18, via interview d'usage. Le QUOI et le chemin utilisateur ; le
> COMMENT détaillé se conçoit ensuite (`superpowers:brainstorming`). Source de
> vérité du comportement livré = le code + `README.md` ; ce PRD fige l'intention
> d'usage, l'inacceptable et le hors-scope.

## Contexte

Un DJ possède une grosse bibliothèque musicale — le plus souvent un gros dossier
unique, parfois des dossiers éparses en bordel. Elle est polluée : fichiers de
qualité douteuse (faux lossless — un `.flac`/`.wav` ré-encodé depuis du lossy),
doublons, métadonnées incomplètes ou fausses, rangement incohérent. Avant de
jouer sereinement (ou juste pour ne plus être inquiété), il faut nettoyer,
vérifier, identifier, dédoublonner et ranger — un travail manuel énorme
aujourd'hui. Sift est une app desktop **gratuite** (Windows + Mac) qui outille ce
travail. Principe directeur : **« déplacer = encoder + ranger »**.

## Objectif

Permettre à un DJ de transformer une bibliothèque musicale brute et douteuse en
une collection propre, vérifiée, dédoublonnée et rangée à sa main — sans jamais
risquer ses fichiers originaux ni dégrader leur qualité en cachette.

## Utilisateur et contextes d'usage

Utilisateur = DJ qui source sa musique (souvent Soulseek) et joue sur platines —
Sift est **le poste de prépa entre Soulseek et les platines**. Organisé ou non
(gros dossier unique, ou dossiers éparses). Deux contextes qui changent où
s'arrête « réussi » :

- **Ranger pour être tranquille** : mettre sa bibliothèque au propre une fois,
  ne plus s'en soucier. Succès = bibliothèque disque propre.
- **Préparer un set** : nettoyer/ranger juste avant de jouer. Succès = collection
  prête dans Rekordbox.

## Chemin utilisateur

### Deux natures, jamais l'une sans l'autre
- **Flux linéaire** pour la grosse conversion de bibliothèque (colonne
  vertébrale : voir les stations ci-dessous).
- **Boîte à outils à la carte** : chaque station est aussi un point d'entrée
  indépendant — un utilisateur peut n'avoir besoin que d'un seul outil
  ponctuellement (juste dédoublonner, juste identifier, juste exporter…).

### Cycle de vie en deux temps
1. **Conversion initiale** : import manuel d'un gros dossier (ou de plusieurs
   dossiers éparses) → traitement de tout le stock existant.
2. **Régime permanent** : une fois la bibliothèque convertie, un **watcher** sur
   les dossiers sources prend en charge les téléchargements futurs au fil de
   l'eau.

### Les stations (colonne vertébrale)
Entrée → Analyse → Dédup → Identification → Rangement → (Export).

- **Entrée** : import manuel d'un dossier (conversion initiale) et/ou dossiers
  surveillés (watcher, régime permanent).
- **Analyse** — *automatique* : verdict de qualité (faux/vrai lossless, clipping,
  troncature, etc.).
- **Dédup** — *automatique* : les doublons sont signalés — par **clé de nom
  normalisée** (`name_key`) sur le flux entrant, et par **empreinte acoustique**
  (chromaprint) à la demande (doublons internes de la Bibliothèque). L'utilisateur
  tranche toujours ce qu'il garde.
- **Identification** — *auto-proposée, confirmée par l'humain* : Discogs propose,
  l'utilisateur valide/corrige (titre, année, pochette, genre).
- **Rangement** — *décision humaine obligatoire* : l'utilisateur **choisit
  toujours la destination** (aucun rangement auto par genre aujourd'hui). Deux
  modes selon le soin voulu :
  - **par lot** vers un bac, pour le gros du volume à ranger vite ;
  - **titre par titre** au poste Revue, pour les morceaux qu'on veut soigner
    (écouter, vérifier) avant de ranger.
- **Sortie / Export** — *satellite optionnel* : export/synchro **Rekordbox**
  (XML, master.db) et/ou formatage **clé USB** pour jouer sur CDJ.

### Branche de rejet et surfaces permanentes
Le parcours n'est pas que le chemin « ranger » ; trois surfaces le complètent —
chacune aussi utilisable à la carte :

- **Écartés** (branche de rejet) : quand l'utilisateur écarte un morceau (mauvaise
  qualité, pas voulu), il n'est **pas supprimé** — il est parqué dans Écartés,
  d'où on peut le **re-sourcer** (liens d'achat, copie du nom pour Soulseek) ou
  l'envoyer à la **corbeille**. C'est le pendant direct de « ne jamais perdre un
  original » : rejeter ≠ effacer.
- **Bibliothèque** : après conversion, parcourir/éditer/re-ranger la collection
  propre, repérer les **doublons internes** (empreinte), voir un **tableau de
  bord** de stats. Surface du régime permanent et de la gestion continue.
- **Journal d'actions** : chaque action de rangement (surtout par lot) est
  **journalisée et annulable** (revert). C'est le support concret de la
  réversibilité — l'utilisateur peut défaire ce qu'il vient de faire.

## Comportements (quand X → Y)

- Quand l'utilisateur importe un dossier (ou qu'un fichier apparaît dans un
  dossier surveillé) → le morceau est mis en file « à traiter ».
- Quand un morceau est analysé → un verdict de qualité est produit et visible
  (faux lossless signalé explicitement).
- Quand deux morceaux se ressemblent (même `name_key`, ou même empreinte à la
  demande) → le doublon est signalé ; Sift **propose** de garder le meilleur
  (verdict de qualité : vrai lossless > faux lossless > bitrate/format), mais la
  décision reste **manuelle** (jamais d'auto-résolution). Le perdant part en
  Écartés (récupérable), jamais supprimé sec.
- Quand l'utilisateur demande l'identification → Discogs propose des candidats ;
  rien n'est appliqué avant confirmation.
- Quand l'utilisateur écarte un morceau → il part dans Écartés (re-sourçable ou
  corbeille), jamais supprimé sèchement.
- Quand l'utilisateur choisit une destination → le morceau est **encodé** (au
  besoin) **et rangé** dans un seul geste (« déplacer = encoder + ranger »).
- Quand l'utilisateur range (titre ou lot) → l'action est **réversible**
  (corbeille/undo, journal des actions).
- Quand l'utilisateur exporte vers Rekordbox → l'écriture master.db est
  **sauvegardée puis vérifiée** avant d'être validée ; les chemins restent
  valides (pas de relink), playlists/cue points préservés.

## Hors-scope explicite

- **Télécharger/acquérir la musique** : Sift identifie et peut donner des liens
  (achat, Soulseek), mais **ne télécharge pas** — l'acquisition se fait ailleurs.
- **Lecteur / logiciel de mix** : l'écoute existe **pour vérifier** la qualité
  et l'identité au poste Revue, pas pour jouer ou mixer. Sift n'est ni un player
  de bibliothèque ni un outil de performance.
- **Autres écosystèmes DJ** (Serato, Traktor, Engine DJ) : **hors-scope actuel**.
  Le cœur (analyse/dédup/identification/rangement/encodage) est agnostique du
  logiciel DJ ; seul l'export est Rekordbox pour l'instant. Porte ouverte plus
  tard, non engagé.
- **Rangement automatique par genre** (ou autre auto-classement de destination) :
  **non implémenté**, piste future — aujourd'hui la destination est toujours
  choisie par l'utilisateur.

## Contraintes d'inacceptable (planchers durs, par rang)

**Tier 1 — promesse centrale (violée une fois = confiance détruite) :**
1. **Ne jamais perdre un original.** Aucune suppression/écrasement d'un fichier
   original sans filet récupérable (corbeille/undo). Toute action destructive est
   réversible ; l'original survit toujours.
2. **Ne jamais dégrader en cachette.** Jamais produire un fichier de moindre
   qualité que l'original (upscale, ré-encodage lossy d'un lossless) sans le
   signaler. Garde-fou anti-upscale strict, transparence totale sur l'encodage.

**Plancher — intégrité de l'écosystème :**
3. **Ne jamais casser Rekordbox.** Aucune corruption du master.db, aucune perte
   de cue points / playlists / historique. Écriture toujours sauvegardée et
   vérifiée (round-trip + rollback) avant d'être confirmée.

## Terminé = démontrable

Deux jalons de succès distincts (l'utilisateur s'arrête à l'un ou l'autre selon
son contexte d'usage) :

- **Jalon 1 — bibliothèque disque propre** (fin du parcours cœur) : tous les
  morceaux traités sont encodés proprement, rangés dans la structure de dossiers
  voulue par l'utilisateur, sans doublon, avec tags corrects. Démontrable dans
  l'explorateur de fichiers et l'écran Bibliothèque de Sift.
- **Jalon 2 — prêt à jouer (optionnel)** : la collection est importée/synchro
  dans Rekordbox avec chemins valides (pas de relink) et playlists, prête à
  charger sur CDJ / clé USB. Démontrable dans Rekordbox et sur la clé.

## Annexe — Réalité technique (déjà en place, rétro-PRD)

Ce ne sont pas des choix à faire mais la stack livrée ; chaque ligne justifie le
choix face au besoin d'usage.

- **App desktop native Win+Mac → Tauri v2.** Fenêtre native légère, backend Rust
  pour le traitement audio lourd, distribution gratuite simple.
- **Analyse audio pur Rust → Symphonia (décodage) + rustfft.** Détection faux
  lossless / qualité sans dépendance externe au décodage.
- **Encodage → FFmpeg bundlé (sidecar).** Encodage fiable multi-formats sans
  installation par l'utilisateur.
- **Dédup → `name_key` (nom normalisé) + empreinte acoustique locale
  (rusty-chromaprint) à la demande.** Détecte les doublons au nom puis au contenu.
  AcoustID en ligne = piste future, non implémenté.
- **Identification → API Discogs.** Base de métadonnées DJ/vinyle de référence.
- **État → SQLite (rusqlite).** File, cache d'analyse, bacs, journal d'actions —
  local, sans serveur.
- **Intégration Pioneer → lecture/écriture master.db + export XML Rekordbox.**
  Seule voie fiable pour livrer une collection jouable sur CDJ.

---

**PRD prêt.** Prochaine étape possible : `superpowers:brainstorming` pour concevoir
une évolution précise (ex. auto-rangement par genre, ou support d'un autre
écosystème) à partir de ce QUOI — une fois ce PRD validé par Antoine.
