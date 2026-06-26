//! IPC surface for the M4 filing loop: reconcile / file / reject / trash a track, manage
//! destination bins, read & write settings, and undo. Thin wrappers that lock the shared
//! Connection, resolve the library root from settings, delegate to the domain modules
//! (naming/encode/tagging/library/actions/filing/settings), and emit `queue:changed` after
//! any mutation so the front refreshes. Errors are flattened to strings (Tauri convention);
//! a missing library root surfaces as the sentinel `"NoLibraryRoot"` so the front can route
//! the user to the settings panel rather than show a raw message.
//!
//! Note: the slow ffmpeg encode runs OUTSIDE the DB lock. `file_track` splits plan/execute/commit
//! so the lock is released around the encode; `file_batch` runs detached on a background thread and
//! takes the lock PER FILE — so a long filing never freezes the UI nor blocks the analysis worker.

use crate::actions::{self, JournalEntry};
use crate::dedup::{self, DupMatch};
use crate::ecartes::{self, EcarteItem};
use crate::encode::Target;
use crate::filing::{self, BatchResult, FileResult, RejectBatchResult};
use crate::library::{self, Bin};
use crate::naming::Canonical;
use crate::settings;
use rusqlite::Connection;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

/// Resolve the configured library root, or the `"NoLibraryRoot"` sentinel error when unset
/// or blank. All filing/bin commands need this.
fn library_root(conn: &Connection) -> Result<PathBuf, String> {
    match settings::get(conn, settings::LIBRARY_ROOT).map_err(|e| e.to_string())? {
        Some(p) if !p.trim().is_empty() => Ok(PathBuf::from(p)),
        _ => Err("NoLibraryRoot".into()),
    }
}

/// The active filename template, falling back to the default when unset.
fn template(conn: &Connection) -> String {
    settings::get_or(conn, settings::FILENAME_TEMPLATE, settings::DEFAULT_TEMPLATE)
        .unwrap_or_else(|_| settings::DEFAULT_TEMPLATE.to_string())
}

/// Reconcile a track's tags + filename into the canonical record + confidence (drives the
/// editable fields and the green/yellow badge in the review pane).
#[tauri::command]
pub fn reconcile(conn: State<'_, Mutex<Connection>>, track_id: i64) -> Result<Canonical, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    filing::reconcile_track(&conn, track_id).map_err(|e| e.to_string())
}

/// File one track into `bin_rel`. `target` overrides the rail default (e.g. force MP3);
/// `edited` overrides the reconciled metadata with the user's corrections.
#[tauri::command]
pub fn file_track(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    track_id: i64,
    bin_rel: String,
    target: Option<Target>,
    edited: Option<Canonical>,
) -> Result<FileResult, String> {
    // Phase 1 under the lock: decide the plan (fast DB reads + guard + dest).
    let plan = {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        let root = library_root(&conn)?;
        let tmpl = template(&conn);
        filing::plan_file(&conn, &root, &tmpl, track_id, &bin_rel, target, edited)
            .map_err(|e| e.to_string())?
    };
    // Phase 2 WITHOUT the lock: the multi-second ffmpeg encode + file moves.
    let log = filing::execute_file(&plan).map_err(|e| e.to_string())?;
    // Phase 3 under the lock: journal + mark filed (rolls back the FS on a DB error).
    let res = {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        filing::commit_file(&conn, &plan, log).map_err(|e| e.to_string())?
    };
    app.emit("queue:changed", ()).ok();
    Ok(res)
}

/// Launch filing of `track_ids` into `bin_rel` IN THE BACKGROUND and return immediately. The
/// actual work (per-file convert/tag/move + journal) runs on a dedicated thread via
/// `run_file_batch`, taking and releasing the DB lock PER FILE — so a long batch never freezes
/// the UI nor blocks the analysis worker (a sync command holding the lock across the whole batch
/// would do both). The library root is resolved synchronously so a missing one fails the invoke
/// right away (front routes to Settings via the `"NoLibraryRoot"` sentinel). When the run finishes
/// it emits `file:done` with the `BatchResult` summary. Filing logic (plan/execute/commit) and the
/// `actions` journal are unchanged — only the execution site and the lock scope move.
#[tauri::command]
pub fn file_batch(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    track_ids: Vec<i64>,
    bin_rel: String,
) -> Result<(), String> {
    let (root, tmpl) = {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        (library_root(&conn)?, template(&conn))
    };
    // Detach onto a named OS thread (blocking work: ffmpeg encodes + fs moves + rusqlite). Fail-fast:
    // if the thread can't even be started, surface it to the front rather than dropping the batch.
    let app_bg = app.clone();
    std::thread::Builder::new()
        .name("file-batch".into())
        .spawn(move || run_file_batch(&app_bg, root, tmpl, track_ids, bin_rel))
        .map_err(|e| format!("file_batch: failed to start background task: {e}"))?;
    Ok(())
}

