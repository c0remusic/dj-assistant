//! IPC surface for the M4 filing loop: reconcile / file / reject / trash a track, manage
//! destination bins, read & write settings, and undo. Thin wrappers that lock the shared
//! Connection, resolve the library root from settings, delegate to the domain modules
//! (naming/encode/tagging/library/actions/filing/settings), and emit `queue:changed` after
//! any mutation so the front refreshes. Errors are flattened to strings (Tauri convention);
//! a missing library root surfaces as the sentinel `"NoLibraryRoot"` so the front can route
//! the user to the settings panel rather than show a raw message.
//!
//! Note: `file_track`/`file_batch` hold the DB lock across the (possibly multi-second) ffmpeg
//! encode. That is acceptable for a single-user desktop app and keeps filing atomic; if the
//! background analysis worker ever contends visibly, move the encode off the lock.

use crate::actions::{self, JournalEntry};
use crate::encode::Target;
use crate::filing::{self, BatchResult, FileResult};
use crate::library::{self, Bin};
use crate::naming::Canonical;
use crate::settings;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

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

/// File every green track of `track_ids` into `bin_rel`; yellow/unreadable/errored ones stay
/// pending and come back in `needs_validation`.
#[tauri::command]
pub fn file_batch(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    track_ids: Vec<i64>,
    bin_rel: String,
) -> Result<BatchResult, String> {
    let res = {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        let root = library_root(&conn)?;
        let tmpl = template(&conn);
        filing::file_batch(&conn, &root, &tmpl, &track_ids, &bin_rel)
    };
    app.emit("queue:changed", ()).ok();
    Ok(res)
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
