//! Background analysis worker. A small thread pool drains pending, not-yet-analysed tracks,
//! runs the M2a engine OFF the DB lock, writes the scalar results back, and pings the UI.
//! Distinct from `watcher.rs` (which feeds the queue); this one consumes it.
use crate::analysis::{self, AnalysisReport, Rail, Verdict};
use rusqlite::Connection;
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Condvar, Mutex};
use tauri::{AppHandle, Emitter, Manager};

struct Queue {
    deque: VecDeque<i64>,
    queued: HashSet<i64>, // ids in the deque OR in-flight (prevents double-enqueue)
    running: usize,
    shutdown: bool,
}

/// Managed state: the shared work queue + a condvar the worker threads park on.
pub struct AnalysisWorker {
    inner: Arc<(Mutex<Queue>, Condvar)>,
}

fn rail_str(r: Rail) -> &'static str {
    match r {
        Rail::Lossless => "lossless",
        Rail::Lossy => "lossy",
        Rail::Unknown => "unknown",
    }
}

fn verdict_str(v: Verdict) -> &'static str {
    match v {
        Verdict::Ok => "ok",
        Verdict::Fake => "fake",
        Verdict::Grey => "grey",
    }
}

/// Ids of tracks that still need analysis: pending and either never analysed OR analysed
/// before the report cache existed (report_json NULL) — so every track ends up with a cached
/// report and opening it is always instant. (`persist_failure` sets report_json='' so broken
/// files don't loop here.)
pub fn select_pending(conn: &Connection) -> rusqlite::Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT id FROM tracks WHERE status='pending' AND (analyzed_at IS NULL OR report_json IS NULL) ORDER BY id",
    )?;
    let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
    rows.collect()
}

/// (done, total): total = current pending; done = pending already analysed.
pub fn progress(conn: &Connection) -> rusqlite::Result<(i64, i64)> {
    let total: i64 =
        conn.query_row("SELECT count(*) FROM tracks WHERE status='pending'", [], |r| r.get(0))?;
    let done: i64 = conn.query_row(
        "SELECT count(*) FROM tracks WHERE status='pending' AND analyzed_at IS NOT NULL",
        [],
        |r| r.get(0),
    )?;
    Ok((done, total))
}

/// Writes a full report into the track row and stamps `analyzed_at`.
pub fn persist_report(conn: &Connection, id: i64, r: &AnalysisReport) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE tracks SET
            verdict=?2, cutoff_hz=?3, bitrate=?4, declared_fmt=?5, real_quality=?6, duration=?7,
            clip_runs=?8, clip_pct=?9, true_peak_dbtp=?10, dc_offset=?11, phase_correlation=?12,
            dual_mono=?13, truncated=?14, silence_head_ms=?15, silence_tail_ms=?16,
            container_ok=?17, codec_error=?18, id3_version=?19, has_cover=?20, tags_cdj_ok=?21,
            report_json=?22, analyzed_at=datetime('now')
         WHERE id=?1",
        rusqlite::params![
            id,
            verdict_str(r.verdict),
            r.cutoff_hz,
            r.declared_bitrate,
            r.declared_format,
            rail_str(r.declared_rail),
            r.duration_sec,
            r.clip_runs,
            r.clip_pct,
            r.true_peak_dbtp,
            r.dc_offset,
            r.phase_correlation,
            r.dual_mono as i64,
            r.truncated as i64,
            r.silence_head_ms,
            r.silence_tail_ms,
            r.container_ok as i64,
            r.codec_error,
            r.id3_version,
            r.has_cover as i64,
            r.tags_cdj_ok as i64,
            // cache the report (spectrogram is empty here — computed on demand) for instant re-open
            serde_json::to_string(r).unwrap_or_default(),
        ],
    )?;
    Ok(())
}

