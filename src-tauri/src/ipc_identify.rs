//! IPC surface for M6a identification. `identify` queries Discogs (token from settings) and
//! returns ranked candidates; `apply_identity_cmd` downloads the cover (best-effort) and
//! persists the chosen candidate. Errors are flattened to stable sentinel codes the front maps
//! to messages: NO_TOKEN, RATE_LIMITED:<s>, NETWORK, PARSE.

use crate::metadata::{self, AppliedIdentity, Candidate, MetadataProvider, Query};
use crate::settings;
use rusqlite::Connection;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

/// Query Discogs for `track_id`'s best-guess artist/title; ranked candidates, best first.
#[tauri::command]
pub fn identify(
    conn: State<'_, Mutex<Connection>>,
    track_id: i64,
) -> Result<Vec<Candidate>, String> {
    let (token, query) = {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        let token = settings::get(&conn, settings::DISCOGS_TOKEN)
            .map_err(|e| e.to_string())?
            .unwrap_or_default();
        let canonical = crate::filing::reconcile_track(&conn, track_id).map_err(|e| e.to_string())?;
        (token, Query { artist: canonical.artist, title: canonical.title, version: canonical.version })
    };
    if token.trim().is_empty() {
        return Err("NO_TOKEN".into());
    }
    let provider = metadata::discogs::Discogs { token };
    provider.search(&query).map_err(|e| e.code())
}

/// Persist a chosen candidate for `track_id`: download its cover (best-effort) then write the
/// metadata + genres. Emits `queue:changed` so the front refreshes.
#[tauri::command]
pub fn apply_identity_cmd(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    track_id: i64,
    candidate: Candidate,
) -> Result<AppliedIdentity, String> {
    // Gate to a known track before doing any work (network download / DB writes) — mirrors the
    // implicit gate `identify` gets from reconcile_track, so a bogus id can't drive a fetch.
    {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        let known = conn
            .query_row("SELECT 1 FROM tracks WHERE id=?1", rusqlite::params![track_id], |_| Ok(()))
            .is_ok();
        if !known {
            return Err("unknown track id".into());
        }
    }
    let cover_path = candidate.cover_url.as_ref().and_then(|url| {
        let dir = app.path().app_cache_dir().ok()?.join("covers");
        metadata::cover::download_cover(&dir, &candidate.release_id, url)
            .ok()
            .map(|p| p.to_string_lossy().to_string())
    });
    let applied = {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        metadata::apply_identity(&conn, track_id, &candidate, cover_path).map_err(|e| e.to_string())?
    };
    app.emit("queue:changed", ()).ok();
    Ok(applied)
}
