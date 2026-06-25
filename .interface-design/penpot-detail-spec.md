# Penpot board spec — faithful implementation reference

> Extrait des boards canoniques (page « Sift — shell », 1240×760) le 2026-06-25 via le
> MCP Penpot, pour implémenter l'app **fidèlement** au visuel (pas seulement aux règles).
> Boards : **Detail** `49975f37-649c-80c0-8008-39eb475e8b73` · **Batch**
> `284acdb7-967e-8038-8008-3a1f415c4596`. Ordre de build validé : Detail → nav → Batch.

## Palette exacte (fills relevés)

| Rôle | Valeur | Shapes |
|---|---|---|
| Fond app | `#2c2c2a` | board, audi |
| Inset / well / dest / seg-bg | `#242422` | dest, seg-bg, topseg-well |
| Pill segmenté actif / carte verdict | `#3b3b39` | seg-act, topseg-act, verc |
| Surface queue (hover) | `#313130` | qs, idc |
| Sélection queue (ligne active) | `#ffffff@0.06` | na-Revue, qs |
| Tint colonne centrale | `#ffffff@0.028` | col2-tint |
| Texte primaire | `#f3f3f1` / titre `#f4f3f0` | d-title, sel-bar |
| Texte secondaire | `#c9c8c1` | nl-Revue (actif), qt, idt |
| Texte tertiaire / labels nav / groupes | `#a5a49c` | g-*, nl-*, dest, dfile |
| Texte tertiaire 2 | `#aeada5` | dv, d-artist, src |
| Marque « Sift » | `#f0ede6` | brand |
| Vert succès (texte/label) | `#5bc08c` | vlab, badge-tx |
| Panneau verdict vert (bg) | `#5bc08c@0.2` | vb |
| Chip LOSSLESS bg | `#5bc08c@0.14` | badge-bg |
| Progress fill / dot ok | `#3c9f70` | prog-fill, qd |
| Dot pending | `#dda63f` | pending-dot |
| Dot danger | `#cf564c` | qd |
| Bouton bleu (File & encode) | `#2f6fe0` (texte `#e5eeff`) | rbtn |
| Bouton Discard | bg `#e2685e@0.2`, texte `#ef8b81` | ebtn, ebtnt |

## Typo
Outfit partout. Tailles relevées : groupes nav `9/400`, nav label `12.5/400`, label rail
(DESTINATION…) `11/600`, queue titre `11/500`, « Ready to file » `16/600`. Titre hero (d-title)
grand (~28). JetBrains Mono pour les valeurs mono (path, kbps, kHz, final name).

## Écran Detail — composition (gauche→droite)
1. **Nav rail** : marque « ▼ Sift » ; groupes **TRAITEMENT** (Accueil, Revue [badge 18]),
   **ORGANISATION** (Sources, Bibliothèque), **EXPORT** (Rekordbox•, Clé USB•) ; Réglages en bas.
   Ligne active = surface `#fff@.06` + barre gauche (`sel-bar`). (NB labels FR dans le board ; app = EN.)
2. **Segmented haut-centre** `topseg` : well `#242422`, actif `#3b3b39` + texte `#f3f3f1`,
   inactif `#a5a49c` — **Detail | Batch**.
3. **Colonne queue** : onglets **Queue · Discarded · Trashed** (dot par onglet) ; `prog-label`
   « 12/18 analyzed · in progress » + `prog-track`/`prog-fill` ; liste : dot statut + titre `qt`,
   ligne sélectionnée = `#fff@.06` + `sel-bar`.
4. **Centre son-d'abord** : `cover` 64 + `d-title` (~28) + `d-artist` + version (`src`) + `dpath` ;
   bande **audi** (play `playb` rond clair + waveform `wf*` (barres `#f3f3f1` opacité variable) +
   temps + chip PITCH) ; ligne **CLAIMED** (`cov-l`/`r`) FLAC · kbps · kHz ; **panneau verdict** `vb`
   (`#5bc08c@.2`) : `vlab` « ✓ Ready to file » + chips **LOSSLESS · NN% MATCH · UNIQUE** (`badge-*`) +
   `vsub` « ▸ Proof (spectrum) » ; **IDENTIFICATION · Discogs** carte `idc` (cover+titre+label/année+genres+Change) ;
   footer hints `kbd` « SPACE listen · ENTER file · BKSP discard · ↑↓ navigate ».
5. **Rail FILE** (droite) : `r-h` FILE ; **DESTINATION** dropdown (`dest` + chevron `i-chev`) ;
   **FINAL NAME** (`dfile`) ; **GENRES** (`tag`/`tag-t`) ; **FORMAT** (FLAC | AIFF · 16-bit · 44.1 kHz) ;
   **File & encode** (`rbtn` bleu) ; **Discard** (`ebtn`) ; `undot` « ↩ Undo last filing ».

## Écran Batch (à faire après le nav)
Même nav + queue. Centre : **READY TO FILE · N** (rows checkbox + titre + chip LOSSLESS + format +
match%) + « Select all N » ; **NEEDS REVIEW · M** (rows non cochables + chips LOSSY/DUPLICATE/NO MATCH
+ « open each in Detail »). Barre bas : DESTINATION dropdown + **Discard (N)** + **File selection (N)** bleu.
Rail droite : **SELECTION** « N selected » ; **DESTINATION** ; **WILL ENCODE** « X tracks → AIFF, Y already
lossless » ; **EXCLUDED** « M need review · filed safely only when clean ».