/// Marks a track analysed-but-failed so the worker doesn't loop on a broken file.
fn persist_failure(conn: &Connection, id: i64, err: &str) -> rusqlite::Result<()> {
    // Set report_json='' (non-null sentinel) so this broken file isn't re-selected forever
    // by select_pending's `report_json IS NULL` backfill clause.
    conn.execute(
        "UPDATE tracks SET container_ok=0, codec_error=?2, report_json='', analyzed_at=datetime('now') WHERE id=?1",
        rusqlite::params![id, err],
    )?;
    Ok(())
}

/// Starts the worker pool and registers its managed state. Call once in setup, after the DB.
pub fn init(app: &AppHandle) {
    let n = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2)
        .clamp(1, 4);
    let worker = AnalysisWorker {
        inner: Arc::new((
            Mutex::new(Queue {
                deque: VecDeque::new(),
                queued: HashSet::new(),
                running: 0,
                shutdown: false,
            }),
            Condvar::new(),
        )),
    };
    let inner = worker.inner.clone();
    app.manage(worker);
    for _ in 0..n {
        let app2 = app.clone();
        let inner2 = inner.clone();
        std::thread::spawn(move || worker_loop(app2, inner2));
    }
    log::info!("analysis worker pool started ({n} threads)");
}

/// Enqueues every pending, not-yet-analysed track not already queued/in-flight, then wakes
/// the pool. Call at startup and after every `queue:changed`.
pub fn refill(app: &AppHandle) {
    let Some(worker) = app.try_state::<AnalysisWorker>() else { return };
    let ids = {
        let state = app.state::<Mutex<Connection>>();
        let Ok(conn) = state.lock() else { return };
        match select_pending(&conn) {
            Ok(v) => v,
            Err(e) => {
                log::error!("worker refill query failed: {e}");
                return;
            }
        }
    };
    let (m, cv) = &*worker.inner;
    let Ok(mut q) = m.lock() else { return };
    let mut added = 0;
    for id in ids {
        if q.queued.insert(id) {
            q.deque.push_back(id);
            added += 1;
        }
    }
    if added > 0 {
        cv.notify_all();
    }
}

/// Blocks until an id is available (or shutdown). Increments `running` for the popped id.
fn pop(inner: &Arc<(Mutex<Queue>, Condvar)>) -> Option<i64> {
    let (m, cv) = &**inner;
    let mut q = m.lock().ok()?;
    loop {
        if q.shutdown {
            return None;
        }
        if let Some(id) = q.deque.pop_front() {
            q.running += 1;
            return Some(id);
        }
        q = cv.wait(q).ok()?;
    }
}

/// Marks an id done: drops it from `queued` (so a later content-change can re-enqueue it)
/// and decrements `running`.
fn finish(inner: &Arc<(Mutex<Queue>, Condvar)>, id: i64) {
    let (m, _) = &**inner;
    if let Ok(mut q) = m.lock() {
        q.queued.remove(&id);
        q.running = q.running.saturating_sub(1);
    }
}

fn read_path(app: &AppHandle, id: i64) -> Option<String> {
    let state = app.state::<Mutex<Connection>>();
    let conn = state.lock().ok()?;
    conn.query_row("SELECT path FROM tracks WHERE id=?1", rusqlite::params![id], |r| r.get(0))
        .ok()
}

/// Locks the DB briefly and writes the analysis outcome for `id`.
fn persist_result(app: &AppHandle, id: i64, path: &str, result: Result<AnalysisReport, String>) {
    let state = app.state::<Mutex<Connection>>();
    let Ok(conn) = state.lock() else { return };
    let written = match &result {
        Ok(rep) => persist_report(&conn, id, rep),
        Err(e) => {
            log::warn!("analyze failed for {path}: {e}");
            persist_failure(&conn, id, e)
        }
    };
    // Don't drop the write silently: if the DB was busy/locked the track stays
    // analysed_at=NULL and gets picked up again by the next refill (queue:changed/scan),
    // but surface it so a persistent failure is visible rather than invisible.
    if let Err(e) = written {
        log::error!("persist failed for {path} (id {id}), will retry on next refill: {e}");
    }
}

