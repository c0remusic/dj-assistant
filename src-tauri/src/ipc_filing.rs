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
use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

/// Shared stop-net cancel flag for the background filing batch (sous-étape 3). Set by `file_cancel`,
/// checked between files by `run_file_batch`, and reset at the start of each new batch. Held in
/// Tauri managed state (see `lib.rs`), so it is shared without an explicit `Arc`.
#[derive(Default)]
pub struct FilingCancel(pub AtomicBool);

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

/// Read-only identity + release facts persisted by `apply_identity` in the `metadata` table.
/// `identified` is true when a Discogs release was chosen (`discogs_release_id` not NULL) — the
/// front then trusts `artist`/`title` here over what `reconcile` recomputes from the file tags
/// (which are untouched until filing). All fields are NULL / `identified:false` when there is no
/// metadata row yet. Fast DB read under the lock, NO network. Deliberately a sibling of `reconcile`
/// rather than folded into `Canonical` (the filename/tag contract): `version` is the remix/dub split
/// off the chosen Discogs title and persisted in `metadata.version` by `apply_identity`, so the
/// picked release survives a reopen; the front falls back to reconcile's version when it is NULL.
#[derive(Serialize)]
pub struct TrackRelease {
    pub artist: Option<String>,
    pub title: Option<String>,
    pub version: Option<String>,
    pub label: Option<String>,
    pub year: Option<i64>,
    pub cover_path: Option<String>,
    /// The track's sub-genres (track_genres), in stored order — the SAME list `write_tags_full`
    /// would join into the file's Genre field. The front shows them on open and uses them (joined)
    /// to detect when the file's tags diverge from the displayed identity.
    pub genres: Vec<String>,
    pub identified: bool,
}

