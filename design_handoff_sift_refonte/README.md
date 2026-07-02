# Handoff : Refonte UI Sift

## Vue d'ensemble
Sift est un assistant DJ (Tauri + vanilla TS) qui analyse, identifie et range des fichiers audio.
Cette refonte redessine les 6 écrans principaux de l'app autour d'une grammaire à 3 colonnes
fixes (Nav / Queue / Inspecteur) et d'un système de couleur strictement à 2 teintes sémantiques
(vert = OK, ambre = doute/erreur), sur une palette neutre gris chaud (clair + sombre).

## À propos des fichiers de design
Les fichiers HTML de ce dossier (`Sift.dc.html`, dossier `screenshots/`) sont des **références de
design** — des maquettes montrant l'apparence et le comportement visés, pas du code de production
à copier tel quel. La tâche consiste à **recréer ces designs dans l'environnement existant du
projet** : vanilla TypeScript + Tauri, en réutilisant les patterns déjà en place dans le repo
(`frontend/styles.css`, les modules de vue comme `report-view.ts`, `chrome.ts`, etc.) plutôt qu'en
important du HTML/CSS généré ailleurs.

## Fidélité
**Haute-fidélité (hifi).** Couleurs, typographie, espacements et la plupart des interactions sont
définitifs. Le développeur doit recréer l'UI pixel-perfect avec les libs/patterns déjà présents
dans le repo (pas de nouvelle lib UI à introduire).

## Grammaire de layout (partagée par tous les écrans)
- **Nav** — 152px, fixe. Fond `var(--nav)`, padding `16px 12px 14px`. Logo (carré 16×16px arrondi
  `4px`, couleur CTA) + "Sift" 18px/600. 6 items de nav (36px de haut, `border-radius:7px`,
  `font-size:12.5px`), compteur en JetBrains Mono 9.5px aligné à droite. Sous les items : zone de
  tâches actives (cartes, masquée si aucune tâche), puis section "Export" (Rekordbox / Clé USB,
  32px de haut, pastille ambre 5px, opacité 0.55 au repos).
- **Queue** — 272px, fixe. Fond `var(--queue)`, header 16px/16px/11px avec titre 13px/600 +
  compteur mono, et pour Revue seulement un toggle segmenté Détail/Lot (fond `var(--track)`,
  padding 2px, pilules 11px).
- **Inspecteur** — flexible (`flex:1`), contenu de l'écran actif, defile verticalement.

Ordre de nav (verrouillé) : **Accueil, Revue, Écartés, Journal, Bibliothèque, Réglages**.

## Écrans

### 1. Accueil — `screenshots/01-accueil.png`
- **But** : superviser les dossiers surveillés (sources), voir leur statut, activer/désactiver la
  surveillance.
- **Layout** : Queue = liste des sources (nom + statut : nouveau/à jour/inaccessible, pastille
  colorée). Inspecteur = détail de la source sélectionnée : fil "Accueil › {source}", titre + chip
  "N nouveaux" (vert), carte "Dossier surveillé" (chemin en JetBrains Mono), bannière contextuelle
  ambre si le dossier surveillé n'est probablement pas le bon (ex: viser `Completed` plutôt que
  `Incomplete`), toggle "Surveiller ce dossier" (case + libellé).
- **État vide** (racine de bibliothèque non définie) : aligné en haut, titre + note explicative,
  PAS de lien "Aller à Revue" (Accueil est déjà le point d'entrée).
- **Barre du bas** : nav clavier (↑↓) + CTA "+ Ajouter un dossier" (pill pleine, `var(--ctaBg)`).

### 2. Revue — `screenshots/02-revue.png` (écran de référence, le plus riche)
- **Queue** : liste de pistes, chaque ligne = titre/artiste + chip statut à droite (lossless vert
  / fake ambre / "analyse…" gris neutre en cours). Ligne sélectionnée : fond `var(--rowActive)`
  + barre latérale 2px `var(--ctaBg)`.
- **Inspecteur**, de haut en bas :
  1. Fil d'ariane "Revue › {titre}".
  2. Fichier source **fusionné** sous titre/artiste (pas de case séparée) : pochette 64×64
     (placeholder rayé si absente), titre 20px/600, artiste + type ("Original"), nom de fichier
     complet en JetBrains Mono tronqué avec ellipse à gauche (`…(04) [40 Thieves ft Qzen]…`).
  3. **Barre de lecture** : waveform pleine largeur (barres verticales, hauteur variable = pics
     réels), temps encadrant écoulé/restant de part et d'autre du bouton play (cercle 40px), clic
     sur la waveform = seek. En dessous : slider **Volume** (fill depuis la gauche, poignée ronde
     13px, piste 3px, pas de %), **Key-lock** au centre (toggle bordé "VERR"/off), slider **Tempo**
     (fill depuis le centre à 0%, repère visuel au milieu, plage réelle -8%..+8%, valeur affichée
     en haut à droite ex. "+0%"). Aucun de ces sliders n'est un `<input type=range>` natif — ce
     sont des divs draggables (mousedown/mousemove) pour permettre ce style de fill asymétrique.
  4. **Preuves** : titre section + chip statut pilule pleine largeur relative (LOSSLESS vert /
     FAKE ambre), visible seulement si un verdict existe. Chip **DUPLICATE** séparée, affichée
     uniquement s'il y a un vrai doublon détecté (jamais de chip "UNIQUE"). Le panneau, une fois
     déplié, affiche une grille de métriques réelles issues du moteur d'analyse (voir mapping
     champs plus bas) : Verdict, Durée, True-peak, Écrêtage, Silence début/fin, Tronqué, Fréquence
     d'échantillonnage, Coupure, Canaux, DC offset, Corrélation de phase, Conteneur OK, Pics.
  5. **Identification · Discogs** : Label / Année / Genre (chaque genre = un tag pilule séparé,
     pas une chaîne concaténée), Compatibilité CDJ, Version ID3. Bouton "Modifier" = bouton bordé
     (pas un lien) qui ouvre un popover de candidats Discogs alternatifs. Badge MATCH/CHECK MATCH
     seulement en cas de doute réel (`CHECK MATCH`, ambre) — un match propre n'affiche rien.
     Morceau non identifié : carte simplifiée avec bouton "Rechercher sur Discogs" → état de
     recherche (icône ⟳) → liste de candidats cliquables qui appliquent label/année/genre en un
     clic.
  6. **Barre du bas** (fixe) : sélecteur Destination (bouton bordé, ouvre popover arborescence —
     voir plus bas), boutons format MP3/AIFF/WAV (segmenté), CTA secondaire rail contextuel
     ("Écarter" par défaut, devient "Ressourcer" ambre si le fichier est FAKE).

