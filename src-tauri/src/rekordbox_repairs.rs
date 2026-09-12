//! M8 Tier 1/2/3 candidate lifecycle: SQL-backed queues of detected `master.db`
//! path repairs, playlist-duplicate groups, and metadata syncs, plus the
//! "apply one candidate" use-cases that call into `rekordbox_masterdb`'s
//! write engine.
//!
//! Extracted from `ipc_library.rs` (2026-07-09, clean-architecture audit F2):
//! this module owns the SQL + validation and has no Tauri dependency, matching
//! the domain-module convention already used by `filing.rs`/`actions.rs`.
//! `ipc_library.rs` keeps only the thin `#[tauri::command]` wrappers that lock
//! the DB `State` and delegate to the `_inner` functions here.

use rusqlite::Connection;
use std::path::Path;

/// One candidate `master.db` path repair (Tier 1), keyed by the Sift `actions.id`
/// that produced it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PendingMasterdbRepair {
    pub id: i64,
    /// `djmdContent.ID` — `None` when `status == "ambiguous"`.
    pub track_id: Option<String>,
    /// Comma-joined candidate `djmdContent.ID`s — set only when `status == "ambiguous"`.
    pub candidate_track_ids: Option<String>,
    /// Each candidate's current `master.db` path, resolved fresh at query time so the user can
    /// tell them apart. `None` when `status != "ambiguous"`, or when `master.db`/the linked XML
    /// couldn't be read at all (degrades gracefully — the row itself still lists, just without
    /// enrichment; never fails the whole `pending_repairs` call for this reason alone).
    pub candidate_tracks: Option<Vec<CandidateTrack>>,
    pub from_path: String,
    pub to_path: String,
    /// "pending" | "ambiguous".
    pub status: String,
    pub detected_at: String,
    /// The Sift app session (`actions.session_id`) that produced this candidate — `None` for
    /// actions recorded before that column existed (migration v8). Used to group candidates in
    /// the UI ("select all for this session") rather than listing every row flat.
    pub session_id: Option<String>,
    /// `metadata.artist` / `metadata.title` of the track the action moved (`actions.track_id`),
    /// for display — same contract as `PendingArtworkSync::artist` (2026-09-08). `None` when the
    /// action has no track or the track no metadata row; callers fall back to the path.
    pub artist: Option<String>,
    pub title: Option<String>,
}

/// One ambiguous-repair candidate, enriched with its current `master.db` path for display.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CandidateTrack {
    pub track_id: String,
    /// `None` if this `track_id` no longer exists in `master.db` (library changed since detection).
    pub folder_path: Option<String>,
}

/// Result of attempting to apply one pending repair.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ApplyRepairOutcome {
    pub id: i64,
    pub ok: bool,
    /// Humanized message on failure; `None` on success.
    pub error: Option<String>,
}

/// Resolves `master.db`'s real OS-standard folder (`actions::rekordbox_pioneer_dir`), gated on
/// an XML actually being linked (the user's opt-in signal for Rekordbox integration) — shared by
/// every M8 apply/scan path that needs a `pioneer_dir`, so the "no XML linked" error stays
/// identical everywhere instead of being retyped in five places.
fn resolve_pioneer_dir(conn: &Connection) -> Result<std::path::PathBuf, String> {
    crate::settings::get(conn, crate::settings::REKORDBOX_XML_PATH)
        .map_err(|e| e.to_string())?
        .ok_or("aucun XML Rekordbox lié — relie un fichier avant de synchroniser")?;
    let dir = crate::actions::rekordbox_pioneer_dir()
        .ok_or_else(|| "impossible de déterminer le dossier Pioneer sur ce système".to_string())?;
    // Impasse A12 ([issue #15](https://github.com/c0remusic/sift/issues/15)) : `rekordbox_pioneer_dir`
    // rend `Some(<chemin codé en dur>)` SANS `exists()`, donc sur une machine sans Rekordbox elle
    // rend un chemin qui n'existe pas et le `ok_or_else` ci-dessus ne tire jamais — il ne se
    // déclenche que si `dirs::config_dir()` elle-même échoue, ce qui n'arrive pas sur les deux
    // cibles. Le seul message qui parle du dossier Pioneer était donc de fait inatteignable.
    //
    // Le contrôle est posé ICI et pas dans `rekordbox_pioneer_dir` : les DÉTECTEURS ont un contrat
    // de no-op silencieux assumé (ils tournent après chaque rangement, en arrière-plan, et ne
    // doivent rien interrompre), alors que ce chemin-ci ne s'atteint que par un clic explicite sur
    // « Appliquer ». Là, échouer en nommant la cause est exactement ce qu'il faut.
    if !dir.is_dir() {
        return Err(format!(
            "dossier Pioneer introuvable ({}) — Rekordbox ne semble pas installé sur cette machine",
            dir.display()
        ));
    }
    Ok(dir)
}

/// One timestamp per batch call (not per row) — shared across every row so rows applied
/// together land under the same backup batch directory.
fn new_batch_stamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

/// Logs + builds the "master.db already written, local bookkeeping failed" message shared by
/// all 3 apply_one_* desync branches (repair/metadata sync/artwork sync) — master.db was already
/// written successfully at the point this is called; the row staying 'pending' means a later
/// retry would re-run the write for no reason, so this is loud and explicit rather than reading
/// as a plain sync failure. `kind` is the log-line noun ("repair"/"metadata sync"/"artwork
/// sync"); `action_phrase` is the full French clause already agreeing in gender/verb for that row
/// kind (e.g. "la réparation a bien été appliquée").
fn desync_error_message(
    kind: &str,
    action_phrase: &str,
    id: i64,
    e: impl std::fmt::Display,
) -> String {
    log::error!("rekordbox {kind} {id}: master.db écrit avec succès mais l'état local n'a pas pu être mis à jour (désync) : {e}");
    format!("{action_phrase} dans master.db, mais Sift n'a pas pu l'enregistrer localement — ne pas réappliquer sans vérifier Rekordbox d'abord ({e})")
}

/// Logs + humanizes a master.db WRITE failure shared by all 3 apply_one_* write branches
/// (repair/metadata sync/artwork sync). The write did NOT succeed (verification failed and rolled
/// back, rollback itself failed, Rekordbox open, track missing…); the raw `MasterDbError` carries
/// the full diagnostic that the humanized user message intentionally drops, so it is logged here
/// before humanizing — the sensitive master.db write path must never fail without a server-side
/// trace (fail-fast, no silent failure). Behaviour is unchanged: the returned string is exactly
/// `humanize_masterdb_error(e)` as before.
fn write_error_message(
    kind: &str,
    id: i64,
    e: &crate::rekordbox_masterdb::MasterDbError,
) -> String {
    log::error!("rekordbox {kind} {id}: échec d'écriture master.db : {e:?}");
    humanize_masterdb_error(e)
}

fn basename(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path)
        .to_string()
}

pub(crate) fn humanize_masterdb_error(e: &crate::rekordbox_masterdb::MasterDbError) -> String {
    use crate::rekordbox_masterdb::MasterDbError;
    match e {
        MasterDbError::RekordboxRunning => "Rekordbox est ouvert — ferme-le avant de synchroniser".to_string(),
        MasterDbError::RegistryRowMissing => "structure de master.db inattendue — synchronisation impossible".to_string(),
        MasterDbError::TrackNotFound { track_id } => format!(
            "piste {track_id} introuvable dans master.db — la bibliothèque Rekordbox a peut-être changé depuis la détection"
        ),
        MasterDbError::WriteVerificationFailedRolledBack(m) => {
            format!("l'écriture a échoué à la vérification, la sauvegarde a été restaurée automatiquement : {m}")
        }
        MasterDbError::WriteVerificationFailedRollbackFailed(m) => format!(
            "l'écriture ET la restauration de la sauvegarde ont échoué — intervention manuelle nécessaire : {m}"
        ),
        MasterDbError::NoDuplicatesToRemove => {
            "aucun doublon à supprimer dans ce groupe — la bibliothèque a peut-être changé depuis le scan".to_string()
        }
        MasterDbError::SongPlaylistEntryNotFound { song_playlist_id } => format!(
            "entrée de playlist {song_playlist_id} introuvable — la bibliothèque Rekordbox a peut-être changé depuis le scan"
        ),
        MasterDbError::NoArtworkPath { track_id } => format!(
            "la piste {track_id} n'a pas de pochette dans master.db — aucune synchro possible"
        ),
        MasterDbError::ArtworkVariantMissing { path } => format!(
            "fichier pochette manquant côté Rekordbox ({path}) — bibliothèque peut-être corrompue ou jamais scannée"
        ),
        MasterDbError::ArtworkWriteVerificationFailedRolledBack(m) => format!(
            "l'écriture de la pochette a échoué à la vérification, la sauvegarde a été restaurée automatiquement : {m}"
        ),
        MasterDbError::ArtworkWriteVerificationFailedRollbackFailed(m) => format!(
            "l'écriture ET la restauration de la pochette ont échoué — intervention manuelle nécessaire : {m}"
        ),
        other => other.to_string(),
    }
}

/// Resolves `master.db`'s real folder (gated on an XML being linked) and reads it once,
/// returning a `track_id -> folder_path` map. `None` if no XML is linked or `master.db` can't
/// be read — callers must degrade gracefully, never treat this as a hard error.
fn read_masterdb_path_map(conn: &Connection) -> Option<std::collections::HashMap<String, String>> {
    crate::settings::get(conn, crate::settings::REKORDBOX_XML_PATH)
        .ok()
        .flatten()?;
    let pioneer_dir = crate::actions::rekordbox_pioneer_dir()?;
    let index =
        crate::rekordbox_masterdb::read_rekordbox_masterdb(&pioneer_dir.join("master.db")).ok()?;
    Some(
        index
            .tracks
            .into_iter()
            .map(|t| (t.track_id, t.folder_path))
            .collect(),
    )
}