#[tauri::command]
pub fn track_release(
    conn: State<'_, Mutex<Connection>>,
    track_id: i64,
) -> Result<TrackRelease, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    let genres = crate::genres::get_genres(&conn, track_id).unwrap_or_default();
    let base = conn
        .query_row(
            "SELECT artist, title, version, label, year, cover_path, discogs_release_id FROM metadata WHERE track_id=?1",
            rusqlite::params![track_id],
            |r| {
                let discogs_release_id: Option<String> = r.get(6)?;
                Ok(TrackRelease {
                    artist: r.get(0)?,
                    title: r.get(1)?,
                    version: r.get(2)?,
                    label: r.get(3)?,
                    year: r.get(4)?,
                    cover_path: r.get(5)?,
                    genres: Vec::new(),
                    identified: discogs_release_id.is_some(),
                })
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(match base {
        Some(mut tr) => {
            tr.genres = genres;
            tr
        }
        None => TrackRelease {
            artist: None,
            title: None,
            version: None,
            label: None,
            year: None,
            cover_path: None,
            genres,
            identified: false,
        },
    })
}

/// The file's REAL tag values (the fields `write_tags_full` owns), read once on open so the front
/// can flag — in memory, no per-keystroke disk read — when the displayed/Discogs identity has not
/// yet been written to the file. `genre_joined` is the single Genre field exactly as the file holds
/// it (the joined form `write_tags_full` produces), so the comparison matches like-for-like. Cover
/// is deliberately omitted (not needed for the comparison, and shipping its bytes would be wasteful).
#[derive(Serialize)]
pub struct FileTags {
    pub artist: Option<String>,
    pub title: Option<String>,
    pub label: Option<String>,
    pub year: Option<i64>,
    pub genre_joined: Option<String>,
}

#[tauri::command]
pub fn track_file_tags(
    conn: State<'_, Mutex<Connection>>,
    track_id: i64,
) -> Result<FileTags, String> {
    // Path under the lock; the actual file read happens AFTER releasing it (a disk read must not
    // freeze every other DB user — same split as apply_tags).
    let path: String = {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        conn.query_row("SELECT path FROM tracks WHERE id=?1", rusqlite::params![track_id], |r| r.get(0))
            .map_err(|_| format!("track {track_id} not found"))?
    };
    let snap = crate::tagging::read_tags_full(&path)?;
    Ok(FileTags {
        artist: snap.artist,
        title: snap.title,
        label: snap.label,
        year: snap.year,
        genre_joined: snap.genre_joined,
    })
}

/// Apply the edited identity (artist/title) + the track's stored enrichment (label/year/genres/
/// cover) onto the file's ID3 tags IN PLACE — no encode, no move, no status change. Captures the
/// OLD tags first and journals them as a revertable `tag_edit` action; returns its batch_id so the
/// front can offer a targeted undo. Works on ANY file, conformant or not. Mirrors filing's tag
/// write (`load_tag_extras` + `write_tags_full`) so an Apply and a File write the same tags.
#[tauri::command]
pub fn apply_tags(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    track_id: i64,
    edited: Canonical,
) -> Result<String, String> {
    // (1) Path + the same enrichment fields filing would write — under the lock.
    let (path, extras) = {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        let path: String = conn
            .query_row("SELECT path FROM tracks WHERE id=?1", rusqlite::params![track_id], |r| r.get(0))
            .map_err(|_| format!("track {track_id} not found"))?;
        let extras = filing::load_tag_extras(&conn, track_id);
        (path, extras)
    };

    // (2) Snapshot the OLD tags BEFORE writing (lock released — pure file read). Fail-fast if the
    // file can't be read: nothing has changed yet.
    let snapshot = crate::tagging::read_tags_full(&path)?;

    // (3) Write the NEW tags: artist/title from the edit, label/year/genres/cover from the DB — the
    // SAME set filing writes. On failure we stop; nothing is journaled.
    crate::tagging::write_tags_full(
        &path,
        &edited.artist,
        &edited.title,
        extras.label.as_deref(),
        extras.year,
        &extras.genres,
        extras.cover_path.as_deref(),
    )?;

    // (4) Journal the snapshot as a revertable tag_edit (from_path = the file, to_path = NULL). No
    // status change, no move — the revert just rewrites the old tags back.
    let meta = serde_json::to_string(&snapshot).map_err(|e| e.to_string())?;
    let batch_id = filing::new_batch_id(track_id);
    {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        actions::record_with_meta(&conn, &batch_id, Some(track_id), "tag_edit", Some(&path), None, Some(&meta))
            .map_err(|e| e.to_string())?;
    }
    app.emit("queue:changed", ()).ok();
    Ok(batch_id)
}

/// File one track into `bin_rel`. `target` overrides the rail default (e.g. force MP3);
/// `edited` overrides the reconciled metadata with the user's corrections. `allow_rail_mismatch`
/// (FIX-1): when the source's declared extension claims lossless but its content is actually
/// lossy (BUG-1 — e.g. an MP3 renamed `.flac`), filing is refused with the `"RAIL_MISMATCH"`
/// sentinel unless this is explicitly `true` — the front shows a confirmation dialog and, if the
/// user proceeds, retries the same call with it set.
#[tauri::command]
pub fn file_track(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    track_id: i64,
    bin_rel: String,
    target: Option<Target>,
    edited: Option<Canonical>,
    allow_rail_mismatch: Option<bool>,
) -> Result<FileResult, String> {
    // Phase 1 under the lock: decide the plan (fast DB reads + guard + dest).
    let plan = {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        let root = library_root(&conn)?;
        let tmpl = template(&conn);
        filing::plan_file(&conn, &root, &tmpl, track_id, &bin_rel, target, edited, allow_rail_mismatch.unwrap_or(false))
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
    // Per-track encode-target override (batch format chips). Absent ids fall back to the auto target
    // derived from the source rail (encode::target_for) — exactly the pre-chips behaviour.
    targets: Option<HashMap<i64, Target>>,
) -> Result<(), String> {
    let (root, tmpl) = {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        (library_root(&conn)?, template(&conn))
    };
    // Reset the cancel flag for THIS batch so a past cancel can't abort it instantly.
    app.state::<FilingCancel>().0.store(false, Ordering::SeqCst);
    // Detach onto a named OS thread (blocking work: ffmpeg encodes + fs moves + rusqlite). Fail-fast:
    // if the thread can't even be started, surface it to the front rather than dropping the batch.
    let app_bg = app.clone();
    std::thread::Builder::new()
        .name("file-batch".into())
        .spawn(move || run_file_batch(&app_bg, root, tmpl, track_ids, bin_rel, targets))
        .map_err(|e| format!("file_batch: failed to start background task: {e}"))?;
    Ok(())
}

/// Request a stop-net cancel of the running filing batch: the file currently being processed
/// finishes, then no new file starts (the flag is checked BETWEEN files in `run_file_batch`, never
/// mid-encode). Nothing is rolled back — already-filed tracks stay filed and the `actions` journal
/// is untouched. A no-op if no batch is running (the next batch resets the flag anyway).
#[tauri::command]
pub fn file_cancel(app: AppHandle) -> Result<(), String> {
    app.state::<FilingCancel>().0.store(true, Ordering::SeqCst);
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
    targets: Option<HashMap<i64, Target>>,
) {
    let state = app.state::<Mutex<Connection>>();
    let cancel = app.state::<FilingCancel>();
    let total = track_ids.len();
    let mut filed = 0usize;
    let mut needs_validation = Vec::new();
    let mut cancelled = false;

    for id in track_ids {
        // Stop-net cancel (sous-étape 3): checked BETWEEN files, never mid `execute_file`, so no
        // file is left half-processed and the DB stays consistent. The in-flight file (if any) has
        // already finished its three phases; we simply don't start a new one. Nothing is rolled
        // back — what is filed stays filed.
        if cancel.0.load(Ordering::SeqCst) {
            cancelled = true;
            break;
        }
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
                Some(c) => match filing::plan_file(
                    &conn,
                    &root,
                    &tmpl,
                    id,
                    &bin_rel,
                    targets.as_ref().and_then(|m| m.get(&id)).copied(),
                    Some(c),
                    // Batch never force-confirms a rail mismatch on the user's behalf — a track
                    // with a disguised source lands in needs_validation like any other filing
                    // error, so the user reviews and confirms it explicitly in Detail mode.
                    false,
                ) {
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

    // Final progress (processed so far) so the zone settles — emitted before `needs_validation`
    // is moved into the summary below.
    app.emit("file:progress", &FileProgress { done: filed + needs_validation.len(), total }).ok();
    // Done: hand the (possibly partial, if cancelled) summary to the front, which refreshes the
    // view (replacing the end-of-batch `queue:changed` the synchronous command used to emit).
    app.emit("file:done", &BatchResult { filed, needs_validation, cancelled }).ok();
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

/// Move a track's file to `.sift-trash` (reversible via undo) and mark it trashed. FIX-6: no
/// library-root precondition — the trash dir lives under Documents, not the library root.
#[tauri::command]
pub fn trash_track(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    track_id: i64,
) -> Result<(), String> {
    {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        filing::trash_track(&conn, track_id).map_err(|e| e.to_string())?;
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
/// `session_id` = Some(sid) restricts to one session; None = all sessions.
/// The front sends `{ sessionId: "..." }` which Tauri maps to `session_id` here.
#[tauri::command]
pub fn list_journal(
    conn: State<'_, Mutex<Connection>>,
    limit: i64,
    session_id: Option<String>,
) -> Result<Vec<JournalEntry>, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    Ok(actions::list_journal(&conn, limit, session_id.as_deref()))
}

/// The current app session ID (generated at launch, persisted in settings). Used by the
/// Journal tab front to filter list_journal to the current session only.
#[tauri::command]
pub fn get_session_id(conn: State<'_, Mutex<Connection>>) -> Result<String, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    settings::get(&conn, settings::CURRENT_SESSION_ID)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "no session_id in settings".to_string())
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
