use crate::{db, ffmpeg};
use rusqlite::Connection;
use serde::Serialize;
use std::sync::Mutex;
use tauri::State;

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