/// Plain (testable) implementation of `ipc_library::rekordbox_masterdb_pending_repairs`.
pub(crate) fn pending_repairs_inner(
    conn: &Connection,
) -> Result<Vec<PendingMasterdbRepair>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT r.id, r.track_id, r.candidate_track_ids, r.from_path, r.to_path, r.status, r.detected_at, a.session_id, m.artist, m.title
             FROM rekordbox_masterdb_repairs r
             LEFT JOIN actions a ON a.id = r.action_id
             LEFT JOIN metadata m ON m.track_id = a.track_id
             WHERE r.status IN ('pending', 'ambiguous')
             ORDER BY r.detected_at",
        )
        .map_err(|e| e.to_string())?;
    let mut rows: Vec<PendingMasterdbRepair> = stmt
        .query_map([], |r| {
            Ok(PendingMasterdbRepair {
                id: r.get(0)?,
                track_id: r.get(1)?,
                candidate_track_ids: r.get(2)?,
                candidate_tracks: None,
                from_path: r.get(3)?,
                to_path: r.get(4)?,
                status: r.get(5)?,
                detected_at: r.get(6)?,
                session_id: r.get(7)?,
                artist: r.get(8)?,
                title: r.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    // Resolve master.db exactly once for the whole batch, not once per ambiguous row.
    if rows.iter().any(|r| r.status == "ambiguous") {
        if let Some(path_map) = read_masterdb_path_map(conn) {
            for row in rows.iter_mut().filter(|r| r.status == "ambiguous") {
                if let Some(ids) = &row.candidate_track_ids {
                    row.candidate_tracks = Some(
                        ids.split(',')
                            .map(|id| CandidateTrack {
                                track_id: id.to_string(),
                                folder_path: path_map.get(id).cloned(),
                            })
                            .collect(),
                    );
                }
            }
        }
    }
    Ok(rows)
}

/// Plain (testable) implementation of `ipc_library::rekordbox_masterdb_dismiss_repair`.
pub(crate) fn dismiss_repair_inner(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE rekordbox_masterdb_repairs SET status='dismissed' WHERE id=?1",
        rusqlite::params![id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Plain (testable) implementation of `ipc_library::rekordbox_masterdb_resolve_ambiguous`.
pub(crate) fn resolve_ambiguous_inner(
    conn: &Connection,
    id: i64,
    chosen_track_id: &str,
) -> Result<(), String> {
    let (candidate_track_ids, status): (Option<String>, String) = conn
        .query_row(
            "SELECT candidate_track_ids, status FROM rekordbox_masterdb_repairs WHERE id=?1",
            rusqlite::params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    if status != "ambiguous" {
        return Err("cette ligne n'est plus ambiguë — rechargement nécessaire".to_string());
    }
    let candidates = candidate_track_ids.unwrap_or_default();
    if !candidates.split(',').any(|c| c == chosen_track_id) {
        return Err("piste choisie invalide pour cette ambiguïté".to_string());
    }

    conn.execute(
        "UPDATE rekordbox_masterdb_repairs SET track_id=?1, candidate_track_ids=NULL, status='pending' WHERE id=?2",
        rusqlite::params![chosen_track_id, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Attempts one repair row. Never calls `repair_track_path` for a row that isn't `pending`
/// with a known `track_id`, or whose `to_path` no longer exists on disk.
fn apply_one_repair(
    conn: &Connection,
    pioneer_dir: &Path,
    backup_root: &Path,
    batch_stamp: &str,
    id: i64,
) -> ApplyRepairOutcome {
    let row = conn.query_row(
        "SELECT track_id, to_path, status FROM rekordbox_masterdb_repairs WHERE id=?1",
        rusqlite::params![id],
        |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        },
    );
    let (track_id, to_path, status) = match row {
        Ok(v) => v,
        Err(e) => {
            return ApplyRepairOutcome {
                id,
                ok: false,
                error: Some(e.to_string()),
            }
        }
    };

    let Some(track_id) = track_id.filter(|_| status == "pending") else {
        return ApplyRepairOutcome {
            id,
            ok: false,
            error: Some("piste ambiguë ou déjà traitée — résolution manuelle requise".to_string()),
        };
    };

    if !std::path::Path::new(&to_path).exists() {
        return ApplyRepairOutcome {
            id,
            ok: false,
            error: Some(
                "le fichier n'existe plus à l'emplacement attendu — la piste a peut-être été déplacée ou annulée depuis"
                    .to_string(),
            ),
        };
    }

    let file_name = basename(&to_path);
    let repair = crate::rekordbox_masterdb::PathRepair {
        track_id,
        new_folder_path: to_path,
        new_file_name_l: file_name.clone(),
        new_file_name_s: file_name,
    };
    let backup_dir = backup_root.join(batch_stamp).join(id.to_string());

    match crate::rekordbox_masterdb::repair_track_path(pioneer_dir, &backup_dir, &repair) {
        Ok(()) => {
            if let Err(e) = conn.execute(
                "UPDATE rekordbox_masterdb_repairs SET status='applied', applied_at=datetime('now') WHERE id=?1",
                rusqlite::params![id],
            ) {
                return ApplyRepairOutcome {
                    id,
                    ok: false,
                    error: Some(desync_error_message("repair", "la réparation a bien été appliquée", id, e)),
                };
            }
            ApplyRepairOutcome {
                id,
                ok: true,
                error: None,
            }
        }
        Err(e) => ApplyRepairOutcome {
            id,
            ok: false,
            error: Some(write_error_message("repair", id, &e)),
        },
    }
}

/// Plain (testable) implementation of `ipc_library::rekordbox_masterdb_apply_repairs`.
/// `backup_root` is the caller-resolved base directory for backups (production:
/// `app_data_dir()/rekordbox-backups`) — kept as a parameter so this stays testable
/// without a Tauri runtime.
pub(crate) fn apply_repairs_inner(
    conn: &Connection,
    backup_root: &Path,
    ids: &[i64],
) -> Result<Vec<ApplyRepairOutcome>, String> {
    let pioneer_dir = resolve_pioneer_dir(conn)?;
    let batch_stamp = new_batch_stamp();

    let mut outcomes = Vec::with_capacity(ids.len());
    for &id in ids {
        outcomes.push(apply_one_repair(
            conn,
            &pioneer_dir,
            backup_root,
            &batch_stamp,
            id,
        ));
    }
    Ok(outcomes)
}

// ── M8 Tier 3: master.db metadata sync candidates ─────────────────────────────

/// One candidate metadata sync (Sift retagged a file linked to Rekordbox), keyed by Sift
/// `track_id` (unlike Tier 1's `PendingMasterdbRepair`, which is keyed by `action_id`) — a fresh
/// retag before the user applies replaces this row rather than adding another.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PendingMetadataSync {
    pub id: i64,
    pub track_id: i64,
    /// `tracks.path`, for display.
    pub sift_path: String,
    /// `djmdContent.ID` — `None` when `status == "ambiguous"`.
    pub rekordbox_track_id: Option<String>,
    pub candidate_track_ids: Option<String>,
    /// Same enrichment discipline as `PendingMasterdbRepair::candidate_tracks` — resolved fresh,
    /// only for `ambiguous` rows, `None` if `master.db` couldn't be read (degrades gracefully).
    pub candidate_tracks: Option<Vec<CandidateTrack>>,
    pub new_artist: Option<String>,
    pub new_title: Option<String>,
    pub new_label: Option<String>,
    pub new_year: Option<i64>,
    pub new_genre: Option<String>,
    /// "pending" | "ambiguous".
    pub status: String,
    pub detected_at: String,
    /// Same session-grouping contract as `PendingMasterdbRepair::session_id`.
    pub session_id: Option<String>,
}

/// Plain (testable) implementation of `ipc_library::rekordbox_masterdb_pending_metadata_syncs`.
pub(crate) fn pending_metadata_syncs_inner(
    conn: &Connection,
) -> Result<Vec<PendingMetadataSync>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT s.id, s.track_id, t.path, s.rekordbox_track_id, s.candidate_track_ids,
                    s.new_artist, s.new_title, s.new_label, s.new_year, s.new_genre, s.status, s.detected_at, a.session_id
             FROM rekordbox_masterdb_metadata_syncs s
             JOIN tracks t ON t.id = s.track_id
             LEFT JOIN actions a ON a.id = s.action_id
             WHERE s.status IN ('pending', 'ambiguous')
             ORDER BY s.detected_at",
        )
        .map_err(|e| e.to_string())?;
    let mut rows: Vec<PendingMetadataSync> = stmt
        .query_map([], |r| {
            Ok(PendingMetadataSync {
                id: r.get(0)?,
                track_id: r.get(1)?,
                sift_path: r.get(2)?,
                rekordbox_track_id: r.get(3)?,
                candidate_track_ids: r.get(4)?,
                candidate_tracks: None,
                new_artist: r.get(5)?,
                new_title: r.get(6)?,
                new_label: r.get(7)?,
                new_year: r.get(8)?,
                new_genre: r.get(9)?,
                status: r.get(10)?,
                detected_at: r.get(11)?,
                session_id: r.get(12)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    if rows.iter().any(|r| r.status == "ambiguous") {
        if let Some(path_map) = read_masterdb_path_map(conn) {
            for row in rows.iter_mut().filter(|r| r.status == "ambiguous") {
                if let Some(ids) = &row.candidate_track_ids {
                    row.candidate_tracks = Some(
                        ids.split(',')
                            .map(|id| CandidateTrack {
                                track_id: id.to_string(),
                                folder_path: path_map.get(id).cloned(),
                            })
                            .collect(),
                    );
                }
            }
        }
    }
    Ok(rows)
}

/// Plain (testable) implementation of `ipc_library::rekordbox_masterdb_dismiss_metadata_sync`.
pub(crate) fn dismiss_metadata_sync_inner(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE rekordbox_masterdb_metadata_syncs SET status='dismissed' WHERE id=?1",
        rusqlite::params![id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Plain (testable) implementation of
/// `ipc_library::rekordbox_masterdb_resolve_ambiguous_metadata_sync`.
pub(crate) fn resolve_ambiguous_metadata_sync_inner(
    conn: &Connection,
    id: i64,
    chosen_track_id: &str,
) -> Result<(), String> {
    let (candidate_track_ids, status): (Option<String>, String) = conn
        .query_row(
            "SELECT candidate_track_ids, status FROM rekordbox_masterdb_metadata_syncs WHERE id=?1",
            rusqlite::params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    if status != "ambiguous" {
        return Err("cette ligne n'est plus ambiguë — rechargement nécessaire".to_string());
    }
    let candidates = candidate_track_ids.unwrap_or_default();
    if !candidates.split(',').any(|c| c == chosen_track_id) {
        return Err("piste choisie invalide pour cette ambiguïté".to_string());
    }

    conn.execute(
        "UPDATE rekordbox_masterdb_metadata_syncs SET rekordbox_track_id=?1, candidate_track_ids=NULL, status='pending' WHERE id=?2",
        rusqlite::params![chosen_track_id, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Result of attempting to apply one pending metadata sync.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ApplyMetadataSyncOutcome {
    pub id: i64,
    pub ok: bool,
    pub error: Option<String>,
}

/// Attempts one metadata sync row. Never calls `sync_track_metadata` for a row that isn't
/// `pending` with a known `rekordbox_track_id`.
fn apply_one_metadata_sync(
    conn: &Connection,
    pioneer_dir: &Path,
    backup_root: &Path,
    batch_stamp: &str,
    id: i64,
) -> ApplyMetadataSyncOutcome {
    let row = conn.query_row(
        "SELECT rekordbox_track_id, new_artist, new_title, new_label, new_year, new_genre, status
         FROM rekordbox_masterdb_metadata_syncs WHERE id=?1",
        rusqlite::params![id],
        |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<i64>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, String>(6)?,
            ))
        },
    );
    let (rekordbox_track_id, new_artist, new_title, new_label, new_year, new_genre, status) =
        match row {
            Ok(v) => v,
            Err(e) => {
                return ApplyMetadataSyncOutcome {
                    id,
                    ok: false,
                    error: Some(e.to_string()),
                }
            }
        };

    let Some(rekordbox_track_id) = rekordbox_track_id.filter(|_| status == "pending") else {
        return ApplyMetadataSyncOutcome {
            id,
            ok: false,
            error: Some("piste ambiguë ou déjà traitée — résolution manuelle requise".to_string()),
        };
    };

    let sync = crate::rekordbox_masterdb::MetadataSync {
        track_id: rekordbox_track_id,
        artist: new_artist,
        title: new_title,
        year: new_year,
        genre: new_genre,
        label: new_label,
    };
    let backup_dir = backup_root.join(batch_stamp).join(id.to_string());

    match crate::rekordbox_masterdb::sync_track_metadata(pioneer_dir, &backup_dir, &sync) {
        Ok(()) => {
            if let Err(e) = conn.execute(
                "UPDATE rekordbox_masterdb_metadata_syncs SET status='applied', applied_at=datetime('now') WHERE id=?1",
                rusqlite::params![id],
            ) {
                return ApplyMetadataSyncOutcome {
                    id,
                    ok: false,
                    error: Some(desync_error_message("metadata sync", "la synchro a bien été appliquée", id, e)),
                };
            }
            ApplyMetadataSyncOutcome {
                id,
                ok: true,
                error: None,
            }
        }
        Err(e) => ApplyMetadataSyncOutcome {
            id,
            ok: false,
            error: Some(write_error_message("metadata sync", id, &e)),
        },
    }
}

/// Plain (testable) implementation of `ipc_library::rekordbox_masterdb_apply_metadata_syncs`.
pub(crate) fn apply_metadata_syncs_inner(
    conn: &Connection,
    backup_root: &Path,
    ids: &[i64],
) -> Result<Vec<ApplyMetadataSyncOutcome>, String> {
    let pioneer_dir = resolve_pioneer_dir(conn)?;
    let batch_stamp = new_batch_stamp();
    let mut outcomes = Vec::with_capacity(ids.len());
    for &id in ids {
        outcomes.push(apply_one_metadata_sync(
            conn,
            &pioneer_dir,
            backup_root,
            &batch_stamp,
            id,
        ));
    }
    Ok(outcomes)
}

// ── M8 Tier 3 (pochette): master.db artwork sync candidates ───────────────────

/// One candidate artwork sync, keyed by Sift `track_id` — a fresh cover before the user applies
/// replaces this row rather than adding another. `cover_path` is the source JPEG path, re-read
/// fresh at apply time (see `rekordbox_masterdb_apply_artwork_syncs`), never resolved bytes.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PendingArtworkSync {
    pub id: i64,
    pub track_id: i64,
    pub sift_path: String,
    pub rekordbox_track_id: Option<String>,
    pub candidate_track_ids: Option<String>,
    pub candidate_tracks: Option<Vec<CandidateTrack>>,
    pub cover_path: String,
    pub status: String,
    pub detected_at: String,
    /// Same session-grouping contract as `PendingMasterdbRepair::session_id`.
    pub session_id: Option<String>,
    /// `metadata.artist` / `metadata.title` of the Sift track, for display — the Rekordbox screen
    /// names a candidate « Artiste — Titre » like its metadata siblings, not by file name
    /// (2026-09-08). `None` when the track has no metadata row yet; callers fall back to the path.
    pub artist: Option<String>,
    pub title: Option<String>,
}

/// Plain (testable) implementation of `rekordbox_masterdb_pending_artwork_syncs`.
pub(crate) fn rekordbox_masterdb_pending_artwork_syncs_inner(
    conn: &Connection,
) -> Result<Vec<PendingArtworkSync>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT s.id, s.track_id, t.path, s.rekordbox_track_id, s.candidate_track_ids, s.cover_path, s.status, s.detected_at, a.session_id, m.artist, m.title
             FROM rekordbox_masterdb_artwork_syncs s
             JOIN tracks t ON t.id = s.track_id
             LEFT JOIN metadata m ON m.track_id = t.id
             LEFT JOIN actions a ON a.id = s.action_id
             WHERE s.status IN ('pending', 'ambiguous')
             ORDER BY s.detected_at",
        )
        .map_err(|e| e.to_string())?;
    let mut rows: Vec<PendingArtworkSync> = stmt
        .query_map([], |r| {
            Ok(PendingArtworkSync {
                id: r.get(0)?,
                track_id: r.get(1)?,
                sift_path: r.get(2)?,
                rekordbox_track_id: r.get(3)?,
                candidate_track_ids: r.get(4)?,
                candidate_tracks: None,
                cover_path: r.get(5)?,
                status: r.get(6)?,
                detected_at: r.get(7)?,
                session_id: r.get(8)?,
                artist: r.get(9)?,
                title: r.get(10)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    if rows.iter().any(|r| r.status == "ambiguous") {
        if let Some(path_map) = read_masterdb_path_map(conn) {
            for row in rows.iter_mut().filter(|r| r.status == "ambiguous") {
                if let Some(ids) = &row.candidate_track_ids {
                    row.candidate_tracks = Some(
                        ids.split(',')
                            .map(|id| CandidateTrack {
                                track_id: id.to_string(),
                                folder_path: path_map.get(id).cloned(),
                            })
                            .collect(),
                    );
                }
            }
        }
    }
    Ok(rows)
}

/// Plain (testable) implementation of `rekordbox_masterdb_dismiss_artwork_sync`.
pub(crate) fn rekordbox_masterdb_dismiss_artwork_sync_inner(
    conn: &Connection,
    id: i64,
) -> Result<(), String> {
    conn.execute(
        "UPDATE rekordbox_masterdb_artwork_syncs SET status='dismissed' WHERE id=?1",
        rusqlite::params![id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Plain (testable) implementation of `rekordbox_masterdb_resolve_ambiguous_artwork_sync`.
pub(crate) fn rekordbox_masterdb_resolve_ambiguous_artwork_sync_inner(
    conn: &Connection,
    id: i64,
    chosen_track_id: &str,
) -> Result<(), String> {
    let (candidate_track_ids, status): (Option<String>, String) = conn
        .query_row(
            "SELECT candidate_track_ids, status FROM rekordbox_masterdb_artwork_syncs WHERE id=?1",
            rusqlite::params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    if status != "ambiguous" {
        return Err("cette ligne n'est plus ambiguë — rechargement nécessaire".to_string());
    }
    let candidates = candidate_track_ids.unwrap_or_default();
    if !candidates.split(',').any(|c| c == chosen_track_id) {
        return Err("piste choisie invalide pour cette ambiguïté".to_string());
    }

    conn.execute(
        "UPDATE rekordbox_masterdb_artwork_syncs SET rekordbox_track_id=?1, candidate_track_ids=NULL, status='pending' WHERE id=?2",
        rusqlite::params![chosen_track_id, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Result of attempting to apply one pending artwork sync.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ApplyArtworkSyncOutcome {
    pub id: i64,
    pub ok: bool,
    pub error: Option<String>,
}

/// Attempts one artwork sync row. Never calls `sync_track_artwork` for a row that isn't `pending`
/// with a known `rekordbox_track_id`; never syncs stale bytes — `cover_path` is re-read from disk
/// here, at apply time, not at detection time.
fn apply_one_artwork_sync(
    conn: &Connection,
    pioneer_dir: &Path,
    backup_root: &Path,
    batch_stamp: &str,
    id: i64,
) -> ApplyArtworkSyncOutcome {
    let row = conn.query_row(
        "SELECT rekordbox_track_id, cover_path, status FROM rekordbox_masterdb_artwork_syncs WHERE id=?1",
        rusqlite::params![id],
        |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)),
    );
    let (rekordbox_track_id, cover_path, status) = match row {
        Ok(v) => v,
        Err(e) => {
            return ApplyArtworkSyncOutcome {
                id,
                ok: false,
                error: Some(e.to_string()),
            }
        }
    };

    let Some(rekordbox_track_id) = rekordbox_track_id.filter(|_| status == "pending") else {
        return ApplyArtworkSyncOutcome {
            id,
            ok: false,
            error: Some("piste ambiguë ou déjà traitée — résolution manuelle requise".to_string()),
        };
    };

    let cover_bytes = match std::fs::read(&cover_path) {
        Ok(b) => b,
        Err(_) => {
            return ApplyArtworkSyncOutcome {
                id,
                ok: false,
                error: Some(format!(
                    "le fichier de pochette source n'existe plus — {cover_path}"
                )),
            };
        }
    };

    let backup_dir = backup_root.join(batch_stamp).join(id.to_string());
    match crate::rekordbox_masterdb::sync_track_artwork(
        pioneer_dir,
        &backup_dir,
        &rekordbox_track_id,
        &cover_bytes,
    ) {
        Ok(()) => {
            if let Err(e) = conn.execute(
                "UPDATE rekordbox_masterdb_artwork_syncs SET status='applied', applied_at=datetime('now') WHERE id=?1",
                rusqlite::params![id],
            ) {
                return ApplyArtworkSyncOutcome {
                    id,
                    ok: false,
                    error: Some(desync_error_message("artwork sync", "la pochette a bien été synchronisée", id, e)),
                };
            }
            ApplyArtworkSyncOutcome {
                id,
                ok: true,
                error: None,
            }
        }
        Err(e) => ApplyArtworkSyncOutcome {
            id,
            ok: false,
            error: Some(write_error_message("artwork sync", id, &e)),
        },
    }
}

/// Plain (testable) implementation of `rekordbox_masterdb_apply_artwork_syncs`.
pub(crate) fn rekordbox_masterdb_apply_artwork_syncs_inner(
    conn: &Connection,
    backup_root: &Path,
    ids: &[i64],
) -> Result<Vec<ApplyArtworkSyncOutcome>, String> {
    let pioneer_dir = resolve_pioneer_dir(conn)?;
    let batch_stamp = new_batch_stamp();
    let mut outcomes = Vec::with_capacity(ids.len());
    for &id in ids {
        outcomes.push(apply_one_artwork_sync(
            conn,
            &pioneer_dir,
            backup_root,
            &batch_stamp,
            id,
        ));
    }
    Ok(outcomes)
}

// ── M8 Tier 2: playlist duplicate-entry dedup ─────────────────────────────────

/// One `djmdSongPlaylist` row involved in a duplicate group — mirrors
/// `rekordbox_masterdb::PlaylistDuplicateEntry` field-for-field, kept as a
/// separate IPC-boundary type per this module's `Serialize`-boundary convention
/// (see `humanize_masterdb_error`'s doc comment for the same rationale
/// applied to `MasterDbError`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlaylistDuplicateEntryDto {
    pub song_playlist_id: String,
    pub track_no: i64,
}

impl From<crate::rekordbox_masterdb::PlaylistDuplicateEntry> for PlaylistDuplicateEntryDto {
    fn from(e: crate::rekordbox_masterdb::PlaylistDuplicateEntry) -> Self {
        Self {
            song_playlist_id: e.song_playlist_id,
            track_no: e.track_no,
        }
    }
}

impl From<PlaylistDuplicateEntryDto> for crate::rekordbox_masterdb::PlaylistDuplicateEntry {
    fn from(e: PlaylistDuplicateEntryDto) -> Self {
        Self {
            song_playlist_id: e.song_playlist_id,
            track_no: e.track_no,
        }
    }
}

/// A set of `djmdSongPlaylist` rows in the same playlist that reference the
/// same track more than once — mirrors
/// `rekordbox_masterdb::PlaylistDuplicateGroup` field-for-field, plus 2
/// display-only fields (`playlist_name`, `track_path`) resolved by the scan
/// command for the UI's benefit. Round-trips through the frontend
/// unmodified: a scan returns these, and the exact same shape is passed
/// back to `rekordbox_masterdb_dedup_playlist_group` — no server-side id or
/// cache needed, the group's own fields are the identity. The write engine
/// only ever reads `playlist_id`/`content_id`/`keep`/`remove` (see the
/// reverse `From` impl below) — `playlist_name`/`track_path` are ignored on
/// that path, never required to be present or correct for a write to
/// succeed.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlaylistDuplicateGroupDto {
    pub playlist_id: String,
    /// `djmdPlaylist.Name`, resolved fresh at scan time. `None` if the
    /// playlist couldn't be found when resolving names (library changed
    /// since detection) — the UI falls back to the raw id.
    pub playlist_name: Option<String>,
    pub content_id: String,
    /// The duplicated track's current `master.db` path, resolved fresh at
    /// scan time. `None` for the same reason as `playlist_name`.
    pub track_path: Option<String>,
    pub keep: PlaylistDuplicateEntryDto,
    pub remove: Vec<PlaylistDuplicateEntryDto>,
}

impl From<crate::rekordbox_masterdb::PlaylistDuplicateGroup> for PlaylistDuplicateGroupDto {
    fn from(g: crate::rekordbox_masterdb::PlaylistDuplicateGroup) -> Self {
        Self {
            playlist_id: g.playlist_id,
            playlist_name: None,
            content_id: g.content_id,
            track_path: None,
            keep: g.keep.into(),
            remove: g.remove.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<PlaylistDuplicateGroupDto> for crate::rekordbox_masterdb::PlaylistDuplicateGroup {
    fn from(g: PlaylistDuplicateGroupDto) -> Self {
        Self {
            playlist_id: g.playlist_id,
            content_id: g.content_id,
            keep: g.keep.into(),
            remove: g.remove.into_iter().map(Into::into).collect(),
        }
    }
}

/// Plain (testable) implementation of `ipc_library::rekordbox_masterdb_scan_playlist_duplicates`.
/// Enriches with `playlist_name`/`track_path` in one extra pass, only when
/// the scan actually found something — same "resolve once for the whole
/// batch, only when needed" discipline as `pending_repairs_inner`'s
/// `candidate_tracks` enrichment.
pub(crate) fn scan_playlist_duplicates_inner(
    conn: &Connection,
) -> Result<Vec<PlaylistDuplicateGroupDto>, String> {
    let pioneer_dir = resolve_pioneer_dir(conn)?;
    let groups =
        crate::rekordbox_masterdb::detect_playlist_duplicates(&pioneer_dir.join("master.db"))
            .map_err(|e| humanize_masterdb_error(&e))?;
    let mut dtos: Vec<PlaylistDuplicateGroupDto> = groups.into_iter().map(Into::into).collect();

    if !dtos.is_empty() {
        let playlist_names =
            crate::rekordbox_masterdb::read_playlist_names(&pioneer_dir.join("master.db")).ok();
        let track_paths = read_masterdb_path_map(conn);
        for dto in &mut dtos {
            if let Some(names) = &playlist_names {
                dto.playlist_name = names.get(&dto.playlist_id).cloned();
            }
            if let Some(paths) = &track_paths {
                dto.track_path = paths.get(&dto.content_id).cloned();
            }
        }
    }
    Ok(dtos)
}

/// Plain (testable) implementation of `ipc_library::rekordbox_masterdb_dedup_playlist_group`.
/// `backup_root` is the caller-resolved base directory for backups
/// (production: `app_data_dir()/rekordbox-backups`), same convention as
/// `apply_repairs_inner`.
pub(crate) fn dedup_playlist_group_inner(
    conn: &Connection,
    backup_root: &Path,
    group: PlaylistDuplicateGroupDto,
) -> Result<(), String> {
    let pioneer_dir = resolve_pioneer_dir(conn)?;
    let batch_stamp = new_batch_stamp();
    let backup_dir = backup_root
        .join(&batch_stamp)
        .join(format!("{}-{}", group.playlist_id, group.content_id));

    crate::rekordbox_masterdb::dedup_playlist_group(&pioneer_dir, &backup_dir, &group.into())
        .map_err(|e| humanize_masterdb_error(&e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        crate::db::run_migrations(&c).unwrap();
        c
    }

    fn seed_pioneer_dir(dir: &std::path::Path) -> std::path::PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::copy(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/rekordbox_master.db"
            ),
            dir.join("master.db"),
        )
        .unwrap();
        crate::actions::set_pioneer_dir_override_for_test(dir.to_path_buf());
        let xml_path = dir.join("masterPlaylists6.xml");
        std::fs::write(&xml_path, b"<DJ_PLAYLISTS/>").unwrap();
        xml_path
    }

    /// Impasse A12 ([issue #15](https://github.com/c0remusic/sift/issues/15)) : sur une machine
    /// sans Rekordbox, un « Appliquer » doit dire que le dossier Pioneer n'est pas là.
    ///
    /// Avant le contrôle `is_dir()`, `rekordbox_pioneer_dir` rendait `Some(<chemin inexistant>)`
    /// et cette fonction le rendait tel quel : l'échec ne se manifestait que plus loin, en erreur
    /// d'ouverture de fichier, avec un message qui ne nommait pas la cause.
    ///
    /// Le contrôle positif compte autant que l'échec : sans lui, un test qui n'observe qu'un `Err`
    /// passerait aussi si la fonction échouait TOUJOURS.
    #[test]
    fn applying_names_a_missing_pioneer_folder_instead_of_failing_later() {
        let tmp = tempfile::tempdir().unwrap();
        let pioneer = tmp.path().join("Pioneer/rekordbox");
        let conn = db();
        let xml_path = seed_pioneer_dir(&pioneer);
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        // Contrôle positif : le dossier existe, la résolution aboutit.
        assert_eq!(resolve_pioneer_dir(&conn).unwrap(), pioneer);

        // Le dossier disparaît (== une machine sans Rekordbox), le réglage XML reste.
        std::fs::remove_dir_all(&pioneer).unwrap();
        let err = resolve_pioneer_dir(&conn).expect_err("un dossier Pioneer absent doit échouer");
        assert!(
            err.contains("dossier Pioneer introuvable"),
            "le message nomme la cause plutôt qu'un échec d'ouverture plus loin : {err}"
        );
    }

    fn seed_repair_row(
        conn: &Connection,
        from_path: &str,
        to_path: &str,
        track_id: Option<&str>,
        status: &str,
    ) -> i64 {
        conn.execute(
            "INSERT INTO actions(type, from_path, to_path) VALUES('move', ?1, ?2)",
            rusqlite::params![from_path, to_path],
        )
        .unwrap();
        let action_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO rekordbox_masterdb_repairs (action_id, track_id, from_path, to_path, status)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![action_id, track_id, from_path, to_path, status],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn pending_repairs_excludes_applied_and_dismissed() {
        let conn = db();
        seed_repair_row(&conn, "a", "a2", Some("1"), "pending");
        seed_repair_row(&conn, "b", "b2", None, "ambiguous");
        seed_repair_row(&conn, "c", "c2", Some("3"), "applied");
        seed_repair_row(&conn, "d", "d2", Some("4"), "dismissed");

        let rows = pending_repairs_inner(&conn).unwrap();
        let statuses: Vec<&str> = rows.iter().map(|r| r.status.as_str()).collect();
        assert_eq!(statuses.len(), 2);
        assert!(statuses.contains(&"pending"));
        assert!(statuses.contains(&"ambiguous"));
    }

    #[test]
    fn dismiss_repair_hides_it_from_pending_list() {
        let conn = db();
        let id = seed_repair_row(&conn, "a", "a2", Some("1"), "pending");
        dismiss_repair_inner(&conn, id).unwrap();
        let rows = pending_repairs_inner(&conn).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn apply_repairs_applies_a_pending_row() {
        let conn = db();
        let tmp = tempfile::tempdir().unwrap();
        let pioneer_dir = tmp.path().join("pioneer");
        let xml_path = seed_pioneer_dir(&pioneer_dir);
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        let new_path = tmp.path().join("track1.flac");
        std::fs::write(&new_path, b"fake audio").unwrap();
        let id = seed_repair_row(
            &conn,
            "D:/FIXTURE/track1.mp3",
            new_path.to_str().unwrap(),
            Some("40000001"),
            "pending",
        );

        let backup_root = tmp.path().join("backups");
        let outcomes = apply_repairs_inner(&conn, &backup_root, &[id]).unwrap();
        assert_eq!(outcomes.len(), 1);
        assert!(
            outcomes[0].ok,
            "expected success, got {:?}",
            outcomes[0].error
        );

        let (status, applied_at): (String, Option<String>) = conn
            .query_row(
                "SELECT status, applied_at FROM rekordbox_masterdb_repairs WHERE id=?1",
                rusqlite::params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(status, "applied");
        assert!(applied_at.is_some());
    }

    #[test]
    fn apply_repairs_two_rows_get_isolated_per_row_backups() {
        let conn = db();
        let tmp = tempfile::tempdir().unwrap();
        let pioneer_dir = tmp.path().join("pioneer");
        let xml_path = seed_pioneer_dir(&pioneer_dir);
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        let new_path_1 = tmp.path().join("track1.flac");
        std::fs::write(&new_path_1, b"fake audio 1").unwrap();
        let new_path_2 = tmp.path().join("track2.flac");
        std::fs::write(&new_path_2, b"fake audio 2").unwrap();
        let id1 = seed_repair_row(
            &conn,
            "D:/FIXTURE/track1.mp3",
            new_path_1.to_str().unwrap(),
            Some("40000001"),
            "pending",
        );
        let id2 = seed_repair_row(
            &conn,
            "D:/FIXTURE/track2.flac",
            new_path_2.to_str().unwrap(),
            Some("40000002"),
            "pending",
        );

        let backup_root = tmp.path().join("backups");
        let outcomes = apply_repairs_inner(&conn, &backup_root, &[id1, id2]).unwrap();
        assert!(outcomes[0].ok, "row 1: {:?}", outcomes[0].error);
        assert!(outcomes[1].ok, "row 2: {:?}", outcomes[1].error);

        let batch_dirs: Vec<_> = std::fs::read_dir(&backup_root).unwrap().collect();
        assert_eq!(
            batch_dirs.len(),
            1,
            "both rows share one batch timestamp directory"
        );
        let batch_dir = batch_dirs[0].as_ref().unwrap().path();
        assert!(batch_dir.join(id1.to_string()).join("master.db").exists());
        assert!(batch_dir.join(id2.to_string()).join("master.db").exists());
    }

    #[test]
    fn apply_repairs_continues_after_one_row_fails() {
        let conn = db();
        let tmp = tempfile::tempdir().unwrap();
        let pioneer_dir = tmp.path().join("pioneer");
        let xml_path = seed_pioneer_dir(&pioneer_dir);
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        let new_path = tmp.path().join("track1.flac");
        std::fs::write(&new_path, b"fake audio").unwrap();
        let id_ok = seed_repair_row(
            &conn,
            "D:/FIXTURE/track1.mp3",
            new_path.to_str().unwrap(),
            Some("40000001"),
            "pending",
        );
        // track_id "99999999" doesn't exist in the fixture — simulates master.db having
        // changed since detection.
        let id_missing_track = seed_repair_row(
            &conn,
            "D:/nope.mp3",
            new_path.to_str().unwrap(),
            Some("99999999"),
            "pending",
        );

        let backup_root = tmp.path().join("backups");
        let outcomes =
            apply_repairs_inner(&conn, &backup_root, &[id_ok, id_missing_track]).unwrap();
        assert!(
            outcomes[0].ok,
            "row 1 should succeed: {:?}",
            outcomes[0].error
        );
        assert!(!outcomes[1].ok, "row 2 should fail");
        assert!(outcomes[1].error.is_some());

        let status_ok: String = conn
            .query_row(
                "SELECT status FROM rekordbox_masterdb_repairs WHERE id=?1",
                rusqlite::params![id_ok],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status_ok, "applied");
        let status_failed: String = conn
            .query_row(
                "SELECT status FROM rekordbox_masterdb_repairs WHERE id=?1",
                rusqlite::params![id_missing_track],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            status_failed, "pending",
            "failed row stays pending, retryable"
        );
    }

    #[test]
    fn apply_repairs_skips_ambiguous_row_without_calling_repair_track_path() {
        let conn = db();
        let tmp = tempfile::tempdir().unwrap();
        let pioneer_dir = tmp.path().join("pioneer");
        let xml_path = seed_pioneer_dir(&pioneer_dir);
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        let before = std::fs::read(pioneer_dir.join("master.db")).unwrap();
        let id = seed_repair_row(
            &conn,
            "D:/FIXTURE/track1.mp3",
            "D:/FIXTURE/renamed/track1.flac",
            None,
            "ambiguous",
        );

        let backup_root = tmp.path().join("backups");
        let outcomes = apply_repairs_inner(&conn, &backup_root, &[id]).unwrap();
        assert!(!outcomes[0].ok);
        assert_eq!(
            outcomes[0].error.as_deref(),
            Some("piste ambiguë ou déjà traitée — résolution manuelle requise")
        );

        // master.db must be byte-identical — repair_track_path was never called.
        let after = std::fs::read(pioneer_dir.join("master.db")).unwrap();
        assert_eq!(before, after);
        assert!(
            !backup_root.exists(),
            "no backup should have been created either"
        );
    }

    #[test]
    fn apply_repairs_fails_fast_when_target_file_missing() {
        let conn = db();
        let tmp = tempfile::tempdir().unwrap();
        let pioneer_dir = tmp.path().join("pioneer");
        let xml_path = seed_pioneer_dir(&pioneer_dir);
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        let before = std::fs::read(pioneer_dir.join("master.db")).unwrap();
        // to_path deliberately points at a file that doesn't exist on disk.
        let missing_path = tmp.path().join("never-created.flac");
        let id = seed_repair_row(
            &conn,
            "D:/FIXTURE/track1.mp3",
            missing_path.to_str().unwrap(),
            Some("40000001"),
            "pending",
        );

        let backup_root = tmp.path().join("backups");
        let outcomes = apply_repairs_inner(&conn, &backup_root, &[id]).unwrap();
        assert!(!outcomes[0].ok);
        assert_eq!(
            outcomes[0].error.as_deref(),
            Some("le fichier n'existe plus à l'emplacement attendu — la piste a peut-être été déplacée ou annulée depuis")
        );

        let after = std::fs::read(pioneer_dir.join("master.db")).unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn apply_repairs_fails_all_when_no_xml_linked() {
        let conn = db();
        let id = seed_repair_row(&conn, "a", "a2", Some("1"), "pending");
        let backup_root = tempfile::tempdir().unwrap().path().join("backups");
        let err = apply_repairs_inner(&conn, &backup_root, &[id]).unwrap_err();
        assert_eq!(
            err,
            "aucun XML Rekordbox lié — relie un fichier avant de synchroniser"
        );
    }

    #[test]
    fn pending_repairs_enriches_ambiguous_candidates_with_paths() {
        let conn = db();
        let tmp = tempfile::tempdir().unwrap();
        let pioneer_dir = tmp.path().join("pioneer");
        let xml_path = seed_pioneer_dir(&pioneer_dir);
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        let id = seed_repair_row(
            &conn,
            "D:/FIXTURE/track1.mp3",
            "D:/FIXTURE/renamed/track1.flac",
            None,
            "ambiguous",
        );
        conn.execute(
            "UPDATE rekordbox_masterdb_repairs SET candidate_track_ids='40000001,40000002' WHERE id=?1",
            rusqlite::params![id],
        )
        .unwrap();

        let index =
            crate::rekordbox_masterdb::read_rekordbox_masterdb(&pioneer_dir.join("master.db"))
                .unwrap();
        let expected: std::collections::HashMap<String, String> = index
            .tracks
            .into_iter()
            .map(|t| (t.track_id, t.folder_path))
            .collect();

        let rows = pending_repairs_inner(&conn).unwrap();
        let row = rows.iter().find(|r| r.id == id).unwrap();
        let candidates = row
            .candidate_tracks
            .as_ref()
            .expect("candidate_tracks populated");
        assert_eq!(candidates.len(), 2);
        for c in candidates {
            assert_eq!(
                c.folder_path.as_deref(),
                expected.get(&c.track_id).map(|s| s.as_str()),
                "candidate {} path mismatch",
                c.track_id
            );
        }
    }

    #[test]
    fn pending_repairs_candidate_with_unknown_id_has_no_path() {
        let conn = db();
        let tmp = tempfile::tempdir().unwrap();
        let pioneer_dir = tmp.path().join("pioneer");
        let xml_path = seed_pioneer_dir(&pioneer_dir);
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        let id = seed_repair_row(
            &conn,
            "D:/FIXTURE/track1.mp3",
            "D:/FIXTURE/renamed/track1.flac",
            None,
            "ambiguous",
        );
        conn.execute(
            "UPDATE rekordbox_masterdb_repairs SET candidate_track_ids='40000001,99999999' WHERE id=?1",
            rusqlite::params![id],
        )
        .unwrap();

        let rows = pending_repairs_inner(&conn).unwrap();
        let row = rows.iter().find(|r| r.id == id).unwrap();
        let candidates = row
            .candidate_tracks
            .as_ref()
            .expect("candidate_tracks populated");
        let unknown = candidates
            .iter()
            .find(|c| c.track_id == "99999999")
            .unwrap();
        assert!(unknown.folder_path.is_none());
        let known = candidates
            .iter()
            .find(|c| c.track_id == "40000001")
            .unwrap();
        assert!(known.folder_path.is_some());
    }

    #[test]
    fn pending_repairs_degrades_gracefully_when_masterdb_unreadable() {
        // No XML linked at all — pioneer_dir can't be resolved.
        let conn = db();
        let id_pending = seed_repair_row(&conn, "a", "a2", Some("1"), "pending");
        let id_ambig = seed_repair_row(&conn, "b", "b2", None, "ambiguous");
        conn.execute(
            "UPDATE rekordbox_masterdb_repairs SET candidate_track_ids='1,2' WHERE id=?1",
            rusqlite::params![id_ambig],
        )
        .unwrap();

        let rows = pending_repairs_inner(&conn).unwrap();
        assert_eq!(
            rows.len(),
            2,
            "both rows still listed despite unresolved pioneer_dir"
        );
        let pending = rows.iter().find(|r| r.id == id_pending).unwrap();
        assert!(pending.candidate_tracks.is_none());
        let ambig = rows.iter().find(|r| r.id == id_ambig).unwrap();
        assert!(
            ambig.candidate_tracks.is_none(),
            "no XML linked -> None, not an error"
        );
    }

    #[test]
    fn resolve_ambiguous_moves_row_to_pending_with_chosen_track() {
        let conn = db();
        let id = seed_repair_row(&conn, "a", "a2", None, "ambiguous");
        conn.execute(
            "UPDATE rekordbox_masterdb_repairs SET candidate_track_ids='40000001,40000002' WHERE id=?1",
            rusqlite::params![id],
        )
        .unwrap();

        resolve_ambiguous_inner(&conn, id, "40000002").unwrap();

        let (track_id, candidates, status): (Option<String>, Option<String>, String) = conn
            .query_row(
                "SELECT track_id, candidate_track_ids, status FROM rekordbox_masterdb_repairs WHERE id=?1",
                rusqlite::params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(track_id.as_deref(), Some("40000002"));
        assert_eq!(candidates, None);
        assert_eq!(status, "pending");
    }

    #[test]
    fn resolve_ambiguous_rejects_track_id_outside_candidate_list() {
        let conn = db();
        let id = seed_repair_row(&conn, "a", "a2", None, "ambiguous");
        conn.execute(
            "UPDATE rekordbox_masterdb_repairs SET candidate_track_ids='40000001,40000002' WHERE id=?1",
            rusqlite::params![id],
        )
        .unwrap();

        let err = resolve_ambiguous_inner(&conn, id, "99999999").unwrap_err();
        assert_eq!(err, "piste choisie invalide pour cette ambiguïté");

        let status: String = conn
            .query_row(
                "SELECT status FROM rekordbox_masterdb_repairs WHERE id=?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "ambiguous", "unchanged on rejection");
    }

    #[test]
    fn resolve_ambiguous_rejects_row_that_is_not_ambiguous() {
        let conn = db();
        let id = seed_repair_row(&conn, "a", "a2", Some("40000001"), "pending");

        let err = resolve_ambiguous_inner(&conn, id, "40000001").unwrap_err();
        assert_eq!(
            err,
            "cette ligne n'est plus ambiguë — rechargement nécessaire"
        );
    }

    #[test]
    fn scan_playlist_duplicates_finds_the_fixture_duplicate() {
        let conn = db();
        let tmp = tempfile::tempdir().unwrap();
        let pioneer_dir = tmp.path().join("pioneer");
        let xml_path = seed_pioneer_dir(&pioneer_dir);
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        let groups = scan_playlist_duplicates_inner(&conn).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].playlist_id, "50000001");
        assert_eq!(groups[0].content_id, "40000001");
        assert_eq!(groups[0].keep.song_playlist_id, "60000001");
        assert_eq!(groups[0].keep.track_no, 1);
        assert_eq!(groups[0].remove.len(), 1);
        assert_eq!(groups[0].remove[0].song_playlist_id, "60000003");
        assert_eq!(groups[0].remove[0].track_no, 3);
    }

    #[test]
    fn scan_playlist_duplicates_fails_fast_when_no_xml_linked() {
        let conn = db();
        let err = scan_playlist_duplicates_inner(&conn).unwrap_err();
        assert_eq!(
            err,
            "aucun XML Rekordbox lié — relie un fichier avant de synchroniser"
        );
    }

    #[test]
    fn scan_playlist_duplicates_enriches_with_playlist_name_and_track_path() {
        let conn = db();
        let tmp = tempfile::tempdir().unwrap();
        let pioneer_dir = tmp.path().join("pioneer");
        let xml_path = seed_pioneer_dir(&pioneer_dir);
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        let groups = scan_playlist_duplicates_inner(&conn).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].playlist_name.as_deref(), Some("Fixture Playlist"));
        assert_eq!(
            groups[0].track_path.as_deref(),
            Some("D:/FIXTURE/track1.mp3")
        );
    }

    #[test]
    fn dedup_playlist_group_command_removes_the_duplicate() {
        let conn = db();
        let tmp = tempfile::tempdir().unwrap();
        let pioneer_dir = tmp.path().join("pioneer");
        let xml_path = seed_pioneer_dir(&pioneer_dir);
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        let groups = scan_playlist_duplicates_inner(&conn).unwrap();
        assert_eq!(groups.len(), 1);
        let group = groups[0].clone();

        let backup_root = tmp.path().join("backups");
        dedup_playlist_group_inner(&conn, &backup_root, group).unwrap();

        let after = scan_playlist_duplicates_inner(&conn).unwrap();
        assert!(after.is_empty(), "duplicate must be gone after dedup");
        assert!(
            backup_root.exists(),
            "a backup must have been created before the write"
        );
    }

    #[test]
    fn dedup_playlist_group_command_fails_fast_when_no_xml_linked() {
        let conn = db();
        let group = PlaylistDuplicateGroupDto {
            playlist_id: "50000001".to_string(),
            playlist_name: None,
            content_id: "40000001".to_string(),
            track_path: None,
            keep: PlaylistDuplicateEntryDto {
                song_playlist_id: "60000001".to_string(),
                track_no: 1,
            },
            remove: vec![PlaylistDuplicateEntryDto {
                song_playlist_id: "60000003".to_string(),
                track_no: 3,
            }],
        };
        let backup_root = tempfile::tempdir().unwrap().path().join("backups");
        let err = dedup_playlist_group_inner(&conn, &backup_root, group).unwrap_err();
        assert_eq!(
            err,
            "aucun XML Rekordbox lié — relie un fichier avant de synchroniser"
        );
    }

    #[test]
    fn dedup_playlist_group_command_humanizes_stale_group_error() {
        let conn = db();
        let tmp = tempfile::tempdir().unwrap();
        let pioneer_dir = tmp.path().join("pioneer");
        let xml_path = seed_pioneer_dir(&pioneer_dir);
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        // A group referencing a song_playlist_id that doesn't exist — simulates the
        // library having changed since the scan that produced this group.
        let stale_group = PlaylistDuplicateGroupDto {
            playlist_id: "50000001".to_string(),
            playlist_name: None,
            content_id: "40000001".to_string(),
            track_path: None,
            keep: PlaylistDuplicateEntryDto {
                song_playlist_id: "60000001".to_string(),
                track_no: 1,
            },
            remove: vec![PlaylistDuplicateEntryDto {
                song_playlist_id: "99999999".to_string(),
                track_no: 9,
            }],
        };
        let backup_root = tmp.path().join("backups");
        let err = dedup_playlist_group_inner(&conn, &backup_root, stale_group).unwrap_err();
        assert!(
            err.contains("99999999"),
            "error should name the missing row so the user understands what changed: {err}"
        );
    }

    fn seed_metadata_sync_row(
        conn: &Connection,
        track_id: i64,
        status: &str,
        rb_track_id: Option<&str>,
        candidates: Option<&str>,
    ) -> i64 {
        conn.execute(
            "INSERT INTO tracks(path, status) VALUES(?1, 'filed')",
            rusqlite::params![format!("D:/t{track_id}.mp3")],
        )
        .ok();
        let action_id = crate::actions::record_row_only(
            conn,
            "b1",
            Some(track_id),
            "tag_edit",
            Some("D:/x.mp3"),
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "INSERT INTO rekordbox_masterdb_metadata_syncs
                 (action_id, track_id, rekordbox_track_id, candidate_track_ids, new_artist, status)
             VALUES (?1, ?2, ?3, ?4, 'New Artist', ?5)",
            rusqlite::params![action_id, track_id, rb_track_id, candidates, status],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn pending_metadata_syncs_excludes_applied_and_dismissed() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(path, status) VALUES('D:/a.mp3', 'filed')",
            [],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();
        seed_metadata_sync_row(&conn, track_id, "pending", Some("40000001"), None);

        conn.execute(
            "INSERT INTO tracks(path, status) VALUES('D:/b.mp3', 'filed')",
            [],
        )
        .unwrap();
        let track_id_2 = conn.last_insert_rowid();
        let id2 = seed_metadata_sync_row(&conn, track_id_2, "applied", Some("40000002"), None);
        conn.execute(
            "UPDATE rekordbox_masterdb_metadata_syncs SET status='dismissed' WHERE id=?1",
            rusqlite::params![id2],
        )
        .ok();

        let rows = pending_metadata_syncs_inner(&conn).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "pending");
    }

    #[test]
    fn dismiss_metadata_sync_marks_dismissed() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(path, status) VALUES('D:/a.mp3', 'filed')",
            [],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();
        let id = seed_metadata_sync_row(&conn, track_id, "pending", Some("40000001"), None);

        dismiss_metadata_sync_inner(&conn, id).unwrap();

        let rows = pending_metadata_syncs_inner(&conn).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn resolve_ambiguous_metadata_sync_moves_to_pending() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(path, status) VALUES('D:/a.mp3', 'filed')",
            [],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();
        let id = seed_metadata_sync_row(
            &conn,
            track_id,
            "ambiguous",
            None,
            Some("40000001,40000002"),
        );

        resolve_ambiguous_metadata_sync_inner(&conn, id, "40000002").unwrap();

        let rows = pending_metadata_syncs_inner(&conn).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "pending");
        assert_eq!(rows[0].rekordbox_track_id.as_deref(), Some("40000002"));
        assert_eq!(rows[0].candidate_track_ids, None);
    }

    #[test]
    fn resolve_ambiguous_metadata_sync_rejects_track_id_outside_candidate_list() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(path, status) VALUES('D:/a.mp3', 'filed')",
            [],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();
        let id = seed_metadata_sync_row(
            &conn,
            track_id,
            "ambiguous",
            None,
            Some("40000001,40000002"),
        );

        let err = resolve_ambiguous_metadata_sync_inner(&conn, id, "99999999").unwrap_err();
        assert!(err.contains("invalide"));
    }

    #[test]
    fn resolve_ambiguous_metadata_sync_rejects_row_that_is_not_ambiguous() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(path, status) VALUES('D:/a.mp3', 'filed')",
            [],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();
        let id = seed_metadata_sync_row(&conn, track_id, "pending", Some("40000001"), None);

        let err = resolve_ambiguous_metadata_sync_inner(&conn, id, "40000001").unwrap_err();
        assert!(err.contains("ambigu"));
    }

    #[test]
    fn apply_metadata_syncs_applies_pending_row() {
        let conn = db();
        let tmp = tempfile::tempdir().unwrap();
        let pioneer_dir = tmp.path().join("pioneer");
        std::fs::create_dir_all(&pioneer_dir).unwrap();
        std::fs::copy(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/rekordbox_master.db"
            ),
            pioneer_dir.join("master.db"),
        )
        .unwrap();
        crate::actions::set_pioneer_dir_override_for_test(pioneer_dir.clone());
        let xml_path = pioneer_dir.join("masterPlaylists6.xml");
        std::fs::write(&xml_path, b"<DJ_PLAYLISTS/>").unwrap();
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        conn.execute(
            "INSERT INTO tracks(path, status) VALUES('D:/FIXTURE/track1.mp3', 'filed')",
            [],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();
        let id = seed_metadata_sync_row(&conn, track_id, "pending", Some("40000001"), None);

        let backup_root = tmp.path().join("backups");
        let outcomes = apply_metadata_syncs_inner(&conn, &backup_root, &[id]).unwrap();
        assert_eq!(outcomes.len(), 1);
        assert!(
            outcomes[0].ok,
            "expected ok, got error: {:?}",
            outcomes[0].error
        );

        let status: String = conn
            .query_row(
                "SELECT status FROM rekordbox_masterdb_metadata_syncs WHERE id=?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "applied");
    }

    #[test]
    fn apply_metadata_syncs_continues_after_one_failure() {
        let conn = db();
        let tmp = tempfile::tempdir().unwrap();
        let pioneer_dir = tmp.path().join("pioneer");
        std::fs::create_dir_all(&pioneer_dir).unwrap();
        std::fs::copy(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/rekordbox_master.db"
            ),
            pioneer_dir.join("master.db"),
        )
        .unwrap();
        crate::actions::set_pioneer_dir_override_for_test(pioneer_dir.clone());
        let xml_path = pioneer_dir.join("masterPlaylists6.xml");
        std::fs::write(&xml_path, b"<DJ_PLAYLISTS/>").unwrap();
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        conn.execute(
            "INSERT INTO tracks(path, status) VALUES('D:/FIXTURE/track1.mp3', 'filed')",
            [],
        )
        .unwrap();
        let track_id_1 = conn.last_insert_rowid();
        let id_ok = seed_metadata_sync_row(&conn, track_id_1, "pending", Some("40000001"), None);

        conn.execute(
            "INSERT INTO tracks(path, status) VALUES('D:/FIXTURE/gone.mp3', 'filed')",
            [],
        )
        .unwrap();
        let track_id_2 = conn.last_insert_rowid();
        let id_fail = seed_metadata_sync_row(&conn, track_id_2, "pending", Some("99999999"), None); // no such djmdContent row

        let backup_root = tmp.path().join("backups");
        let outcomes = apply_metadata_syncs_inner(&conn, &backup_root, &[id_ok, id_fail]).unwrap();
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes[0].ok);
        assert!(!outcomes[1].ok);
        assert!(outcomes[1]
            .error
            .as_deref()
            .unwrap()
            .contains("introuvable"));

        let status_ok: String = conn
            .query_row(
                "SELECT status FROM rekordbox_masterdb_metadata_syncs WHERE id=?1",
                rusqlite::params![id_ok],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status_ok, "applied");
        let status_fail: String = conn
            .query_row(
                "SELECT status FROM rekordbox_masterdb_metadata_syncs WHERE id=?1",
                rusqlite::params![id_fail],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            status_fail, "pending",
            "a failed row must stay pending, retryable"
        );
    }

    #[test]
    fn apply_metadata_syncs_rejects_ambiguous_row_without_calling_engine() {
        let conn = db();
        let tmp = tempfile::tempdir().unwrap();
        let pioneer_dir = tmp.path().join("pioneer");
        std::fs::create_dir_all(&pioneer_dir).unwrap();
        std::fs::copy(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/rekordbox_master.db"
            ),
            pioneer_dir.join("master.db"),
        )
        .unwrap();
        crate::actions::set_pioneer_dir_override_for_test(pioneer_dir.clone());
        let xml_path = pioneer_dir.join("masterPlaylists6.xml");
        std::fs::write(&xml_path, b"<DJ_PLAYLISTS/>").unwrap();
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        conn.execute(
            "INSERT INTO tracks(path, status) VALUES('D:/a.mp3', 'filed')",
            [],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();
        let id = seed_metadata_sync_row(
            &conn,
            track_id,
            "ambiguous",
            None,
            Some("40000001,40000002"),
        );

        let backup_root = tmp.path().join("backups");
        let outcomes = apply_metadata_syncs_inner(&conn, &backup_root, &[id]).unwrap();
        assert_eq!(outcomes.len(), 1);
        assert!(!outcomes[0].ok);
        assert!(outcomes[0].error.as_deref().unwrap().contains("ambigu"));

        let status: String = conn
            .query_row(
                "SELECT status FROM rekordbox_masterdb_metadata_syncs WHERE id=?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "ambiguous", "must not have been touched");
    }

    #[test]
    fn humanize_masterdb_error_covers_artwork_variants() {
        use crate::rekordbox_masterdb::MasterDbError;
        assert!(humanize_masterdb_error(&MasterDbError::NoArtworkPath {
            track_id: "40000001".to_string()
        })
        .contains("40000001"));
        assert!(
            humanize_masterdb_error(&MasterDbError::ArtworkVariantMissing {
                path: "C:/x/artwork_m.jpg".to_string()
            })
            .contains("artwork_m.jpg")
        );
        assert!(
            humanize_masterdb_error(&MasterDbError::ArtworkWriteVerificationFailedRolledBack(
                "dim mismatch".to_string()
            ))
            .contains("dim mismatch")
        );
        assert!(humanize_masterdb_error(
            &MasterDbError::ArtworkWriteVerificationFailedRollbackFailed(
                "dim mismatch".to_string()
            )
        )
        .contains("intervention manuelle"));
    }

    fn seed_artwork_sync_row(
        conn: &Connection,
        track_id: i64,
        status: &str,
        rb_track_id: Option<&str>,
        candidates: Option<&str>,
        cover_path: &str,
    ) -> i64 {
        let action_id = crate::actions::record_row_only(
            conn,
            "b1",
            Some(track_id),
            "tag_edit",
            Some("D:/x.mp3"),
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "INSERT INTO rekordbox_masterdb_artwork_syncs
                 (action_id, track_id, rekordbox_track_id, candidate_track_ids, cover_path, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                action_id,
                track_id,
                rb_track_id,
                candidates,
                cover_path,
                status
            ],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn tiny_jpeg(w: u32, h: u32) -> Vec<u8> {
        use image::ImageEncoder;
        let img = image::RgbImage::from_pixel(w, h, image::Rgb([5, 5, 5]));
        let mut buf = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 90)
            .write_image(img.as_raw(), w, h, image::ExtendedColorType::Rgb8)
            .unwrap();
        buf
    }

    #[test]
    fn apply_artwork_syncs_applies_pending_row() {
        let conn = db();
        let tmp = tempfile::tempdir().unwrap();
        let pioneer_dir = tmp.path().join("pioneer");
        std::fs::create_dir_all(&pioneer_dir).unwrap();
        std::fs::copy(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/rekordbox_master.db"
            ),
            pioneer_dir.join("master.db"),
        )
        .unwrap();
        crate::actions::set_pioneer_dir_override_for_test(pioneer_dir.clone());
        let xml_path = pioneer_dir.join("masterPlaylists6.xml");
        std::fs::write(&xml_path, b"<DJ_PLAYLISTS/>").unwrap();
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        // track 40000001's fixture ImagePath: /PIONEER/Artwork/aaaa/artwork.jpg — seed the 3
        // variant files the engine expects to already exist.
        let share_dir = pioneer_dir.join("share/PIONEER/Artwork/aaaa");
        std::fs::create_dir_all(&share_dir).unwrap();
        std::fs::write(share_dir.join("artwork.jpg"), tiny_jpeg(100, 100)).unwrap();
        std::fs::write(share_dir.join("artwork_m.jpg"), tiny_jpeg(50, 50)).unwrap();
        std::fs::write(share_dir.join("artwork_s.jpg"), tiny_jpeg(20, 20)).unwrap();

        let cover_dir = tmp.path().join("covers");
        std::fs::create_dir_all(&cover_dir).unwrap();
        let cover_path = cover_dir.join("new_cover.jpg");
        std::fs::write(&cover_path, tiny_jpeg(300, 300)).unwrap();

        conn.execute(
            "INSERT INTO tracks(path, status) VALUES('D:/FIXTURE/track1.mp3', 'filed')",
            [],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();
        let id = seed_artwork_sync_row(
            &conn,
            track_id,
            "pending",
            Some("40000001"),
            None,
            cover_path.to_str().unwrap(),
        );

        let backup_root = tmp.path().join("backups");
        let outcomes =
            rekordbox_masterdb_apply_artwork_syncs_inner(&conn, &backup_root, &[id]).unwrap();
        assert_eq!(outcomes.len(), 1);
        assert!(
            outcomes[0].ok,
            "expected ok, got error: {:?}",
            outcomes[0].error
        );

        let status: String = conn
            .query_row(
                "SELECT status FROM rekordbox_masterdb_artwork_syncs WHERE id=?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "applied");
        assert_eq!(
            image::image_dimensions(share_dir.join("artwork.jpg")).unwrap(),
            (100, 100)
        );
    }

    #[test]
    fn apply_artwork_syncs_fails_loudly_when_source_cover_file_is_gone() {
        let conn = db();
        let tmp = tempfile::tempdir().unwrap();
        let pioneer_dir = tmp.path().join("pioneer");
        std::fs::create_dir_all(&pioneer_dir).unwrap();
        std::fs::copy(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/rekordbox_master.db"
            ),
            pioneer_dir.join("master.db"),
        )
        .unwrap();
        crate::actions::set_pioneer_dir_override_for_test(pioneer_dir.clone());
        let xml_path = pioneer_dir.join("masterPlaylists6.xml");
        std::fs::write(&xml_path, b"<DJ_PLAYLISTS/>").unwrap();
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        conn.execute(
            "INSERT INTO tracks(path, status) VALUES('D:/FIXTURE/track1.mp3', 'filed')",
            [],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();
        let missing_cover = tmp.path().join("does-not-exist.jpg");
        let id = seed_artwork_sync_row(
            &conn,
            track_id,
            "pending",
            Some("40000001"),
            None,
            missing_cover.to_str().unwrap(),
        );

        let backup_root = tmp.path().join("backups");
        let outcomes =
            rekordbox_masterdb_apply_artwork_syncs_inner(&conn, &backup_root, &[id]).unwrap();
        assert_eq!(outcomes.len(), 1);
        assert!(!outcomes[0].ok);
        assert!(outcomes[0]
            .error
            .as_deref()
            .unwrap()
            .contains("n'existe plus"));

        let status: String = conn
            .query_row(
                "SELECT status FROM rekordbox_masterdb_artwork_syncs WHERE id=?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            status, "pending",
            "a failed row must stay pending, retryable"
        );
    }

    #[test]
    fn apply_artwork_syncs_rejects_ambiguous_row_without_calling_engine() {
        let conn = db();
        let tmp = tempfile::tempdir().unwrap();
        let pioneer_dir = tmp.path().join("pioneer");
        std::fs::create_dir_all(&pioneer_dir).unwrap();
        std::fs::copy(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/rekordbox_master.db"
            ),
            pioneer_dir.join("master.db"),
        )
        .unwrap();
        crate::actions::set_pioneer_dir_override_for_test(pioneer_dir.clone());
        let xml_path = pioneer_dir.join("masterPlaylists6.xml");
        std::fs::write(&xml_path, b"<DJ_PLAYLISTS/>").unwrap();
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        conn.execute(
            "INSERT INTO tracks(path, status) VALUES('D:/a.mp3', 'filed')",
            [],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();
        let id = seed_artwork_sync_row(
            &conn,
            track_id,
            "ambiguous",
            None,
            Some("40000001,40000002"),
            "/cache/a.jpg",
        );

        let backup_root = tmp.path().join("backups");
        let outcomes =
            rekordbox_masterdb_apply_artwork_syncs_inner(&conn, &backup_root, &[id]).unwrap();
        assert_eq!(outcomes.len(), 1);
        assert!(!outcomes[0].ok);
        assert!(outcomes[0].error.as_deref().unwrap().contains("ambigu"));

        let status: String = conn
            .query_row(
                "SELECT status FROM rekordbox_masterdb_artwork_syncs WHERE id=?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "ambiguous", "must not have been touched");
    }

    #[test]
    fn pending_artwork_syncs_excludes_applied_and_dismissed() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(path, status) VALUES('D:/a.mp3', 'filed')",
            [],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();
        seed_artwork_sync_row(
            &conn,
            track_id,
            "pending",
            Some("40000001"),
            None,
            "/cache/a.jpg",
        );

        conn.execute(
            "INSERT INTO tracks(path, status) VALUES('D:/b.mp3', 'filed')",
            [],
        )
        .unwrap();
        let track_id_2 = conn.last_insert_rowid();
        let id2 = seed_artwork_sync_row(
            &conn,
            track_id_2,
            "applied",
            Some("40000002"),
            None,
            "/cache/b.jpg",
        );
        conn.execute(
            "UPDATE rekordbox_masterdb_artwork_syncs SET status='dismissed' WHERE id=?1",
            rusqlite::params![id2],
        )
        .ok();

        let rows = rekordbox_masterdb_pending_artwork_syncs_inner(&conn).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "pending");
        assert_eq!(rows[0].cover_path, "/cache/a.jpg");
        // Sans ligne `metadata`, pas de nom : le front retombe sur le chemin.
        assert_eq!(rows[0].artist, None);
        assert_eq!(rows[0].title, None);
    }

    /// La rangée « Pochettes » de l'écran Rekordbox dit « Artiste — Titre » (2026-09-08) : les deux
    /// viennent de `metadata`, jointe par `track_id`, et manquent sans casser la ligne.
    #[test]
    fn pending_artwork_syncs_carry_artist_and_title_from_metadata() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(path, status) VALUES('D:/a.mp3', 'filed')",
            [],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO metadata(track_id, artist, title) VALUES(?1, 'Olsvangèr & MYKI', 'The Triss')",
            rusqlite::params![track_id],
        )
        .unwrap();
        seed_artwork_sync_row(
            &conn,
            track_id,
            "pending",
            Some("40000001"),
            None,
            "/cache/a.jpg",
        );

        let rows = rekordbox_masterdb_pending_artwork_syncs_inner(&conn).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].artist.as_deref(), Some("Olsvangèr & MYKI"));
        assert_eq!(rows[0].title.as_deref(), Some("The Triss"));
    }

    /// Miroir manuel de `shared/contracts.ts` (§ Architecture, CLAUDE.md) : destructuration
    /// exhaustive, sans `..` — un champ ajouté ici casse la compilation tant que le miroir TS n'a
    /// pas suivi. Ne détecte pas un désaccord de type, seulement un champ manquant.
    #[test]
    fn pending_artwork_sync_shape_matches_contracts_ts() {
        let v = PendingArtworkSync {
            id: 0,
            track_id: 0,
            sift_path: String::new(),
            rekordbox_track_id: None,
            candidate_track_ids: None,
            candidate_tracks: None,
            cover_path: String::new(),
            status: String::new(),
            detected_at: String::new(),
            session_id: None,
            artist: None,
            title: None,
        };
        let PendingArtworkSync {
            id,
            track_id,
            sift_path,
            rekordbox_track_id,
            candidate_track_ids,
            candidate_tracks,
            cover_path,
            status,
            detected_at,
            session_id,
            artist,
            title,
        } = v;
        let _ = (
            id,
            track_id,
            sift_path,
            rekordbox_track_id,
            candidate_track_ids,
            candidate_tracks,
            cover_path,
            status,
            detected_at,
            session_id,
            artist,
            title,
        );
    }

    #[test]
    fn pending_masterdb_repair_shape_matches_contracts_ts() {
        let v = PendingMasterdbRepair {
            id: 0,
            track_id: None,
            candidate_track_ids: None,
            candidate_tracks: None,
            from_path: String::new(),
            to_path: String::new(),
            status: String::new(),
            detected_at: String::new(),
            session_id: None,
            artist: None,
            title: None,
        };
        let PendingMasterdbRepair {
            id,
            track_id,
            candidate_track_ids,
            candidate_tracks,
            from_path,
            to_path,
            status,
            detected_at,
            session_id,
            artist,
            title,
        } = v;
        let _ = (
            id,
            track_id,
            candidate_track_ids,
            candidate_tracks,
            from_path,
            to_path,
            status,
            detected_at,
            session_id,
            artist,
            title,
        );
    }

    #[test]
    fn dismiss_artwork_sync_marks_dismissed() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(path, status) VALUES('D:/a.mp3', 'filed')",
            [],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();
        let id = seed_artwork_sync_row(
            &conn,
            track_id,
            "pending",
            Some("40000001"),
            None,
            "/cache/a.jpg",
        );

        rekordbox_masterdb_dismiss_artwork_sync_inner(&conn, id).unwrap();

        let rows = rekordbox_masterdb_pending_artwork_syncs_inner(&conn).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn resolve_ambiguous_artwork_sync_moves_to_pending() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(path, status) VALUES('D:/a.mp3', 'filed')",
            [],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();
        let id = seed_artwork_sync_row(
            &conn,
            track_id,
            "ambiguous",
            None,
            Some("40000001,40000002"),
            "/cache/a.jpg",
        );

        rekordbox_masterdb_resolve_ambiguous_artwork_sync_inner(&conn, id, "40000002").unwrap();

        let rows = rekordbox_masterdb_pending_artwork_syncs_inner(&conn).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "pending");
        assert_eq!(rows[0].rekordbox_track_id.as_deref(), Some("40000002"));
        assert_eq!(rows[0].candidate_track_ids, None);
    }

    #[test]
    fn resolve_ambiguous_artwork_sync_rejects_track_id_outside_candidate_list() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(path, status) VALUES('D:/a.mp3', 'filed')",
            [],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();
        let id = seed_artwork_sync_row(
            &conn,
            track_id,
            "ambiguous",
            None,
            Some("40000001,40000002"),
            "/cache/a.jpg",
        );

        let err = rekordbox_masterdb_resolve_ambiguous_artwork_sync_inner(&conn, id, "99999999")
            .unwrap_err();
        assert!(err.contains("invalide"));
    }

    #[test]
    fn resolve_ambiguous_artwork_sync_rejects_row_that_is_not_ambiguous() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(path, status) VALUES('D:/a.mp3', 'filed')",
            [],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();
        let id = seed_artwork_sync_row(
            &conn,
            track_id,
            "pending",
            Some("40000001"),
            None,
            "/cache/a.jpg",
        );

        let err = rekordbox_masterdb_resolve_ambiguous_artwork_sync_inner(&conn, id, "40000001")
            .unwrap_err();
        assert!(err.contains("ambigu"));
    }
}