/// Per-file filing progress for the global progress zone (`kind="file"` row). `done` = files
/// processed so far (filed or bounced to needs_validation), `total` = the batch size.
#[derive(Serialize)]
struct FileProgress {
    done: usize,
    total: usize,
}

/// Background body of `file_batch` (off the main thread). Files each id by REUSING the three
/// filing primitives with a PER-FILE lock window: phase 1 `plan_file` (lock) → phase 2
/// `execute_file` (NO lock: the slow ffmpeg encode + fs moves) → phase 3 `commit_file` (lock:
/// journal `actions` + mark filed). A track with no auto-file canonical (yellow + no Discogs
/// identity) or that errors mid-filing is left pending and reported in `needs_validation` — same
/// outcome as the old in-process `filing::file_batch`. Emits `file:done` with the summary.
fn run_file_batch(
    app: &AppHandle,
    root: PathBuf,
    tmpl: String,
    track_ids: Vec<i64>,
    bin_rel: String,
) {
    let state = app.state::<Mutex<Connection>>();
    let total = track_ids.len();
    let mut filed = 0usize;
    let mut needs_validation = Vec::new();

    for id in track_ids {
        // Progress (sous-étape 2): files completed before this one (filed or bounced). Emitted at
        // the TOP so it also covers the iterations that `continue`/`break` out of the body below —
        // the filing logic itself is untouched. The front feeds the zone's kind="file" row from it.
        app.emit("file:progress", &FileProgress { done: filed + needs_validation.len(), total }).ok();
        // Phase 1 (lock): pick the auto-file canonical + build the plan. No fileable name → pending.
        let plan = {
            let conn = match state.lock() {
                Ok(c) => c,
                Err(e) => {
                    log::error!("file_batch: DB lock poisoned before file {id}: {e}");
                    break;
                }
            };
            match filing::batch_canonical(&conn, id) {
                Some(c) => match filing::plan_file(&conn, &root, &tmpl, id, &bin_rel, None, Some(c)) {
                    Ok(p) => p,
                    Err(_) => {
                        needs_validation.push(id);
                        continue;
                    }
                },
                None => {
                    needs_validation.push(id);
                    continue;
                }
            }
        };
        // Phase 2 (NO lock): the slow ffmpeg encode + file moves.
        let log = match filing::execute_file(&plan) {
            Ok(l) => l,
            Err(_) => {
                needs_validation.push(id);
                continue;
            }
        };
        // Phase 3 (lock): journal the effects + mark filed (rolls back the FS on a DB error).
        let conn = match state.lock() {
            Ok(c) => c,
            Err(e) => {
                log::error!("file_batch: DB lock poisoned committing file {id}: {e}");
                break;
            }
        };
        match filing::commit_file(&conn, &plan, log) {
            Ok(_) => filed += 1,
            Err(_) => needs_validation.push(id),
        }
    }

    // Final progress (all processed) so the zone flashes 100% "done" before hiding — emitted
    // before `needs_validation` is moved into the summary below.
    app.emit("file:progress", &FileProgress { done: filed + needs_validation.len(), total }).ok();
    // Done: hand the summary to the front, which refreshes the view (replacing the end-of-batch
    // `queue:changed` the synchronous command used to emit).
    app.emit("file:done", &BatchResult { filed, needs_validation }).ok();
}

/// Mark a track for re-sourcing (Écartés). Status-only at this milestone.
#[tauri::command]
pub fn reject_track(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    track_id: i64,
) -> Result<(), String> {
    {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        filing::reject_track(&conn, track_id).map_err(|e| e.to_string())?;
    }
    app.emit("queue:changed", ()).ok();
    Ok(())
}

/// Reject a batch of tracks for re-sourcing (each → Écartés). Status-only at this milestone.
/// Returns how many were marked and which ids failed (a misfire is reported, never aborts the rest).
#[tauri::command]
pub fn reject_batch(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    track_ids: Vec<i64>,
) -> Result<RejectBatchResult, String> {
    let res = {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        filing::reject_batch(&conn, &track_ids)
    };
    app.emit("queue:changed", ()).ok();
    Ok(res)
}

