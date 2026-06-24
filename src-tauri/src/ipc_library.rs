//! IPC surface for the M6b library browser: read-only listing + facets of filed tracks.
use crate::library::{self, LibraryFacets, LibraryFilter, LibraryTrack};
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