fn worker_loop(app: AppHandle, inner: Arc<(Mutex<Queue>, Condvar)>) {
    while let Some(id) = pop(&inner) {
        if let Some(path) = read_path(&app, id) {
            // Heavy work runs WITHOUT holding the DB lock — UI stays responsive.
            let result = analysis::analyze(&path, false);
            persist_result(&app, id, &path, result);
        }
        finish(&inner, id);
        app.emit("analysis:changed", ()).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{AnalysisReport, Rail, Spectrogram, Verdict};

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::run_migrations(&conn).unwrap();
        conn.execute("INSERT INTO sources (path) VALUES ('root')", []).unwrap();
        conn
    }

    fn add_pending(conn: &Connection, path: &str) -> i64 {
        conn.execute(
            "INSERT INTO tracks (path, source_id, status) VALUES (?1, 1, 'pending')",
            rusqlite::params![path],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn fake_report() -> AnalysisReport {
        AnalysisReport {
            path: "x.flac".into(),
            sample_rate: 44100,
            channels: 2,
            duration_sec: 123.0,
            declared_format: "flac".into(),
            declared_bitrate: Some(900),
            declared_rail: Rail::Lossless,
            cutoff_hz: 16000.0,
            verdict: Verdict::Fake,
            peaks: vec![],
            spectrogram: Spectrogram { frames: 0, bins: 0, hz_per_bin: 0.0, sec_per_frame: 0.0, mag_db: vec![] },
            clip_runs: 2,
            clip_pct: 1.5,
            true_peak_dbtp: -0.3,
            dc_offset: 0.001,
            phase_correlation: 0.8,
            dual_mono: false,
            container_ok: true,
            codec_error: None,
            truncated: false,
            silence_head_ms: 10,
            silence_tail_ms: 20,
            id3_version: Some("ID3".into()),
            tags_cdj_ok: true,
            has_cover: true,
        }
    }

    #[test]
    fn select_pending_returns_unanalysed_or_uncached() {
        let conn = db();
        let a = add_pending(&conn, "a.flac"); // never analysed → selected
        let b = add_pending(&conn, "b.flac"); // analysed + report cached → NOT selected
        let c = add_pending(&conn, "c.flac"); // filed → NOT selected
        let d = add_pending(&conn, "d.flac"); // analysed but no report cache → selected (backfill)
        conn.execute("UPDATE tracks SET analyzed_at=datetime('now'), report_json='{}' WHERE id=?1", [b]).unwrap();
        conn.execute("UPDATE tracks SET status='filed' WHERE id=?1", [c]).unwrap();
        conn.execute("UPDATE tracks SET analyzed_at=datetime('now') WHERE id=?1", [d]).unwrap();
        assert_eq!(select_pending(&conn).unwrap(), vec![a, d]);
    }

    #[test]
    fn persist_report_writes_columns_and_marks_analysed() {
        let conn = db();
        let id = add_pending(&conn, "x.flac");
        persist_report(&conn, id, &fake_report()).unwrap();
        let (verdict, cutoff, dual, analyzed): (String, f64, i64, Option<String>) = conn
            .query_row(
                "SELECT verdict, cutoff_hz, dual_mono, analyzed_at FROM tracks WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(verdict, "fake");
        assert!((cutoff - 16000.0).abs() < 1e-3);
        assert_eq!(dual, 0);
        assert!(analyzed.is_some(), "analyzed_at stamped");
        // and it leaves select_pending empty now
        assert!(select_pending(&conn).unwrap().is_empty());
    }

    #[test]
    fn progress_counts_done_over_total() {
        let conn = db();
        let _a = add_pending(&conn, "a.flac");
        let b = add_pending(&conn, "b.flac");
        persist_report(&conn, b, &fake_report()).unwrap();
        assert_eq!(progress(&conn).unwrap(), (1, 2));
    }
}