/// Move a track's file to `.sift-trash` (reversible via undo) and mark it trashed.
#[tauri::command]
pub fn trash_track(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    track_id: i64,
) -> Result<(), String> {
    {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        let root = library_root(&conn)?;
        filing::trash_track(&conn, &root, track_id).map_err(|e| e.to_string())?;
    }
    app.emit("queue:changed", ()).ok();
    Ok(())
}

/// List all destination bins (recursive subdirs of the library root).
#[tauri::command]
pub fn list_bins(conn: State<'_, Mutex<Connection>>) -> Result<Vec<Bin>, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    let root = library_root(&conn)?;
    Ok(library::list_bins(&root))
}

/// Create a new bin under `parent_rel` ("" = root level). Returns the created bin.
#[tauri::command]
pub fn create_bin(
    conn: State<'_, Mutex<Connection>>,
    parent_rel: String,
    name: String,
) -> Result<Bin, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    let root = library_root(&conn)?;
    library::create_bin(&root, &parent_rel, &name)
}

/// Undo the most recent live batch (LIFO). Returns the reverted batch id, or null when there
/// is nothing to undo.
#[tauri::command]
pub fn undo_last(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
) -> Result<Option<String>, String> {
    let res = {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        actions::undo_last(&conn).map_err(|e| e.to_string())?
    };
    app.emit("queue:changed", ()).ok();
    Ok(res)
}

/// Revert a specific batch by id (used from the journal). Blocked if a newer action depends
/// on the same track.
#[tauri::command]
pub fn revert_batch(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    batch_id: String,
) -> Result<(), String> {
    {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        actions::revert_batch(&conn, &batch_id).map_err(|e| e.to_string())?;
    }
    app.emit("queue:changed", ()).ok();
    Ok(())
}

/// Recent live (not-yet-undone) batches, newest first, for the journal UI.
#[tauri::command]
pub fn list_journal(
    conn: State<'_, Mutex<Connection>>,
    limit: i64,
) -> Result<Vec<JournalEntry>, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    Ok(actions::list_journal(&conn, limit))
}

/// Best duplicate match for a track (by name; sound-confirmed once the acoustic layer lands).
#[tauri::command]
pub fn find_duplicate(
    conn: State<'_, Mutex<Connection>>,
    track_id: i64,
) -> Result<Option<DupMatch>, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    dedup::find_duplicate(&conn, track_id).map_err(|e| e.to_string())
}

/// List the rejected/trashed tracks for the Écartés view.
#[tauri::command]
pub fn list_ecartes(conn: State<'_, Mutex<Connection>>) -> Result<Vec<EcarteItem>, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    ecartes::list_ecartes(&conn).map_err(|e| e.to_string())
}

/// Restore a trashed track's file and re-queue it (status pending).
#[tauri::command]
pub fn restore_track(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    track_id: i64,
) -> Result<(), String> {
    {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        ecartes::restore_track(&conn, track_id)?;
    }
    app.emit("queue:changed", ()).ok();
    Ok(())
}

/// Put a re-sourcing track back into the queue (undo a "Re-sourcer" misclick).
#[tauri::command]
pub fn requeue_track(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    track_id: i64,
) -> Result<(), String> {
    {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        ecartes::requeue_track(&conn, track_id)?;
    }
    app.emit("queue:changed", ()).ok();
    Ok(())
}

/// Permanently empty the bin (delete trashed files). Returns how many were purged.
#[tauri::command]
pub fn purge_trash(app: AppHandle, conn: State<'_, Mutex<Connection>>) -> Result<usize, String> {
    let n = {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        ecartes::purge_trash(&conn)?
    };
    app.emit("queue:changed", ()).ok();
    Ok(n)
}

/// Read one app setting (null when unset).
#[tauri::command]
pub fn get_setting(
    conn: State<'_, Mutex<Connection>>,
    key: String,
) -> Result<Option<String>, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    settings::get(&conn, &key).map_err(|e| e.to_string())
}

/// Write one app setting (e.g. the library root chosen in the settings panel).
#[tauri::command]
pub fn set_setting(
    conn: State<'_, Mutex<Connection>>,
    key: String,
    value: String,
) -> Result<(), String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    settings::set(&conn, &key, &value).map_err(|e| e.to_string())
}
