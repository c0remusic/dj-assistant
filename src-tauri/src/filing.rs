//! Turn a reviewed track into a filed library file: ① convert (only if not conformant)
//! → ② tag + name → ③ move into the chosen bin, recording every step as one undoable
//! batch (see actions.rs). Mono-location: conformant files are moved; converted files
//! land in the bin and the original goes to `.sift-trash` (restorable via undo). Composes
//! naming/encode/tagging/library/actions/settings.

use crate::encode::{self, EncodeError, Target};
use crate::naming::{self, Canonical};
use crate::{actions, library, tagging};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Sentinel destination meaning "file in place": the track's destination is its OWN source
/// folder, not a bin under the library root. Travels through `bin_rel` like any other
/// destination (the single decision channel) — `plan_file` resolves it instead of `safe_join`.
/// The frontend mirrors this exact literal (`shared/contracts.ts` `FILE_IN_PLACE`); keep them
/// in sync. Must never reach `library::safe_join` (it would create a literal `__SOURCE__` dir).
pub const FILE_IN_PLACE: &str = "__SOURCE__";

/// Why filing could not complete (nothing is left half-filed on these — see ordering).
#[derive(Debug, Clone, PartialEq)]
pub enum FilingError {
    NotFound,
    Upscale,
    Encode(String),
    Tag(String),
    Io(String),
    Db(String),
}

impl std::fmt::Display for FilingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FilingError::NotFound => write!(f, "track not found"),
            FilingError::Upscale => write!(f, "refused: cannot upscale lossy to lossless"),
            FilingError::Encode(m) => write!(f, "encode: {m}"),
            FilingError::Tag(m) => write!(f, "tag: {m}"),
            FilingError::Io(m) => write!(f, "io: {m}"),
            FilingError::Db(m) => write!(f, "db: {m}"),
        }
    }
}

impl From<rusqlite::Error> for FilingError {
    fn from(e: rusqlite::Error) -> Self {
        FilingError::Db(e.to_string())
    }
}

/// Result of filing one track.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FileResult {
    pub path: String,
    pub batch_id: String,
}

/// Outcome of a batch filing.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BatchResult {
    pub filed: usize,
    pub needs_validation: Vec<i64>,
    /// True when the run was stop-net cancelled before processing every id (the summary is then
    /// partial: what was filed before the stop stays filed — nothing is rolled back).
    pub cancelled: bool,
}

/// Outcome of a batch reject (re-sourcing): how many were marked, and which ids failed — so the
/// UI can flag a misfire instead of silently dropping it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RejectBatchResult {
    pub rejected: usize,
    pub failed: Vec<i64>,
}

/// Source path of a track by id.
fn track_path(conn: &Connection, track_id: i64) -> Result<String, FilingError> {
    conn.query_row("SELECT path FROM tracks WHERE id=?1", params![track_id], |r| r.get(0))
        .map_err(|_| FilingError::NotFound)
}

/// Lowercased extension (no dot) of a path.
fn ext_of(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Filename (with extension) component of a path.
fn file_name_of(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string()
}

/// A unique batch id: track id + millis + a process-monotonic counter, so two filings of the
/// same track within the same millisecond (file → undo → re-file) can never share a batch_id.
/// Shared with `apply_tags` so a tag-edit batch gets the same collision-free id scheme.
pub(crate) fn new_batch_id(track_id: i64) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{track_id}-{ms}-{seq}")
}

