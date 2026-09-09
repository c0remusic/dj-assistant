use rusqlite::Connection;
use std::sync::{Mutex, MutexGuard};
use tauri::State;

/// Locks the app's shared `Connection`, mapping a poisoned-mutex error to the
/// `String` every IPC command already returns. Extracted from ~40 duplicated
/// `conn.lock().map_err(|e| e.to_string())?` call sites across `ipc*.rs`.
pub fn lock_conn<'a>(
    conn: &'a State<'_, Mutex<Connection>>,
) -> Result<MutexGuard<'a, Connection>, String> {
    conn.lock().map_err(|e| e.to_string())
}

/// Ordered list of migrations. Index + 1 == the schema version it brings the DB to.
/// NEVER reorder or edit an existing entry once shipped — only append.
const MIGRATIONS: &[&str] = &[
    // v1 — initial schema (matches docs/plan-implementation.md "Données")
    r#"
    CREATE TABLE tracks (
        id INTEGER PRIMARY KEY,
        path TEXT NOT NULL UNIQUE,
        hash TEXT,
        fingerprint TEXT,
        -- MORTE. Déclarée en v1 et jamais renseignée par le code de production : ni
        -- `scanner::upsert_file`, ni `worker::persist_report` (qui écrit `declared_fmt`), ni
        -- `filing` (qui écrit `target_format`). Elle était NULL sur toute vraie base, et six
        -- lectures s'appuyaient dessus — les compteurs du tableau de bord Bibliothèque, son filtre
        -- Lossless/MP3, sa colonne Format, et le classement des doublons (voir 76e474e). Toutes
        -- repointées sur `target_format`. NE PAS l'utiliser ; ne pas la supprimer non plus : un
        -- DROP ferait tourner une migration sur les bases utilisateurs pour récupérer zéro octet
        -- (même arbitrage que `custom_tags`, tranché par Antoine le 2026-07-30).
        format TEXT,
        bitrate INTEGER,
        duration REAL,
        declared_fmt TEXT,
        real_quality TEXT,
        verdict TEXT,                 -- ok | fake | grey
        status TEXT NOT NULL DEFAULT 'pending', -- pending | filed | resourcing | trash
        folder TEXT,
        clip_runs INTEGER,
        clip_pct REAL,
        true_peak_dbtp REAL,
        dc_offset REAL,
        phase_correlation REAL,
        truncated INTEGER,            -- bool 0/1
        silence_head_ms INTEGER,
        silence_tail_ms INTEGER,
        has_cover INTEGER,            -- bool 0/1
        tags_cdj_ok INTEGER,          -- bool 0/1
        created_at TEXT NOT NULL DEFAULT (datetime('now'))
    );
    CREATE TABLE metadata (
        track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
        artist TEXT, title TEXT, label TEXT, year INTEGER,
        genre TEXT, bpm INTEGER, cover_path TEXT,
        discogs_release_id TEXT, source TEXT
    );
    CREATE TABLE custom_tags (
        track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
        tag TEXT NOT NULL,
        PRIMARY KEY (track_id, tag)
    );
    CREATE TABLE actions (
        id INTEGER PRIMARY KEY,
        track_id INTEGER REFERENCES tracks(id) ON DELETE SET NULL,
        type TEXT NOT NULL,           -- convert | move | trash | reject
        from_path TEXT,
        to_path TEXT,
        ts TEXT NOT NULL DEFAULT (datetime('now'))
    );
    CREATE TABLE sources (
        id INTEGER PRIMARY KEY,
        path TEXT NOT NULL UNIQUE,
        watched INTEGER NOT NULL DEFAULT 1  -- bool 0/1
    );
    "#,
    // v2 — M1 watcher/queue: link tracks to a source + cheap "seen" identity (size+mtime)
    r#"
    ALTER TABLE tracks ADD COLUMN source_id INTEGER REFERENCES sources(id) ON DELETE CASCADE;
    ALTER TABLE tracks ADD COLUMN filename TEXT;
    ALTER TABLE tracks ADD COLUMN size_bytes INTEGER;
    ALTER TABLE tracks ADD COLUMN mtime INTEGER;
    ALTER TABLE sources ADD COLUMN created_at TEXT;
    CREATE INDEX idx_tracks_source ON tracks(source_id);
    CREATE INDEX idx_tracks_status ON tracks(status);
    "#,
    // v3 — M2b analysis worker: report columns missing from v1 + the "analyzed" marker.
    r#"
    ALTER TABLE tracks ADD COLUMN cutoff_hz REAL;
    ALTER TABLE tracks ADD COLUMN dual_mono INTEGER;     -- 0/1
    ALTER TABLE tracks ADD COLUMN container_ok INTEGER;  -- 0/1
    ALTER TABLE tracks ADD COLUMN codec_error TEXT;
    ALTER TABLE tracks ADD COLUMN id3_version TEXT;
    ALTER TABLE tracks ADD COLUMN analyzed_at TEXT;      -- NULL = not yet analysed
    CREATE INDEX idx_tracks_analyzed ON tracks(analyzed_at);
    "#,
    // v4 — M4 filing loop: per-track target/confidence, version metadata, undo bookkeeping
    // on actions, and a key/value settings store (library root, filename template, purge).
    r#"
    ALTER TABLE tracks ADD COLUMN target_format TEXT;     -- 'mp3_320' | 'aiff_16_44' | 'wav_16_44'
    ALTER TABLE tracks ADD COLUMN confidence TEXT;        -- 'green' | 'yellow'
    ALTER TABLE metadata ADD COLUMN version TEXT;         -- 'Original Mix', 'Remix'…
    ALTER TABLE actions ADD COLUMN undone INTEGER NOT NULL DEFAULT 0;  -- 0/1
    ALTER TABLE actions ADD COLUMN batch_id TEXT;         -- groups one filing's rows
    CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
    "#,
    // v5 — cache the full analysis report as JSON so re-opening an already-analysed track is
    // instant (no re-decode). Since FIX-3 the display spectrogram is part of that JSON (the
    // "sans spectrogram" this comment used to claim stopped being true then). Cleared by the
    // scanner when a file changes.
    r#"
    ALTER TABLE tracks ADD COLUMN report_json TEXT;
    "#,
    // v6 — M6a Discogs identification: per-track sub-genres (Discogs "style"), multiple per
    // track, ordered. metadata.genre stays for back-compat but track_genres is the source.
    r#"
    CREATE TABLE track_genres (
        track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
        genre    TEXT NOT NULL,
        ord      INTEGER NOT NULL,
        PRIMARY KEY (track_id, genre)
    );
    CREATE INDEX idx_track_genres_track ON track_genres(track_id);
    "#,
    // v7 — revertable "Apply ID3 tags": a free-form JSON column on the journal where the
    // tag_edit action stores the OLD tags captured before the write, so a revert can restore
    // them. Other action types leave it NULL.
    r#"
    ALTER TABLE actions ADD COLUMN meta TEXT;
    "#,
    // v8 — Journal session grouping: tag each new action with the app session that produced it.
    // Actions from before this migration keep session_id = NULL → front shows them under "Antérieur".
    r#"
    ALTER TABLE actions ADD COLUMN session_id TEXT;
    "#,
    // v9 — report_json is otherwise unversioned: a content-only change to the analysis engine
    // (e.g. spectrogram resolution) leaves old cached rows structurally valid but stale, so
    // nothing would ever invalidate them. Rows from before this migration get NULL, which never
    // matches analysis::REPORT_CACHE_VERSION — ipc.rs treats that as a cache miss and self-heals.
    r#"
    ALTER TABLE tracks ADD COLUMN report_cache_ver INTEGER;
    "#,
    // v10 — composite indexes for the dashboard/facet queries: folder facets filter on
    // (status='filed', folder) and the "à re-sourcer" card on (status='filed', verdict).
    r#"
    CREATE INDEX IF NOT EXISTS idx_tracks_status_folder ON tracks(status, folder);
    CREATE INDEX IF NOT EXISTS idx_tracks_status_verdict ON tracks(status, verdict);
    "#,
    // v11 — M8 Tier 1 IPC wiring: candidate master.db path repairs detected read-only on
    // filing (docs/superpowers/specs/2026-07-06-m8-tier1-ipc-wiring-design.md). track_id is
    // NULL when 2+ djmdContent rows matched the same from_path (ambiguous, never auto-repaired
    // — see candidate_track_ids). UNIQUE(action_id): a second detection pass for the same
    // journaled move never duplicates the row.
    r#"
    CREATE TABLE rekordbox_masterdb_repairs (
        id INTEGER PRIMARY KEY,
        action_id INTEGER NOT NULL REFERENCES actions(id) ON DELETE CASCADE,
        track_id TEXT,
        candidate_track_ids TEXT,
        from_path TEXT NOT NULL,
        to_path TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'pending',
        detected_at TEXT NOT NULL DEFAULT (datetime('now')),
        applied_at TEXT,
        UNIQUE(action_id)
    );
    CREATE INDEX idx_rkbmdb_repairs_status ON rekordbox_masterdb_repairs(status);
    "#,
    // v12 — Apple system-colors palette: per-source manual color override.
    // NULL = auto-assign by add-order (frontend/source-color.ts computes it from
    // id order, no need to store the derived value); a hue name persists an
    // explicit override — writable today only through the set_source_color IPC,
    // the Accueil picker died when the screen merged into the rail.
    r#"
    ALTER TABLE sources ADD COLUMN color_key TEXT;
    "#,
    // v13 — M8 Tier 3 IPC wiring: candidate master.db metadata syncs detected read-only
    // whenever Sift writes ID3 tags on a file linked to Rekordbox (filing, apply_tags,
    // update_metadata). Keyed by Sift track_id (not action_id like v11's repairs table) —
    // a retag before the user syncs replaces the pending candidate, it never accumulates.
    // rekordbox_track_id is NULL when 2+ djmdContent rows matched the same path (ambiguous,
    // never auto-resolved — see candidate_track_ids).
    r#"
    CREATE TABLE rekordbox_masterdb_metadata_syncs (
        id INTEGER PRIMARY KEY,
        action_id INTEGER NOT NULL REFERENCES actions(id) ON DELETE CASCADE,
        track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
        rekordbox_track_id TEXT,
        candidate_track_ids TEXT,
        new_artist TEXT,
        new_title TEXT,
        new_label TEXT,
        new_year INTEGER,
        new_genre TEXT,
        status TEXT NOT NULL DEFAULT 'pending',
        detected_at TEXT NOT NULL DEFAULT (datetime('now')),
        applied_at TEXT,
        UNIQUE(track_id)
    );
    CREATE INDEX idx_rkbmdb_metasync_status ON rekordbox_masterdb_metadata_syncs(status);
    "#,
    // v14 — M8 Tier 3 IPC wiring (artwork): candidate master.db artwork syncs detected read-only
    // whenever Sift writes a NEW cover onto a file linked to Rekordbox (filing, apply_tags,
    // update_metadata) — only when cover_path is actually Some on that write, unlike v13's
    // metadata syncs which always fire. Keyed by Sift track_id, replaced on every fresh cover.
    // cover_path is a string (the source JPEG path), never resolved image bytes — re-read fresh
    // at apply time so a stale/moved file fails loudly instead of syncing wrong bytes.
    r#"
    CREATE TABLE rekordbox_masterdb_artwork_syncs (
        id INTEGER PRIMARY KEY,
        action_id INTEGER NOT NULL REFERENCES actions(id) ON DELETE CASCADE,
        track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
        rekordbox_track_id TEXT,
        candidate_track_ids TEXT,
        cover_path TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'pending',
        detected_at TEXT NOT NULL DEFAULT (datetime('now')),
        applied_at TEXT,
        UNIQUE(track_id)
    );
    CREATE INDEX idx_rkbmdb_artsync_status ON rekordbox_masterdb_artwork_syncs(status);
    "#,
    // v15 — per-track analysis attempt counter. A file that fails to decode is marked by
    // worker::persist_failure but stays status='pending' with verdict NULL, so it keeps showing
    // as "needs analysis" forever and the "Non analysés (N)" count can never reach zero for a
    // genuinely unrepairable file. This counter, incremented on every failed analysis, gives that
    // state a terminal condition: once it reaches the frontend threshold (MAX_ANALYSIS_ATTEMPTS,
    // shared/contracts.ts) the track drops out of the count / bulk-retry set (a manual per-row
    // retry resets it to 0 for a fresh chance).
    r#"
    ALTER TABLE tracks ADD COLUMN analysis_attempts INTEGER NOT NULL DEFAULT 0;
    "#,
    // v16 — FIRST content migration of this repo (v1..v15 are all DDL). `mag_db` moved to base85
    // and analysis::REPORT_CACHE_VERSION was bumped to 6, which makes every cached report
    // unservable (ipc.rs treats any other version as a miss) — but a bump ERASES NOTHING: the
    // rows would sit there, inflated, until each track happens to be reopened one by one
    // (worker::select_needing_analysis only re-selects on `report_json IS NULL`, never on a stale version).
    // Measured 2026-07-27 on the production DB: 3907 rows, 6.63 GB of report_json, 99.3% of them
    // already permanently unservable. This clears them in one pass.
    //
    // NULL, never '': the empty string is worker::persist_failure's permanent-decode-failure
    // sentinel (read by worker::select_needing_analysis and queue::list_pending) — writing '' here would
    // mark all 3907 tracks as broken files. With NULL they are simply re-analysed in the
    // background at next start, which is also what repopulates the cache in the new format.
    //
    // The WHERE guard is not an optimisation: without it this UPDATE also rewrites the rows that
    // already hold '' — persist_failure's permanent-failure sentinel — turning them back into NULL.
    // select_needing_analysis would then re-queue those broken files, the decode would fail again, and
    // analysis_attempts would climb toward MAX_ANALYSIS_ATTEMPTS. Measured on the production DB:
    // 3 such rows, all at 1 attempt. Skipping them costs nothing and preserves the invariant this
    // very migration argues for.
    //
    // NO VACUUM here, deliberately. The UPDATE frees pages into the freelist without shrinking the
    // file, and the obvious fix (appending `VACUUM;`) does NOT work under WAL — VACUUM rewrites
    // into the WAL and the main file is only truncated at checkpoint. Worse, running it here puts
    // a multi-GB rewrite on the startup path of a migration whose failure aborts db::open
    // (lib.rs `.expect("db open failed")`): a full disk or an antivirus holding the file would
    // panic the app at every launch until the condition clears. Reclaiming the space is a one-off
    // maintenance gesture, done with the app closed — not something the boot path should attempt.
    r#"
    UPDATE tracks SET report_json=NULL, report_cache_ver=NULL
    WHERE report_json IS NOT NULL AND report_json <> '';
    "#,
    // v17 — cache d'occupation par volume (graphique de l'écran Clé USB).
    //
    // Le parcours d'un volume ne lit que des métadonnées, mais il lit TOUTES les entrées : sur une
    // clé bien remplie ça se compte en secondes, et l'écran serait relancé à chaque visite. La clé
    // primaire est l'identité de disque déjà calculée par `usb_format` (PNPDeviceID + série
    // matérielle + taille + séries de volumes), donc deux clés différentes ne peuvent pas se
    // marcher dessus même branchées sur le même port.
    //
    // `free_bytes` n'est pas décoratif : c'est la clé d'invalidation. Un volume dont l'espace libre
    // a bougé a vu son contenu bouger, et le cache est ignoré. Comparer une date ne dirait rien —
    // un cache d'hier peut être juste, un cache d'il y a dix secondes peut être faux.
    //
    // `buckets_json` plutôt qu'une table fille : le contenu fait une douzaine de lignes, il n'est
    // jamais interrogé autrement qu'en bloc, et le dépôt stocke déjà du JSON en colonne
    // (`tracks.report_json`).
    r#"
    CREATE TABLE IF NOT EXISTS volume_usage (
        volume_key   TEXT PRIMARY KEY,
        scanned_at   INTEGER NOT NULL,
        total_bytes  INTEGER NOT NULL,
        free_bytes   INTEGER NOT NULL,
        file_count   INTEGER NOT NULL,
        buckets_json TEXT NOT NULL
    );
    "#,
    // v18 — version du schéma de classement dans le cache d'occupation.
    //
    // v17 s'invalide sur l'espace libre, ce qui détecte un contenu qui bouge. Mais pas une RÈGLE de
    // classement qui change : `.aif` et `.aiff` viennent d'être fusionnés en un seul format, et un
    // disque intact resservirait indéfiniment sa ventilation d'avant, coupée en deux.
    //
    // Le dépôt connaît déjà ce piège — c'est toute la raison d'être de la migration v16, où un
    // `REPORT_CACHE_VERSION` bumpé sans purge avait laissé 3907 rapports inservables. Ici la
    // version est stockée AVEC la donnée, donc un changement de règle rend simplement les lignes
    // périmées invisibles à `read_cache`, sans migration de rattrapage à écrire.
    //
    // DEFAULT 0 : les lignes écrites par la v17 portaient le schéma 1 mais ne le disaient pas.
    // 0 ne correspond à aucune version émise, donc elles sont toutes rejetées au prochain accès et
    // recalculées — ce qu'on veut, puisqu'elles ont justement l'ancien découpage.
    r#"
    ALTER TABLE volume_usage ADD COLUMN scheme_version INTEGER NOT NULL DEFAULT 0;
    "#,
    // v19 — Phase 4 du chantier d'évolution architecturale. Rend le dédoublonnage incrémental.
    //
    // Le défaut mesuré (`bench_dedup.rs`) n'était pas le O(n²) lui-même — énumérer les
    // 112 M paires d'une bibliothèque de 15 000 pistes coûte 123 ms. C'était la granularité
    // d'invalidation : `library::filed_signature` vaut `(COUNT(*), MAX(id))`, donc ranger UNE
    // piste faisait tout recalculer, ~2 min 31 s, dans une commande Tauri synchrone. Et le
    // cache vivant en RAM, chaque redémarrage le repayait.
    //
    // On mémorise donc le RÉSULTAT DES COMPARAISONS, qui ne bouge que pour les pistes touchées,
    // au lieu du comptage agrégé, qui bouge à chaque rangement.
    //
    // `dup_edges` ne porte QUE les paires dont la similarité atteint le seuil — quelques
    // centaines de lignes, pas les 928 135 candidates. `a_id < b_id` est un invariant tenu par
    // le code d'insertion : sans lui la même paire pourrait exister dans les deux sens, et
    // `PRIMARY KEY` ne l'attraperait pas.
    //
    // `dup_scanned` porte l'invariant central : toute PAIRE de cette table a été évaluée.
    // Ajouter une piste = la comparer à tout `dup_scanned`, insérer ses arêtes, puis l'y
    // ajouter. La récurrence tient.
    //
    // `ON DELETE CASCADE` sur les trois clés étrangères : c'est ce qui rend la suppression
    // exacte et gratuite. Un union-find en RAM ne sait PAS défaire une fusion — il aurait fallu
    // retomber sur un scan complet à chaque doublon résolu, c'est-à-dire précisément quand
    // l'écran sert à quelque chose. `db::open` active `PRAGMA foreign_keys`.
    r#"
    CREATE TABLE dup_edges (
        a_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
        b_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
        similarity REAL NOT NULL,
        PRIMARY KEY (a_id, b_id)
    );
    CREATE INDEX idx_dup_edges_b ON dup_edges(b_id);

    CREATE TABLE dup_scanned (
        track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE
    );
    "#,
    // v20 — SECONDE migration de contenu, et exactement le même piège que v16 pris par l'autre
    // bout. v16 avait bumpé REPORT_CACHE_VERSION à 6 puis vidé le cache ; toute la bibliothèque
    // s'est ré-analysée et réécrite en version 6. Le cap `MAX_PEAKS` (commit 689b700) est arrivé
    // APRÈS, le même jour, SANS toucher la version — donc les rapports écrits entre les deux
    // portent une enveloppe non plafonnée tout en étant indiscernables des bons, et
    // `worker::select_needing_analysis` ne re-sélectionne que sur `report_json` NULL.
    //
    // Mesuré sur la base de production le 2026-08-03 : 2 703 rapports sur 2 710 dans cet état,
    // 10 000 à 50 000 points d'enveloppe au lieu de 4 000. 1,00 Go de `peaks` là où le cap en
    // prévoyait 0,12 — 45 % de la base pour des points qu'aucun écran ne dessine.
    //
    // PAS de bump de REPORT_CACHE_VERSION : le FORMAT n'a pas changé, seule la donnée est
    // mauvaise. Un bump invaliderait aussi les 7 rapports corrects. `peaks_step`, ajouté par le
    // commit du cap, distingue exactement les deux populations : absent = écrit avant le cap.
    //
    // Le coût est mesuré, pas supposé (`bench_sqlite::bench_analysis_cost_on_real_tracks`) :
    // l'analyse tourne à 594x le temps réel, donc ré-analyser les 297 h d'audio de cette
    // bibliothèque coûte ~30 min sur un thread, quelques minutes sur le pool. C'est ce qui a
    // écarté l'alternative — un mécanisme de correction de données en Rust, pour re-capper en
    // place sans redécoder : machinerie permanente contre deux minutes de fond.
    //
    // Mêmes précautions que v16, pour les mêmes raisons : NULL et jamais '' (sentinelle de
    // `persist_failure`), et le garde `report_json <> ''` protège les fichiers déjà connus comme
    // illisibles. NO VACUUM ici non plus — voir v16.
    r#"
    UPDATE tracks SET report_json=NULL, report_cache_ver=NULL
    WHERE report_json IS NOT NULL AND report_json <> ''
      AND json_extract(report_json, '$.peaks_step') IS NULL;
    "#,
    // v21 — TROISIÈME migration de contenu, même famille que v16 et v20 : le format est bon, la
    // donnée est fausse pour une sous-population que le code ne peut plus produire.
    //
    // Deux défauts du détecteur de coupure, mesurés sur la base de production le 2026-08-17 et
    // corrigés le même jour (`spectrum.rs::SEARCH_FLOOR_HZ`, `verdict.rs::NO_MEASUREMENT_HZ`) :
    //
    // - **Le pied de basse pris pour une coupure.** La boucle de `detect_cutoff` n'avait pas de
    //   plancher, or le plateau de graves d'un morceau suivi du médium satisfait littéralement
    //   « chute de 18 dB sur 500 Hz qui ne récupère jamais ». 10 fichiers rendaient entre 571 et
    //   1367 Hz — dont 4 exactement à 571,0 Hz, le bin le plus bas testable — et étaient marqués
    //   FAKE. La sonde `spectrum::tests::ltas_probe` a montré qu'il n'y a AUCUNE falaise dans ces
    //   fichiers : spectre lisse jusqu'à 21 kHz.
    // - **Un décodage vide lu comme une mesure.** 2 MP3 de plus de six minutes, déclarés
    //   320 kbps, `codec_error` NULL, portaient `cutoff_hz = 0`, que le verdict lisait comme une
    //   coupure à 0 Hz — donc FAKE.
    //
    // Discriminant : `cutoff_hz < 2000`. Il est exact et pas approché — depuis le correctif,
    // `detect_cutoff` ne peut plus rendre que `nyq_hz` ou une valeur au-dessus de son plancher de
    // 2 kHz, donc TOUTE ligne sous 2000 vient forcément d'un des deux chemins cassés. Contrôle
    // sur la base réelle : zéro fichier entre 1400 et 8400 Hz, la population visée est isolée.
    //
    // Portée mesurée : 12 lignes sur 2705, soit moins de 0,5 % de la bibliothèque — c'est ce qui
    // écarte le bump de `REPORT_CACHE_VERSION`, qui ré-analyserait les 2693 autres pour rien
    // (v16 l'a fait et a coûté toute la bibliothèque). Le FORMAT n'a pas changé.
    //
    // `verdict` et `cutoff_hz` sont mis à NULL en plus du rapport : ils ne sont couverts par
    // AUCUNE version de cache — c'est le défaut « une seule des trois sorties d'étape est
    // versionnée » relevé sur la map — donc rien d'autre ne les corrigerait, et une piste dont le
    // rapport revient bon garderait un badge FAKE faux dans la file, la Bibliothèque et le compte
    // « à re-sourcer ».
    //
    // Mêmes précautions que v16 et v20 : NULL et jamais '' (sentinelle de `persist_failure`), et
    // le garde `report_json <> ''` protège les fichiers déjà connus comme illisibles. NO VACUUM.
    r#"
    UPDATE tracks SET report_json=NULL, report_cache_ver=NULL, verdict=NULL, cutoff_hz=NULL
    WHERE cutoff_hz IS NOT NULL AND cutoff_hz < 2000
      AND (report_json IS NULL OR report_json <> '');
    "#,
    // v22 — versions de cache de l'empreinte et du verdict (issue #39).
    //
    // Des trois sorties d'étape mises en cache par l'analyse, une seule était versionnée : le
    // rapport, par `report_cache_ver` (v9), lu par `ipc::analyze_path`. L'empreinte acoustique et
    // le verdict ne l'étaient par rien — le commentaire de la v21 juste au-dessus le constate déjà
    // en toutes lettres. Conséquence : un changement d'algorithme ne casse rien de VISIBLE. Le
    // dédoublonnage comparerait des empreintes anciennes à des neuves, c'est-à-dire des choses
    // incomparables, et une bibliothèque porterait deux générations de verdicts mélangées. Pas un
    // plantage : un taux de doublons qui change sans raison affichable, sur une fonction dont
    // l'utilisateur ne peut pas vérifier le résultat à la main.
    //
    // Les deux UPDATE ne sont PAS une purge, c'est l'inverse : ils **backfillent** la version
    // courante sur les lignes dont on peut établir qu'elles ont bien été produites par le code
    // d'aujourd'hui. Sans eux, toute ligne existante partirait à NULL, donc périmée, et cette
    // migration déclencherait le jour de sa livraison exactement ce que le ticket refuse de
    // décider d'avance : ré-empreinter la bibliothèque entière (un DÉCODAGE audio par piste — le
    // coût même par lequel `db.rs:393` écarte déjà un bump dans un cas voisin) et la ré-analyser.
    // Ce ticket demande que le désaccord soit DÉTECTABLE ; la décision d'invalider appartient au
    // jour du bump.
    //
    // Le backfill dit vrai, il ne se contente pas d'arranger :
    //
    // - `fingerprint_ver = 1` : toute empreinte non vide en base a été produite par
    //   `fingerprint::compute_for_path` sous rusty-chromaprint 0.3 + `preset_test1`, ce qui est
    //   précisément la définition de la version 1. Le garde `fingerprint <> ''` laisse à NULL une
    //   colonne vide, qui n'est de toute façon pas un cache.
    // - `verdict_ver = 1` : seulement pour les lignes dont le rapport compagnon est LUI-MÊME à
    //   jour (`report_cache_ver = 8`). `worker::persist_report` écrit verdict et rapport dans le
    //   même UPDATE : un rapport en version courante atteste que le verdict d'à côté sort du même
    //   passage. Une ligne au rapport périmé garde `verdict_ver` NULL — donc un verdict périmé,
    //   ce qu'elle est réellement.
    //
    // Les deux `1` et le `8` sont des LITTÉRAUX, pas les constantes Rust, et c'est délibéré : une
    // migration est gelée dans le temps. Écrire `FINGERPRINT_CACHE_VERSION` ici ferait qu'un futur
    // bump réécrirait rétroactivement le sens de cette migration et stamperait « courant » des
    // lignes produites par l'ancien algorithme — le défaut exact que la colonne existe pour
    // empêcher.
    r#"
    ALTER TABLE tracks ADD COLUMN fingerprint_ver INTEGER;
    ALTER TABLE tracks ADD COLUMN verdict_ver INTEGER;
    UPDATE tracks SET fingerprint_ver = 1
    WHERE fingerprint IS NOT NULL AND fingerprint <> '';
    UPDATE tracks SET verdict_ver = 1
    WHERE verdict IS NOT NULL AND report_cache_ver = 8;
    "#,
];

