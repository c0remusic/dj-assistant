//! IPC surface for the M4 filing loop: reconcile / file / reject / trash a track, manage
//! destination bins, read & write settings, and undo. Thin wrappers that lock the shared
//! Connection, resolve the library root from settings, delegate to the domain modules
//! (naming/encode/tagging/library/actions/filing/settings), and emit `queue:changed` after
//! any mutation so the front refreshes. Errors are flattened to strings (Tauri convention);
//! a missing library root surfaces as the sentinel `"NoLibraryRoot"` so the front can route
//! the user to the settings panel rather than show a raw message. Depuis #54 (2026-09-02), ce
//! refus est devenu une exigence de l'ARBRE et non du rangement : une destination EN PLACE ou un
//! dossier externe se règle sans racine, et n'a jamais lu la sienne (`library_root_for`).
//!
//! Note: the slow ffmpeg encode runs OUTSIDE the DB lock. `file_track` splits plan/execute/commit
//! so the lock is released around the encode; `file_batch` runs detached on a background thread and
//! takes the lock PER FILE — so a long filing never freezes the UI nor blocks the analysis worker.
//! Since P5 (PRD 2026-07-27, D3/D5) `file_track` is detached too: the invoke returns as soon as the
//! plan is settled and the encode runs on a background thread that reports through
//! `file:track:done` — the click is acknowledged, the conversion finishes behind it.
//! The same rule now holds for every other disk-touching command here: `reconcile` (tag read),
//! `trash_track` (byte-for-byte copy) and `list_bins` (recursive walk of the library tree) resolve
//! what they need under the lock, release it, do the I/O, and only re-take it to write.

use crate::actions::{self, JournalEntry};
use crate::db;
use crate::dedup::{self, DupMatch};
use crate::ecartes::{self, EcarteItem};
use crate::encode::Target;
use crate::filing::{self, BatchResult, FileResult, RejectBatchResult};
use crate::library::{self, Bin};
use crate::naming::Canonical;
use crate::settings;
use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager, State};

/// Shared stop-net cancel flag for the background filing batch (sous-étape 3). Set by `file_cancel`,
/// checked between files by `run_file_batch`, and reset at the start of each new batch. Held in
/// Tauri managed state (see `lib.rs`), so it is shared without an explicit `Arc`.
#[derive(Default)]
pub struct FilingCancel(pub AtomicBool);

/// Resolve the configured library root, or the `"NoLibraryRoot"` sentinel error when unset
/// or blank. Réservée aux commandes qui parlent de l'ARBRE lui-même (`list_bins`) : depuis #54,
/// un rangement ne passe plus par ici sans avoir demandé si sa destination vise l'arbre — voir
/// `library_root_for`.
fn library_root(conn: &Connection) -> Result<PathBuf, String> {
    match settings::get(conn, settings::LIBRARY_ROOT).map_err(|e| e.to_string())? {
        Some(p) if !p.trim().is_empty() => Ok(PathBuf::from(p)),
        _ => Err("NoLibraryRoot".into()),
    }
}

/// La racine dont CETTE destination a besoin — `None` quand elle n'en a pas besoin du tout.
///
/// Décision #54 (2026-09-02) : « on doit pouvoir convertir où on veut sans racine de bibliothèque ».
/// Un rangement EN PLACE (`FILE_IN_PLACE`) ou vers un dossier externe (`EXTERNAL_DEST_PREFIX`)
/// n'a jamais LU la racine dans `plan_file` — il la réclamait pourtant, parce que la lecture était
/// posée à l'entrée de la commande plutôt qu'au point qui s'en sert. Le refus `"NoLibraryRoot"`
/// n'existe donc plus que pour une destination de l'arbre.
fn library_root_for(conn: &Connection, bin_rel: &str) -> Result<Option<PathBuf>, String> {
    if filing::needs_library_root(bin_rel) {
        Ok(Some(library_root(conn)?))
    } else {
        Ok(None)
    }
}

/// The active filename template, falling back to the default when unset.
fn template(conn: &Connection) -> String {
    settings::get_or(
        conn,
        settings::FILENAME_TEMPLATE,
        settings::DEFAULT_TEMPLATE,
    )
    .unwrap_or_else(|_| settings::DEFAULT_TEMPLATE.to_string())
}

/// Reconcile a track's tags + filename into the canonical record + confidence (drives the
/// editable fields and the green/yellow badge in the review pane).
#[tauri::command]
pub fn reconcile(conn: State<'_, Mutex<Connection>>, track_id: i64) -> Result<Canonical, String> {
    // Path under the lock; reading the file's embedded tags happens AFTER releasing it (a disk
    // read must not freeze every other DB user — same split as track_file_tags / apply_tags).
    // Nothing is written, and the only state read under the lock is the path itself: if another
    // thread moves the file in between, the tag read simply falls back to the filename reconcile,
    // exactly as it would for any file that vanished — no DB row can be left inconsistent.
    let path = {
        let conn = db::lock_conn(&conn)?;
        filing::track_path(&conn, track_id).map_err(|e| e.to_string())?
    };
    Ok(filing::reconcile_path(&path))
}

