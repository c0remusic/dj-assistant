//! Turn a reviewed track into a filed library file: ① convert (only if not conformant)
//! → ② tag + name → ③ move into the chosen bin, recording every step as one undoable
//! batch (see actions.rs). Mono-location: conformant files are moved; converted files
//! land in the bin and the original goes to `.sift-trash` (restorable via undo). Composes
//! naming/encode/tagging/library/actions/settings.
#![allow(dead_code)]

use crate::encode::{self, EncodeError, Target};
use crate::naming::{self, Canonical};
use crate::{actions, library, tagging};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::Path;

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

/// A monotonic-ish batch id: track id + millis since epoch.
fn new_batch_id(track_id: i64) -> String {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{track_id}-{ms}")
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

/// Move `source` into `<root>/.sift-trash/<track_id>__<name>`, recording a `trash` action
/// in `batch_id`. Returns the trash path.
fn move_to_trash(
    conn: &Connection,
    root: &Path,
    track_id: i64,
    batch_id: &str,
    source: &str,
) -> Result<String, FilingError> {
    let trash_dir = root.join(".sift-trash");
    std::fs::create_dir_all(&trash_dir).map_err(|e| FilingError::Io(e.to_string()))?;
    let dest = trash_dir.join(format!("{track_id}__{}", file_name_of(source)));
    let dest = library::ensure_unique(&dest);
    std::fs::rename(source, &dest).map_err(|e| FilingError::Io(e.to_string()))?;
    let dest_str = dest.to_string_lossy().to_string();
    actions::record(conn, batch_id, Some(track_id), "trash", Some(source), Some(&dest_str))
        .map_err(|e| FilingError::Db(e.to_string()))?;
    Ok(dest_str)
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

/// File one track into `bin_rel` under `root`. See module docs for the ordering and the
/// mono-location / undo contract.
pub fn file_track(
    conn: &Connection,
    root: &Path,
    template: &str,
    track_id: i64,
    bin_rel: &str,
    override_target: Option<Target>,
    edited: Option<Canonical>,
) -> Result<FileResult, FilingError> {
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

    let dest_dir = library::safe_join(root, bin_rel).map_err(FilingError::Io)?;
    std::fs::create_dir_all(&dest_dir).map_err(|e| FilingError::Io(e.to_string()))?;
    let filename = naming::render_filename(template, &canonical, target.ext());
    let dest = library::ensure_unique(&dest_dir.join(&filename));
    let dest_str = dest.to_string_lossy().to_string();
    let batch_id = new_batch_id(track_id);

    if encode::is_conformant(&source, target) {
        // tag in place, then move the single physical file into the bin
        tagging::write_tags(&source, &canonical.artist, &canonical.title)
            .map_err(FilingError::Tag)?;
        std::fs::rename(&source, &dest).map_err(|e| FilingError::Io(e.to_string()))?;
        // If journaling fails AFTER the move, roll the file back so it is never lost/orphaned.
        if let Err(e) =
            actions::record(conn, &batch_id, Some(track_id), "move", Some(&source), Some(&dest_str))
        {
            let _ = std::fs::rename(&dest, &source);
            return Err(FilingError::Db(e.to_string()));
        }
    } else {
        // transcode into the bin, tag the result, then trash the original (mono-location)
        encode::encode(&source, &dest_str, target).map_err(|e| match e {
            EncodeError::Upscale => FilingError::Upscale,
            EncodeError::Ffmpeg(m) => FilingError::Encode(m),
        })?;
        // If journaling fails AFTER the encode, delete the orphan transcode (source untouched).
        if let Err(e) =
            actions::record(conn, &batch_id, Some(track_id), "convert", Some(&source), Some(&dest_str))
        {
            let _ = std::fs::remove_file(&dest_str);
            return Err(FilingError::Db(e.to_string()));
        }
        tagging::write_tags(&dest_str, &canonical.artist, &canonical.title)
            .map_err(FilingError::Tag)?;
        move_to_trash(conn, root, track_id, &batch_id, &source)?;
    }

    // mark filed + persist target/confidence/metadata
    let conf = match canonical.confidence {
        naming::Confidence::Green => "green",
        naming::Confidence::Yellow => "yellow",
    };
    let target_str = match target {
        Target::Mp3320 => "mp3_320",
        Target::Aiff1644 => "aiff_16_44",
        Target::Wav1644 => "wav_16_44",
    };
    conn.execute(
        "UPDATE tracks SET status='filed', folder=?2, target_format=?3, confidence=?4 WHERE id=?1",
        params![track_id, bin_rel, target_str, conf],
    )?;
    save_metadata(conn, track_id, &canonical)?;

    Ok(FileResult { path: dest_str, batch_id })
}

/// File every green track of `track_ids` into `bin_rel`; leave yellow (or unreadable) ones
/// pending and return their ids in `needs_validation`. A track that errors during filing is
/// also returned as needing validation (not silently dropped).
pub fn file_batch(
    conn: &Connection,
    root: &Path,
    template: &str,
    track_ids: &[i64],
    bin_rel: &str,
) -> BatchResult {
    let mut filed = 0usize;
    let mut needs_validation = Vec::new();
    for &id in track_ids {
        match reconcile_track(conn, id) {
            Ok(c) if c.confidence == naming::Confidence::Green => {
                match file_track(conn, root, template, id, bin_rel, None, Some(c)) {
                    Ok(_) => filed += 1,
                    Err(_) => needs_validation.push(id),
                }
            }
            _ => needs_validation.push(id),
        }
    }
    BatchResult { filed, needs_validation }
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
}