/// Reconcile a track's canonical metadata from its embedded tags + filename. Used to pick
/// the green/yellow confidence and to seed the editable fields.
pub fn reconcile_track(conn: &Connection, track_id: i64) -> Result<Canonical, FilingError> {
    let path = track_path(conn, track_id)?;
    let (artist, title) = tagging::read_artist_title(&path);
    let stem = Path::new(&path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    Ok(naming::reconcile(&artist, &title, &stem))
}

/// FS-only: move `source` into `<root>/.sift-trash/<track_id>__<name>` (collision-free).
/// Returns the trash path. No DB — journaling is the caller's job.
fn trash_file_fs(root: &Path, track_id: i64, source: &str) -> Result<String, FilingError> {
    let trash_dir = root.join(".sift-trash");
    std::fs::create_dir_all(&trash_dir).map_err(|e| FilingError::Io(e.to_string()))?;
    let dest = library::ensure_unique(&trash_dir.join(format!("{track_id}__{}", file_name_of(source))), None);
    std::fs::rename(source, &dest).map_err(|e| FilingError::Io(e.to_string()))?;
    Ok(dest.to_string_lossy().to_string())
}

/// Trash a file AND journal it (used by `trash_track`, which is fast — no encode, so holding
/// the DB lock across it is fine).
fn move_to_trash(
    conn: &Connection,
    root: &Path,
    track_id: i64,
    batch_id: &str,
    source: &str,
) -> Result<String, FilingError> {
    let dest = trash_file_fs(root, track_id, source)?;
    actions::record(conn, batch_id, Some(track_id), "trash", Some(source), Some(&dest))
        .map_err(|e| FilingError::Db(e.to_string()))?;
    Ok(dest)
}

/// Persist canonical metadata for a track (upsert into `metadata`).
fn save_metadata(conn: &Connection, track_id: i64, c: &Canonical) -> Result<(), FilingError> {
    conn.execute(
        "INSERT INTO metadata(track_id, artist, title, version) VALUES(?1,?2,?3,?4)
         ON CONFLICT(track_id) DO UPDATE SET artist=excluded.artist, title=excluded.title, version=excluded.version",
        params![track_id, c.artist, c.title, c.version],
    )?;
    Ok(())
}

/// Enrichment tag fields loaded once (under the lock) so phase 2 writes them without DB access.
#[derive(Default, Clone)]
pub struct TagExtras {
    pub label: Option<String>,
    pub year: Option<i64>,
    pub genres: Vec<String>,
    pub cover_path: Option<String>,
}

/// Load the enrichment tag fields (label, year, genres, cover) for a track from the DB. The single
/// source of these values for tag writes — used by both `plan_file` (filing) and `apply_tags` (the
/// in-place ID3 write), so the two write the SAME label/year/genres/cover a track carries.
pub fn load_tag_extras(conn: &Connection, track_id: i64) -> TagExtras {
    TagExtras {
        label: conn
            .query_row("SELECT label FROM metadata WHERE track_id=?1", params![track_id], |r| r.get::<_, Option<String>>(0))
            .ok().flatten(),
        year: conn
            .query_row("SELECT year FROM metadata WHERE track_id=?1", params![track_id], |r| r.get::<_, Option<i64>>(0))
            .ok().flatten(),
        genres: crate::genres::get_genres(conn, track_id).unwrap_or_default(),
        cover_path: conn
            .query_row("SELECT cover_path FROM metadata WHERE track_id=?1", params![track_id], |r| r.get::<_, Option<String>>(0))
            .ok().flatten(),
    }
}

/// A filing decided under the DB lock (phase 1), ready to run lock-free (phase 2) and then
/// be committed under the lock (phase 3). Holding the connection across the multi-second
/// ffmpeg encode would freeze every other DB user (analysis workers + all IPC); splitting
/// lets the slow encode run lock-free. See `ipc_filing::file_track`.
pub struct FilePlan {
    track_id: i64,
    batch_id: String,
    source: String,
    dest: String,
    conformant: bool,
    target: Target,
    canonical: Canonical,
    bin_rel: String,
    root: PathBuf,
    extras: TagExtras,
}

/// One filesystem effect performed in phase 2, to be journaled in phase 3. `meta` carries the
/// optional JSON payload of the journal's `meta` column — used by the conformant filing's `tag_edit`
/// row to stash the OLD tags (so a revert can restore them); `None` for the plain move/convert/trash.
pub struct FsLog {
    kind: &'static str,
    from: String,
    to: String,
    meta: Option<String>,
}

/// The string value persisted in `tracks.target_format`.
fn target_str(target: Target) -> &'static str {
    match target {
        Target::Mp3320 => "mp3_320",
        Target::Aiff1644 => "aiff_16_44",
        Target::Wav1644 => "wav_16_44",
    }
}

