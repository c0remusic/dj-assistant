# M1 — Watcher + file « à traiter » — Design

> Jalon M1 de Sift (`dj-assistant`). Source de vérité fonctionnelle : `docs/plan-implementation.md`.
> Ce document fige le design d'implémentation validé au brainstorm du 2026-06-12.
>
> **But du jalon (livrable) :** déposer un dossier dans une source surveillée → la file
> « à traiter » se remplit toute seule, visible en live dans l'UI. **Aucune analyse,
> aucun rangement** en M1 (c'est M2+).

## Décisions cadrées (brainstorm 2026-06-12)

| Sujet | Décision |
|---|---|
| **Détection « fichier complet »** | On surveille le dossier **`Completed`** de Soulseek (Nicotine+/SoulseekQt) : l'incomplet vit ailleurs, un fichier qui apparaît dans `Completed` est fini (arrive par `move`/rename atomique). |
| **Pas de stability-check explicite** | Aucun polling de taille stable. Le scan initial lit des fichiers déjà complets ; en live le **debouncer `notify-debouncer-full`** (fenêtre de settle ~500 ms, requis de toute façon pour coalescer les events) suffit. Cas résiduel (move inter-disques, partiel) : rare, auto-rattrapé au prochain event (mtime/taille change → re-`pending`) et détecté par M2 (troncature). On ne paie pas la latence pour ce cas. |
| **Modèle de file** | La file **EST** `tracks WHERE status='pending'`. Pas de table `queue` séparée. Les colonnes d'analyse restent `NULL` jusqu'à M2. |
| **Identité d'un fichier** | Le **chemin canonique** (clé `UNIQUE`). `mtime` + `size_bytes` servent à détecter un changement (re-téléchargement au même nom). Un re-DL sous un autre nom = nouvelle entrée → la **dédup (M5)** rattrape, pas le watcher. |
| **Scan vs flux** | À l'ajout d'un dossier (et au démarrage pour chaque source) → **scan complet** + réconciliation (diff). Dossier déjà connu → on ne touche que la différence. |
| **Fichier disparu du dossier** | S'il quitte le sas sans avoir été rangé par Sift (déplacé/supprimé à la main) → sa ligne `pending` **est retirée de la file** (pas de `status='missing'`). Un fichier rangé par Sift (M4) n'a pas « disparu » : son `status` change (`filed`) et pointe ailleurs — il n'est plus dans le sas, donc plus dans la file. |
| **Multi-dossiers** | Plusieurs sources surveillées dès M1 (ex. `Completed` Soulseek + dossier achats Beatport). |
| **Périmètre UI** | Accueil (sources + compteurs + warning + progression scan) **et** vue file peuplée en live. Pas de waveform/spectro/verdict/action (M2-M4). |

## Architecture

### Backend (`src-tauri/src/`)

Nouveaux modules, une responsabilité chacun, testables isolément :

- **`sources.rs`** — CRUD des dossiers surveillés en DB ; canonicalisation du chemin ; état (enabled / inaccessible).
- **`scanner.rs`** — walk récursif d'un dossier (`walkdir`, `follow_links(false)`) + **réconciliation** (diff) avec la DB. Logique pure et testable (entrée : listing disque + listing DB → sorties : insert/update/delete).
- **`watcher.rs`** — surveillance live via le crate **`notify`** + **`notify-debouncer-full`**. Démarre/arrête un watcher recursif par source active ; traduit les events débouncés en mutations de file.
- **`queue.rs`** — lectures : `tracks WHERE status='pending'` + compteurs par source et total.
- extensions à **`ipc.rs`** (commands) et **`db.rs`** (requêtes + migration de schéma).

**Orchestration retenue :** *initial walk + debounced watch*. À l'ajout/au démarrage : `walkdir`
complet → réconciliation. Puis `notify-debouncer-full` prend le relais pour le live. (Alternative
écartée : events `notify` bruts + débounce maison — plus de code, bugs de timing.)

Le scan tourne en **tâche de fond non-bloquante** (`tauri::async_runtime::spawn`), throttlée, pour
ne pas geler l'UI ni saturer le disque (un backlog ~15 000 fichiers est attendu).

### Frontend (`frontend/`)

Câblage sur la maquette existante (`index.html` + `frontend/`), pas de nouvelle techno :

- **Accueil** : liste des sources (ajout via **picker natif Tauri** — `tauri-plugin-dialog` ;
  retrait), **compteur par source** + total en file, **bannière d'avertissement** « pointe Sift
  sur ton dossier *Completed*, pas *Incomplete* », indicateur de **scan en cours** (« scan… X »),
  badge d'erreur si une source est inaccessible.
- **Revue / file** : liste des `pending` peuplée **en live** (nom de fichier brut, dossier source,
  chemin). Aucune action/lecteur/badge verdict (M2-M4).

## Flux de données

1. **Ajout d'un dossier** (picker natif) → insert `sources` → lance un **scan complet en tâche de
   fond**.
2. **Scan / réconciliation** — pour chaque fichier audio trouvé, comparaison par **chemin** :
   - inconnu → `INSERT tracks` (`status='pending'`),
   - connu, `mtime`+`size_bytes` identiques → ignoré,
   - connu, changé → re-`pending`.
   Puis : lignes `pending` de cette source dont le fichier n'existe plus sur disque → **supprimées**.
3. **Watch live** — event débouncé « fichier créé/modifié » dans une source → upsert `pending`.
   Event « supprimé » → si la ligne est `pending`, retirée de la file.
4. **Events Tauri** émis vers le front (`source:scan-progress { source_id, scanned }`,
   `queue:changed`) → l'UI se met à jour **en live**, sans polling.

## Données (SQLite — on étend l'existant)

- **`sources`** : `id`, `path` (`UNIQUE`, canonique), `enabled`, `created_at`.
- **`tracks`** (champs utilisés en M1) : `id`, `source_id` (FK), `path` (`UNIQUE`), `filename`,
  `size_bytes`, `mtime`, `status` (`DEFAULT 'pending'`), `created_at`. Les colonnes d'analyse
  (verdict, bitrate, clip_*, …) existent déjà au schéma initial et restent **NULL** jusqu'à M2.
- Migration via `PRAGMA user_version` (convention M0).
- **Extensions audio** détectées : `mp3, flac, wav, aif, aiff, m4a, aac, ogg, opus`. Tout le reste
  est ignoré.

## IPC (commands Rust ↔ front, contrats typés dans `shared/contracts.ts`)

- `add_source(path: string) -> Source` — canonicalise, insère, déclenche le scan de fond.
- `remove_source(id) -> void` — stoppe le watch et retire la source. Ses `tracks` **`pending`**
  sont supprimés (la file ne doit pas garder d'items d'un dossier qu'on ne surveille plus ; les
  fichiers restent sur disque). En M1 tout est `pending` ; les `tracks` déjà rangés (`filed`, M4+)
  conserveront leur ligne — gérés à ce jalon-là.
- `list_sources() -> Source[]` — avec `pending_count` par source + état (ok/inaccessible).
- `list_queue() -> QueueItem[]` — les `pending` (filename, source folder, path).
- `rescan_source(id) -> void` — relance une réconciliation manuelle.

Events émis : `source:scan-progress`, `queue:changed`.

## Erreurs & cas limites

- **Permission refusée** sur un dossier/sous-dossier → erreur loggée, le scan/watch continue sur le
  reste.
- **Fichier qui s'évapore** entre l'event et l'`INSERT` → ignoré (pas d'erreur fatale).
- **Symlinks** → non suivis (`follow_links(false)`), évite les boucles.
- **DB occupée** (`SQLITE_BUSY`) → retry borné.
- **Source inaccessible** (disque débranché) → marquée en erreur dans Accueil, **pas de crash** ;
  le watch reprend si la source redevient accessible (sur `rescan` ou redémarrage).
- **Fichiers non-audio** → filtrés par extension, jamais mis en file.

## Tests

- **Rust unit — `scanner`** : réconciliation sur un dossier temp fixture → vérifie les 4 cas
  (nouveau → insert ; inchangé → no-op ; changé → re-pending ; disparu → delete).
- **Rust unit** : filtre d'extensions (audio vs non-audio) ; canonicalisation de chemin.
- **Smoke IPC** : `add_source` → `list_sources` renvoie la source + `pending_count` correct ;
  déposer un fichier audio dans un temp dir surveillé → apparaît dans `list_queue`.

## Hors périmètre M1 (rappel)

Analyse FFmpeg, waveform/spectrogramme, verdict fake, bitrate réel, lecture/player, conversion,
rangement, dédup par empreinte, garde-fou Rekordbox. Tout ça = M2 et au-delà.