/// Live preview of the filename Sift will actually produce, using the SAME
/// `naming::render_filename` (real template + `sanitize()`) the actual filing path calls
/// (FIX-12) — the front used to hardcode "{artist} - {title}" and skip `sanitize()` entirely,
/// so a title containing `/` previewed a name that would never match the real, sanitized file.
///
/// `template`: `None` (le cas de tous les appelants historiques) = le modèle ENREGISTRÉ. `Some`
/// = un modèle candidat, pas encore enregistré — c'est ce qui permet à l'écran Réglages de
/// montrer l'effet d'une saisie en cours SANS la persister, et surtout de le montrer via cette
/// commande plutôt qu'en réimplémentant `render_filename` + `sanitize` en TypeScript. Un modèle
/// candidat vide ou blanc retombe sur l'enregistré : c'est un état transitoire de frappe, pas
/// une demande de nom vide.
#[tauri::command]
pub fn preview_filename(
    conn: State<'_, Mutex<Connection>>,
    edited: Canonical,
    ext: String,
    template: Option<String>,
) -> Result<String, String> {
    let conn = db::lock_conn(&conn)?;
    let tmpl = match template {
        Some(t) if !t.trim().is_empty() => t,
        _ => self::template(&conn),
    };
    Ok(crate::naming::render_filename(&tmpl, &edited, &ext))
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
    let conn = db::lock_conn(&conn)?;
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
        let conn = db::lock_conn(&conn)?;
        conn.query_row(
            "SELECT path FROM tracks WHERE id=?1",
            rusqlite::params![track_id],
            |r| r.get(0),
        )
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

/// Builds the M8 Tier 3 `MetadataSyncValues` from an apply_tags edit — factored out as a pure
/// function (no I/O, no lock) so the value-mapping is unit-testable without a Tauri AppHandle/State.
fn metadata_sync_values_for_apply_tags(
    edited: &Canonical,
    extras: &filing::TagExtras,
    // The label actually written to the file (edited value, or the stored one as fallback) — the
    // Rekordbox sync candidate must record what the tag write decided, not the raw stored label.
    label: Option<&str>,
) -> actions::MetadataSyncValues {
    let (genre, label) = actions::sanitize_genre_label(&extras.genres, label);
    actions::MetadataSyncValues {
        artist: Some(edited.artist.clone()),
        title: Some(crate::naming::tag_title(edited)),
        label,
        year: extras.year,
        genre,
    }
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
        let conn = db::lock_conn(&conn)?;
        let path: String = conn
            .query_row(
                "SELECT path FROM tracks WHERE id=?1",
                rusqlite::params![track_id],
                |r| r.get(0),
            )
            .map_err(|_| format!("track {track_id} not found"))?;
        let extras = filing::load_tag_extras(&conn, track_id);
        (path, extras)
    };

    // (2) Snapshot the OLD tags BEFORE writing (lock released — pure file read). Fail-fast if the
    // file can't be read: nothing has changed yet.
    let snapshot = crate::tagging::read_tags_full(&path)?;

    // Label is now editable in the Revue pane (it rides on the Canonical): prefer the edited value,
    // falling back to the stored release label when the user left it blank/untouched. Trim so a lone
    // space never counts as an edit, and apply the same non-empty discipline as write_tags_full.
    // `edited_label` (the real edit only) is what gets persisted to the DB below; `label` (edit or
    // fallback) is what gets written to the file and recorded as the Rekordbox sync candidate.
    let edited_label = edited
        .label
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let label = edited_label.or(extras.label.as_deref());

    // (3) Write the NEW tags: artist/title/label from the edit, year/genres/cover from the DB — the
    // SAME set filing writes. On failure we stop; nothing is journaled. Title includes the version
    // suffix via naming::tag_title (same as filing.rs, same as the rendered filename) — previously
    // this passed &edited.title alone, silently dropping version from the actual ID3 tag.
    crate::tagging::write_tags_full(
        &path,
        &edited.artist,
        &crate::naming::tag_title(&edited),
        label,
        extras.year,
        &extras.genres,
        extras.cover_path.as_deref(),
    )?;

    // (4) Journal the snapshot as a revertable tag_edit (from_path = the file, to_path = NULL). No
    // status change, no move — the revert just rewrites the old tags back.
    let meta = serde_json::to_string(&snapshot).map_err(|e| e.to_string())?;
    let batch_id = filing::new_batch_id(track_id);
    {
        let conn = db::lock_conn(&conn)?;
        // Persist an edited label to the `metadata` table so a close+reopen reads it back (the file
        // now holds it, but `track_release` reads the DB) — label-only upsert, artist/title/version
        // and any release link untouched. Only a real edit writes; leaving the field as the stored
        // label is a no-op here. The revert of this tag_edit rewrites the file's OLD tags but not
        // this DB row (same as every other apply_tags field), so a reverted label reads as a normal
        // file-vs-display discrepancy, which the banner is meant to surface.
        if let Some(l) = edited_label {
            crate::metadata::set_metadata_label(&conn, track_id, l).map_err(|e| e.to_string())?;
        }
        let action_id = actions::record_with_meta(
            &conn,
            &batch_id,
            Some(track_id),
            "tag_edit",
            Some(&path),
            None,
            Some(&meta),
        )
        .map_err(|e| e.to_string())?;

        // M8 Tier 3: detect (read-only) a metadata sync candidate when linked to Rekordbox, and
        // (if this track has a stored cover) an artwork sync candidate too. Both detectors need
        // the same decrypted `master.db` index — resolve it ONCE here (mirrors filing.rs's
        // post-commit loop) rather than have each detector independently decrypt the file.
        if let Some(index) = actions::resolve_masterdb_index_if_linked(&conn) {
            let values = metadata_sync_values_for_apply_tags(&edited, &extras, label);
            actions::detect_masterdb_metadata_sync_with_index(
                &conn, &index, &path, track_id, &values, action_id,
            );

            // Only when this track actually has a stored cover — apply_tags never changes the
            // cover itself (it just re-applies whatever's already in `extras`), so this only
            // matters the first time a cover exists and hasn't been synced yet.
            if let Some(cover_path) = &extras.cover_path {
                actions::detect_masterdb_artwork_sync_with_index(
                    &conn, &index, &path, track_id, cover_path, action_id,
                );
            }
        }
    }
    app.emit("queue:changed", ()).ok();
    Ok(batch_id)
}

/// Refusal sentinel: this track's previous conversion is still running. Stable string (same
/// convention as `"NoLibraryRoot"` / `"RAIL_MISMATCH"`) so the front can word it — mirrored in
/// `filing-actions.ts`.
const ALREADY_FILING: &str = "ALREADY_FILING";

/// The interactive filings currently running in the background: the TRACKS they hold and the
/// DESTINATIONS they have claimed. Both windows were zero-width until P5, because the encode ran
/// inside the invoke; now that it does not, both have to be closed explicitly:
///
/// - `tracks` is the double-filing guard. A track keeps its `pending` status until its conversion
///   commits, so any front path that walks the queue (the auto-advance, the gone-file recovery
///   chain, a stale rail after a navigation) can still hand it back and file it a SECOND time from
///   the same source. The invariant belongs here, not in the front, because the front is exactly
///   what cannot be trusted to have refreshed.
/// - `dests` replaces the empty `reserved` set `file_track` used to pass `plan_file` (justified by
///   "phase 2 runs before any next plan" — no longer true). Two conversions launched back to back
///   are planned while NEITHER file exists on disk, so two tracks reconciling to the same name
///   would be handed the SAME destination and the second encode would land on the first. Same role
///   as `run_file_batch`'s local `reserved` set, which is seeded from this one so a batch cannot
///   plan onto an interactive in-flight destination either.
#[derive(Default)]
struct InFlightFilings {
    tracks: HashSet<i64>,
    dests: HashSet<String>,
}

fn inflight() -> &'static Mutex<InFlightFilings> {
    static REG: std::sync::OnceLock<Mutex<InFlightFilings>> = std::sync::OnceLock::new();
    REG.get_or_init(|| Mutex::new(InFlightFilings::default()))
}

/// Snapshot of the currently-claimed destinations. CLONED rather than held: this lock is always
/// released before the DB lock is taken, so the two are never nested in either order. A poisoned
/// registry fails the filing loudly instead of silently planning without reservations (which is
/// exactly how two tracks would end up sharing one destination).
fn reserved_dests() -> Result<HashSet<String>, String> {
    match inflight().lock() {
        Ok(g) => Ok(g.dests.clone()),
        Err(e) => {
            log::error!("file_track: in-flight filing registry poisoned: {e}");
            Err("in-flight filing registry poisoned".to_string())
        }
    }
}

/// Whether `track_id` is currently held by an in-flight filing. Sibling of `reserved_dests()`, same
/// lock discipline (taken and released here, never across the DB lock): it lets the BATCH path
/// honour the same "not twice" invariant as the interactive one, instead of relying on the front
/// having filtered the track out of its selection.
fn is_filing_inflight(track_id: i64) -> Result<bool, String> {
    match inflight().lock() {
        Ok(g) => Ok(g.tracks.contains(&track_id)),
        Err(e) => {
            log::error!("file_batch: in-flight filing registry poisoned: {e}");
            Err("in-flight filing registry poisoned".to_string())
        }
    }
}

