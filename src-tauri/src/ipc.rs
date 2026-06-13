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
    let conn = conn.lock().map_err(|e| e.to_string())?;
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
        let conn = conn.lock().map_err(|e| e.to_string())?;
        sources::add(&conn, &path).map_err(|e| e.to_string())?
    };
    spawn_scan(app, id);
    let conn = conn.lock().map_err(|e| e.to_string())?;
    sources::list(&conn)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| "source not found after insert".to_string())
}

#[tauri::command]
pub fn list_sources(conn: State<'_, Mutex<Connection>>) -> Result<Vec<sources::Source>, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    sources::list(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_source(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    id: i64,
) -> Result<(), String> {
    {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        sources::remove(&conn, id).map_err(|e| e.to_string())?;
    }
    crate::watcher::stop(&app, id);
    app.emit("queue:changed", ()).ok();
    crate::worker::refill(&app);
    Ok(())
}

#[tauri::command]
pub fn list_queue(conn: State<'_, Mutex<Connection>>) -> Result<Vec<queue::QueueItem>, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    let mut items = queue::list_pending(&conn).map_err(|e| e.to_string())?;
    // Annotate name-duplicate items so the queue can badge them before they're opened.
    let dups = crate::dedup::name_dups(&conn).map_err(|e| e.to_string())?;
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
        let conn = conn.lock().map_err(|e| e.to_string())?;
        sources::set_watched(&conn, id, watched).map_err(|e| e.to_string())?
    };
    if watched {
        crate::watcher::start(&app, id, &path);
    } else {
        crate::watcher::stop(&app, id);
    }
    Ok(())
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
    let mut scan_ids: Vec<i64> = Vec::new();
    {
        let conn = conn.lock().map_err(|e| e.to_string())?;
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
                    if let Some(root) = &dest_root {
                        let name = pb.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        if !name.is_empty() && crate::library::create_bin(root, "", name).is_ok() {
                            folders_added += 1;
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
                files_added += conn
                    .execute(
                        "INSERT INTO tracks (path, filename, status, created_at)
                         VALUES (?1, ?2, 'pending', datetime('now'))
                         ON CONFLICT(path) DO NOTHING",
                        rusqlite::params![p, filename],
                    )
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    for id in scan_ids {
        spawn_scan(app.clone(), id);
    }
    app.emit("queue:changed", ()).ok();
    crate::worker::refill(&app);
    Ok(ImportResult { files_added, folders_added })
}

#[derive(Serialize)]
pub struct AnalysisProgress {
    pub done: i64,
    pub total: i64,
}

/// Background-analysis progress: how many pending tracks are already analysed, out of total.
#[tauri::command]
pub fn analysis_progress(
    conn: State<'_, Mutex<Connection>>,
) -> Result<AnalysisProgress, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    let (done, total) = crate::worker::progress(&conn).map_err(|e| e.to_string())?;
    Ok(AnalysisProgress { done, total })
}

/// Run the analysis engine on a track and return the full report. Constrained to paths Sift
/// already knows (present in `tracks`) so the webview can't turn this into an arbitrary
/// file-read / decode oracle on any path on disk.
#[tauri::command]
pub fn analyze_path(
    conn: State<'_, Mutex<Connection>>,
    path: String,
    with_spectrogram: bool,
) -> Result<crate::analysis::AnalysisReport, String> {
    {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        let known = conn
            .query_row("SELECT 1 FROM tracks WHERE path=?1 LIMIT 1", rusqlite::params![path], |_| Ok(()))
            .is_ok();
        if !known {
            return Err("unknown track path".into());
        }
    }
    crate::analysis::analyze(&path, with_spectrogram)
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

/// Runs a reconcile for `source_id` on a background thread (walkdir is blocking IO),
/// then starts the live watcher and notifies the front. Errors are logged, not fatal.
fn spawn_scan(app: AppHandle, source_id: i64) {
    std::thread::spawn(move || {
        let path: Option<String> = {
            let state = app.state::<Mutex<Connection>>();
            let conn = match state.lock() {
                Ok(c) => c,
                Err(_) => return,
            };
            conn.query_row(
                "SELECT path FROM sources WHERE id=?1",
                rusqlite::params![source_id],
                |r| r.get(0),
            )
            .ok()
        };
        let Some(path) = path else { return };

        let result = {
            let state = app.state::<Mutex<Connection>>();
            let conn = match state.lock() {
                Ok(c) => c,
                Err(_) => return,
            };
            scanner::reconcile(&conn, source_id, std::path::Path::new(&path))
        };
        match result {
            Ok(stats) => log::info!("scan source {source_id}: {stats:?}"),
            Err(e) => log::error!("scan source {source_id} failed: {e}"),
        }
        crate::watcher::start(&app, source_id, &path);
        app.emit("queue:changed", ()).ok();
        crate::worker::refill(&app);
    });
}
