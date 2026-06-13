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

**In (M5) — nom d'abord, son pour confirmer (léger) :**
- **Préfiltre par le nom** : comparer les noms normalisés (artiste + titre réconciliés).
  Cheap, sur **tous** les morceaux, dès l'import. C'est l'entrée de la détection.
- **Confirmation par le son, à la demande** : uniquement pour les morceaux qui **matchent
  déjà par le nom**, calculer/comparer l'empreinte Chromaprint pour confirmer que c'est bien
  le même enregistrement. On ne calcule **pas** l'empreinte de tout le monde — seulement quand
  un nom concorde. L'empreinte calculée est mise en cache dans `tracks.fingerprint`.
- `find_duplicate(track_id)` : (1) candidats par nom parmi `pending` + `filed` ; (2) pour
  ceux-là, confirmer par le son. Renvoie le meilleur match + sa nature : `both` (nom **et**
  son = sûr) ou `name` (noms concordent mais sons différents/indisponibles = à vérifier).
- Matcher acoustique générique (réutilisable par le futur scan biblio).
- Signalement en Revue (bannière, formulée selon la nature) ; aucune action nouvelle
  (l'utilisateur Écarte ou Range).

**Out (M5) :** doublons au **même son mais noms totalement différents** (ex. `track01.mp3`
vs un nom propre) — non attrapés ici puisqu'on part du nom ; relèvent du scan biblio (M5b).

**Out (M5b et au-delà) :**
- Scan de la biblio existante (backfill des empreintes manquantes + groupement + UI Biblio de
  résolution garder/jeter). Réutilise le moteur de M5 — voir « Extensibilité ».
- Suggestion auto « garder le meilleur » sur doublons en file.

## Key decisions

| Sujet | Décision |
|---|---|
| Ordre | **Nom d'abord, son ensuite.** Le nom normalisé (artiste+titre réconciliés, minuscules, ponctuation/accents normalisés) filtre les candidats ; le son ne sert qu'à **confirmer** les candidats trouvés par le nom. Plus léger : pas d'empreinte calculée pour tout le monde. |
| Empreinte | `rusty-chromaprint` (crate Rust pur) nourri par le PCM décodé, **calculée à la demande** (sur un match de nom) et **mise en cache** dans `tracks.fingerprint` pour ne pas recalculer. Pas de binaire `fpcalc` à bundler. |
| Nature du match | `both` (nom + son concordent) = sûr ; `name` (noms concordent, sons différents ou empreinte indisponible) = « à vérifier ». La bannière formule selon la nature. |
| Quand | À la demande, lors d'un match de nom (pas dans la passe d'analyse). Mise en cache une fois calculée et conservée à travers le classement (filing ne l'efface pas). |
| Périmètre M5 | Flux entrant uniquement : doublons dans la file + contre les morceaux déjà rangés **par Sift** (qui ont une empreinte). |
| Comparaison | Similarité via le matcher du crate, seuil prudent (un remix = audio différent = pas un doublon). Pairwise sur un petit set candidat. |
| Action | **Signalement seul** : bannière en Revue, l'utilisateur décide (Écarter / Ranger quand même). Pas de blocage, pas d'auto-route. |
| Stockage | Réutilise `tracks.fingerprint` (déjà au schéma). Pas de migration. |

## Architecture

### Moteur (Rust, `src-tauri/src/`)

- **Clé de nom (dans `naming.rs`, réutilisé)** : `name_key(&Canonical) -> String` — artiste +
  titre normalisés (minuscules, espaces collapsés, ponctuation/accents retirés) pour comparer
  deux noms de façon robuste. Pur, déjà adossé à `reconcile`. C'est le **filtre d'entrée**.
- **`fingerprint.rs`** — l'empreinte sonore, réutilisable, sans I/O DB :
  - `compute_for_path(path) -> Result<Vec<u32>, _>` : décode le fichier (réutilise le décodeur
    de l'analyse) puis calcule l'empreinte Chromaprint. Appelé **à la demande**, pas dans la
    passe d'analyse.
  - `encode(&[u32]) -> String` / `decode(&str) -> Vec<u32>` : sérialisation compacte pour la
    colonne `tracks.fingerprint`.
  - `similarity(a, b) -> f32` (0..1) + `MATCH_THRESHOLD`, et `best_match(target, candidates)` :
    le cœur de comparaison, réutilisable par le futur scan biblio.
- **`dedup.rs`** — la couche DB, **nom d'abord puis son** :
  - `find_duplicate(conn, track_id) -> Option<DupMatch>` :
    1. **nom** : trouver les candidats (`pending` ≠ lui + `filed`) dont le `name_key` est égal ;
    2. **son (à la demande)** : pour ces candidats seulement, récupérer l'empreinte
       (`tracks.fingerprint` si déjà en cache, sinon `compute_for_path` + écrire le cache),
       comparer via `best_match` ;
    3. renvoyer `DupMatch { id, status, folder, filename, kind, score }` où `kind = both` si
       le son confirme, sinon `name`. `None` si aucun nom ne concorde.
  - S'il n'y a **aucun match de nom**, on ne touche pas au son → léger.
  - (M5b : `scan_library() -> Vec<DupGroup>` se posera ici, mêmes `name_key` + `best_match`.)
- **IPC (`ipc_filing.rs`)** : `find_duplicate(track_id) -> Option<DupMatch>`.
- **Worker / analyse :** *inchangé* — on **n'ajoute pas** le calcul d'empreinte à la passe
  d'analyse. L'empreinte ne se calcule que lors d'un match de nom (et reste en cache ensuite).

### Frontend (`frontend/`)

- `shared/contracts.ts` : `DupMatch { id, status, folder, filename, kind, score }`.
- `ipc.ts` : `findDuplicate(trackId)`.
- `filing.ts` : à l'ouverture d'un morceau (`openFilingInto`), appeler `findDuplicate` ; si
  match, afficher une bannière en tête du `#mid` (style maquette `dupBanner`), formulée selon
  `kind` :
  - `filed` → « Déjà rangé : <dossier>/<nom> » ; `pending` → « Doublon d'un fichier en file :
    <nom> ».
  - `both` = affirmatif (« doublon ») ; `name` = prudent (« même artiste/titre — possible
    doublon, à vérifier »).
  Aucune action nouvelle (Écarter existe déjà).

## Flux de données

1. Scan → `pending` (nom déjà connu → le filtre par nom est prêt immédiatement).
2. Revue ouvre un morceau → `find_duplicate` : match par nom ? si oui, on calcule/compare le
   son (et on met l'empreinte en cache) → bannière (`both` sûr, `name` à vérifier). Sinon rien.
3. L'utilisateur Écarte (→ corbeille/Écartés, M4b) ou Range quand même.

## Gestion d'erreurs

- Pas de match de nom → `find_duplicate` renvoie `None` sans toucher au son.
- Match de nom mais empreinte impossible (audio trop court/corrompu, fichier déplacé) →
  renvoie `kind = name` (« à vérifier »), jamais de blocage ni de crash.
- Seuil prudent pour éviter un faux « doublon son » entre morceaux proches mais différents.
- `find_duplicate` écrit seulement le cache d'empreinte (`tracks.fingerprint`) quand il le
  calcule ; aucune autre mutation.

## Tests

- `fingerprint.rs` : déterminisme (même fichier → même empreinte) ; `similarity` ~1 pour deux
  encodages d'une même source (mp3 vs flac de la même piste, fixtures), ~bas pour deux pistes
  différentes ; round-trip `encode`/`decode`.
- `naming.rs` : `name_key` égalise « Larry Heard - Mystery of Love » et « larry_heard mystery
  of love.mp3 », distingue deux titres différents.
- `dedup.rs` : sans match de nom → `None` (et le son n'est pas calculé) ; match de nom +
  son qui confirme → `kind=both` (`pending`↔`pending` et `pending`↔`filed`) ; match de nom
  mais son différent/indisponible → `kind=name` ; s'ignore lui-même ; l'empreinte calculée
  est bien mise en cache (pas recalculée au 2ᵉ appel).
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