/// Phase 1 (under the DB lock): resolve metadata + the collision-free destination and apply
/// the no-upscale guard. No slow work — only fast DB reads and a `create_dir_all`.
pub fn plan_file(
    conn: &Connection,
    root: &Path,
    template: &str,
    track_id: i64,
    bin_rel: &str,
    override_target: Option<Target>,
    edited: Option<Canonical>,
) -> Result<FilePlan, FilingError> {
    let source = track_path(conn, track_id)?;
    let canonical = match edited {
        Some(c) => c,
        None => reconcile_track(conn, track_id)?,
    };

    let source_rail = crate::analysis::tags::rail_from_ext(&ext_of(&source));
    let target = override_target.unwrap_or_else(|| encode::target_for(source_rail));
    if encode::guard_no_upscale(source_rail, target).is_err() {
        return Err(FilingError::Upscale);
    }

    // A conformant file is MOVED as-is (no transcode), so its container is unchanged — keep its own
    // extension instead of forcing target.ext(). This stops a `.aif` source from being renamed to
    // `.aiff`: with a single possible output name, a blocked revert (external lock, os error 32 —
    // proved in the revert-duplicate relevé) can no longer strand a `.aif` beside a `.aiff`. The
    // conversion path produces a genuinely new file, which keeps the canonical target extension.
    let conformant = encode::is_conformant(&source, target);
    let out_ext = if conformant { ext_of(&source) } else { target.ext().to_string() };

    // The single point where the destination directory is decided. The `FILE_IN_PLACE` sentinel
    // means "file into the track's own source folder" — resolve it to `source.parent()` and NEVER
    // route it through `safe_join` (which would sanitize it into a literal `root/__SOURCE__` dir).
    let dest_dir = if bin_rel == FILE_IN_PLACE {
        Path::new(&source)
            .parent()
            .ok_or_else(|| FilingError::Io("source file has no parent directory".into()))?
            .to_path_buf()
    } else {
        library::safe_join(root, bin_rel).map_err(FilingError::Io)?
    };
    std::fs::create_dir_all(&dest_dir).map_err(|e| FilingError::Io(e.to_string()))?;
    let filename = naming::render_filename(template, &canonical, &out_ext);
    // Ignore the source itself as a collision ONLY for the conformant (move) path: filing a
    // conformant track in place onto its own (already-correct) name must keep that name, not bump
    // it to " (2)". The non-conformant path ENCODES source → dest, so dest must never equal source
    // (FFmpeg reading and writing the same file would corrupt it) — keep the normal collision bump.
    let ignore_self = if conformant { Some(Path::new(&source)) } else { None };
    let dest = library::ensure_unique(&dest_dir.join(&filename), ignore_self);

    let extras = load_tag_extras(conn, track_id);

    Ok(FilePlan {
        conformant,
        source,
        dest: dest.to_string_lossy().to_string(),
        target,
        canonical,
        bin_rel: bin_rel.to_string(),
        root: root.to_path_buf(),
        batch_id: new_batch_id(track_id),
        track_id,
        extras,
    })
}

/// Phase 2 (NO DB lock): the slow work — tag + move, or encode + tag + trash. Leaves the
/// filesystem clean on its own failure (no orphan transcode). Returns the effects to journal.
pub fn execute_file(plan: &FilePlan) -> Result<Vec<FsLog>, FilingError> {
    let mut log = Vec::new();
    if plan.conformant {
        // A conformant filing tags the file IN PLACE then MOVES it — no trashed original to restore
        // from. So capture the OLD tags FIRST (fail clear if unreadable — never file without the net),
        // and journal them as a `tag_edit` row BEFORE the `move`. revert_batch undoes newest-first, so
        // it reverses the move (file back at `source`) THEN restores the old tags at `source` — the
        // exact path the tag_edit row points at. Reuses the B4 snapshot/restore mechanism verbatim.
        let old_tags = tagging::read_tags_full(&plan.source).map_err(FilingError::Tag)?;
        let snapshot = serde_json::to_string(&old_tags)
            .map_err(|e| FilingError::Tag(format!("serialize tag snapshot: {e}")))?;
        log.push(FsLog { kind: "tag_edit", from: plan.source.clone(), to: plan.source.clone(), meta: Some(snapshot) });
        tagging::write_tags_full(
            &plan.source, &plan.canonical.artist, &plan.canonical.title,
            plan.extras.label.as_deref(), plan.extras.year, &plan.extras.genres,
            plan.extras.cover_path.as_deref(),
        ).map_err(FilingError::Tag)?;
        std::fs::rename(&plan.source, &plan.dest).map_err(|e| FilingError::Io(e.to_string()))?;
        log.push(FsLog { kind: "move", from: plan.source.clone(), to: plan.dest.clone(), meta: None });
    } else {
        // transcode into the bin, tag the result, then trash the original (mono-location)
        encode::encode(&plan.source, &plan.dest, plan.target).map_err(|e| match e {
            EncodeError::Upscale => FilingError::Upscale,
            EncodeError::Ffmpeg(m) => FilingError::Encode(m),
        })?;
        if let Err(e) = tagging::write_tags_full(
            &plan.dest, &plan.canonical.artist, &plan.canonical.title,
            plan.extras.label.as_deref(), plan.extras.year, &plan.extras.genres,
            plan.extras.cover_path.as_deref(),
        ) {
            let _ = std::fs::remove_file(&plan.dest); // drop the orphan transcode
            return Err(FilingError::Tag(e));
        }
        log.push(FsLog { kind: "convert", from: plan.source.clone(), to: plan.dest.clone(), meta: None });
        match trash_file_fs(&plan.root, plan.track_id, &plan.source) {
            Ok(trash) => log.push(FsLog { kind: "trash", from: plan.source.clone(), to: trash, meta: None }),
            Err(e) => {
                let _ = std::fs::remove_file(&plan.dest);
                return Err(e);
            }
        }
    }
    Ok(log)
}

