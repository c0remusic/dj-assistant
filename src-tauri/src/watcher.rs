//! Live folder watching. One debounced recursive watcher per source; its ~500 ms settle
//! window also coalesces the burst of events a file move produces (no explicit
//! stability polling — see the M1 design doc). On audio create/modify → upsert pending;
//! on delete → forget the pending row. Each batch emits `queue:changed`.
use crate::scanner;
use notify_debouncer_full::{
    new_debouncer,
    notify::{EventKind, RecursiveMode},
    DebounceEventResult, Debouncer, RecommendedCache,
};
use notify_debouncer_full::notify::RecommendedWatcher;
use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

type Handle = Debouncer<RecommendedWatcher, RecommendedCache>;

/// Map of source_id → live debouncer. Stored in Tauri managed state so handles stay alive.
#[derive(Default)]
pub struct Watchers(pub Mutex<HashMap<i64, Handle>>);

/// Registers the empty watcher state. Call once in setup, before `start_all`.
pub fn init_state(app: &AppHandle) {
    app.manage(Watchers::default());
}

/// Starts (or restarts) watchers for every source currently in the DB.
pub fn start_all(app: &AppHandle) {
    let rows: Vec<(i64, String)> = {
        let state = app.state::<Mutex<Connection>>();
        let Ok(conn) = state.lock() else { return };
        let Ok(mut stmt) = conn.prepare("SELECT id, path FROM sources WHERE watched=1") else {
            return;
        };
        let Ok(rows) = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))) else {
            return;
        };
        rows.filter_map(|r| r.ok()).collect()
    };
    for (id, path) in rows {
        start(app, id, &path);
    }
}

/// Starts a recursive debounced watcher on `path` for `source_id`. Replaces any existing one.
pub fn start(app: &AppHandle, source_id: i64, path: &str) {
    if !std::path::Path::new(path).is_dir() {
        log::warn!("watch skipped, not a dir: {path}");
        return;
    }
    let app2 = app.clone();
    let debouncer = new_debouncer(
        Duration::from_millis(500),
        None,
        move |res: DebounceEventResult| handle_events(&app2, source_id, res),
    );
    let mut debouncer = match debouncer {
        Ok(d) => d,
        Err(e) => {
            log::error!("debouncer init failed for {path}: {e}");
            return;
        }
    };
    if let Err(e) = debouncer.watch(std::path::Path::new(path), RecursiveMode::Recursive) {
        log::error!("watch failed for {path}: {e}");
        return;
    }
    {
        let watchers = app.state::<Watchers>();
        if let Ok(mut map) = watchers.0.lock() {
            map.insert(source_id, debouncer);
        };
    }
}

/// Stops and drops the watcher for `source_id`, if any.
pub fn stop(app: &AppHandle, source_id: i64) {
    {
        let watchers = app.state::<Watchers>();
        if let Ok(mut map) = watchers.0.lock() {
            map.remove(&source_id);
        };
    }
}

/// Applies a debounced batch of FS events to the DB, then notifies the front.
fn handle_events(app: &AppHandle, source_id: i64, res: DebounceEventResult) {
    let events = match res {
        Ok(ev) => ev,
        Err(errs) => {
            for e in errs {
                log::warn!("watch error: {e}");
            }
            return;
        }
    };
    let state = app.state::<Mutex<Connection>>();
    let Ok(conn) = state.lock() else { return };
    let mut touched = false;

    for ev in events {
        // DebouncedEvent derefs to notify::Event, so .kind and .paths are accessible
        for path in &ev.paths {
            match ev.kind {
                EventKind::Create(_) | EventKind::Modify(_) => {
                    if path.is_file() && scanner::is_audio(path) {
                        if let Ok(meta) = path.metadata() {
                            let f = scanner::DiskFile {
                                path: path.to_string_lossy().into_owned(),
                                filename: path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().into_owned())
                                    .unwrap_or_default(),
                                size_bytes: meta.len() as i64,
                                mtime: meta
                                    .modified()
                                    .ok()
                                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                    .map(|d| d.as_secs() as i64)
                                    .unwrap_or(0),
                            };
                            if scanner::upsert_file(&conn, source_id, &f).is_ok() {
                                touched = true;
                            }
                        }
                    }
                }
                EventKind::Remove(_) => {
                    let p = path.to_string_lossy();
                    if let Ok(n) = scanner::forget_path(&conn, &p) {
                        if n > 0 {
                            touched = true;
                        }
                    }
                }
                _ => {}
            }
        }
    }
    drop(conn);
    if touched {
        app.emit("queue:changed", ()).ok();
    }
}