/// Claim `track_id` and `dest` until this filing settles. Refuses with `ALREADY_FILING` when that
/// track is already converting — the point where "a track that left the queue cannot be filed
/// twice" is actually enforced.
fn reserve_filing(track_id: i64, dest: &str) -> Result<(), String> {
    match inflight().lock() {
        Ok(mut g) => {
            if !g.tracks.insert(track_id) {
                return Err(ALREADY_FILING.to_string());
            }
            // The destination was already claimed by ANOTHER in-flight filing. Reachable because
            // `reserved_dests()` is read before the DB lock while this claim is taken after the
            // plan (the lock + `plan_file`'s own I/O sit in between), so two concurrent filings of
            // two different tracks reconciling to the same name can both plan onto it. Refuse
            // rather than let two encodes write the same path — and give the track back.
            if !g.dests.insert(dest.to_string()) {
                g.tracks.remove(&track_id);
                log::error!(
                    "file_track: destination already claimed by another in-flight filing: {dest}"
                );
                return Err("destination déjà réservée par une conversion en cours".to_string());
            }
            Ok(())
        }
        Err(e) => {
            log::error!("file_track: in-flight filing registry poisoned: {e}");
            Err("in-flight filing registry poisoned".to_string())
        }
    }
}

/// Drop the claim. Best-effort by construction (it runs at the very end of the background thread,
/// where there is no caller left to fail), but never silent: a poisoned registry is logged.
fn release_filing(track_id: i64, dest: &str) {
    match inflight().lock() {
        Ok(mut g) => {
            g.tracks.remove(&track_id);
            g.dests.remove(dest);
        }
        Err(e) => log::error!("file_track: in-flight filing registry poisoned: {e}"),
    }
}

/// The outcome of the background half of an interactive `file_track`, emitted as `file:track:done`.
/// Mirrors `shared/contracts.ts`'s `TrackFileOutcome`. `error: Some(_)` means the filing did NOT
/// happen and the track is still `pending` — the same "needs validation" bounce `run_file_batch`
/// reports in bulk (`BatchResult::needs_validation`), reported here for one interactive track.
#[derive(Serialize)]
pub struct TrackFileOutcome {
    pub track_id: i64,
    pub batch_id: String,
    /// The filed path — `Some` only when the filing actually committed.
    pub path: Option<String>,
    /// Failure cause, `None` on success.
    pub error: Option<String>,
}

/// Background body of `file_track` (off the invoke thread): phase 2 (the multi-second ffmpeg
/// encode and file moves, NO lock) then phase 3 (journal + mark filed, lock taken and released).
/// Emits `file:track:done` in EVERY outcome — success, encode failure, poisoned lock, or a panic —
/// so the front is never left waiting on an event that will not come (a track it believes is still
/// converting is hidden from the queue). `queue:changed` is emitted only when something actually
/// changed, i.e. a committed filing.
fn run_file_track(app: &AppHandle, plan: filing::FilePlan) {
    // Resolved once, at fn scope: a `State` handle taken inside the match arm below would be
    // dropped while its `MutexGuard` is still borrowed (E0597) — same shape as `run_file_batch`.
    let state = app.state::<Mutex<Connection>>();
    let track_id = plan.track_id();
    let batch_id = plan.batch_id().to_string();
    let dest = plan.dest_path().to_string();

    // Phase 2. `execute_file` decodes/encodes an arbitrary user file through the ffmpeg sidecar and
    // writes tags with lofty: the same "heavy work on an unvetted user file, on a thread nobody
    // joins" shape as worker.rs's analysis loop, so it gets the same catch_unwind treatment — a
    // panic here must become a normal failure, not a silently vanished thread.
    let executed = {
        // Place prise juste avant l'encode et rendue à la sortie du bloc : le plafond global ne
        // couvre que `execute_file`, pas la lecture de `master.db` ni le commit qui suivent. Sans
        // elle, N clics sur N pistes lançaient N ffmpeg concurrents (cf. `ENCODE_SLOTS`).
        let _slot = EncodeSlot::acquire();
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| filing::execute_file(&plan)))
        {
            Ok(r) => r.map_err(|e| {
                log::error!("file_track: execute failed for track {track_id}: {e:?}");
                e.to_string()
            }),
            Err(payload) => {
                log::error!("file_track: execute panicked for track {track_id}: {payload:?}");
                Err("conversion interrompue (panic)".to_string())
            }
        }
    };

    // Le `master.db` se lit AVANT de reprendre le verrou (déchiffrement SQLCipher multi-Mo) —
    // `commit_file` le faisait verrou tenu. Une seule piste ici, donc un seul déchiffrement dans
    // les deux cas ; ce qui change est qu'il ne bloque plus tout le reste de l'app.
    let masterdb_index = {
        let path = match state.lock() {
            Ok(conn) => actions::masterdb_path_if_linked(&conn),
            Err(e) => {
                log::error!("file_track: DB lock poisoned resolving master.db path: {e}");
                None
            }
        };
        path.as_deref().and_then(actions::read_masterdb_index)
    };

    // Phase 3.
    let result = match executed {
        Ok(log) => match state.lock() {
            Ok(conn) => filing::commit_file(&conn, &plan, log, None, masterdb_index.as_ref())
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Err(e) => {
                log::error!("file_track: DB lock poisoned committing track {track_id}: {e}");
                Err("db lock poisoned".to_string())
            }
        },
        Err(e) => Err(e),
    };

    // Release the claim BEFORE announcing the outcome: the front may re-file this track the moment
    // it sees a failure, and that new plan must be free to take both the track and this name back.
    release_filing(track_id, &dest);

    let filed = result.is_ok();
    let outcome = match result {
        Ok(()) => TrackFileOutcome {
            track_id,
            batch_id,
            path: Some(dest),
            error: None,
        },
        Err(e) => TrackFileOutcome {
            track_id,
            batch_id,
            path: None,
            error: Some(e),
        },
    };
    app.emit("file:track:done", &outcome).ok();
    if filed {
        app.emit("queue:changed", ()).ok();
    }
}

/// File one track into `bin_rel`. `target` overrides the rail default (e.g. force MP3);
/// `edited` overrides the reconciled metadata with the user's corrections. `allow_rail_mismatch`
/// (FIX-1): when the source's declared extension claims lossless but its content is actually
/// lossy (BUG-1 — e.g. an MP3 renamed `.flac`), filing is refused with the `"RAIL_MISMATCH"`
/// sentinel unless this is explicitly `true` — the front shows a confirmation dialog and, if the
/// user proceeds, retries the same call with it set.
///
/// ASYNCHRONOUS since P5 (PRD 2026-07-27, D3/D5): only phase 1 — the plan — runs inside the invoke,
/// so the click is acknowledged in milliseconds instead of waiting out the ffmpeg encode. The
/// returned `FileResult` IS that acknowledgement: destination path and journal batch id are both
/// settled at plan time. Every refusal the front knows how to handle (`"NoLibraryRoot"`,
/// `"RAIL_MISMATCH"`, upscale, track not found) is still raised synchronously, before anything
/// starts — the confirm-and-retry dance in `filing-actions.ts` is unchanged, and one more refusal
/// joins them: `"ALREADY_FILING"` when this track's previous conversion is still running. The real
/// outcome arrives later on `file:track:done` (see `run_file_track`); a failure there leaves the
/// track `pending`, to be picked up again from the queue.
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
    // Read (and release) the reservation registry BEFORE taking the DB lock — never nested.
    let reserved = reserved_dests()?;
    // Phase 1 under the lock: decide the plan (fast DB reads + guard + dest).
    let plan = {
        let conn = db::lock_conn(&conn)?;
        let root = library_root_for(&conn, &bin_rel)?;
        let tmpl = template(&conn);
        filing::plan_file(
            &conn,
            root.as_deref(),
            &tmpl,
            track_id,
            &bin_rel,
            target,
            edited,
            allow_rail_mismatch.unwrap_or(false),
            &reserved,
        )
        .map_err(|e| e.to_string())?
    };
    let ack = FileResult {
        path: plan.dest_path().to_string(),
        batch_id: plan.batch_id().to_string(),
    };
    // Claim the track and its destination for as long as the conversion runs (see
    // `InFlightFilings`). This is also where a second filing of the SAME track is refused.
    reserve_filing(track_id, &ack.path)?;
    // Phases 2 and 3 detached. Fail-fast: if the thread can't even start, drop the claim and
    // surface it — the front then knows nothing was launched, instead of waiting on an event
    // that would never be emitted.
    let app_bg = app.clone();
    if let Err(e) = std::thread::Builder::new()
        .name("file-track".into())
        .spawn(move || run_file_track(&app_bg, plan))
    {
        release_filing(track_id, &ack.path);
        return Err(format!("file_track: failed to start background task: {e}"));
    }
    Ok(ack)
}

