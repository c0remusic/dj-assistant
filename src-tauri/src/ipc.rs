use crate::{db, ffmpeg, queue, scanner, sources};
use rusqlite::Connection;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Serialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
}

#[derive(Serialize)]
pub struct DbHealth {
    pub schema_version: i64,
    pub tables: i64,
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "Sift".into(),
        version: env!("CARGO_PKG_VERSION").into(),
    }
}

#[tauri::command]
pub fn db_health(conn: State<'_, Mutex<Connection>>) -> Result<DbHealth, String> {
    let conn = db::lock_conn(&conn)?;
    Ok(DbHealth {
        schema_version: db::schema_version(&conn).map_err(|e| e.to_string())?,
        tables: db::table_count(&conn).map_err(|e| e.to_string())?,
    })
}

#[tauri::command]
pub fn ffmpeg_version() -> Result<String, String> {
    ffmpeg::version()
}

/// Smoke-test reporter: lets the frontend echo the IPC result to the Rust log (stdout),
/// so the full JS→command→backend chain can be verified from the dev terminal.
#[tauri::command]
pub fn report_smoke(ok: bool, detail: String) {
    if ok {
        log::info!("SMOKE OK :: {detail}");
    } else {
        log::error!("SMOKE FAIL :: {detail}");
    }
}

