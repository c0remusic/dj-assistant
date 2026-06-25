//! IPC surface for M6a identification. `identify` queries Discogs (token from settings) and
//! returns ranked candidates; `apply_identity_cmd` downloads the cover (best-effort) and
//! persists the chosen candidate. Errors are flattened to stable sentinel codes the front maps
//! to messages: NO_TOKEN, RATE_LIMITED:<s>, NETWORK, PARSE.

use crate::metadata::{self, AppliedIdentity, BatchPick, Candidate, MetadataProvider, Query};
use crate::settings;
use rusqlite::Connection;
use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;
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

/// Result of identifying a batch: how many got an auto-applied Discogs match, which ids had no
/// result, and which failed (each with a stable code). A misfire is reported, never aborts the rest.
#[derive(Serialize)]
pub struct IdentifyBatchResult {
    pub identified: usize,
    pub no_match: Vec<i64>,
    pub failed: Vec<BatchFailure>,
}

#[derive(Serialize)]
pub struct BatchFailure {
    pub id: i64,
    pub code: String,
}

/// Launch identification for many tracks IN THE BACKGROUND and return immediately. The actual
/// work (Discogs search + per-track apply) runs on a dedicated thread via `run_identify_batch`,
/// so the loop never blocks the main thread (a sync command runs on it, which would freeze the
/// window). The token is read synchronously so a missing one fails the invoke right away (the
/// front maps NO_TOKEN to a Settings prompt). When the background run finishes it emits
/// `identify:done` with the summary. Metadata-only and reversible — the engine (`pick_batch` /
/// `apply_identity`) is unchanged; only its execution site moves to the background thread.
#[tauri::command]
pub fn identify_batch(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    track_ids: Vec<i64>,
) -> Result<(), String> {
    let token = {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        settings::get(&conn, settings::DISCOGS_TOKEN)
            .map_err(|e| e.to_string())?
            .unwrap_or_default()
    };
    if token.trim().is_empty() {
        return Err("NO_TOKEN".into());
    }

    // Detach onto a named OS thread (the work is blocking: ureq + sleeps + rusqlite). Fail-fast:
    // if the thread can't even be started, surface it to the front rather than dropping the batch.
    let app_bg = app.clone();
    std::thread::Builder::new()
        .name("identify-batch".into())
        .spawn(move || run_identify_batch(&app_bg, token, track_ids))
        .map_err(|e| format!("identify_batch: failed to start background task: {e}"))?;
    Ok(())
}

/// Background body of `identify_batch` (runs off the main thread). Reconciles each id to a query,
/// searches Discogs (`pick_batch`), applies each top hit with a per-track DB write, then emits
/// `identify:done` with the summary. The shared connection is pulled from the app's managed state.
fn run_identify_batch(app: &AppHandle, token: String, track_ids: Vec<i64>) {
    let state = app.state::<Mutex<Connection>>();

    // Build the per-track queries up front (one lock); record any that can't be reconciled.
    let (queries, mut failed) = {
        let conn = match state.lock() {
            Ok(c) => c,
            Err(e) => {
                log::error!("identify_batch: DB lock poisoned before run: {e}");
                return;
            }
        };
        let mut queries = Vec::new();
        let mut failed: Vec<BatchFailure> = Vec::new();
        for id in &track_ids {
            match crate::filing::reconcile_track(&conn, *id) {
                Ok(c) => queries.push((
                    *id,
                    Query { artist: c.artist, title: c.title, version: c.version },
                )),
                Err(_) => failed.push(BatchFailure { id: *id, code: "RECONCILE".into() }),
            }
        }
        (queries, failed)
    };

    let provider = metadata::discogs::Discogs { token };
    let picks = metadata::pick_batch(&provider, &queries, |s| {
        std::thread::sleep(Duration::from_secs(s));
    });

    let mut identified = 0usize;
    let mut no_match = Vec::new();
    for (id, pick) in picks {
        match pick {
            BatchPick::Picked(c) => {
                // Cover download is best-effort and done outside the DB lock (network IO).
                let cover_path = c.cover_url.as_ref().and_then(|url| {
                    let dir = app.path().app_cache_dir().ok()?.join("covers");
                    metadata::cover::download_cover(&dir, &c.release_id, url)
                        .ok()
                        .map(|p| p.to_string_lossy().to_string())
                });
                let conn = match state.lock() {
                    Ok(c) => c,
                    Err(e) => {
                        log::error!("identify_batch: DB lock poisoned mid-run: {e}");
                        return;
                    }
                };
                match metadata::apply_identity(&conn, id, &c, cover_path) {
                    Ok(_) => identified += 1,
                    Err(e) => failed.push(BatchFailure { id, code: e.to_string() }),
                }
            }
            BatchPick::NoMatch => no_match.push(id),
            BatchPick::Failed(code) => failed.push(BatchFailure { id, code }),
        }
    }

    // Done: hand the summary to the front, which refreshes the view (replacing the end-of-batch
    // `queue:changed` that previously triggered the refresh).
    app.emit("identify:done", &IdentifyBatchResult { identified, no_match, failed })
        .ok();
}
