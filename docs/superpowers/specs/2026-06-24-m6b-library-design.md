# M6b — Onglet Bibliothèque (design)

> Statut : design validé (brainstorm 2026-06-24). Source de vérité du périmètre :
> la maquette `frontend/app.js` (`renderBiblio`), **améliorée** (cf. principe
> mockup-first : partir de la maquette mais enrichir/mieux intégrer, pas porter à
> l'identique). Méthode : fail fast, pas de fallback, changements chirurgicaux, TDD.

## But

Rendre la biblio rangée **visible et gérable** : parcourir les morceaux `filed`,
les ré-écouter, éditer leurs métadonnées, nettoyer les doublons internes, et lire
un tableau de bord — pour passer de « traiter le flux entrant » à « gérer sa
collection ». Ferme la boucle M6 (les métadonnées Discogs deviennent exploitables).

## Hors périmètre

- Onglets **Rekordbox** (`renderRkb`) et **Clé USB** (`renderCle`) de la maquette
  → jalon **M7**, pas ici.
- Réparation automatique des chemins Rekordbox → M8.

## Approche

Construire l'onglet dans l'**app TS réelle** (`frontend/main.ts`), pas dans le
shell maquette `app.js` (legacy, données en dur — sert de référence visuelle).
**Réutiliser `report-view.ts`** pour le détail par morceau (waveform/spectro/
verdict/lecteur déjà câblés) et le faire évoluer en **composant détail unifié**
partagé Revue ↔ Bibliothèque (play + verdict + édition + identify + actions) →
cohérence UX et zéro duplication.

Alternative écartée : recâbler `renderBiblio` de `app.js` (mauvais — c'est le
shell maquette non typé, hors architecture TS courante).

## Données (existant)

- `tracks` : `status` (`pending|filed|resourcing|trash`), `folder`, `duration`,
  `has_cover`, verdict, format/bitrate. Index `idx_tracks_status`.
- `metadata` (1:1 via `track_id`) : `artist, title, label, year, genre, bpm,
  cover_path, discogs_release_id, source`.
- `track_genres` : genres multiples par morceau.
- `settings` : `LIBRARY_ROOT` (racine biblio, déjà posée par « Choisir la racine »).
- Empreinte : `fingerprint.rs` (Chromaprint) + seuil de similarité existant ;
  `dedup.rs` (`find_duplicate`).

## Backend — nouvelles commandes (`ipc_library.rs`)

| Commande | Entrée | Sortie / effet |
|---|---|---|
| `list_library` | `folder?`, `quality?` (`all\|lossless\|mp3`), `q?` (recherche), `sort?` | `Vec<LibraryTrack>` : id, path, artist, title, format, bitrate, bpm, duration, year, label, genres[], discogs_release_id, has_cover/cover_path, verdict, folder. `WHERE status='filed'` + join metadata/track_genres, filtré/trié côté SQL. |
| `library_folders` | — | facettes pour la colonne gauche : par **dossier** (sous-dossiers réels de `LIBRARY_ROOT`) et par **genre** (`track_genres`), chacune avec compteur. |
| `scan_library_duplicates` | — | `Vec<DupGroup>` : groupes de morceaux `filed` acoustiquement identiques (empreinte Chromaprint, seuil existant), avec similarité. Chaque membre porte un flag `recommend_keep` (heuristique : lossless > lossy, puis bitrate, puis durée, puis non-tronqué ; égalité → 1er). |
| `update_metadata` | `track_id`, champs éditables (artist, title, label, year, genres[], cover) | Écrit **le fichier d'abord** (`tagging::write`) **puis** la DB (`metadata` + `track_genres`). Si l'écriture fichier échoue → `Err`, DB intacte (pas d'état incohérent). |
| `library_stats` | — | `DashboardStats` : total, lossless vs mp3 (counts), doublons restants (= nb de groupes de `scan_library_duplicates`), faux à re-sourcer (verdict `fake`), répartition par genre. |

**Réutilisé tel quel** : `open_url` (→ `https://www.discogs.com/release/{id}`),
`analyze_path` + `playback_url` (waveform/spectro/lecteur), `identify` +
`apply_identity_cmd` (ré-identifier Discogs), `file_track` + `trash_track`
(re-ranger / supprimer), `get_setting` / `set_setting` (racine).

## Frontend — fonctions (maquette + améliorations)

1. **Onglet Bibliothèque** dans la nav (à côté Accueil/Revue/Écartés).
2. **Barre de recherche** (réelle, sur artiste/titre/chemin) + **chips de filtre**
   actionnables : Tous / Lossless / MP3 / **À re-sourcer (faux)** / **Doublons**.
3. **Colonne de facettes** à gauche : bascule **Dossiers** ↔ **Genres**, compteurs
   réels, clic = filtre. *(v2 : mini-barre ratio lossless par dossier.)*
4. **Liste** : par morceau → bouton play, **vignette pochette** (si `cover_path`),
   titre, pill format, BPM, durée, **badge verdict** (Authentique/faux/grey), et
   **lien Discogs** si `discogs_release_id` sinon bouton **Identifier**.
5. **En-tête** : nom du dossier/genre courant + compteur réel.
6. **Mini-lecteur** : le **vrai player `report-view`** (waveform peaks réelle, seek,
   tempo/key-lock), pas les barres factices.
7. **Détail/édition** (composant unifié) : waveform/spectro/verdict + **champs
   éditables inline** (artiste/titre/genres/année/label/pochette) → `update_metadata` ;
   boutons **Voir la release** (`open_url`), **Ré-identifier** (flux Discogs),
   **Re-ranger** / **Supprimer**.
8. **Scanner de doublons internes** : bouton « Lancer » → `scan_library_duplicates`
   → groupes avec similarité ; le gardé **recommandé est pré-sélectionné** (badge de
   raison) ; **Résoudre** = `trash_track` sur les autres. Sinon « Aucun doublon ».
9. **Dashboard** : cartes stats (`library_stats`), chaque stat **cliquable → applique
   le filtre** correspondant à la liste (ex. « 12 faux » → chip À re-sourcer).

## Flux de données

nav → Bibliothèque → `library_folders` + `library_stats` + `list_library` → rendu
liste/facettes/dashboard → sélection morceau → détail (`analyze_path` à la demande
pour waveform/spectro) → édition → `update_metadata` (fichier puis DB) → refresh
liste. Scanner doublons et ré-identifier suivent leurs flux dédiés.

## Gestion d'erreurs

`Result<…, String>` serde sur l'IPC, fail fast. `update_metadata` : fichier avant
DB. Fichier absent/verrouillé → message clair, aucun changement partiel. Pas de
`LIBRARY_ROOT` défini → l'onglet invite à choisir la racine (réutilise l'UI Accueil).

## Tests

- **Backend (TDD, in-process SQLite + Symphonia)** :
  - `list_library` joint métadonnées/genres et respecte filtres/tri (seed filed).
  - `library_folders` compte juste par dossier et par genre.
  - `update_metadata` : round-trip — écrit puis relit les tags du fichier + la DB.
  - `scan_library_duplicates` groupe des doublons seedés (même empreinte, formats
    différents) et `recommend_keep` choisit le lossless.
  - `library_stats` agrège lossless/mp3/faux/doublons/genres sur un seed connu.
- **Frontend** : pas de test auto (TS vanilla) → vérification via `run` (lancer
  l'app, parcourir, éditer un morceau, lancer le scan, lire le dashboard).

## Découpage (chacun : plan → TDD → run)

1. **Parcourir** — `list_library` + `library_folders` + onglet liste + facettes +
   recherche/chips + mini-lecteur (`report-view`).
2. **Éditer + identifier** — composant détail unifié + `update_metadata` +
   ré-identifier + lien release.
3. **Doublons internes** — `scan_library_duplicates` + UI groupes + recommandation +
   Résoudre.
4. **Dashboard** — `library_stats` + cartes actionnables.
5. **Audit de conformité (post-M6b)** — comparer les sections **déjà construites**
   (Accueil, Revue, Écartés) aux fonctions de la maquette `app.js` correspondantes
   (`renderHome`, détail Revue, `renderEcarts`), produire une liste d'écarts/manques
   à corriger. (Consigne Antoine 2026-06-24.)
