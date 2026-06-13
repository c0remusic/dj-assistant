# M5 — Déduplication par empreinte acoustique — Design

**Status:** approved for planning
**Date:** 2026-06-13
**Milestone:** M5 (fin du MVP — dédup)

## Goal

Détecter les doublons **indépendamment du nom de fichier et de l'encodage**, par empreinte
acoustique (Chromaprint). M5 livre la détection sur le **flux entrant** (signalement en
Revue). L'architecture est conçue pour que le **scan de la bibliothèque existante** (doublons
internes, vue de résolution dans l'onglet Bibliothèque) se branche dessus ensuite (M5b) sans
refonte.

## Scope

**In (M5) :**
- Empreinte Chromaprint calculée à l'analyse (le worker décode déjà le PCM), stockée dans
  `tracks.fingerprint`, sur **tout** morceau analysé.
- Matcher générique : comparer une empreinte à un set de candidats, renvoyer le meilleur match
  au-dessus d'un seuil conservateur.
- `find_duplicate(track_id)` : compare un morceau aux `pending` + `filed` ayant une empreinte.
- Signalement en Revue (bannière) ; aucune action nouvelle (l'utilisateur Écarte ou Range).

**Out (M5b et au-delà) :**
- Scan de la biblio existante (backfill des empreintes manquantes + groupement + UI Biblio de
  résolution garder/jeter). Réutilise le moteur de M5 — voir « Extensibilité ».
- Suggestion auto « garder le meilleur » sur doublons en file.

## Key decisions

| Sujet | Décision |
|---|---|
| Empreinte | `rusty-chromaprint` (crate Rust pur) nourri par le PCM déjà décodé. Pas de binaire `fpcalc` à bundler. |
| Quand | Pendant la passe d'analyse existante (M2b worker). Une fois par morceau. Conservée à travers le classement (filing ne l'efface pas). |
| Périmètre M5 | Flux entrant uniquement : doublons dans la file + contre les morceaux déjà rangés **par Sift** (qui ont une empreinte). |
| Comparaison | Similarité via le matcher du crate, seuil prudent (un remix = audio différent = pas un doublon). Pairwise sur un petit set candidat. |
| Action | **Signalement seul** : bannière en Revue, l'utilisateur décide (Écarter / Ranger quand même). Pas de blocage, pas d'auto-route. |
| Stockage | Réutilise `tracks.fingerprint` (déjà au schéma). Pas de migration. |

## Architecture

### Moteur (Rust, `src-tauri/src/`)

- **`fingerprint.rs`** — pur, réutilisable, sans I/O DB :
  - `compute(samples: &[i16], sample_rate: u32, channels: u32) -> Result<Vec<u32>, _>` :
    calcule l'empreinte Chromaprint à partir du PCM (le worker fournit déjà du `s16`).
  - `encode(&[u32]) -> String` / `decode(&str) -> Vec<u32>` : sérialisation compacte pour la
    colonne `tracks.fingerprint` (ex. ints séparés par virgule, ou base64).
  - `similarity(a: &[u32], b: &[u32]) -> f32` (0..1) + `MATCH_THRESHOLD` : comparaison de deux
    empreintes. Le cœur réutilisable par le flux entrant ET le futur scan biblio.
  - `best_match(target: &[u32], candidates: &[(i64, Vec<u32>)]) -> Option<(i64, f32)>` :
    meilleur candidat au-dessus du seuil. Générique (ne connaît pas le statut des tracks).
- **`dedup.rs`** — la couche DB au-dessus du moteur :
  - `find_duplicate(conn, track_id) -> Option<DupMatch>` : charge l'empreinte du morceau +
    les empreintes des candidats (`pending` autres que lui + `filed`), appelle `best_match`,
    renvoie `DupMatch { id, status, folder, filename, score }` ou `None`.
  - (M5b : `scan_library() -> Vec<DupGroup>` se posera ici, même `best_match`.)
- **Intégration worker (`worker.rs` / `analysis`)** : après le décodage de la passe d'analyse,
  appeler `fingerprint::compute` et persister via `UPDATE tracks SET fingerprint=?`. Échec =
  laisser `fingerprint` NULL (pas de dédup pour ce morceau, non bloquant).
- **IPC (`ipc_filing.rs` ou `ipc.rs`)** : `find_duplicate(track_id) -> Option<DupMatch>`.

### Frontend (`frontend/`)

- `shared/contracts.ts` : `DupMatch { id, status, folder, filename, score }`.
- `ipc.ts` : `findDuplicate(trackId)`.
- `filing.ts` : à l'ouverture d'un morceau (`openFilingInto`), appeler `findDuplicate` ; si
  match, afficher une bannière en tête du `#mid` (style maquette `dupBanner`) :
  - même statut `pending` → « Doublon d'un fichier en file : <nom> » ;
  - `filed` → « Déjà rangé : <dossier>/<nom> ».
  Badge verdict peut afficher « doublon ». Aucune action nouvelle (Écarter existe déjà).

## Flux de données

1. Scan → `pending`. 2. Worker analyse : décode PCM → verdict + **empreinte** stockée.
3. Revue ouvre un morceau → `find_duplicate` → bannière si match. 4. L'utilisateur Écarte
(→ corbeille/Écartés, M4b) ou Range quand même.

## Gestion d'erreurs

- Empreinte impossible (audio trop court/corrompu) → `fingerprint` NULL, `find_duplicate`
  renvoie `None`. Jamais de blocage ni de crash.
- Seuil prudent pour éviter un faux « doublon » entre morceaux proches mais différents.
- `find_duplicate` est en lecture seule ; aucune mutation.

## Tests

- `fingerprint.rs` : déterminisme (même fichier → même empreinte) ; `similarity` ~1 pour deux
  encodages d'une même source (mp3 vs flac de la même piste, fixtures), ~bas pour deux pistes
  différentes ; round-trip `encode`/`decode`.
- `dedup.rs` : `find_duplicate` détecte un doublon `pending`↔`pending` et `pending`↔`filed`,
  ignore les morceaux sans empreinte, s'ignore lui-même.
- Front : type-check ; vérif manuelle de la bannière en Revue.

## Extensibilité — M5b (scan biblio, dans l'onglet Bibliothèque)

Conçu pour, pas livré ici. Le futur scan réutilise `fingerprint::{compute, similarity,
best_match}` et la colonne `tracks.fingerprint` :
1. **Backfill** : empreinter les fichiers déjà rangés (status `filed`) qui n'ont pas encore
   d'empreinte (rangés avant M5 ou importés autrement) — une passe sur leurs chemins.
2. **Groupement** : regrouper les `filed` par match acoustique en `DupGroup` (clustering
   pairwise via `similarity`).
3. **UI Biblio** : vue de résolution (garder / jeter) comme la maquette `renderBiblio`
   (DUP_GROUPS). Réutilise `trash_track`/`restore` de M4b.
Aucune des structures de M5 ne doit supposer « flux entrant uniquement » : `best_match` et le
stockage d'empreinte s'appliquent à n'importe quel ensemble de morceaux.

## Open items (non bloquants)

- Choix exact de sérialisation de l'empreinte (CSV d'ints vs base64) — tranché à l'implémentation.
- Valeur du seuil `MATCH_THRESHOLD` — calibrée sur fixtures (deux encodages d'une même piste).