/// Launch filing of `track_ids` into `bin_rel` IN THE BACKGROUND and return immediately. The
/// actual work (per-file convert/tag/move + journal) runs on a dedicated thread via
/// `run_file_batch`, taking and releasing the DB lock PER FILE — so a long batch never freezes
/// the UI nor blocks the analysis worker (a sync command holding the lock across the whole batch
/// would do both). La racine est résolue synchroniquement — et seulement si `bin_rel` vise l'arbre
/// (#54) — pour qu'une racine manquante fasse échouer l'invoke tout de suite plutôt qu'en plein lot
/// (front routes to Settings via the `"NoLibraryRoot"` sentinel). When the run finishes
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
        let conn = db::lock_conn(&conn)?;
        (library_root_for(&conn, &bin_rel)?, template(&conn))
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

/// Bounded phase-2 worker count. FFmpeg already uses several internal threads per process, so we
/// deliberately UNDER-subscribe (half the cores) and cap at 4 — spawning one ffmpeg per core would
/// oversubscribe the CPU and thrash the disk without going faster. Min 1 (never zero).
pub(crate) fn phase2_worker_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| (n.get() / 2).max(1))
        .unwrap_or(2)
        .min(4)
}

/// Plafond GLOBAL d'encodes ffmpeg simultanés, partagé par le chemin interactif (`run_file_track`)
/// et le pool de phase 2 du batch (`run_file_batch`).
///
/// Pourquoi global : le batch se plafonnait déjà à `phase2_worker_count()`, mais `file_track` ne se
/// bornait que PAR PISTE (`ALREADY_FILING`) — rien n'empêchait N clics sur N pistes différentes de
/// lancer N ffmpeg concurrents, soit exactement l'oversubscription CPU/disque que le pool du batch
/// existe pour éviter. La borne était posée d'un côté et absente de l'autre.
///
/// Un jeton est tenu le temps d'UN `execute_file`, jamais le temps d'un batch : un clic pendant un
/// batch attend au pire la fin de l'encode en cours, pas la fin du batch.
static ENCODE_SLOTS: OnceLock<(Mutex<usize>, Condvar)> = OnceLock::new();

fn encode_slots() -> &'static (Mutex<usize>, Condvar) {
    ENCODE_SLOTS.get_or_init(|| (Mutex::new(phase2_worker_count()), Condvar::new()))
}

/// Jeton RAII d'une place d'encodage. La place revient au `Drop`, y compris si `execute_file`
/// panique — c'est ce qui empêche une panique de retirer définitivement une place du pool, mode de
/// dégradation silencieuse déjà rencontré sur `worker.rs`.
struct EncodeSlot;

impl EncodeSlot {
    /// Bloque jusqu'à ce qu'une place se libère.
    ///
    /// Sur mutex empoisonné on reprend le compteur au lieu de propager : une panique pendant la
    /// courte section critique ne corrompt pas un simple `usize`, alors qu'abandonner ici
    /// bloquerait tout classement pour le reste de la session. L'incident est loggé, jamais avalé
    /// en silence.
    fn acquire() -> Self {
        let (lock, cv) = encode_slots();
        let mut free = lock.lock().unwrap_or_else(|p| {
            log::error!("encode slots: mutex empoisonné à l'acquisition, compteur repris tel quel");
            p.into_inner()
        });
        while *free == 0 {
            free = cv.wait(free).unwrap_or_else(|p| {
                log::error!("encode slots: mutex empoisonné en attente, compteur repris tel quel");
                p.into_inner()
            });
        }
        *free -= 1;
        EncodeSlot
    }
}

impl Drop for EncodeSlot {
    fn drop(&mut self) {
        let (lock, cv) = encode_slots();
        let mut free = lock.lock().unwrap_or_else(|p| p.into_inner());
        *free += 1;
        cv.notify_one();
    }
}

/// A planned track ready for phase 2 (the concurrent encode). Carries its position in the original
/// batch order so phase 3 can commit and report in a stable order regardless of encode finish order.
struct PlannedJob {
    idx: usize,
    id: i64,
    plan: filing::FilePlan,
}

/// The outcome of one job's phase 2, sent back to the dispatcher for the serial phase 3.
struct Phase2Outcome {
    idx: usize,
    id: i64,
    plan: filing::FilePlan,
    /// `Some(log)` = encode+moves succeeded, ready to commit. `None` = execute_file failed (ou a
    /// paniqué) → la piste part en needs_validation.
    ///
    /// L'invariant « le système de fichiers est laissé propre » est réel depuis le 2026-07-28
    /// seulement (audit CR-3). Il était affirmé ici alors qu'une branche l'enfreignait : sur le
    /// chemin conformant, un échec du `move` sortait en laissant les NOUVEAUX tags déjà écrits en
    /// place sur le fichier source, sans ligne de journal — donc sans revert possible. `rollback_fs`
    /// est désormais appelé sur cette branche. Ne pas réaffirmer cet invariant ailleurs sans
    /// vérifier qu'aucune sortie d'erreur de `execute_file` ne l'a enfreint.
    log: Option<Vec<filing::FsLog>>,
    error: Option<String>,
}