/// Reverse phase-2 filesystem effects (newest first) — used when phase 3 cannot commit.
fn rollback_fs(log: &[FsLog]) {
    for fs in log.iter().rev() {
        match fs.kind {
            "move" | "trash" => {
                let _ = std::fs::rename(&fs.to, &fs.from);
            }
            "convert" => {
                let _ = std::fs::remove_file(&fs.to);
            }
            // Conformant filing: undo the in-place tag write by restoring the captured old tags at
            // `from` (the file is back there — the move row, newer, was reversed just above). Reuses
            // the B4 restore; best-effort like the rest of this rollback (errors are swallowed).
            "tag_edit" => {
                if let Some(meta) = &fs.meta {
                    if let Ok(snap) = serde_json::from_str::<tagging::TagsSnapshot>(meta) {
                        let _ = tagging::restore_tags(&fs.from, &snap);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Phase 3 (under the DB lock): journal the effects + mark the track filed. On any DB error,
/// reverse the filesystem effects and the partial journal so nothing is left half-filed.
pub fn commit_file(conn: &Connection, plan: &FilePlan, log: Vec<FsLog>) -> Result<FileResult, FilingError> {
    let undo = |conn: &Connection| {
        rollback_fs(&log);
        let _ = conn.execute("DELETE FROM actions WHERE batch_id=?1", params![plan.batch_id]);
    };
    for fs in &log {
        if let Err(e) = actions::record_with_meta(
            conn, &plan.batch_id, Some(plan.track_id), fs.kind, Some(&fs.from), Some(&fs.to), fs.meta.as_deref(),
        ) {
            undo(conn);
            return Err(FilingError::Db(e.to_string()));
        }
    }
    let conf = match plan.canonical.confidence {
        naming::Confidence::Green => "green",
        naming::Confidence::Yellow => "yellow",
    };
    if let Err(e) = conn.execute(
        "UPDATE tracks SET status='filed', folder=?2, target_format=?3, confidence=?4 WHERE id=?1",
        params![plan.track_id, plan.bin_rel, target_str(plan.target), conf],
    ) {
        undo(conn);
        return Err(FilingError::Db(e.to_string()));
    }
    save_metadata(conn, plan.track_id, &plan.canonical)?;
    Ok(FileResult { path: plan.dest.clone(), batch_id: plan.batch_id.clone() })
}

/// File one track into `bin_rel` under `root`, holding `conn` throughout — a synchronous test
/// convenience that chains the three phases under a single lock. Production never holds the lock
/// across the encode: the interactive path (`ipc_filing::file_track`) and the detached batch
/// (`ipc_filing::run_file_batch`) run the phases with the lock released around it. See module docs
/// for the ordering and the mono-location / undo contract.
#[cfg(test)]
pub fn file_track(
    conn: &Connection,
    root: &Path,
    template: &str,
    track_id: i64,
    bin_rel: &str,
    override_target: Option<Target>,
    edited: Option<Canonical>,
) -> Result<FileResult, FilingError> {
    let plan = plan_file(conn, root, template, track_id, bin_rel, override_target, edited)?;
    let log = execute_file(&plan)?;
    commit_file(conn, &plan, log)
}

/// Canonical metadata persisted by an earlier Discogs identification (the `metadata` table),
/// if present and usable. A Discogs/manual match is a high-confidence name, so it's returned
/// Green — this is what lets a per-track identity applied in Review feed `file_batch` (whose
/// tag-based reconcile would otherwise ignore the applied identity). `None` = no usable row,
/// fall back to reconcile.
fn canonical_from_metadata(conn: &Connection, track_id: i64) -> rusqlite::Result<Option<Canonical>> {
    let row = conn.query_row(
        "SELECT artist, title FROM metadata WHERE track_id=?1",
        params![track_id],
        |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?)),
    );
    match row {
        Ok((Some(a), Some(t))) if !a.trim().is_empty() && !t.trim().is_empty() => Ok(Some(Canonical {
            artist: a,
            title: t,
            version: None,
            confidence: naming::Confidence::Green,
        })),
        Ok(_) => Ok(None),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Pick the canonical name to AUTO-file a batch track on, or `None` if it must stay pending for
/// manual review. A track identified via Discogs (persisted in `metadata`) files on that
/// high-confidence name; otherwise the tag/filename reconcile must come out Green. Pure DB read —
/// the detached batch loop (`ipc_filing::file_batch`) calls this under the per-file lock, before
/// planning the file, then runs the same plan/execute/commit phases as the interactive path.
pub fn batch_canonical(conn: &Connection, track_id: i64) -> Option<Canonical> {
    match canonical_from_metadata(conn, track_id) {
        Ok(Some(c)) => Some(c),
        Ok(None) => match reconcile_track(conn, track_id) {
            Ok(c) if c.confidence == naming::Confidence::Green => Some(c),
            _ => None,
        },
        Err(_) => None,
    }
}

/// Mark a track for re-sourcing (goes to Écartés, M4b): status `resourcing` + a `reject`
/// action. The file is not moved at this milestone.
pub fn reject_track(conn: &Connection, track_id: i64) -> Result<(), FilingError> {
    let source = track_path(conn, track_id)?;
    let batch_id = new_batch_id(track_id);
    actions::record(conn, &batch_id, Some(track_id), "reject", Some(&source), None)
        .map_err(|e| FilingError::Db(e.to_string()))?;
    conn.execute("UPDATE tracks SET status='resourcing' WHERE id=?1", params![track_id])?;
    Ok(())
}

/// Reject every track of `track_ids` for re-sourcing (each → Écartés, status-only like
/// `reject_track`). A track that errors is reported in `failed` rather than aborting the batch,
/// so one bad id never strands the rest — mirroring `file_batch`'s fail-soft, no-panic shape.
pub fn reject_batch(conn: &Connection, track_ids: &[i64]) -> RejectBatchResult {
    let mut rejected = 0usize;
    let mut failed = Vec::new();
    for &id in track_ids {
        match reject_track(conn, id) {
            Ok(()) => rejected += 1,
            Err(_) => failed.push(id),
        }
    }
    RejectBatchResult { rejected, failed }
}

/// Move a track's file to `.sift-trash` and mark it `trash` (reversible via undo).
pub fn trash_track(conn: &Connection, root: &Path, track_id: i64) -> Result<(), FilingError> {
    let source = track_path(conn, track_id)?;
    let batch_id = new_batch_id(track_id);
    move_to_trash(conn, root, track_id, &batch_id, &source)?;
    conn.execute("UPDATE tracks SET status='trash' WHERE id=?1", params![track_id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::run_migrations(&conn).unwrap();
        conn
    }

    fn fixture(name: &str) -> Option<String> {
        let p = format!("fixtures/{name}");
        if std::path::Path::new(&p).exists() {
            Some(p)
        } else {
            None
        }
    }

    /// Copy a fixture into `dir` and insert a pending track row pointing at the copy.
    fn seed_track(conn: &Connection, dir: &Path, fixture_name: &str, as_name: &str) -> Option<(i64, std::path::PathBuf)> {
        let src = fixture(fixture_name)?;
        let copy = dir.join(as_name);
        std::fs::copy(&src, &copy).unwrap();
        conn.execute(
            "INSERT INTO tracks(path, status) VALUES(?1, 'pending')",
            params![copy.to_str().unwrap()],
        )
        .unwrap();
        Some((conn.last_insert_rowid(), copy))
    }

    #[test]
    fn canonical_from_metadata_prefers_persisted_identity() {
        let conn = db();
        conn.execute("INSERT INTO tracks(id, path, status) VALUES(1,'/x.flac','pending')", [])
            .unwrap();
        // No metadata row → None (file_batch then falls back to the tag/filename reconcile).
        assert!(canonical_from_metadata(&conn, 1).unwrap().is_none());

        // A Discogs identity → a Green canonical on that name (what lets a per-track applied identity feed file_batch).
        conn.execute(
            "INSERT INTO metadata(track_id, artist, title, source) VALUES(1,'Larry Heard','Can You Feel It','discogs')",
            [],
        )
        .unwrap();
        let c = canonical_from_metadata(&conn, 1).unwrap().expect("metadata present");
        assert_eq!(c.artist, "Larry Heard");
        assert_eq!(c.title, "Can You Feel It");
        assert_eq!(c.confidence, crate::naming::Confidence::Green);

        // A blank-name row must be treated as absent (never file on an empty name).
        conn.execute("UPDATE metadata SET artist='', title='' WHERE track_id=1", [])
            .unwrap();
        assert!(canonical_from_metadata(&conn, 1).unwrap().is_none());
    }

    #[test]
    fn reconcile_track_reads_filename_when_tags_absent() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let Some((id, _)) = seed_track(&conn, dir.path(), "real_lossless.flac", "Robert Owens - Bring Down the Walls.flac") else {
            eprintln!("skip: no fixture");
            return;
        };
        let c = reconcile_track(&conn, id).unwrap();
        assert_eq!(c.artist, "Robert Owens");
        assert_eq!(c.title, "Bring Down the Walls");
    }

    #[test]
    fn files_conformant_mp3_by_moving() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(root.join("House")).unwrap();
        let Some((id, src)) = seed_track(&conn, dir.path(), "real_320.mp3", "src.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };

        let res = file_track(&conn, &root, "{artist} - {title}", id, "House", None, Some(Canonical {
            artist: "Larry Heard".into(), title: "Can You Feel It".into(), version: None,
            confidence: crate::naming::Confidence::Green,
        })).unwrap();

        // moved into bin, original gone (mono-location), one move action
        assert!(std::path::Path::new(&res.path).exists());
        assert!(!src.exists());
        assert!(res.path.ends_with("Larry Heard - Can You Feel It.mp3"));
        let (status, folder): (String, Option<String>) = conn
            .query_row("SELECT status, folder FROM tracks WHERE id=?1", params![id], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!(status, "filed");
        assert_eq!(folder.as_deref(), Some("House"));
        let moves: i64 = conn.query_row("SELECT count(*) FROM actions WHERE type='move' AND undone=0", [], |r| r.get(0)).unwrap();
        assert_eq!(moves, 1);
    }

    /// Reverting a CONFORMANT filing must remove the Discogs tags FROM THE FILE (restore the old
    /// ones), not just move the file back — else the file still carries the applied tags and the B9
    /// "not written" marker would wrongly stay hidden. The conformant filing journals tag_edit+move;
    /// revert undoes the move (file → source) THEN restores the captured old tags at source.
    #[test]
    fn revert_of_conformant_filing_restores_old_file_tags() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(root.join("House")).unwrap();
        let Some((id, src)) = seed_track(&conn, dir.path(), "real_320.mp3", "src.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        // Give the source file KNOWN old tags before filing.
        crate::tagging::write_tags_full(src.to_str().unwrap(), "OLD Artist", "OLD Title", None, None, &[], None).unwrap();

        let res = file_track(&conn, &root, "{artist} - {title}", id, "House", None, Some(Canonical {
            artist: "NEW Artist".into(), title: "NEW Title".into(), version: None,
            confidence: crate::naming::Confidence::Green,
        })).unwrap();
        // Filed: file moved into the bin, carrying the NEW tags.
        let after = crate::tagging::read_tags_full(&res.path).unwrap();
        assert_eq!(after.artist.as_deref(), Some("NEW Artist"), "filing wrote the new tags");

        // Revert the whole filing batch (move undone, then old tags restored).
        crate::actions::revert_batch(&conn, &res.batch_id).unwrap();

        assert!(src.exists(), "the file is moved back to its source");
        assert!(!std::path::Path::new(&res.path).exists(), "nothing left at the bin destination");
        let restored = crate::tagging::read_tags_full(src.to_str().unwrap()).unwrap();
        assert_eq!(restored.artist.as_deref(), Some("OLD Artist"), "old file tags restored on revert");
        assert_eq!(restored.title.as_deref(), Some("OLD Title"));
    }

    #[test]
    fn files_flac_by_converting_to_aiff_and_trashing_original() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(root.join("House")).unwrap();
        let Some((id, src)) = seed_track(&conn, dir.path(), "real_lossless.flac", "src.flac") else {
            eprintln!("skip: no fixture");
            return;
        };
        crate::ffmpeg::init_ffmpeg_path();

        let res = file_track(&conn, &root, "{artist} - {title}", id, "House", None, Some(Canonical {
            artist: "Theo Parrish".into(), title: "Falling Up".into(), version: None,
            confidence: crate::naming::Confidence::Green,
        })).unwrap();

        // converted AIFF lands in the bin; conformant to target
        assert!(res.path.ends_with("Theo Parrish - Falling Up.aiff"));
        assert!(crate::encode::is_conformant(&res.path, crate::encode::Target::Aiff1644));
        // original is in .sift-trash, not at its source location (mono-location)
        assert!(!src.exists());
        let convert_rows: i64 = conn.query_row("SELECT count(*) FROM actions WHERE type='convert' AND undone=0", [], |r| r.get(0)).unwrap();
        let trash_rows: i64 = conn.query_row("SELECT count(*) FROM actions WHERE type='trash' AND undone=0", [], |r| r.get(0)).unwrap();
        assert_eq!(convert_rows, 1);
        assert_eq!(trash_rows, 1);
    }

    /// Root fix for the `.aif`/`.aiff` revert-duplicate: a CONFORMANT AIFF is moved (no transcode),
    /// so it must keep its own extension instead of being forced to the canonical `.aiff`. We build a
    /// conformant 3-letter `.aif` by encoding the lossless fixture to AIFF 16/44.1, then file it and
    /// assert the destination stays `.aif` and the action was a `move` (not `convert`). With a single
    /// possible output name, a later blocked revert can no longer leave a `.aif` next to a `.aiff`.
    #[test]
    fn files_conformant_aif_preserving_its_extension() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(root.join("House")).unwrap();
        let Some(flac) = fixture("real_lossless.flac") else {
            eprintln!("skip: no fixture");
            return;
        };
        crate::ffmpeg::init_ffmpeg_path();

        // A conformant source whose extension is the 3-letter `.aif` (the case formerly forced to `.aiff`).
        let aif_src = dir.path().join("src.aif");
        crate::encode::encode(&flac, aif_src.to_str().unwrap(), crate::encode::Target::Aiff1644).unwrap();
        assert!(crate::encode::is_conformant(aif_src.to_str().unwrap(), crate::encode::Target::Aiff1644), "the built .aif is conformant");
        conn.execute("INSERT INTO tracks(path, status) VALUES(?1, 'pending')", params![aif_src.to_str().unwrap()]).unwrap();
        let id = conn.last_insert_rowid();

        let res = file_track(&conn, &root, "{artist} - {title}", id, "House", None, Some(Canonical {
            artist: "Larry Heard".into(), title: "Can You Feel It".into(), version: None,
            confidence: crate::naming::Confidence::Green,
        })).unwrap();

        // Moved keeping `.aif` — NOT forced to the 4-letter `.aiff`.
        assert!(res.path.ends_with("Larry Heard - Can You Feel It.aif"), "dest keeps .aif: {}", res.path);
        assert!(!res.path.ends_with(".aiff"), "must not force .aiff on a moved conformant file");
        assert!(std::path::Path::new(&res.path).exists());
        assert!(!aif_src.exists(), "moved out of source (mono-location)");
        // It was a pure MOVE: no conversion, no trash.
        let moves: i64 = conn.query_row("SELECT count(*) FROM actions WHERE type='move' AND undone=0", [], |r| r.get(0)).unwrap();
        let converts: i64 = conn.query_row("SELECT count(*) FROM actions WHERE type='convert' AND undone=0", [], |r| r.get(0)).unwrap();
        assert_eq!(moves, 1, "conformant .aif is moved");
        assert_eq!(converts, 0, "no conversion for an already-conformant file");
    }

    #[test]
    fn file_track_refuses_lossy_to_aiff() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(&root).unwrap();
        let Some((id, _)) = seed_track(&conn, dir.path(), "real_320.mp3", "src.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        let err = file_track(&conn, &root, "{artist} - {title}", id, "", Some(Target::Aiff1644), Some(Canonical {
            artist: "X".into(), title: "Y".into(), version: None, confidence: crate::naming::Confidence::Green,
        }));
        assert_eq!(err, Err(FilingError::Upscale));
    }

    #[test]
    fn reject_track_sets_resourcing_and_records() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let Some((id, _)) = seed_track(&conn, dir.path(), "real_320.mp3", "src.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        reject_track(&conn, id).unwrap();
        let status: String = conn.query_row("SELECT status FROM tracks WHERE id=?1", params![id], |r| r.get(0)).unwrap();
        assert_eq!(status, "resourcing");
        let rejects: i64 = conn.query_row("SELECT count(*) FROM actions WHERE type='reject'", [], |r| r.get(0)).unwrap();
        assert_eq!(rejects, 1);
    }

    #[test]
    fn reject_batch_marks_all_and_collects_bad_ids() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let (Some((a, _)), Some((b, _))) = (
            seed_track(&conn, dir.path(), "real_320.mp3", "a.mp3"),
            seed_track(&conn, dir.path(), "real_320.mp3", "b.mp3"),
        ) else {
            eprintln!("skip: no fixture");
            return;
        };
        // 999 is not a real track id → reject_track errors → reported in `failed`, batch not aborted.
        let res = reject_batch(&conn, &[a, b, 999]);
        assert_eq!(res, RejectBatchResult { rejected: 2, failed: vec![999] });
        let resourced: i64 = conn
            .query_row("SELECT count(*) FROM tracks WHERE status='resourcing'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(resourced, 2);
    }

    #[test]
    fn trash_track_moves_to_sift_trash() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(&root).unwrap();
        let Some((id, src)) = seed_track(&conn, dir.path(), "real_320.mp3", "src.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        trash_track(&conn, &root, id).unwrap();
        assert!(!src.exists());
        let status: String = conn.query_row("SELECT status FROM tracks WHERE id=?1", params![id], |r| r.get(0)).unwrap();
        assert_eq!(status, "trash");
        assert!(root.join(".sift-trash").read_dir().unwrap().count() >= 1);
    }

    #[test]
    fn filing_writes_applied_genres_to_the_file() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(root.join("House")).unwrap();
        let Some((id, _src)) = seed_track(&conn, dir.path(), "real_320.mp3", "src.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };

        // seed a Discogs identity for the track BEFORE filing
        let cand = crate::metadata::Candidate {
            artist: "Larry Heard".into(),
            title: "Mystery of Love".into(),
            label: Some("Alleviated".into()),
            year: Some(1986),
            styles: vec!["Deep House".into()],
            country: None,
            format: None,
            cover_url: None,
            release_id: "12345".into(),
            source: "discogs".into(),
        };
        crate::metadata::apply_identity(&conn, id, &cand, None).unwrap();

        let res = file_track(&conn, &root, "{artist} - {title}", id, "House", None, Some(Canonical {
            artist: "Larry Heard".into(),
            title: "Mystery of Love".into(),
            version: None,
            confidence: crate::naming::Confidence::Green,
        })).unwrap();

        use lofty::file::TaggedFileExt;
        use lofty::probe::Probe;
        use lofty::tag::ItemKey;
        let tagged = Probe::open(&res.path).unwrap().read().unwrap();
        let tag = tagged.primary_tag().unwrap();
        let genre = tag.get_string(&ItemKey::Genre).unwrap_or("");
        assert!(genre.contains("Deep House"), "filed file has applied genre; got {genre:?}");
    }
}
