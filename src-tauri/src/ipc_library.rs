//! IPC surface for the M6b library browser: read-only listing + facets of filed tracks,
//! plus the `update_metadata` command for inline editing in the Bibliothèque.
use crate::library::{self, LibraryFacets, LibraryFilter, LibraryTrack};
use crate::metadata::{self, MetadataEdit};
use rusqlite::Connection;
use std::sync::Mutex;
use tauri::State;

/// Filed tracks joined to metadata + genres, filtered (folder / quality / genre / q).
#[tauri::command]
pub fn list_library(
    conn: State<'_, Mutex<Connection>>,
    filter: Option<LibraryFilter>,
) -> Result<Vec<LibraryTrack>, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    library::list_filed(&conn, &filter.unwrap_or_default()).map_err(|e| e.to_string())
}

/// Folder + genre facet counts for the sidebar.
#[tauri::command]
pub fn library_folders(conn: State<'_, Mutex<Connection>>) -> Result<LibraryFacets, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    library::folder_facets(&conn).map_err(|e| e.to_string())
}

/// Edit a filed track's metadata: writes the file tags first, then updates the DB.
/// If the file write fails the DB is left untouched (no partial state).
#[tauri::command]
pub fn update_metadata(
    conn: State<'_, Mutex<Connection>>,
    track_id: i64,
    edit: MetadataEdit,
) -> Result<(), String> {
    // (1) Look up the track path — error immediately if unknown.
    let path: String = {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT path FROM tracks WHERE id=?1",
            rusqlite::params![track_id],
            |r| r.get(0),
        )
        .map_err(|_| format!("track {track_id} not found"))?
    };

    // (2) Write the file tags. Lock is released before this call; if it fails we stop here and
    // the DB is untouched.
    crate::tagging::write_tags_full(
        &path,
        &edit.artist,
        &edit.title,
        edit.label.as_deref(),
        edit.year,
        &edit.genres,
        edit.cover_path.as_deref(),
    )?;

    // (3) Persist to the DB only after the file write succeeded.
    let conn = conn.lock().map_err(|e| e.to_string())?;
    metadata::update_metadata_db(&conn, track_id, &edit).map_err(|e| e.to_string())
}