/// Background body of `file_batch` (off the main thread). Three stages:
///  - **Phase 1 (serial, DB lock per file)**: pick the auto-file canonical + `plan_file`. Resolving
///    every destination serially, under the lock, BEFORE any file is written lets each plan reserve
///    its dest so two tracks reconciling to the same name never collide (see `plan_file`'s
///    `reserved` set). No fileable name / plan error → needs_validation.
///  - **Phase 2 (CONCURRENT, NO lock)**: a bounded pool of `phase2_worker_count()` std threads runs
///    the slow ffmpeg encode + fs moves (`execute_file`) in parallel. No DB access here.
///  - **Phase 3 (serial, DB lock per file)**: `commit_file` (journal + mark filed) for each
///    successfully-encoded job, IN ORIGINAL BATCH ORDER, emitting one progress tick per file.
///
/// Cancellation (`FilingCancel`): checked (1) in phase 1 — a cancelled batch stops PLANNING new
/// tracks, and (2) by each worker before it pulls a new job — an in-flight encode finishes cleanly,
/// but no not-yet-started job begins. Cancelled/unstarted planned jobs are reported in
/// needs_validation (they were never filed). A track with no auto-file name is untouched, exactly
/// as before — only the execution shape (serial → pooled phase 2) changed.
fn run_file_batch(
    app: &AppHandle,
    // `None` quand la destination du lot n'a pas besoin de racine (en place, dossier externe) —
    // résolu une fois par `library_root_for` avant le détachement. Voir `plan_file`.
    root: Option<PathBuf>,
    tmpl: String,
    track_ids: Vec<i64>,
    bin_rel: String,
    targets: Option<HashMap<i64, Target>>,
) {
    use std::sync::mpsc;

    let state = app.state::<Mutex<Connection>>();
    let cancel = app.state::<FilingCancel>();
    let total = track_ids.len();
    let mut needs_validation = Vec::new();
    let mut cancelled = false;

    // ---- Phase 1 (serial, under the lock): plan every fileable track, reserving each dest. ----
    let mut jobs: Vec<PlannedJob> = Vec::new();
    // Seeded with the destinations an interactive `file_track` is currently converting (P5): those
    // files are not on disk yet either, so a plain empty set would let this batch plan straight
    // onto one of them. A poisoned registry is logged and the batch continues with what it can
    // reserve on its own — bailing out of an already-launched batch would be worse.
    let mut reserved: HashSet<String> = reserved_dests().unwrap_or_default();
    // The claims THIS batch publishes into the shared registry, released in one place once phase 3
    // is over (see the end of this function). Kept as a side list rather than a field on
    // `PlannedJob` so a job cancelled before any worker popped it is released too, and so a claim
    // that could not be taken is never released on someone else's behalf.
    let mut claims: Vec<(i64, String)> = Vec::new();
    for (idx, id) in track_ids.iter().copied().enumerate() {
        // Cancel: stop planning new tracks. Ones not yet planned are simply never started.
        if cancel.0.load(Ordering::SeqCst) {
            cancelled = true;
            break;
        }
        // Same invariant as the interactive path: a track whose conversion is already running must
        // not be filed a SECOND time from the same source. Checked before planning so the batch
        // doesn't pay for a plan it would have to throw away. `Err` (poisoned registry) is treated
        // as "not in flight": the batch keeps running, and the claim below still refuses the real
        // collision — bailing out of an already-launched batch would be worse.
        if is_filing_inflight(id).unwrap_or(false) {
            log::warn!("file_batch: piste {id} déjà en cours de rangement, mise en validation");
            needs_validation.push(id);
            continue;
        }
        let conn = match state.lock() {
            Ok(c) => c,
            Err(e) => {
                log::error!("file_batch: DB lock poisoned planning file {id}: {e}");
                cancelled = true;
                break;
            }
        };
        let plan = match filing::batch_canonical(&conn, id) {
            Some(c) => match filing::plan_file(
                &conn,
                root.as_deref(),
                &tmpl,
                id,
                &bin_rel,
                targets.as_ref().and_then(|m| m.get(&id)).copied(),
                Some(c),
                // Batch never force-confirms a rail mismatch on the user's behalf — a track with a
                // disguised source lands in needs_validation like any other filing error.
                false,
                &reserved,
            ) {
                Ok(p) => p,
                // Sans cette trace, un lot qui rebondit N pistes n'affiche qu'un compte : la
                // variante de `FilingError` est la SEULE information qui dit pourquoi, et elle
                // était jetée ici.
                Err(e) => {
                    log::error!("file_batch: plan_file a échoué pour la piste {id}: {e:?}");
                    needs_validation.push(id);
                    continue;
                }
            },
            None => {
                log::warn!("file_batch: piste {id} sans identité canonique, mise en validation");
                needs_validation.push(id);
                continue;
            }
        };
        drop(conn);
        // Publish the claim in the SHARED registry too, not just in the local `reserved` set: an
        // interactive `file_track` launched while this batch runs reads that registry (and only
        // that one) to avoid planning onto a destination whose file isn't written yet, and to
        // refuse a track already being converted here.
        match reserve_filing(id, plan.dest_path()) {
            Ok(()) => claims.push((id, plan.dest_path().to_string())),
            Err(e) => {
                // Lost the race against a filing started between the check above and here (either
                // on this track or on this exact destination) — bounce it like any other
                // planning-time refusal rather than encode onto a contested path.
                log::error!("file_batch: could not claim track {id}: {e}");
                needs_validation.push(id);
                continue;
            }
        }
        // Reserve this dest so a later plan for a same-named track bumps past it (the file isn't
        // written until the concurrent phase 2 below).
        reserved.insert(plan.dest_path().to_string());
        jobs.push(PlannedJob { idx, id, plan });
    }

    // ---- Phase 2 (concurrent, NO lock): pooled ffmpeg encode + fs moves. ----
    // A shared job queue (Mutex<vec-as-stack drained via pop>) feeds N workers; each worker checks
    // the cancel flag before taking a job so an in-flight encode finishes but no new one starts.
    let (result_tx, result_rx) = mpsc::channel::<Phase2Outcome>();
    let dispatched = jobs.len();
    let queue = std::sync::Arc::new(Mutex::new(jobs));
    let worker_n = phase2_worker_count().min(dispatched.max(1));
    let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    let mut handles = Vec::new();
    for _ in 0..worker_n {
        let queue = std::sync::Arc::clone(&queue);
        let tx = result_tx.clone();
        let wcancel = std::sync::Arc::clone(&cancel_flag);
        handles.push(std::thread::spawn(move || {
            loop {
                // Cancel between jobs: an in-flight execute_file finishes, no new job is pulled.
                if wcancel.load(Ordering::SeqCst) {
                    break;
                }
                let job = {
                    match queue.lock() {
                        Ok(mut q) => q.pop(),
                        Err(e) => {
                            // Sortir en silence ici rendait un worker aveugle sans une trace:
                            // même défaut que celui corrigé dans worker.rs (.claude/rules/rust.md,
                            // « Mutex empoisonne : logger avant de bailer, jamais un retour muet »).
                            log::error!("file_batch: job queue poisoned, worker stops: {e}");
                            break;
                        }
                    }
                };
                let Some(job) = job else { break };
                // catch_unwind, comme `run_file_track` (phase 2 du chemin UNITAIRE, plus haut dans
                // ce fichier) qui l'a depuis un audit précédent. `execute_file` décode et réencode
                // un fichier utilisateur arbitraire via le sidecar ffmpeg puis écrit des tags avec
                // lofty: surface d'entrée non maîtrisée, sur un thread que personne ne join.
                //
                // Sans cette garde, un panic tuait le worker AVANT son `tx.send`. La piste
                // n'apparaissait alors NI dans `filed` NI dans `needs_validation` du BatchResult
                // final: elle n'était pas dans `outcomes` (aucun envoi) et pas non plus dans le
                // rattrapage de fin de fonction, qui ne reprend que les jobs jamais depiles — or
                // celui-ci l'avait été. L'écran la peignait « fait ». Audit 2026-07-28, CC-2.
                // Même plafond global que le chemin unitaire (cf. `ENCODE_SLOTS`). Le pool est déjà
                // borné à `worker_n`, mais il doit décompter des MÊMES places que `file_track`,
                // sinon batch et clics s'additionnent au lieu de partager la borne.
                let executed = {
                    let _slot = EncodeSlot::acquire();
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        filing::execute_file(&job.plan)
                    }))
                };
                let (log, error) = match executed {
                    Ok(Ok(log)) => (Some(log), None),
                    Ok(Err(e)) => {
                        let msg = format!("{e:?}");
                        log::error!("file_batch: execute failed for track {}: {msg}", job.id);
                        (None, Some(msg))
                    }
                    Err(payload) => {
                        let msg = format!("{payload:?}");
                        log::error!("file_batch: execute panicked for track {}: {msg}", job.id);
                        (None, Some(msg))
                    }
                };
                if tx
                    .send(Phase2Outcome {
                        idx: job.idx,
                        id: job.id,
                        plan: job.plan,
                        log,
                        error,
                    })
                    .is_err()
                {
                    break; // dispatcher gone — stop
                }
            }
        }));
    }
    drop(result_tx); // so result_rx closes once every worker has dropped its clone

    // Poll the cancel flag while collecting: propagate a user cancel to the workers so they stop
    // pulling new jobs. Collect outcomes as they finish (any order), then commit in batch order.
    let mut outcomes: Vec<Phase2Outcome> = Vec::with_capacity(dispatched);
    loop {
        if cancel.0.load(Ordering::SeqCst) {
            cancel_flag.store(true, Ordering::SeqCst);
            cancelled = true;
        }
        match result_rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(o) => outcomes.push(o),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    for h in handles {
        let _ = h.join();
    }

    // Commit in original batch order so progress ticks and journal ids advance monotonically.
    outcomes.sort_by_key(|o| o.idx);

    // ---- Phase 3 (serial, DB lock per file): commit each encoded job, emit progress per file. ----
    let mut filed = 0usize;
    let mut filed_ids: Vec<i64> = Vec::new();
    let mut errors: Vec<filing::BatchError> = Vec::new();
    // Accumulates every (from, to) pair needing a linked-Rekordbox-XML repair across the WHOLE
    // batch, instead of each commit_file call doing its own read+parse+write of the same file
    // (audited 2026-07-05, finding P4 — up to 200 independent cycles on a 200-track batch).
    // Flushed once, after the loop, via actions::repair_rekordbox_xml_batch.
    let mut xml_repair_pairs: Vec<(String, String)> = Vec::new();
    // Même raisonnement que `xml_repair_pairs`, pour le `master.db` : `commit_file` le résolvait
    // lui-même, donc une fois par piste ET sous le verrou global. Sur un lot de 200 pistes c'était
    // 200 déchiffrements SQLCipher multi-Mo du même fichier, verrou tenu, pendant que le reste de
    // l'app attendait. Résolu ici : le chemin sous un verrou court (lecture de réglage), la lecture
    // du fichier VERROU RELÂCHÉ, une seule fois pour tout le lot.
    let masterdb_index = {
        let path = match state.lock() {
            Ok(conn) => actions::masterdb_path_if_linked(&conn),
            Err(e) => {
                log::error!("file_batch: DB lock poisoned resolving master.db path: {e}");
                None
            }
        };
        path.as_deref().and_then(actions::read_masterdb_index)
    };
    // `done` = every track whose fate is settled: planning-time needs_validation + each processed
    // outcome. Emitted before the loop (settles the planning-time bounces) and after each commit.
    app.emit(
        "file:progress",
        &FileProgress {
            done: needs_validation.len(),
            total,
        },
    )
    .ok();
    for o in outcomes {
        match o.log {
            Some(log) => {
                let conn = match state.lock() {
                    Ok(c) => c,
                    Err(e) => {
                        let msg = format!("DB lock poisoned: {e}");
                        log::error!("file_batch: DB lock poisoned committing file {}: {e}", o.id);
                        needs_validation.push(o.id);
                        errors.push(filing::BatchError {
                            track_id: o.id,
                            message: msg,
                        });
                        app.emit(
                            "file:progress",
                            &FileProgress {
                                done: filed + needs_validation.len(),
                                total,
                            },
                        )
                        .ok();
                        continue;
                    }
                };
                match filing::commit_file(
                    &conn,
                    &o.plan,
                    log,
                    Some(&mut xml_repair_pairs),
                    masterdb_index.as_ref(),
                ) {
                    Ok(_) => {
                        filed += 1;
                        filed_ids.push(o.id);
                    }
                    Err(e) => {
                        needs_validation.push(o.id);
                        errors.push(filing::BatchError {
                            track_id: o.id,
                            message: e.to_string(),
                        });
                    }
                }
            }
            None => {
                needs_validation.push(o.id);
                if let Some(msg) = o.error {
                    errors.push(filing::BatchError {
                        track_id: o.id,
                        message: msg,
                    });
                }
            }
        }
        app.emit(
            "file:progress",
            &FileProgress {
                done: filed + needs_validation.len(),
                total,
            },
        )
        .ok();
    }

    if !xml_repair_pairs.is_empty() {
        if let Ok(conn) = state.lock() {
            actions::repair_rekordbox_xml_batch(&conn, &xml_repair_pairs);
        }
    }

    // Planned-but-never-started jobs (cancelled before any worker popped them) remain in `queue`:
    // they were never encoded → report as unfiled.
    if let Ok(q) = queue.lock() {
        for job in q.iter() {
            needs_validation.push(job.id);
        }
    }

    // Every claim this batch published, dropped in one place — success, failure and cancellation
    // alike. Done only now: until phase 3 has committed, the destination files are the ones this
    // batch is still writing, and an interactive filing must not be allowed to plan onto them.
    for (id, dest) in claims {
        release_filing(id, &dest);
    }

    app.emit(
        "file:progress",
        &FileProgress {
            done: filed + needs_validation.len(),
            total,
        },
    )
    .ok();
    app.emit(
        "file:done",
        &BatchResult {
            filed,
            needs_validation,
            cancelled,
            filed_ids,
            errors,
        },
    )
    .ok();
}