### 3. Revue — Mode Lot — `screenshots/03-revue-lot.png`
- Activé via le toggle Détail/Lot en haut de la Queue. Vue dédiée (pas de simples checkboxes en
  overlay) : groupé par statut **Prêts / À vérifier / En analyse**, case à cocher par groupe (tout
  sélectionner) et par piste, compteur "N sélectionné(s) sur Total" dans le titre.
- Cliquer "Déplacer la sélection" lance une simulation de rangement PISTE PAR PISTE (états
  wait → running → done/fail, ~450ms/piste, ~15% d'échec simulé), affichée en liste live dans le
  panneau, + une tâche "Rangement" apparaît dans le rail de nav (barre de progression + bouton
  Arrêter). À la fin : pistes réussies → Bibliothèque + une entrée Journal groupée ; échouées →
  reviennent dans la file.

### 4. Écartés — `screenshots/04-ecartes-vide.png`
- Liste des pistes écartées depuis Revue, avec possibilité de restaurer ("Remettre en revue").
- État vide (illustré) : aligné en haut, titre + note + lien "Aller à Revue →" (cul-de-sac réel).

### 5. Journal — `screenshots/05-journal.png`
- Historique d'actions (Filé / Écarté), chaque ligne = action + timestamp. Le détail d'une entrée
  **groupée** (filing par lot) liste chaque piste filée individuellement (jamais juste "Filé N
  pistes"). Bouton "Annuler cette action" (revert) — si l'entrée couvre plus de 10 pistes, une
  confirmation explicite est demandée avant d'exécuter.

### 6. Bibliothèque — `screenshots/06-bibliotheque.png`
- Pistes déjà rangées. Actions : "Mettre à jour les tags" (confirme par toast), export vers
  Rekordbox / Clé USB (lance une vraie tâche de progression simulée dans la zone de progression,
  pas un placeholder statique), "Remettre en revue".
- État vide : même traitement qu'Écartés (lien "Aller à Revue →").

### 7. Réglages — `screenshots/07-reglages.png`
- Une seule page qui défile, séparée par carte : **Discogs** (jeton d'accès, bouton Modifier),
  **Bibliothèque** (dossier racine + bouton Changer… + lien "Oublier le dossier racine" pour
  démontrer l'état vide d'Accueil), **Fichiers** (format par défaut au filing), **Apparence**
  (thème Auto/Clair/Sombre, segmenté 3 options). PAS d'onglets exclusifs : le rail Queue liste les
  catégories + sous-champs et un clic fait défiler (scroll-to) jusqu'à la bonne carte.

### Popover Destination (ancré, tous écrans avec sélection de fichier)
- Ancré au bouton "Destination" en `position:fixed` (coordonnées recalculées à l'ouverture et au
  resize via `getBoundingClientRect` — ne PAS utiliser `position:absolute`, qui se ferait clipper
  par l'`overflow:hidden` du conteneur racine).
- Contenu : légende du chemin disque réel en haut, arborescence réelle (carets expand/collapse,
  "+ nouveau" imbriqué pour créer un sous-dossier), filtre texte qui bascule vers une liste plate
  des résultats, checkbox "Sur place" (ne pas déplacer le fichier), ligne "📁 Parcourir un autre
  dossier…" pour sortir de l'arborescence interne (démo : `window.prompt`).
- Fermeture : clic sur backdrop invisible, ou touche Échap.

## Interactions & comportements transverses
- **Zone de progression** : cartes de tâches (Analyse / Identification / Rangement / Export) en
  bas de la Nav, masquées si aucune tâche active ; migrent dans le rail d'action pendant le Mode
  Lot. Bouton "Arrêter" sur la tâche de filing en cours.
- **Rail contextuel secondaire** (barre du bas de Revue) : "Écarter" par défaut ; devient
  "Ressourcer" (ambre) si le fichier en cours est FAKE.
- **Hover** : quasi tout élément cliquable a un état hover — fond `var(--rowActive)` pour
  lignes/listes, `filter:brightness(0.93–0.95)` pour boutons pleins/chips.
- **Mode sombre** : suit `prefers-color-scheme` par défaut ; override manuel Auto/Clair/Sombre
  dans Réglages > Apparence, persisté dans `localStorage` sous la clé `sift-theme`.

## Design Tokens
Voir **`DESIGN.md`** (inclus dans ce dossier) pour la liste complète et exacte : palette mode
clair, palette mode sombre, couleurs sémantiques, typographie (Outfit + JetBrains Mono), grammaire
de layout, et spec de chaque composant réutilisable (carte, bouton bordé, CTA primaire, pastille
clavier, chip de statut, tag genre, slider custom, état vide). Ces valeurs sont la source de
vérité — ne pas en réinventer de nouvelles pendant l'implémentation.

Points à ne pas perdre en implémentant :
- **2 couleurs sémantiques seulement** (vert `#4C7B57` / ambre `#B07A28`) — jamais de bleu ni de
  rouge, jamais une 3e teinte. Le gris neutre sert pour "en cours" (pas un jugement).
- Verdict réel dans le moteur : `ok | fake | grey`. **MATCH est qualitatif** (`MATCH` vert /
  `CHECK MATCH` ambre selon confidence) — **jamais un pourcentage**.
- **KEY** sur la barre de transport = verrou de tonalité (key-lock), **PAS un code Camelot**.
- **Tempo réel** = slider entier `-8%..+8%`, fill depuis le centre.

## Mapping vers le code réel (`dj-assistant-m6a`, lecture seule)
Stack : Vite vanilla TS + Tauri. Fichiers à modifier/étendre par écran :
- **Transport / Preuves / Identification (Revue)** → `frontend/report-view.ts`, `identify-shared.ts`
- **Zone de progression** → `frontend/progress-zone.ts` (`mountProgressZone`) — tâches
  analyze/identify/file, états `running/stopping/error/done`, bouton Stop optionnel par tâche.
- **Mode Lot** → `frontend/batch-tracklist.ts`
- **Rangement / Journal / undo** → `frontend/filing.ts`
- **Accueil / sources surveillées** → `frontend/home-sources.ts`
- **Nav / structure 3 colonnes** → `frontend/chrome.ts`
- **Styles globaux existants** → `frontend/styles.css` (base à faire évoluer vers les tokens de
  `DESIGN.md`, pas à remplacer d'un bloc)
- **Contrat de données** → `shared/contracts.ts` — `AnalysisReport` réel : `cutoff_hz`,
  `clip_runs`/`clip_pct`, `true_peak_dbtp`, `dc_offset`, `phase_correlation`,
  `silence_head_ms`/`silence_tail_ms`, `truncated`, `container_ok`, `codec_error`, `id3_version`,
  `tags_cdj_ok`, `has_cover`, `spectrogram`, `peaks`. Tous ces champs sont ceux affichés dans le
  panneau Preuves déplié de la maquette — ne pas en inventer d'autres.

Documents de contexte déjà lus côté repo (à consulter si besoin de creuser une décision produit) :
`PRODUCT.md`, `docs/brief-refonte-ui-2026-07-01.md`, `docs/plan-implementation.md`,
`docs/superpowers/plans/m2a-analysis-engine`, `docs/superpowers/plans/m4b-ecartes`.

## Assets
Aucun asset externe (photo/logo) — les pochettes et illustrations sont des placeholders rayés
génériques dans la maquette ; à remplacer par les vraies pochettes/couvertures issues des fichiers
audio une fois branché sur le vrai pipeline.

## Fichiers de ce dossier
- `README.md` — ce document.
- `DESIGN.md` — design tokens complets (couleurs, typo, composants), clair + sombre.
- `Sift.dc.html` — maquette hi-fi interactive complète, référence visuelle et comportementale pour
  les 6 écrans + mode Lot + popover Destination. Ouvrir dans un navigateur pour interagir.
- `screenshots/` — captures des 6 écrans + mode Lot, en pleine résolution desktop (1440×900),
  pour référence rapide sans ouvrir le HTML : `01-accueil.png`, `02-revue.png`,
  `03-revue-lot.png`, `04-ecartes-vide.png`, `05-journal.png`, `06-bibliotheque.png`,
  `07-reglages.png`.