/// Adds a watched folder, then kicks off a background full scan + reconcile.
/// Returns the source immediately (count 0); the scan emits `queue:changed` when done.
#[tauri::command]
pub fn add_source(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    path: String,
) -> Result<sources::Source, String> {
    let id = {
        let conn = db::lock_conn(&conn)?;
        sources::add(&conn, &path).map_err(|e| e.to_string())?
    };
    spawn_scan(app, id);
    // Fetch just the inserted row instead of re-listing every source and filtering in memory.
    // Mirrors the shape of `sources::list` (pending_count + on-disk accessibility) for one id.
    let conn = db::lock_conn(&conn)?;
    conn.query_row(
        "SELECT s.id, s.path,
                (SELECT count(*) FROM tracks t WHERE t.source_id=s.id AND t.status='pending'),
                (SELECT count(*) FROM tracks t WHERE t.source_id=s.id),
                s.watched, s.color_key
         FROM sources s WHERE s.id=?1",
        rusqlite::params![id],
        |r| {
            let path: String = r.get(1)?;
            let accessible = std::path::Path::new(&path).is_dir();
            Ok(sources::Source {
                id: r.get(0)?,
                path,
                pending_count: r.get(2)?,
                track_count: r.get(3)?,
                accessible,
                watched: r.get::<_, i64>(4)? != 0,
                color_key: r.get(5)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_sources(conn: State<'_, Mutex<Connection>>) -> Result<Vec<sources::Source>, String> {
    let conn = db::lock_conn(&conn)?;
    sources::list(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_source(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    id: i64,
) -> Result<(), String> {
    {
        let conn = db::lock_conn(&conn)?;
        sources::remove(&conn, id).map_err(|e| e.to_string())?;
    }
    crate::watcher::stop(&app, id);
    app.emit("queue:changed", ()).ok();
    crate::worker::refill(&app);
    Ok(())
}

#[tauri::command]
pub fn list_queue(conn: State<'_, Mutex<Connection>>) -> Result<Vec<queue::QueueItem>, String> {
    // Deux lectures brèves sous le verrou, puis le verrou tombe. Le regroupement par nom
    // (`group_name_dups`) normalise le nom de CHAQUE piste de la bibliothèque — il tournait sous le
    // verrou global, donc à chaque ouverture de la file d'attente, pendant que le pool d'analyse
    // attendait. Rien ici n'est un read-modify-write : les deux lectures ne servent qu'à annoter
    // des lignes déjà chargées, une piste ajoutée entre-temps sera vue au rafraîchissement suivant.
    let (mut items, dup_rows) = {
        let conn = db::lock_conn(&conn)?;
        let items = queue::list_pending(&conn).map_err(|e| e.to_string())?;
        let rows = crate::dedup::load_name_dup_rows(&conn).map_err(|e| e.to_string())?;
        (items, rows)
    };

    // Annotate name-duplicate items so the queue can badge them before they're opened.
    let dups = crate::dedup::group_name_dups(&dup_rows);
    for it in &mut items {
        it.dup = dups.contains(&it.id);
    }
    Ok(items)
}

/// Enables/disables live watching for a source: persists the flag and starts or stops
/// the watcher accordingly.
#[tauri::command]
pub fn set_source_watched(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    id: i64,
    watched: bool,
) -> Result<(), String> {
    let path = {
        let conn = db::lock_conn(&conn)?;
        sources::set_watched(&conn, id, watched).map_err(|e| e.to_string())?
    };
    if watched {
        crate::watcher::start(&app, id, &path);
    } else {
        crate::watcher::stop(&app, id);
    }
    Ok(())
}

/// Sets or clears a source's manual color override (one of the 5 categorical
/// hue keys, or None to fall back to auto-assignment by add-order — computed
/// frontend-side in frontend/source-color.ts, never stored).
///
/// The value is validated here, before the DB: the `color_key` column has no `CHECK`
/// (`db.rs`), so this boundary is the only guard against an arbitrary value polluting the
/// base (`sources::validate_color_key`, mirror-pinned to the frontend cycle).
#[tauri::command]
pub fn set_source_color(
    conn: State<'_, Mutex<Connection>>,
    id: i64,
    color_key: Option<String>,
) -> Result<(), String> {
    sources::validate_color_key(color_key.as_deref())?;
    let conn = db::lock_conn(&conn)?;
    sources::set_color(&conn, id, color_key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rescan_source(app: AppHandle, id: i64) -> Result<(), String> {
    spawn_scan(app, id);
    Ok(())
}

#[derive(Serialize)]
pub struct ImportResult {
    pub files_added: usize,
    pub folders_added: usize,
    /// Impasse A5 ([issue #15](https://github.com/c0remusic/sift/issues/15)) : pourquoi rien n'a
    /// été pris, quand la raison n'est PAS le contenu déposé.
    ///
    /// Deux compteurs à zéro voulaient dire trois choses : le dépôt ne contenait rien
    /// d'importable, aucune racine de bibliothèque n'était réglée (donc un dossier déposé sur
    /// « Où on va » ne pouvait devenir un bac), ou la création du bac a échoué — et l'écran disait
    /// la première, « Rien d'importable dans ce dépôt ». Le message accusait le contenu déposé au
    /// lieu de nommer le réglage absent, à un endroit où le commentaire du code nommait déjà le
    /// trou (`chrome.ts`).
    ///
    /// `None` = les compteurs suffisent à expliquer le résultat. Un message ici prime sur eux.
    pub blocked_by: Option<String>,
}

/// Import OS-dropped paths. Audio files always become pending queue items (deduped by
/// path). Directories depend on `mode`: `"dest"` registers each as a destination bin under
/// the library root (used when dropping onto "Où on va"); anything else (`"source"`,
/// default) adds each as a watched source, scanned in the background. Emits `queue:changed`.
#[tauri::command]
pub fn import_paths(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    paths: Vec<String>,
    mode: Option<String>,
) -> Result<ImportResult, String> {
    let as_dest = mode.as_deref() == Some("dest");
    let mut files_added = 0usize;
    let mut folders_added = 0usize;
    let mut blocked_by: Option<String> = None;
    let mut scan_ids: Vec<i64> = Vec::new();
    {
        let conn = db::lock_conn(&conn)?;
        let dest_root = if as_dest {
            crate::settings::get(&conn, crate::settings::LIBRARY_ROOT)
                .ok()
                .flatten()
                .filter(|p| !p.trim().is_empty())
                .map(std::path::PathBuf::from)
        } else {
            None
        };
        for p in &paths {
            let pb = std::path::Path::new(p);
            if pb.is_dir() {
                if as_dest {
                    // register a new destination bin named after the dropped folder
                    match &dest_root {
                        Some(root) => {
                            let name = pb.file_name().and_then(|n| n.to_str()).unwrap_or("");
                            // Le garde `name.is_empty()` reste un `if` SÉPARÉ et non un bras de
                            // match : un match évalue son scrutin d'abord, donc `create_bin` serait
                            // appelée avec un nom vide avant que le garde ne s'applique. Le `&&`
                            // d'origine court-circuitait ; cette forme le préserve.
                            if !name.is_empty() {
                                // Second aplatissement d'A5, au même endroit :
                                // `create_bin(..).is_ok()` jetait l'échec réel, si bien qu'un droit
                                // d'écriture manquant sur la racine se lisait « rien d'importable ».
                                match crate::library::create_bin(root, "", name) {
                                    Ok(_) => folders_added += 1,
                                    Err(e) => {
                                        blocked_by.get_or_insert(format!(
                                            "Impossible de créer le bac « {name} » : {e}"
                                        ));
                                    }
                                }
                            }
                        }
                        // Le cas de l'inventaire : un dossier déposé sur « Où on va » sans racine
                        // de bibliothèque réglée. Rien ne peut aboutir, et ce n'est pas la faute
                        // de ce qui a été déposé.
                        None => {
                            blocked_by.get_or_insert(
                                "Aucune racine de bibliothèque n'est réglée — un dossier déposé ici ne peut pas devenir une destination. Choisis-la dans Réglages."
                                    .to_string(),
                            );
                        }
                    }
                } else if let Ok(id) = sources::add(&conn, p) {
                    folders_added += 1;
                    scan_ids.push(id);
                }
            } else if scanner::is_audio(pb) {
                let filename = pb
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                files_added +=
                    scanner::add_loose_file(&conn, p, &filename).map_err(|e| e.to_string())?;
            }
        }
    }
    for id in scan_ids {
        spawn_scan(app.clone(), id);
    }
    app.emit("queue:changed", ()).ok();
    crate::worker::refill(&app);
    Ok(ImportResult {
        files_added,
        folders_added,
        blocked_by,
    })
}

#[derive(Serialize)]
pub struct AnalysisProgress {
    pub done: i64,
    pub total: i64,
}

/// Background-analysis progress: how many pending tracks are already analysed, out of total.
#[tauri::command]
pub fn analysis_progress(conn: State<'_, Mutex<Connection>>) -> Result<AnalysisProgress, String> {
    let conn = db::lock_conn(&conn)?;
    let (done, total) = crate::worker::progress(&conn).map_err(|e| e.to_string())?;
    Ok(AnalysisProgress { done, total })
}

/// Run the analysis engine on a track and return the full report. Constrained to paths Sift
/// already knows (present in `tracks`) so the webview can't turn this into an arbitrary
/// file-read / decode oracle on any path on disk.
#[tauri::command]
pub fn analyze_path(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    path: String,
    with_spectrogram: bool,
    // Only the genuine user-driven open of a track (openReportInto) passes true: on a confirmed
    // gone file it may drop the stale pending row. Background readers that call analyze_path as a
    // pure read — prefetch of the next track, the spectrogram re-fetch, the startup self-test —
    // pass false, so an observation never silently mutates the queue out from under the user
    // (review: a 400ms prefetch or a self-test over moved files was a hidden bulk row-deleter).
    allow_forget: bool,
) -> Result<crate::analysis::AnalysisReport, String> {
    {
        let conn = db::lock_conn(&conn)?;
        // ALWAYS require a known track first (security: not an arbitrary-file decode oracle),
        // for every path that can reach analyse() — incl. spectrogram requests and tracks
        // whose cache is the empty failure sentinel.
        let known = conn
            .query_row(
                "SELECT 1 FROM tracks WHERE path=?1 LIMIT 1",
                rusqlite::params![path],
                |_| Ok(()),
            )
            .is_ok();
        if !known {
            return Err("unknown track path".into());
        }
        // Serve the cached report instantly (no re-decode). FIX-3: the cache now carries the
        // spectrogram too (worker.rs analyzes with_spectrogram=true), so a spectrogram request
        // can also be served from cache — unless this row predates that fix (empty grid), in
        // which case fall through to a fresh decode below.
        let cached: Option<(Option<String>, Option<i64>)> = conn
            .query_row(
                "SELECT report_json, report_cache_ver FROM tracks WHERE path=?1",
                rusqlite::params![path],
                |r| Ok((r.get::<_, Option<String>>(0)?, r.get(1)?)),
            )
            .ok();
        // report_cache_ver guards against content-only changes to analyze() (e.g. spectrogram
        // resolution, or v9's re-meaning of tags_cdj_ok) that don't touch AnalysisReport's JSON
        // shape — see REPORT_CACHE_VERSION's doc comment. The rule itself lives in
        // `analysis::cached_report`, frozen by its own test.
        if let Some(json) = cached.and_then(|(j, ver)| crate::analysis::cached_report(j, ver)) {
            // report_json can also predate an AnalysisReport field being added (e.g. FIX-11's
            // est_kbps) and fail to deserialize even at the right cache version. Treat that
            // the same as a cache miss — fall through to a fresh decode, which self-heals the
            // row below — instead of hard-failing analyze_path for every pre-existing track.
            if let Ok(report) = serde_json::from_str::<crate::analysis::AnalysisReport>(&json) {
                if !with_spectrogram || !report.spectrogram.mag_db.is_empty() {
                    return Ok(report);
                }
            }
        }
    }
    let report = match crate::analysis::analyze(&path, with_spectrogram) {
        Ok(r) => r,
        Err(e) => {
            // The file is confirmed gone (decode.rs's open_format hits NotFound) — unlike the
            // live watcher's delete handler (watcher.rs) or a manual rescan (scanner::reconcile),
            // nothing else guarantees this pending row ever gets dropped for an unwatched source
            // or before the watcher processes the event, so a client repeatedly reopening the
            // next `pending` row could reopen this exact same gone track forever (found live,
            // 2026-07-20 — a frontend-only skip-list still cycled between multiple gone tracks).
            // Fix at the source: drop the row here too, same as the watcher does.
            // Re-check existence right here (not just trusting `e`'s text) to shrink a real
            // TOCTOU window flagged in review: `analyze()`'s NotFound and this forget_path call
            // aren't atomic — if the file gets recreated in between (e.g. re-downloaded), a
            // stale error text would otherwise delete a row the watcher may have just re-added
            // as freshly pending.
            if allow_forget
                && e.contains(crate::analysis::decode::FILE_GONE)
                && !std::path::Path::new(&path).exists()
            {
                let conn = db::lock_conn(&conn)?;
                match crate::scanner::forget_path(&conn, &path) {
                    Ok(n) if n > 0 => {
                        drop(conn);
                        app.emit("queue:changed", ()).ok();
                    }
                    Ok(_) => {}
                    Err(err) => log::error!("analyze_path: forget_path failed for {path}: {err}"),
                }
            }
            return Err(e);
        }
    };
    // self-heal the cache: store the freshly-computed report — SANS sa grille de spectrogramme —
    // pour que la prochaine ouverture de cette piste soit instantanée. Le report rendu à
    // l'appelant garde la sienne : seule la version écrite en base est allégée.
    let mut report = report;
    match conn.lock() {
        Ok(conn) => {
            let _ = heal_cache(&conn, &path, &mut report);
        }
        Err(_) => log::error!("analyze_path: DB mutex poisoned, skipping report cache write"),
    }
    Ok(report)
}

/// Réécrit la ligne depuis le rapport FRAIS qu'`analyze_path` vient de calculer : rapport, version
/// de rapport, **et verdict + version de verdict**.
///
/// Le verdict était absent de cet UPDATE, et c'est le trou que documentait `VERDICT_CACHE_VERSION`
/// (verdict.rs) : la réparation rendait la ligne courante côté RAPPORT tout en lui laissant dans sa
/// colonne `verdict` la valeur produite par un moteur qui n'existe plus — or c'est cette colonne,
/// pas le rapport, que lisent la Bibliothèque, les Écartés et le compte « à re-sourcer ». Rien ne
/// justifiait l'écart : `report` porte déjà `verdict`, calculé par le moteur COURANT dans l'appel
/// qui précède. Les quatre colonnes partent donc dans le même `UPDATE`, comme dans
/// `worker::persist_report`, et ne peuvent plus diverger à l'écriture.
///
/// La forme stockée passe par `worker::verdict_str`, jamais par un littéral recopié : c'est la même
/// fonction que le pool, donc les deux producteurs de la colonne écrivent le même alphabet.
fn heal_cache(
    conn: &Connection,
    path: &str,
    report: &mut crate::analysis::AnalysisReport,
) -> rusqlite::Result<usize> {
    let Some(json) = cache_json(report) else {
        return Ok(0);
    };
    conn.execute(
        "UPDATE tracks SET report_json=?2, report_cache_ver=?3, verdict=?4, verdict_ver=?5
         WHERE path=?1",
        rusqlite::params![
            path,
            json,
            crate::analysis::REPORT_CACHE_VERSION,
            crate::worker::verdict_str(report.verdict),
            crate::analysis::verdict::VERDICT_CACHE_VERSION,
        ],
    )
}

/// Force a re-analysis of the given tracks (still-pending only): clears their cached verdict
/// and analysis timestamp so `worker::select_needing_analysis` picks them back up, then wakes the pool.
/// Used for tracks stuck unanalysed (e.g. a transient decode error on first pass) — the user
/// asks Sift to try again rather than waiting indefinitely with no way to retry.
#[tauri::command]
pub fn reanalyze_tracks(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    track_ids: Vec<i64>,
) -> Result<usize, String> {
    let n = {
        let conn = db::lock_conn(&conn)?;
        queue::reset_analysis(&conn, &track_ids).map_err(|e| e.to_string())?
    };
    // Always re-render the queue, even when n==0 (every targeted row had already left `pending`).
    // The frontend's reanalyze buttons disable themselves on click and rely on this queue:changed
    // to re-enable via a re-render; without an unconditional emit an n==0 result leaves them stuck
    // disabled with no error (review-caught). refill only matters when rows were actually reset.
    app.emit("queue:changed", ()).ok();
    if n > 0 {
        crate::worker::refill(&app);
    }
    Ok(n)
}

/// Sérialise `report` pour le cache `tracks.report_json`, **sans** sa grille de spectrogramme,
/// puis la lui rend. Emprunte `&mut` plutôt que de cloner : la grille pèse ~376 ko, et la rendre
/// intacte à l'appelant fait partie du contrat — le rapport retourné à la webview doit rester
/// complet, c'est seulement la copie EN BASE qui est allégée.
///
/// Pourquoi ce n'est pas un détail d'implémentation : c'est le seul point du code où le rapport
/// est écrit en base côté IPC, donc le seul endroit où la grille pourrait se réintroduire dans le
/// cache sans que personne le remarque — exactement ce qui est arrivé au cap `MAX_PEAKS`
/// (migration v20). L'invariant est verrouillé par `cache_json_ne_stocke_jamais_la_grille`.
fn cache_json(report: &mut crate::analysis::AnalysisReport) -> Option<String> {
    let grid = std::mem::take(&mut report.spectrogram);
    let json = serde_json::to_string(&*report).ok();
    report.spectrogram = grid;
    json
}

/// Return a filesystem path the webview's audio engine can actually play, AND grant the asset
/// protocol read access to exactly that one file. Chromium plays mp3/wav/flac/m4a/ogg directly,
/// but NOT AIFF — so for .aif/.aiff we transcode once to a cached temp WAV and return that.
/// The caller wraps the result with convertFileSrc.
///
/// The `asset:` scope starts EMPTY (`tauri.conf.json`), so this command is the only door for
/// audio. That is also why it refuses a path the DB doesn't know: without that check it would
/// hand out read access to any path on demand, and the empty scope would buy nothing — an
/// injected script would call `playback_url` on `~/.ssh/id_rsa` and then simply read it. The
/// check narrows the blast radius to files Sift already indexed, which the webview can enumerate
/// through the normal listing commands anyway.
#[tauri::command]
pub fn playback_url(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    path: String,
) -> Result<String, String> {
    {
        let c = db::lock_conn(&conn)?;
        let known: i64 = c
            .query_row(
                "SELECT count(*) FROM tracks WHERE path=?1",
                rusqlite::params![path],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if known == 0 {
            return Err(format!("chemin inconnu de la bibliothèque: {path}"));
        }
    }
    let ext = std::path::Path::new(&path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext != "aif" && ext != "aiff" {
        // Browser can play it as-is — it just has to become readable first.
        app.asset_protocol_scope()
            .allow_file(&path)
            .map_err(|e| e.to_string())?;
        return Ok(path);
    }
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    let dir = std::env::temp_dir().join("sift-play");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let out = dir.join(format!("{:x}.wav", h.finish()));
    // Re-encode unless a cached WAV exists AND is newer than the source — so a replaced file
    // (or a squatted/garbage temp at the predictable name) isn't served stale.
    let fresh = match (std::fs::metadata(&out), std::fs::metadata(&path)) {
        (Ok(o), Ok(s)) => match (o.modified(), s.modified()) {
            (Ok(om), Ok(sm)) => om >= sm,
            _ => false,
        },
        _ => false,
    };
    if !fresh {
        crate::encode::encode(
            &path,
            &out.to_string_lossy(),
            crate::encode::Target::Wav1644,
        )
        .map_err(|e| e.to_string())?;
    }
    // The temp WAV, not the source: it is what the webview will actually fetch.
    app.asset_protocol_scope()
        .allow_file(&out)
        .map_err(|e| e.to_string())?;
    Ok(out.to_string_lossy().to_string())
}

/// Open an external URL in the user's default browser (used by the Écartés buy links).
/// Only http(s) is accepted, so the command can't be coerced into launching a local program.
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("only http(s) urls are allowed".into());
    }
    open::that(&url).map_err(|e| e.to_string())
}

/// Impasse A4 ([issue #15](https://github.com/c0remusic/sift/issues/15)) : dire au front qu'un
/// scan n'a pas eu lieu.
///
/// Sans ça, `spawn_scan` avait quatre sorties silencieuses — deux `return` nus et deux
/// `log::error!` — après lesquelles la source restait à `pending_count = 0`, ce que l'écran
/// Accueil peint d'une pastille verte « À jour ». Le cas transitoire (scan encore en cours) se
/// corrige tout seul ; ces quatre-là rendaient le mensonge PERMANENT, et un journal serveur que
/// personne ne lit n'est pas un signal.
///
/// Un événement plutôt qu'une colonne : rien ici n'a besoin de survivre au redémarrage — un scan
/// se rejoue, et la migration de schéma que coûterait une colonne d'état serait payée par toutes
/// les bases utilisateurs pour une information qui vaut le temps d'une session.
fn emit_scan_failed(app: &AppHandle, source_id: i64, reason: String) {
    log::error!("scan source {source_id} failed: {reason}");
    app.emit("scan:failed", (source_id, reason)).ok();
}

/// Runs a reconcile for `source_id` on a background thread (walkdir is blocking IO),
/// then starts the live watcher and notifies the front. Errors are logged AND surfaced to the
/// front via `scan:failed` — see `emit_scan_failed` for why a log alone was not enough.
fn spawn_scan(app: AppHandle, source_id: i64) {
    std::thread::spawn(move || {
        // Use a SEPARATE connection: a full-folder walkdir + per-file upserts must not hold
        // the shared Mutex<Connection> — doing so froze every IPC call and the analysis
        // workers for the whole scan. WAL + busy_timeout let this second connection write
        // concurrently (writers serialize briefly instead of erroring).
        let db_path = match app.path().app_data_dir() {
            Ok(d) => d.join("sift.db"),
            Err(e) => {
                emit_scan_failed(
                    &app,
                    source_id,
                    format!("dossier de données de l'application introuvable : {e}"),
                );
                return;
            }
        };
        let conn = match Connection::open(&db_path) {
            Ok(c) => c,
            Err(e) => {
                emit_scan_failed(
                    &app,
                    source_id,
                    format!("ouverture de la base échouée : {e}"),
                );
                return;
            }
        };
        let _ = conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;");

        let path: Option<String> = conn
            .query_row(
                "SELECT path FROM sources WHERE id=?1",
                rusqlite::params![source_id],
                |r| r.get(0),
            )
            .ok();
        let Some(path) = path else {
            // La ligne source a disparu entre l'ordre de scan et son exécution — un retrait
            // concurrent, typiquement. Rien à scanner, mais le front doit cesser d'attendre.
            emit_scan_failed(
                &app,
                source_id,
                "ce dossier surveillé n'existe plus en base".to_string(),
            );
            return;
        };

        // Ré-émet queue:changed tous les PROGRESS_BATCH fichiers net-changés (scanner.rs) pendant
        // le scan, en plus de l'émission finale ci-dessous — le front debounce déjà sa redraw
        // à 150ms (sift-live.ts) donc aucune saturation IPC/UI même sur une grosse bibliothèque.
        match scanner::reconcile_with_progress(
            &conn,
            source_id,
            std::path::Path::new(&path),
            |_done| {
                app.emit("queue:changed", ()).ok();
            },
        ) {
            Ok(stats) => log::info!("scan source {source_id}: {stats:?}"),
            Err(e) => emit_scan_failed(&app, source_id, e.to_string()),
        }
        crate::watcher::start(&app, source_id, &path);
        app.emit("queue:changed", ()).ok();
        crate::worker::refill(&app);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::Spectrogram;

    /// L'invariant qui tient la Phase 5 : ce qui part en base ne contient JAMAIS la grille, et le
    /// rapport rendu à l'appelant la garde intacte. Les deux moitiés comptent — n'écrire que la
    /// première laisserait la webview sans spectrogramme au moment précis où elle vient de le
    /// demander.
    #[test]
    fn cache_json_ne_stocke_jamais_la_grille() {
        let mut r = crate::worker::tests::fake_report();
        r.spectrogram = Spectrogram {
            frames: 2,
            bins: 3,
            hz_per_bin: 21.5,
            sec_per_frame: 0.05,
            mag_db: vec![9, 8, 7, 6, 5, 4],
        };

        let json = cache_json(&mut r).expect("sérialisation");

        let relu: crate::analysis::AnalysisReport =
            serde_json::from_str(&json).expect("le cache doit rester deserialisable");
        assert!(
            relu.spectrogram.mag_db.is_empty(),
            "la grille est partie en base — c'est 450 ko par piste qui reviennent"
        );
        assert_eq!(relu.spectrogram.frames, 0);
        // Ce qui DOIT survivre au passage en cache : le verdict et la waveform, les deux choses
        // qui doivent rester instantanées à l'ouverture d'une piste.
        assert_eq!(relu.peaks, r.peaks);
        assert_eq!(relu.verdict, r.verdict);
        assert_eq!(relu.cutoff_hz, r.cutoff_hz);
        // Et le rapport de l'appelant est rendu intact.
        assert_eq!(r.spectrogram.mag_db, vec![9, 8, 7, 6, 5, 4]);
        assert_eq!(r.spectrogram.frames, 2);
    }

    /// La réparation de cache écrit les QUATRE colonnes, pas deux. Aucun test ne figeait l'ancien
    /// comportement (réparation sans verdict) — c'était un trou, pas une décision : la doc de
    /// `VERDICT_CACHE_VERSION` le nommait noir sur blanc comme le chemin par lequel `verdict` et
    /// `report_cache_ver` divergeaient.
    ///
    /// La ligne part d'un verdict PÉRIMÉ et faux (`"ok"` en version 1) : c'est l'état réel d'une
    /// piste rangée après un bump, celui que `verdict::cached` efface à la lecture.
    #[test]
    fn heal_cache_repare_aussi_le_verdict_pas_seulement_le_rapport() {
        let conn = Connection::open_in_memory().expect("base memoire");
        crate::db::run_migrations(&conn).expect("migrations");
        conn.execute(
            "INSERT INTO tracks (path, filename, status, verdict, verdict_ver, report_json,
                                 report_cache_ver, analyzed_at)
             VALUES ('x.flac', 'x.flac', 'filed', 'ok', 1, '{}', 1, datetime('now'))",
            [],
        )
        .expect("seed");

        let mut r = crate::worker::tests::fake_report(); // verdict = Fake
        let n = heal_cache(&conn, "x.flac", &mut r).expect("update");

        assert_eq!(n, 1);
        let (verdict, ver, rapport_ver): (Option<String>, Option<i64>, Option<i64>) = conn
            .query_row(
                "SELECT verdict, verdict_ver, report_cache_ver FROM tracks WHERE path='x.flac'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("relecture");
        assert_eq!(
            verdict.as_deref(),
            Some("fake"),
            "le verdict frais est en base : sans lui la Bibliotheque garde le badge de l'ancien moteur"
        );
        assert_eq!(
            ver,
            Some(crate::analysis::verdict::VERDICT_CACHE_VERSION),
            "stampe a la version courante, sinon verdict::cached l'efface a la lecture"
        );
        assert_eq!(rapport_ver, Some(crate::analysis::REPORT_CACHE_VERSION));
    }
}
