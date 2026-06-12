# M2b — Worker d'analyse fond + cache DB — Design

> Jalon M2b de Sift. Suite de M2a (moteur `analysis::analyze`). Source de vérité fonctionnelle :
> `docs/plan-implementation.md` (§ M2, « Worker background »). Découpage A (cf. spec M2a).

## But
Dès qu'un fichier entre dans la file (`status='pending'`), il est **analysé automatiquement en
tâche de fond**, en **parallèle** (plusieurs cœurs), **sans ralentir l'UI**, et le résultat est
**mis en cache en DB**. Reprend après redémarrage, pas de double-analyse.

## Décisions
| Sujet | Décision |
|---|---|
| **Parallélisme** | Pool de `N = min(cores, 4)` threads workers. Suffisant pour le flux ; cape à 4 pour ne pas thrasher le disque sur le backlog. Configurable plus tard (Réglages). |
| **Ne pas bloquer l'UI** | Le décodage/DSP tournent **hors verrou DB**. La connexion (Mutex partagé) n'est prise que brièvement : lire le `(id,path)` à traiter, puis écrire le rapport. L'UI n'est jamais bloquée pendant une analyse. |
| **Sélection du travail** | `SELECT id, path FROM tracks WHERE analyzed_at IS NULL AND status='pending'`. Une ligne est « analysée » quand `analyzed_at` est non-NULL. |
| **Pas de double-analyse** | `analyzed_at` marque le fait. Si le fichier change (taille/mtime), `scanner::upsert_file` repasse `status='pending'` **et remet `analyzed_at=NULL`** → ré-analyse. |
| **Déclenchement** | Au démarrage (rattrapage des pending non analysés) **et** à chaque `queue:changed` (nouveaux arrivants) → `worker::refill`. Dédup : un id déjà en file/en cours n'est pas renvoyé. |
| **`with_spectrogram`** | Toujours `false` (le worker ne stocke que des scalaires ; spectrogramme = à la demande en M2c). |
| **Progression** | Event `analysis:changed` après chaque fichier ; commande `analysis_progress() -> {done,total,running}`. UI : barre/texte « X / Y analysés » + badge verdict dans la file. |
| **Échec d'analyse** | Loggé ; on écrit quand même `analyzed_at` + `container_ok=false`/`codec_error` pour ne pas boucler sur un fichier cassé. |

## Architecture (`src-tauri/src/`)
- **`worker.rs`** (NOUVEAU — distinct de `watcher.rs`) :
  - État managé `AnalysisWorker` = `Arc<Shared>` où `Shared = { queue: Mutex<{ deque: VecDeque<i64>, queued: HashSet<i64>, running: usize }>, cv: Condvar }` + total/done counters.
  - `init(app, n_threads)` : démarre N threads workers (bloquent sur la Condvar quand vide).
  - `refill(app)` : query des ids `pending && analyzed_at IS NULL` pas déjà `queued` → push + notify.
  - chaque worker : pop un id → lire `path` (verrou court) → `analyze(path,false)` (HORS verrou) → `persist` (verrou court) → emit `analysis:changed`.
- **`db.rs`** : migration **v3** — colonnes manquantes pour le rapport + `analyzed_at`.
- **`scanner.rs`** : `upsert_file` remet `analyzed_at=NULL` quand le contenu change.
- **`ipc.rs`** : commande `analysis_progress`. Appeler `worker::refill` partout où `queue:changed`
  est émis (fin de `spawn_scan`, `watcher::handle_events`, `remove_source`).
- **`lib.rs`** : `worker::init(...)` dans `setup`, après la DB ; `refill` au démarrage.

## Données — migration v3 (append-only)
```sql
ALTER TABLE tracks ADD COLUMN cutoff_hz REAL;
ALTER TABLE tracks ADD COLUMN dual_mono INTEGER;       -- 0/1
ALTER TABLE tracks ADD COLUMN container_ok INTEGER;    -- 0/1
ALTER TABLE tracks ADD COLUMN codec_error TEXT;
ALTER TABLE tracks ADD COLUMN id3_version TEXT;
ALTER TABLE tracks ADD COLUMN analyzed_at TEXT;        -- NULL = pas encore analysé
CREATE INDEX idx_tracks_analyzed ON tracks(analyzed_at);
```
Mapping `AnalysisReport` → colonnes `tracks` (existantes + v3) :
`verdict→verdict`, `cutoff_hz→cutoff_hz`, `declared_bitrate→bitrate`, `declared_format→declared_fmt`,
`declared_rail→real_quality`, `duration_sec→duration`, `clip_runs/clip_pct/true_peak_dbtp/dc_offset/
phase_correlation/truncated/silence_head_ms/silence_tail_ms/has_cover/tags_cdj_ok` → colonnes
homonymes, `dual_mono→dual_mono`, `container_ok→container_ok`, `codec_error→codec_error`,
`id3_version→id3_version`, + `analyzed_at=datetime('now')`. (peaks/spectrogram NON stockés.)

## Frontend (minimal — le vrai UI Revue = M2c)
- `QueueItem` gagne `verdict: string|null`. `list_queue` le renvoie.
- `renderQueue` (sift-live) : pastille couleur selon verdict (ok/fake/grey/—) devant le nom.
- Petit texte de progression « N / M analysés » sur Accueil/Revue, rafraîchi sur `analysis:changed`.

## Tests
- **`worker` (unit)** : `persist` écrit toutes les colonnes (DB in-memory + un AnalysisReport factice) ; `refill`/sélection ne renvoie que les `pending && analyzed_at IS NULL` ; dédup (même id pas deux fois).
- **`scanner`** : `upsert_file` sur contenu changé remet `analyzed_at=NULL` (étendre un test existant).
- **`db`** : migration v3 ajoute les colonnes (réutiliser le pattern `tracks_has_*_columns`).
- Pas de test d'intégration threads (non-déterministe) ; la logique testable (`persist`, `select`,
  `refill`) est isolée des threads.

## Hors périmètre M2b
UI Revue complète (waveform/spectro/badges riches, M2c), règles auto (M4), priorité de thread
OS fine, réglage du parallélisme dans l'UI (plus tard).