/// Mark a track for re-sourcing (Écartés). Status-only at this milestone.
#[tauri::command]
pub fn reject_track(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    track_id: i64,
) -> Result<(), String> {
    {
        let conn = db::lock_conn(&conn)?;
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
        let conn = db::lock_conn(&conn)?;
        filing::reject_batch(&conn, &track_ids)
    };
    app.emit("queue:changed", ()).ok();
    Ok(res)
}

/// Reveal a track's file in the OS file manager, with the file itself selected.
///
/// Takes a `track_id`, never a path: the path is resolved from the database, so the front can't
/// hand this command an arbitrary target. That matters because the Windows arm spawns a process —
/// with a caller-supplied path it would be a way to point Explorer anywhere on disk.
///
/// Missing file = an error, not a silent fallback to the parent directory. Opening the folder of a
/// file that isn't there would look like a success and tell the user nothing about why the track
/// can't be found, which is exactly the question they opened the folder to answer.
///
/// ⚠️ `explorer.exe` exits with a NON-ZERO status even when it succeeds — a documented quirk. So the
/// status is deliberately not inspected here; only a failure to spawn is reported. Reading the exit
/// code would turn every successful reveal into an error message.
#[tauri::command]
pub fn reveal_track(conn: State<'_, Mutex<Connection>>, track_id: i64) -> Result<(), String> {
    let path = {
        let conn = db::lock_conn(&conn)?;
        filing::track_path(&conn, track_id).map_err(|e| e.to_string())?
    };
    if !std::path::Path::new(&path).exists() {
        return Err(format!("fichier introuvable sur le disque : {path}"));
    }
    reveal_in_file_manager(&path)
}

/// The per-platform half of `reveal_track`. Three `#[cfg]` FUNCTIONS rather than three `#[cfg]`
/// blocks inside one body: only one of them is ever compiled, so a block form needs a `return` to
/// leave the function — and `-D clippy::needless_return` rejects that `return` precisely because,
/// on the compiled target, it IS the last expression.
#[cfg(windows)]
fn reveal_in_file_manager(path: &str) -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(format!("/select,{path}"))
        .spawn()
        .map_err(|e| format!("ouverture de l'explorateur impossible : {e}"))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn reveal_in_file_manager(path: &str) -> Result<(), String> {
    std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .spawn()
        .map_err(|e| format!("ouverture du Finder impossible : {e}"))?;
    Ok(())
}