/// Applies ONE migration and its `user_version` bump in a SINGLE transaction, so a batch that
/// fails halfway leaves neither a partial schema nor a half-bumped version — see the test
/// `une_migration_echouee_ne_laisse_ni_schema_partiel_ni_version` for what that costs when it
/// isn't atomic. `unchecked_transaction` since we only hold `&Connection` (same reason as
/// `filing::commit_file`). `PRAGMA user_version` is a header write and IS transactional, so it
/// rolls back with the DDL. This is why no entry of `MIGRATIONS` may ever contain its own
/// `BEGIN`/`COMMIT` — none does today.
fn apply_migration(conn: &Connection, sql: &str, version: i64) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(sql)?;
    tx.execute_batch(&format!("PRAGMA user_version = {version}"))?;
    tx.commit()
}

/// Applies any migrations the DB hasn't seen yet, tracked via PRAGMA user_version.
/// Idempotent: running twice is a no-op the second time.
pub fn run_migrations(conn: &Connection) -> rusqlite::Result<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate() {
        let version = (i + 1) as i64;
        if version > current {
            apply_migration(conn, sql, version)?;
        }
    }
    Ok(())
}

/// Opens (creating if needed) the DB at `path`, enables foreign keys, runs migrations.
pub fn open(path: &std::path::Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    // WAL + a busy timeout so concurrent access waits instead of erroring (prep for moving
    // off the single-connection model; harmless with one connection today).
    conn.execute_batch(
        "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;",
    )?;
    run_migrations(&conn)?;
    Ok(conn)
}

