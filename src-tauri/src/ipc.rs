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
    Ok(())
}

#[tauri::command]
pub fn list_queue(conn: State<'_, Mutex<Connection>>) -> Result<Vec<queue::QueueItem>, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    queue::list_pending(&conn).map_err(|e| e.to_string())
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
    });
}