/// Neither target Sift ships on. Reveal-with-selection has no portable equivalent, so the parent
/// directory is the honest best effort — and this arm is only reached on a platform we don't build.
#[cfg(not(any(windows, target_os = "macos")))]
fn reveal_in_file_manager(path: &str) -> Result<(), String> {
    let dir = std::path::Path::new(path)
        .parent()
        .ok_or_else(|| format!("aucun dossier parent pour {path}"))?;
    open::that(dir).map_err(|e| e.to_string())
}

/// Move a track's file to `.sift-trash` (reversible via undo) and mark it trashed. FIX-6: no
/// library-root precondition — the trash dir lives under Documents, not the library root.
#[tauri::command]
pub fn trash_track(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    track_id: i64,
) -> Result<(), String> {
    // Same plan/execute/commit split as `file_track`: the copy into the trash dir is a
    // byte-for-byte copy across disks (the trash lives under Documents, the library rarely does),
    // so it must not run under the global connection mutex.
    // (1) Source path under the lock.
    let source = {
        let conn = db::lock_conn(&conn)?;
        filing::track_path(&conn, track_id).map_err(|e| e.to_string())?
    };
    // (2) The copy + verify + delete, lock released. If another thread moved or filed this track
    // in the meantime the source is simply gone and this fails HERE — before anything is
    // journaled and before the status changes, so the DB is left exactly as it was.
    let dest = filing::trash_file_fs(track_id, &source).map_err(|e| e.to_string())?;
    // (3) Journal + status under the lock. The file is already in the trash at this point, which
    // was also true of the pre-split code (it did the FS move before journaling, under one lock).
    {
        let conn = db::lock_conn(&conn)?;
        filing::commit_trash(&conn, track_id, &source, &dest).map_err(|e| e.to_string())?;
    }
    app.emit("queue:changed", ()).ok();
    Ok(())
}

/// List all destination bins (recursive subdirs of the library root).
#[tauri::command]
pub fn list_bins(conn: State<'_, Mutex<Connection>>) -> Result<Vec<Bin>, String> {
    // Root under the lock; the RECURSIVE walk of the whole library tree happens after releasing
    // it. Nothing is written and nothing else is read from the DB, so there is no state to go
    // stale: a root reconfigured mid-call just means this listing reflects the root that was
    // configured when the command started — which is what any caller already gets.
    let root = {
        let conn = db::lock_conn(&conn)?;
        library_root(&conn)?
    };
    Ok(library::list_bins(&root))
}

/// Undo the most recent live batch (LIFO). Returns the reverted batch id, or null when there
/// is nothing to undo.
#[tauri::command]
pub fn undo_last(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
) -> Result<Option<String>, String> {
    let res = {
        let conn = db::lock_conn(&conn)?;
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
        let conn = db::lock_conn(&conn)?;
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
    let conn = db::lock_conn(&conn)?;
    // `map_err` et non `Ok(...)` depuis le 2026-08-17 : `actions::list_journal` rend un `Result`
    // (impasse A17, issue #15). Ce wrapper ne pouvait rien remonter tant que la fonction rendait
    // un `Vec`, et l'écran Journal peignait « Rien dans cette session » sur une base illisible.
    actions::list_journal(&conn, limit, session_id.as_deref()).map_err(|e| e.to_string())
}

/// The current app session ID (generated at launch, persisted in settings). Used by the
/// Journal tab front to filter list_journal to the current session only.
#[tauri::command]
pub fn get_session_id(conn: State<'_, Mutex<Connection>>) -> Result<String, String> {
    let conn = db::lock_conn(&conn)?;
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
    let conn = db::lock_conn(&conn)?;
    dedup::find_duplicate(&conn, track_id).map_err(|e| e.to_string())
}

/// List the rejected/trashed tracks for the Écartés view.
#[tauri::command]
pub fn list_ecartes(conn: State<'_, Mutex<Connection>>) -> Result<Vec<EcarteItem>, String> {
    let conn = db::lock_conn(&conn)?;
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
        let conn = db::lock_conn(&conn)?;
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
        let conn = db::lock_conn(&conn)?;
        ecartes::requeue_track(&conn, track_id)?;
    }
    app.emit("queue:changed", ()).ok();
    Ok(())
}

/// Permanently empty the bin (delete trashed files). Returns what was purged and what resisted.
#[tauri::command]
pub fn purge_trash(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
) -> Result<ecartes::PurgeResult, String> {
    let res = {
        let conn = db::lock_conn(&conn)?;
        ecartes::purge_trash(&conn)?
    };
    app.emit("queue:changed", ()).ok();
    Ok(res)
}

/// Read one app setting (null when unset).
#[tauri::command]
pub fn get_setting(
    conn: State<'_, Mutex<Connection>>,
    key: String,
) -> Result<Option<String>, String> {
    let conn = db::lock_conn(&conn)?;
    settings::get(&conn, &key).map_err(|e| e.to_string())
}