/// Current schema version (PRAGMA user_version).
pub fn schema_version(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0))
}

/// Count of user tables (excludes sqlite internal tables).
pub fn table_count(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
        [],
        |r| r.get(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn migrations_bring_db_to_latest_version() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), MIGRATIONS.len() as i64);
    }

    #[test]
    fn migrations_create_all_tables() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        // v4 adds `settings`, v6 adds `track_genres`, v11 adds `rekordbox_masterdb_repairs`,
        // v13 adds `rekordbox_masterdb_metadata_syncs`, v14 adds `rekordbox_masterdb_artwork_syncs`,
        // v17 adds `volume_usage`, v19 adds `dup_edges` and `dup_scanned`
        assert_eq!(table_count(&conn).unwrap(), 13);
    }

    /// Une migration qui casse à mi-parcours ne doit laisser NI la table déjà créée, NI la
    /// version incrémentée. Sans transaction, `execute_batch` valide chaque instruction en
    /// auto-commit : la première table survivait et `user_version` restait en arrière, donc au
    /// démarrage suivant la MÊME migration rejouait et mourait sur « table already exists ».
    /// Base utilisateur inouvrable, sans chemin de retour.
    #[test]
    fn une_migration_echouee_ne_laisse_ni_schema_partiel_ni_version() {
        let conn = Connection::open_in_memory().unwrap();
        // Deux instructions : la première passe, la seconde est du SQL invalide.
        let cassee = "CREATE TABLE moitie (id INTEGER); CREATE TABLE ;";
        assert!(apply_migration(&conn, cassee, 1).is_err());
        assert_eq!(
            schema_version(&conn).unwrap(),
            0,
            "user_version a bougé alors que la migration a échoué"
        );
        let restees: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='moitie'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(restees, 0, "la table de la migration échouée a survécu");
    }

    /// v20 doit viser EXACTEMENT les rapports écrits avant le cap `MAX_PEAKS`, reconnaissables à
    /// l'absence de `peaks_step`. Trois populations dans le même test, parce que c'est la
    /// distinction qui porte tout le risque : effacer un rapport correct coûterait une ré-analyse
    /// inutile, et effacer la sentinelle `''` re-mettrait en file un fichier connu comme illisible.
    #[test]
    fn migration_v20_ne_vide_que_les_rapports_anterieurs_au_cap() {
        let conn = Connection::open_in_memory().unwrap();
        for sql in MIGRATIONS.iter().take(19) {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch("PRAGMA user_version = 19").unwrap();
        // 1 — écrit avant le cap : pas de peaks_step. À vider.
        conn.execute(
            "INSERT INTO tracks (id, path, status, report_json, report_cache_ver)
             VALUES (1, '/avant.mp3', 'pending', '{\"peaks\":[0.1,0.2],\"verdict\":\"ok\"}', 6)",
            [],
        )
        .unwrap();
        // 2 — écrit après le cap : peaks_step présent. À garder.
        conn.execute(
            "INSERT INTO tracks (id, path, status, report_json, report_cache_ver)
             VALUES (2, '/apres.mp3', 'pending', '{\"peaks\":[0.1],\"peaks_step\":4096}', 6)",
            [],
        )
        .unwrap();
        // 3 — sentinelle de persist_failure. À laisser strictement intacte.
        conn.execute(
            "INSERT INTO tracks (id, path, status, report_json, analysis_attempts)
             VALUES (3, '/casse.mp3', 'pending', '', 1)",
            [],
        )
        .unwrap();

        run_migrations(&conn).unwrap();

        let lu = |id: i64| -> Option<String> {
            conn.query_row(
                "SELECT report_json FROM tracks WHERE id=?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(lu(1), None, "le rapport antérieur au cap n'a pas été vidé");
        assert!(
            lu(2).is_some_and(|s| s.contains("peaks_step")),
            "un rapport déjà plafonné a été vidé — coût: une re-analyse pour rien"
        );
        assert_eq!(
            lu(3),
            Some(String::new()),
            "la sentinelle de fichier illisible a été effacée — le fichier repasserait en file"
        );
        let ver: Option<i64> = conn
            .query_row("SELECT report_cache_ver FROM tracks WHERE id=1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(ver, None, "report_cache_ver doit tomber avec le rapport");
    }

    /// v21 ne doit reprendre QUE les lignes produites par les deux chemins cassés du détecteur de
    /// coupure, et rendre leur verdict à l'indéterminé plutôt que de le laisser à FAKE.
    ///
    /// Le test porte les deux populations en même temps, parce que le risque est symétrique : trop
    /// large, on ré-analyse une bibliothèque entière pour rien (ce que v16 a coûté) ; trop étroit,
    /// un badge FAKE faux reste en place dans la file, la Bibliothèque et le compte
    /// « à re-sourcer », qu'aucune version de cache ne corrigerait — `verdict` et `cutoff_hz` ne
    /// sont couverts par aucune.
    #[test]
    fn migration_v21_ne_reprend_que_les_coupures_impossibles() {
        let conn = Connection::open_in_memory().unwrap();
        for sql in MIGRATIONS.iter().take(20) {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch("PRAGMA user_version = 20").unwrap();
        // 1 — pied de basse pris pour une coupure (bin 53). À reprendre.
        conn.execute(
            "INSERT INTO tracks (id, path, status, cutoff_hz, verdict, report_json, report_cache_ver)
             VALUES (1, '/basse.aif', 'pending', 571.0, 'fake', '{\"peaks_step\":4096}', 6)",
            [],
        )
        .unwrap();
        // 2 — décodage vide lu comme une mesure. À reprendre.
        conn.execute(
            "INSERT INTO tracks (id, path, status, cutoff_hz, verdict, report_json, report_cache_ver)
             VALUES (2, '/vide.mp3', 'pending', 0.0, 'fake', '{\"peaks_step\":4096}', 6)",
            [],
        )
        .unwrap();
        // 3 — VRAI faux lossless, coupure à 15,9 kHz. Doit rester intact : le re-décoder ne
        //     changerait rien et son verdict est juste.
        conn.execute(
            "INSERT INTO tracks (id, path, status, cutoff_hz, verdict, report_json, report_cache_ver)
             VALUES (3, '/vrai-faux.aif', 'pending', 15967.0, 'fake', '{\"peaks_step\":4096}', 6)",
            [],
        )
        .unwrap();
        // 4 — sentinelle de persist_failure, avec un cutoff bas hérité. À laisser strictement
        //     intacte : la vider remettrait un fichier illisible en file.
        conn.execute(
            "INSERT INTO tracks (id, path, status, cutoff_hz, report_json, analysis_attempts)
             VALUES (4, '/casse.mp3', 'pending', 0.0, '', 1)",
            [],
        )
        .unwrap();

        run_migrations(&conn).unwrap();

        let row = |id: i64| -> (Option<String>, Option<String>, Option<f64>) {
            conn.query_row(
                "SELECT report_json, verdict, cutoff_hz FROM tracks WHERE id=?1",
                rusqlite::params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap()
        };
        for id in [1i64, 2] {
            let (json, verdict, cutoff) = row(id);
            assert_eq!(json, None, "ligne {id} : le rapport doit tomber");
            assert_eq!(
                verdict, None,
                "ligne {id} : un verdict FAKE faux resterait sinon — rien d'autre ne le corrige"
            );
            assert_eq!(cutoff, None, "ligne {id} : la mesure fausse doit tomber");
        }
        let (json3, verdict3, cutoff3) = row(3);
        assert!(
            json3.is_some(),
            "un vrai faux lossless a été repris — coût: une re-analyse pour rien"
        );
        assert_eq!(verdict3.as_deref(), Some("fake"), "son verdict était juste");
        assert_eq!(cutoff3, Some(15967.0));
        assert_eq!(
            row(4).0,
            Some(String::new()),
            "la sentinelle de fichier illisible a été effacée — le fichier repasserait en file"
        );
    }

    /// v22 ne PURGE rien : elle stampe la version courante sur ce dont on peut établir que le code
    /// d'aujourd'hui l'a produit, et laisse NULL — donc périmé — tout le reste.
    ///
    /// Les quatre populations sont dans le même test parce que le risque est symétrique, comme
    /// pour v20 et v21. Trop large : la migration déclare « courant » un verdict d'un autre moteur,
    /// exactement ce que la colonne existe pour empêcher. Trop étroit : elle périme toute la
    /// bibliothèque le jour de sa livraison — une empreinte périmée coûte un DÉCODAGE audio par
    /// piste, et `db.rs:393` écarte déjà un bump pour ce coût-là.
    ///
    /// Les littéraux `1` et `8` sont ceux de la migration, délibérément : elle est gelée dans le
    /// temps, et un futur bump des constantes Rust ne doit rien changer à ce qu'elle a fait.
    #[test]
    fn migration_v22_stampe_le_courant_et_laisse_le_reste_perime() {
        let conn = Connection::open_in_memory().unwrap();
        for sql in MIGRATIONS.iter().take(21) {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch("PRAGMA user_version = 21").unwrap();
        // 1 — empreinte présente + verdict dont le rapport compagnon est à jour. Les deux à stamper.
        conn.execute(
            "INSERT INTO tracks (id, path, status, fingerprint, verdict, report_cache_ver)
             VALUES (1, '/a.mp3', 'filed', '1,2,3', 'fake', 8)",
            [],
        )
        .unwrap();
        // 2 — empreinte VIDE : ce n'est pas un cache, rien à stamper.
        conn.execute(
            "INSERT INTO tracks (id, path, status, fingerprint) VALUES (2, '/b.mp3', 'filed', '')",
            [],
        )
        .unwrap();
        // 3 — verdict présent mais rapport compagnon PÉRIMÉ (version 7). Rien n'atteste que ce
        //     verdict sort du moteur courant : il doit rester périmé, c'est ce qu'il est.
        conn.execute(
            "INSERT INTO tracks (id, path, status, verdict, report_cache_ver)
             VALUES (3, '/c.mp3', 'filed', 'ok', 7)",
            [],
        )
        .unwrap();
        // 4 — sentinelle de `persist_failure` : verdict NULL, rapport ''. Aucune version ne doit
        //     apparaître, sinon la ligne prétendrait porter un verdict courant qu'elle n'a pas.
        conn.execute(
            "INSERT INTO tracks (id, path, status, verdict, report_json, analysis_attempts)
             VALUES (4, '/d.mp3', 'pending', NULL, '', 1)",
            [],
        )
        .unwrap();

        run_migrations(&conn).unwrap();

        let vers = |id: i64| -> (Option<i64>, Option<i64>) {
            conn.query_row(
                "SELECT fingerprint_ver, verdict_ver FROM tracks WHERE id=?1",
                rusqlite::params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap()
        };
        assert_eq!(
            vers(1),
            (Some(1), Some(1)),
            "empreinte réelle + verdict au rapport courant : les deux devaient être stampes, \
             sinon la livraison de cette migration re-empreinte et re-analyse toute la bibliotheque"
        );
        assert_eq!(
            vers(2).0,
            None,
            "une empreinte vide n'est pas un cache — la stamper la rendrait servable"
        );
        assert_eq!(
            vers(3).1,
            None,
            "verdict au rapport périmé : rien n'atteste qu'il sort du moteur courant"
        );
        assert_eq!(
            vers(4),
            (None, None),
            "la sentinelle de fichier illisible ne doit porter aucune version"
        );
        // La sentinelle elle-même reste intacte : v22 est purement additive, contrairement à
        // v16/v20/v21 qui sont des migrations de contenu.
        let sentinelle: Option<String> = conn
            .query_row("SELECT report_json FROM tracks WHERE id=4", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(sentinelle.as_deref(), Some(""));
        assert_eq!(schema_version(&conn).unwrap(), MIGRATIONS.len() as i64);
    }

    #[test]
    fn migrations_are_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        run_migrations(&conn).unwrap(); // second run must not error or duplicate
        assert_eq!(table_count(&conn).unwrap(), 13);
    }

    /// v16 must actually WIPE the inflated report cache, not merely be declared. Applies v1..v15
    /// by hand, seeds a row in the pre-v16 state, then lets `run_migrations` finish the job.
    #[test]
    fn migration_v16_clears_the_stale_report_cache() {
        let conn = Connection::open_in_memory().unwrap();
        for sql in MIGRATIONS.iter().take(15) {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch("PRAGMA user_version = 15").unwrap();
        conn.execute(
            "INSERT INTO tracks (id, path, status, report_json, report_cache_ver)
             VALUES (1, '/a.mp3', 'pending', 'X', 5)",
            [],
        )
        .unwrap();
        // Row 2 carries persist_failure's permanent-decode-failure sentinel. The wipe must LEAVE
        // IT ALONE: turning '' back into NULL would re-queue a file already known to be broken and
        // burn another analysis_attempt on it.
        conn.execute(
            "INSERT INTO tracks (id, path, status, report_json, report_cache_ver, analysis_attempts)
             VALUES (2, '/broken.mp3', 'pending', '', NULL, 1)",
            [],
        )
        .unwrap();

        run_migrations(&conn).unwrap();

        let sentinel: Option<String> = conn
            .query_row("SELECT report_json FROM tracks WHERE id=2", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            sentinel.as_deref(),
            Some(""),
            "the permanent-failure sentinel '' must survive the wipe, or the broken file gets re-queued"
        );

        let (json, ver): (Option<String>, Option<i64>) = conn
            .query_row(
                "SELECT report_json, report_cache_ver FROM tracks WHERE id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            json, None,
            "report_json must be NULL, not '' — '' is the permanent-decode-failure sentinel"
        );
        assert_eq!(ver, None, "report_cache_ver must be NULL");
        assert_eq!(schema_version(&conn).unwrap(), MIGRATIONS.len() as i64);
    }

    #[test]
    fn migrations_reach_v2() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), MIGRATIONS.len() as i64);
        assert!(MIGRATIONS.len() >= 2, "M1 adds migration v2");
    }

    #[test]
    fn tracks_has_m1_columns() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let cols: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('tracks')")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for c in ["source_id", "filename", "size_bytes", "mtime"] {
            assert!(cols.contains(&c.to_string()), "tracks missing column {c}");
        }
    }

    #[test]
    fn tracks_has_m2b_columns() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let cols: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('tracks')")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for c in [
            "cutoff_hz",
            "dual_mono",
            "container_ok",
            "codec_error",
            "id3_version",
            "analyzed_at",
        ] {
            assert!(cols.contains(&c.to_string()), "tracks missing column {c}");
        }
    }

    #[test]
    fn rekordbox_masterdb_repairs_table_has_expected_columns() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let cols: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('rekordbox_masterdb_repairs')")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for c in [
            "id",
            "action_id",
            "track_id",
            "candidate_track_ids",
            "from_path",
            "to_path",
            "status",
            "detected_at",
            "applied_at",
        ] {
            assert!(
                cols.contains(&c.to_string()),
                "rekordbox_masterdb_repairs missing column {c}"
            );
        }
    }

    #[test]
    fn tracks_has_m4_columns() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let cols: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('tracks')")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for c in ["target_format", "confidence"] {
            assert!(cols.contains(&c.to_string()), "tracks missing column {c}");
        }
    }

    #[test]
    fn actions_and_settings_have_m4_shape() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let acols: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('actions')")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for c in ["undone", "batch_id"] {
            assert!(acols.contains(&c.to_string()), "actions missing column {c}");
        }
        // settings table exists and is writable
        conn.execute("INSERT INTO settings(key,value) VALUES('k','v')", [])
            .expect("settings table usable");
    }

    #[test]
    fn actions_has_v7_meta_column() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let acols: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('actions')")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert!(
            acols.contains(&"meta".to_string()),
            "actions missing column meta"
        );
    }

    #[test]
    fn actions_has_v8_session_id_column() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let acols: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('actions')")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert!(
            acols.contains(&"session_id".to_string()),
            "actions missing column session_id"
        );
    }
}