/// Write one app setting (e.g. the library root chosen in the settings panel).
#[tauri::command]
pub fn set_setting(
    conn: State<'_, Mutex<Connection>>,
    key: String,
    value: String,
) -> Result<(), String> {
    let conn = db::lock_conn(&conn)?;
    settings::set(&conn, &key, &value).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open_in_memory");
        crate::db::run_migrations(&conn).expect("run_migrations");
        conn
    }

    /// LE seam de production de #54 : c'est cette fonction, et elle seule, qui décide si une
    /// commande de rangement va lire la racine. Les trois cas qu'elle doit distinguer.
    ///
    /// Elle est testée ICI et pas seulement à travers `plan_file` parce que c'est elle qui court en
    /// prod AVANT le plan : une régression qui remettrait `library_root(&conn)?` inconditionnel
    /// ferait échouer l'invoke sans jamais atteindre `plan_file`, donc sans faire tomber un seul
    /// test de `filing.rs`.
    #[test]
    fn library_root_for_only_reads_the_root_for_tree_destinations() {
        let conn = mem_db();

        // (1) Sans racine réglée, une destination de l'arbre refuse — sentinelle inchangée.
        assert_eq!(
            library_root_for(&conn, "House/Deep"),
            Err("NoLibraryRoot".to_string())
        );

        // (2) Sans racine réglée, les deux destinations sans-racine ne la réclament PAS.
        assert_eq!(
            library_root_for(&conn, crate::filing::FILE_IN_PLACE),
            Ok(None)
        );
        assert_eq!(
            library_root_for(
                &conn,
                &format!("{}/tmp/ailleurs", crate::filing::EXTERNAL_DEST_PREFIX)
            ),
            Ok(None)
        );

        // (3) Avec une racine réglée, la destination d'arbre la reçoit telle quelle.
        settings::set(&conn, settings::LIBRARY_ROOT, "/music/dj").expect("seed library_root");
        assert_eq!(
            library_root_for(&conn, "House/Deep"),
            Ok(Some(PathBuf::from("/music/dj")))
        );
        // …et les sans-racine continuent de ne rien lire, racine ou pas.
        assert_eq!(
            library_root_for(&conn, crate::filing::FILE_IN_PLACE),
            Ok(None)
        );

        // (4) Une racine BLANCHE vaut absente (garde historique de `library_root`).
        settings::set(&conn, settings::LIBRARY_ROOT, "   ").expect("seed blank root");
        assert_eq!(
            library_root_for(&conn, ""),
            Err("NoLibraryRoot".to_string()),
            "la racine elle-même est un bac de l'arbre"
        );
    }

    /// Le plafond global tient sous charge : on lance quatre fois plus de prétendants qu'il n'y a
    /// de places et on mesure le pic de porteurs simultanés.
    ///
    /// La seconde assertion est le TÉMOIN de l'expérience : si le pic vaut 1 alors que le plafond
    /// est plus grand, aucune concurrence n'a réellement eu lieu et la première assertion passerait
    /// pour de mauvaises raisons — un test vert ne prouverait alors rien.
    #[test]
    fn encode_slots_hold_the_global_cap_under_contention() {
        use std::sync::atomic::AtomicUsize;
        use std::sync::Arc;

        let cap = phase2_worker_count();
        let live = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let handles: Vec<_> = (0..cap * 4)
            .map(|_| {
                let live = Arc::clone(&live);
                let peak = Arc::clone(&peak);
                std::thread::spawn(move || {
                    let _slot = EncodeSlot::acquire();
                    let now = live.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    // Assez long pour que les prétendants se chevauchent vraiment.
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    live.fetch_sub(1, Ordering::SeqCst);
                })
            })
            .collect();
        for h in handles {
            h.join().expect("un thread de test a paniqué");
        }

        let observed = peak.load(Ordering::SeqCst);
        assert!(
            observed <= cap,
            "pic de {observed} encodes simultanés pour un plafond de {cap}"
        );
        assert!(
            cap == 1 || observed > 1,
            "pic de {observed} : les threads ne se sont jamais chevauchés, le test ne prouve rien"
        );
    }

    #[test]
    fn metadata_sync_values_for_apply_tags_maps_fields_and_joins_genres() {
        let edited = crate::naming::Canonical {
            artist: "Larry Heard".to_string(),
            title: "Mystery of Love".to_string(),
            version: None,
            label: Some("Alleviated Records".to_string()),
            confidence: crate::naming::Confidence::Green,
        };
        let extras = filing::TagExtras {
            label: Some("Alleviated".to_string()),
            year: Some(1985),
            genres: vec!["House".to_string(), "Deep House".to_string()],
            cover_path: None,
        };

        // The sync candidate records the label ACTUALLY written (the resolved one passed in), not the
        // raw stored `extras.label` — pass the edited value to prove the param, not extras, decides.
        let values =
            metadata_sync_values_for_apply_tags(&edited, &extras, Some("Alleviated Records"));

        assert_eq!(values.artist.as_deref(), Some("Larry Heard"));
        assert_eq!(values.title.as_deref(), Some("Mystery of Love"));
        assert_eq!(values.label.as_deref(), Some("Alleviated Records"));
        assert_eq!(values.year, Some(1985));
        assert_eq!(values.genre.as_deref(), Some("House; Deep House"));
    }

    #[test]
    fn metadata_sync_values_for_apply_tags_empty_genres_is_none() {
        let edited = crate::naming::Canonical {
            artist: "A".to_string(),
            title: "B".to_string(),
            version: None,
            label: None,
            confidence: crate::naming::Confidence::Green,
        };
        let extras = filing::TagExtras::default();
        let values = metadata_sync_values_for_apply_tags(&edited, &extras, None);
        assert_eq!(values.genre, None);
    }

    /// Mirrors shared/contracts.ts's `FileProgress`. Exhaustive destructure (no `..`): fails to
    /// compile if a field is added/removed/renamed on the Rust struct — the forcing function to
    /// also update contracts.ts. Phase 2 — docs/superpowers/plans/2026-07-13-phase2-ipc-contract-tests.md.
    #[test]
    fn file_progress_shape_matches_contracts_ts() {
        let v = FileProgress { done: 0, total: 0 };
        let FileProgress { done, total } = v;
        let _ = (done, total);
    }

    /// The P5 invariant, tested at its enforcement point rather than through the UI: while a
    /// track's conversion runs in the background it stays `pending`, so nothing but this registry
    /// stops a second `file_track` from encoding the same source again. Uses ids/paths of its own
    /// so it cannot collide with anything else running in parallel.
    #[test]
    fn reserve_filing_refuses_the_same_track_twice_until_released() {
        let (id, dest) = (-4242i64, "C:/nowhere/reserve-filing-test.aiff");
        assert_eq!(reserve_filing(id, dest), Ok(()));
        // Same track again while in flight → refused with the sentinel the front words.
        assert_eq!(reserve_filing(id, dest), Err(ALREADY_FILING.to_string()));
        // ...and its destination is visible to every other planner (interactive AND batch).
        assert!(reserved_dests().is_ok_and(|d| d.contains(dest)));
        release_filing(id, dest);
        assert!(reserved_dests().is_ok_and(|d| !d.contains(dest)));
        // Released → filable again (this is the retry path after a failed conversion).
        assert_eq!(reserve_filing(id, dest), Ok(()));
        release_filing(id, dest);
    }

    /// Two DIFFERENT tracks reconciling to the SAME destination: the second claim must be refused
    /// (not silently accepted, which is how two encodes end up writing the same path) and must not
    /// leave its track behind in the registry. Ids/dest are unique to this test — the registry is a
    /// process-wide static and tests run in parallel.
    #[test]
    fn reserve_filing_refuses_a_destination_another_filing_already_claimed() {
        let (a, b) = (-8801i64, -8802i64);
        let dest = "C:/nowhere/reserve-filing-dest-collision.aiff";
        assert_eq!(reserve_filing(a, dest), Ok(()));
        assert!(is_filing_inflight(a).is_ok_and(|v| v));
        let err = reserve_filing(b, dest).expect_err("same dest must be refused");
        assert_ne!(err, ALREADY_FILING.to_string(), "b is a different track");
        // The refused track was NOT left claimed — it can be filed elsewhere right away.
        assert!(is_filing_inflight(b).is_ok_and(|v| !v));
        release_filing(a, dest);
        assert!(is_filing_inflight(a).is_ok_and(|v| !v));
    }

    /// Mirrors shared/contracts.ts's `TrackFileOutcome` (the `file:track:done` payload). Exhaustive
    /// destructure (no `..`): fails to compile if a field is added/removed/renamed on the Rust
    /// struct — the forcing function to also update contracts.ts, since the front reads this
    /// payload to settle a filing it has already acknowledged (P5).
    #[test]
    fn track_file_outcome_shape_matches_contracts_ts() {
        let v = TrackFileOutcome {
            track_id: 1,
            batch_id: String::new(),
            path: None,
            error: None,
        };
        let TrackFileOutcome {
            track_id,
            batch_id,
            path,
            error,
        } = v;
        let _ = (track_id, batch_id, path, error);
    }

    /// Mirrors shared/contracts.ts's `TrackRelease`. Exhaustive destructure (no `..`): fails to
    /// compile if a field is added/removed/renamed on the Rust struct — the forcing function to
    /// also update contracts.ts. Phase 2 — docs/superpowers/plans/2026-07-13-phase2-ipc-contract-tests.md.
    #[test]
    fn track_release_shape_matches_contracts_ts() {
        let v = TrackRelease {
            artist: None,
            title: None,
            version: None,
            label: None,
            year: None,
            cover_path: None,
            genres: Vec::new(),
            identified: false,
        };
        let TrackRelease {
            artist,
            title,
            version,
            label,
            year,
            cover_path,
            genres,
            identified,
        } = v;
        let _ = (
            artist, title, version, label, year, cover_path, genres, identified,
        );
    }
}
