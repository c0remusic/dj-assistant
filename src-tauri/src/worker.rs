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

pub(crate) fn verdict_str(v: Verdict) -> &'static str {
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
///
/// `typeof(report_json)='null'` rather than `report_json IS NULL`: identical result, but `IS NULL`
/// forces SQLite to load an ~800 KB value per row just to find out it is absent (see queue.rs
/// list_pending for the measurement). This runs on every `queue:changed`, so it is the same hot
/// path, and the sentinel test still works — `''` is text, never `'null'`.
///
/// Troisième clause depuis l'issue #39 : **un verdict présent mais périmé** vaut « à re-analyser ».
/// C'est la lecture qui va avec `verdict_ver`, sur le modèle d'`ipc::analyze_path` — désaccord de
/// version = défaut de cache, on recalcule. Sans elle, la version serait détectable et jamais
/// réparée : `queue::list_pending` afficherait « non analysé » pour toujours sur des pistes que le
/// pool ne reprendrait jamais.
///
/// Deux bornes, et elles sont délibérées :
///
/// - `verdict IS NOT NULL` : une piste SANS verdict n'est pas périmée, elle est non analysée — et
///   surtout, `persist_failure` laisse exactement cet état (verdict NULL, `verdict_ver` NULL,
///   `report_json=''`). Sans ce garde, un fichier illisible redeviendrait éligible à chaque
///   passage, échouerait à chaque fois, et `analysis_attempts` grimperait tout seul jusqu'au seuil
///   terminal. C'est le piège que la sentinelle `''` existe pour éviter.
/// - `status='pending'` (déjà là) : la bibliothèque RANGÉE n'est jamais reprise ici. Invalider
///   3907 pistes rangées d'un bump est ce qu'a coûté la v16 ; cette décision-là appartient au jour
///   du bump, pas à ce filet.
pub fn select_pending(conn: &Connection) -> rusqlite::Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT id FROM tracks \
         WHERE status='pending' \
           AND (analyzed_at IS NULL OR typeof(report_json)='null' \
                OR (verdict IS NOT NULL AND verdict_ver IS NOT ?1)) \
         ORDER BY id",
    )?;
    let rows = stmt.query_map(
        rusqlite::params![analysis::verdict::VERDICT_CACHE_VERSION],
        |r| r.get::<_, i64>(0),
    )?;
    rows.collect()
}

/// (done, total): total = current pending; done = pending already analysed.
pub fn progress(conn: &Connection) -> rusqlite::Result<(i64, i64)> {
    // « done » = le complément EXACT de la clause de `refill` (ci-dessus) : une piste est faite
    // quand le pool n'a plus rien à lui faire. Jusqu'au 2026-09-06, done ne regardait que
    // `analyzed_at IS NOT NULL` — les re-analyses que refill sélectionne pourtant
    // (`report_json` nul, `verdict_ver` périmée après un bump de VERDICT_CACHE_VERSION, comme la
    // masse post-#52) étaient invisibles du signal : `analysis_progress` disait « repos » pendant
    // que le pool tournait sur des centaines de pistes, et la zone de progression comme la rangée
    // du pied de file affichaient un compte inerte au lieu d'une progression (observé par Antoine
    // sur la vraie fenêtre, 846 « non analysées » décroissantes avec progress à 3372/3372).
    let total: i64 = conn.query_row(
        "SELECT count(*) FROM tracks WHERE status='pending'",
        [],
        |r| r.get(0),
    )?;
    let done: i64 = conn.query_row(
        "SELECT count(*) FROM tracks \
         WHERE status='pending' \
           AND NOT (analyzed_at IS NULL OR typeof(report_json)='null' \
                    OR (verdict IS NOT NULL AND verdict_ver IS NOT ?1))",
        rusqlite::params![crate::analysis::verdict::VERDICT_CACHE_VERSION],
        |r| r.get(0),
    )?;
    Ok((done, total))
}

/// Writes a full report into the track row and stamps `analyzed_at`. `report_json` is the
/// ALREADY-SERIALISED report: `serde_json::to_string` on a full `AnalysisReport` (display
/// spectrogram included) is the heavy part of this write and needs no DB state, so the caller
/// does it BEFORE taking the connection mutex — same plan/execute/commit split as
/// `ipc_filing::apply_tags`. Passing it in rather than recomputing it here is what keeps that
/// work provably outside the lock.
pub fn persist_report(
    conn: &Connection,
    id: i64,
    r: &AnalysisReport,
    report_json: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE tracks SET
            verdict=?2, cutoff_hz=?3, bitrate=?4, declared_fmt=?5, real_quality=?6, duration=?7,
            clip_runs=?8, clip_pct=?9, true_peak_dbtp=?10, dc_offset=?11, phase_correlation=?12,
            dual_mono=?13, truncated=?14, silence_head_ms=?15, silence_tail_ms=?16,
            container_ok=?17, codec_error=?18, id3_version=?19, has_cover=?20, tags_cdj_ok=?21,
            report_json=?22, report_cache_ver=?23, verdict_ver=?24, analyzed_at=datetime('now')
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
            // Cache le rapport SANS la grille de spectrogramme : verdict, pics de waveform et
            // métadonnées, c'est-à-dire ce qui doit être instantané à la ré-ouverture. La grille,
            // elle, se recalcule à l'ouverture du collapse Diagnostic — décision du 2026-08-03,
            // chiffrée dans le commentaire de `worker_loop` (~450 ko par piste contre 631 ms
            // gagnées ; base passée de 4,11 Go à 119 Mo). Sérialisé par l'appelant, hors du lock.
            report_json,
            analysis::REPORT_CACHE_VERSION,
            // Dans le MÊME UPDATE que `verdict`, et c'est tout l'intérêt : les deux ne peuvent pas
            // diverger à l'écriture. Ce qui les faisait diverger APRÈS, c'est
            // `ipc::analyze_path`, qui répare `report_json`/`report_cache_ver` sans toucher au
            // verdict — d'où une constante distincte (issue #39, `verdict::VERDICT_CACHE_VERSION`).
            analysis::verdict::VERDICT_CACHE_VERSION,
        ],
    )?;
    Ok(())
}

/// Marks a track analysed-but-failed so the worker doesn't loop on a broken file.
fn persist_failure(conn: &Connection, id: i64, err: &str) -> rusqlite::Result<()> {
    // Set report_json='' (non-null sentinel) so this broken file isn't re-selected forever
    // by select_pending's `report_json IS NULL` backfill clause.
    // Also clear `verdict` (review-caught bug: this UPDATE used to leave it untouched — a track
    // that had a real verdict from a PRIOR successful analysis, then had its content change
    // (scanner.rs resets analyzed_at/report_json but keeps the old verdict), then failed on
    // re-analysis here, kept displaying that stale verdict as if it were still current/valid,
    // and `queue::QueueItem::needs_analysis` — verdict-aware specifically so a failure is never
    // silently invisible — had no way to tell the two apart). Invariant this restores: `verdict`
    // is non-NULL if and only if it reflects the CURRENT file's most recent successful analysis.
    //
    // Also NULL every derived analysis column a PRIOR successful run may have written (real_quality
    // drives the queue rail, plus cutoff/bitrate/loudness/tag facts): same staleness class as the
    // verdict — a failed re-analysis must not leave the old file's measurements presented as if they
    // were the current file's. Keep only the failure markers (container_ok=0, codec_error). Bump
    // analysis_attempts so a permanently-broken file eventually reaches a terminal state (frontend
    // threshold MAX_ANALYSIS_ATTEMPTS, shared/contracts.ts) instead of inflating "Non analysés (N)".
    conn.execute(
        "UPDATE tracks SET
            verdict=NULL, container_ok=0, codec_error=?2, report_json='',
            analyzed_at=datetime('now'), analysis_attempts=analysis_attempts+1,
            real_quality=NULL, cutoff_hz=NULL, bitrate=NULL, declared_fmt=NULL, duration=NULL,
            clip_runs=NULL, clip_pct=NULL, true_peak_dbtp=NULL, dc_offset=NULL,
            phase_correlation=NULL, dual_mono=NULL, truncated=NULL, silence_head_ms=NULL,
            silence_tail_ms=NULL, id3_version=NULL, has_cover=NULL, tags_cdj_ok=NULL,
            report_cache_ver=NULL, verdict_ver=NULL
         WHERE id=?1",
        rusqlite::params![id, err],
    )?;
    Ok(())
}

/// Taille du pool d'analyse. Extraite de `init` pour que `bench_cpu_budget` mesure la VRAIE
/// formule de production au lieu d'une copie qui dériverait en silence — c'est exactement le
/// défaut relevé par l'audit du 2026-08-05 sur la ligne 6 du diagnostic architectural.
/// Son pendant côté encodage est `ipc_filing::phase2_worker_count`.
pub(crate) fn analysis_pool_size() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2)
        .clamp(1, 8)
}

/// Starts the worker pool and registers its managed state. Call once in setup, after the DB.
pub fn init(app: &AppHandle) {
    let n = analysis_pool_size();
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
    let Some(worker) = app.try_state::<AnalysisWorker>() else {
        return;
    };
    let ids = {
        let state = app.state::<Mutex<Connection>>();
        let Ok(conn) = state.lock() else {
            log::error!("worker refill: DB connection mutex poisoned, skipping refill");
            return;
        };
        match select_pending(&conn) {
            Ok(v) => v,
            Err(e) => {
                log::error!("worker refill query failed: {e}");
                return;
            }
        }
    };
    let (m, cv) = &*worker.inner;
    // Le pool tourne sans supervision : un verrou empoisonné avalé ici l'arrête d'alimenter en
    // silence, et l'analyse a l'air simplement « terminée » (`.claude/rules/rust.md`).
    let mut q = match m.lock() {
        Ok(q) => q,
        Err(e) => {
            log::error!("worker refill: verrou de file empoisonné, aucun id enfilé: {e}");
            return;
        }
    };
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
    // `None` fait sortir le thread de sa boucle DÉFINITIVEMENT — il n'est jamais relancé. C'est le
    // rétrécissement silencieux du pool décrit dans `.claude/rules/rust.md` : sans trace, il ne
    // reste qu'une analyse qui n'avance plus.
    let mut q = match m.lock() {
        Ok(q) => q,
        Err(e) => {
            log::error!("worker pop: verrou de file empoisonné, ce thread s'arrête: {e}");
            return None;
        }
    };
    loop {
        if q.shutdown {
            return None;
        }
        if let Some(id) = q.deque.pop_front() {
            q.running += 1;
            return Some(id);
        }
        q = match cv.wait(q) {
            Ok(q) => q,
            Err(e) => {
                log::error!("worker pop: attente sur condvar empoisonnée, ce thread s'arrête: {e}");
                return None;
            }
        };
    }
}

/// Marks an id done: drops it from `queued` (so a later content-change can re-enqueue it)
/// and decrements `running`.
fn finish(inner: &Arc<(Mutex<Queue>, Condvar)>, id: i64) {
    let (m, _) = &**inner;
    match m.lock() {
        Ok(mut q) => {
            q.queued.remove(&id);
            q.running = q.running.saturating_sub(1);
        }
        // `running` reste alors compté à vie et l'id ne peut plus jamais être ré-enfilé : la piste
        // devient invisible à toute nouvelle analyse.
        Err(e) => log::error!(
            "worker finish({id}): verrou de file empoisonné, l'id reste marqué en cours: {e}"
        ),
    }
}

fn read_path(app: &AppHandle, id: i64) -> Option<String> {
    let state = app.state::<Mutex<Connection>>();
    let conn = match state.lock() {
        Ok(conn) => conn,
        Err(_) => {
            log::error!("worker read_path({id}): DB connection mutex poisoned");
            return None;
        }
    };
    conn.query_row(
        "SELECT path FROM tracks WHERE id=?1",
        rusqlite::params![id],
        |r| r.get(0),
    )
    .ok()
}

/// Locks the DB briefly and writes the analysis outcome for `id`.
fn persist_result(app: &AppHandle, id: i64, path: &str, result: Result<AnalysisReport, String>) {
    // Serialise the report BEFORE taking the lock. It is pure CPU over an owned value (no DB
    // state is read, so nothing can change under us) and it is not small: measured 2026-07-27 on
    // the production DB, a report averaged 1657 KB of JSON. Doing it under the global connection
    // mutex stalled every other DB user — the whole analysis pool and every IPC command — for
    // work that never needed the lock. `unwrap_or_default()` behaviour is unchanged.
    let report_json = match &result {
        Ok(rep) => serde_json::to_string(rep).unwrap_or_default(),
        // persist_failure writes its own '' sentinel; nothing to pre-serialise here.
        Err(_) => String::new(),
    };
    let state = app.state::<Mutex<Connection>>();
    let Ok(conn) = state.lock() else {
        log::error!(
            "worker persist_result({id}, {path}): DB connection mutex poisoned, result lost"
        );
        return;
    };
    let written = match &result {
        Ok(rep) => persist_report(&conn, id, rep, &report_json),
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
            //
            // `false` = ne PAS collecter la grille d'affichage. FIX-3 (2026-07) l'avait mise à
            // `true` pour que le clic sur le spectrogramme n'ait jamais à redécoder, puis
            // l'encodage base85 (v16) a été présenté comme LE correctif de taille. Mesuré sur la
            // base de production le 2026-08-03, après base85 : 2 714 pistes, 817 ko de rapport
            // moyen, dont 1,21 Go de spectrogrammes — 4,11 Go de fichier pour 7,6 Mo de données
            // réelles. L'encodage a divisé le gaspillage, il ne l'a pas supprimé.
            //
            // Le marché refusé, une fois ses deux côtés chiffrés : le cache coûtait ~450 ko par
            // piste et faisait gagner 631 ms à l'ouverture du collapse
            // (`bench_sqlite::bench_analysis_cost_on_real_tracks`). Il est recalculé à la demande,
            // ce que `report-view.ts::wireSpectrogram` sait déjà faire — le chemin existait,
            // il n'était simplement jamais emprunté. Le verdict et la waveform, eux, restent en
            // cache : ce sont eux qui doivent être instantanés.
            //
            // Note utile si ce flag retente `true` : la FFT tourne de toute façon (le verdict en
            // dépend, voir `SpectrumAccumulator::new`). Ce flag ne décide que de la CONSERVATION
            // de la grille, pas de son calcul — d'où les 91 ms d'écart seulement entre les deux.
            //
            // analyze() decodes arbitrary user-supplied audio files (Symphonia/FFT); catch a
            // panic here so one corrupt file doesn't silently kill this pool thread forever.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                analysis::analyze(&path, false)
            }))
            .unwrap_or_else(|payload| {
                let msg = payload
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "unknown panic".to_string());
                log::error!("analyze panicked for {path} (id {id}): {msg}");
                Err(format!("analysis panicked: {msg}"))
            });
            persist_result(&app, id, &path, result);
        }
        finish(&inner, id);
        app.emit("analysis:changed", ()).ok();
    }
}

#[cfg(test)]
// `pub(crate)` pour que `fake_report` serve aussi aux tests d'`ipc.rs` : construire un
// `AnalysisReport` à la main coûte 25 champs, et deux copies divergeraient au premier champ ajouté.
pub(crate) mod tests {
    use super::*;
    use crate::analysis::{AnalysisReport, Rail, Spectrogram, Verdict};

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::run_migrations(&conn).unwrap();
        conn.execute("INSERT INTO sources (path) VALUES ('root')", [])
            .unwrap();
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

    pub(crate) fn fake_report() -> AnalysisReport {
        AnalysisReport {
            path: "x.flac".into(),
            sample_rate: 44100,
            channels: 2,
            duration_sec: 123.0,
            hf_flatness_db: Some(-3.0),
            hf_flatness_top_db: Some(-3.0),
            decoded_duration_sec: 123.0,
            quant_likelihood: None,
            declared_format: "flac".into(),
            declared_bitrate: Some(900),
            declared_rail: Rail::Lossless,
            cutoff_hz: 16000.0,
            verdict: Verdict::Fake,
            container_mismatch: false,
            est_kbps: 128,
            peaks: vec![],
            peaks_step: 512,
            spectrogram: Spectrogram {
                frames: 0,
                bins: 0,
                hz_per_bin: 0.0,
                sec_per_frame: 0.0,
                mag_db: vec![],
            },
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
            // Valeur que la PRODUCTION émet depuis le 2026-09-01 (`tags.rs::read` : nom de type
            // `lofty`). Le seed portait « ID3 », le stub que ce chantier a justement supprimé.
            id3_version: Some("Id3v2".into()),
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
        conn.execute(
            "UPDATE tracks SET analyzed_at=datetime('now'), report_json='{}' WHERE id=?1",
            [b],
        )
        .unwrap();
        conn.execute("UPDATE tracks SET status='filed' WHERE id=?1", [c])
            .unwrap();
        conn.execute(
            "UPDATE tracks SET analyzed_at=datetime('now') WHERE id=?1",
            [d],
        )
        .unwrap();
        assert_eq!(select_pending(&conn).unwrap(), vec![a, d]);
    }

    /// La lecture qui répare le verdict : un verdict PRÉSENT dont la version a été distancée
    /// redevient éligible à l'analyse (issue #39) — et un échec de décodage, lui, ne l'est
    /// TOUJOURS pas.
    ///
    /// Ce second cas porte tout le risque, et c'est pourquoi la clause exige `verdict IS NOT NULL`.
    /// `persist_failure` laisse verdict NULL, `verdict_ver` NULL et `report_json=''` : sans ce
    /// garde, cette ligne serait « périmée » à chaque passage, le fichier illisible serait repris
    /// en boucle, le décodage échouerait à chaque fois et `analysis_attempts` grimperait tout seul
    /// jusqu'au seuil terminal. C'est exactement le piège que la sentinelle `''` existe pour
    /// éviter, et une version de cache mal gardée le rouvrirait par l'autre bout.
    ///
    /// La mutation se fait sur la ligne, pas sur la `const` : écrire `VERDICT_CACHE_VERSION + 1`
    /// place la ligne dans l'état qu'elle aurait le jour d'un bump. Relatif, donc encore vrai
    /// après ce bump.
    #[test]
    fn select_pending_reprend_un_verdict_perime_mais_jamais_un_echec_de_decodage() {
        let conn = db();
        let courant = add_pending(&conn, "courant.flac");
        let perime = add_pending(&conn, "perime.flac");
        let jamais_stampe = add_pending(&conn, "prev22.flac");
        let casse = add_pending(&conn, "casse.mp3");

        // Trois pistes analysées avec succès : rapport en cache, verdict écrit.
        let stampe = |id: i64, ver: Option<i64>| {
            conn.execute(
                "UPDATE tracks SET analyzed_at=datetime('now'), report_json='{}', \
                 verdict='ok', verdict_ver=?2 WHERE id=?1",
                rusqlite::params![id, ver],
            )
            .unwrap();
        };
        stampe(courant, Some(analysis::verdict::VERDICT_CACHE_VERSION));
        stampe(perime, Some(analysis::verdict::VERDICT_CACHE_VERSION + 1));
        stampe(jamais_stampe, None); // ligne d'avant la v22 que le backfill n'a pas atteinte
                                     // Et une piste dans l'état exact que laisse `persist_failure`.
        conn.execute(
            "UPDATE tracks SET analyzed_at=datetime('now'), report_json='', verdict=NULL, \
             verdict_ver=NULL, analysis_attempts=1 WHERE id=?1",
            rusqlite::params![casse],
        )
        .unwrap();

        let selected = select_pending(&conn).unwrap();
        assert!(
            !selected.contains(&courant),
            "un verdict à la version courante n'a aucune raison d'être recalculé"
        );
        assert!(
            selected.contains(&perime),
            "un verdict d'un autre moteur doit être repris — sinon la version est détectable \
             et jamais réparée, et la file affiche « non analysé » pour toujours"
        );
        assert!(
            selected.contains(&jamais_stampe),
            "une version absente vaut une version différente : défaut de cache"
        );
        assert!(
            !selected.contains(&casse),
            "un fichier illisible serait repris en boucle et brûlerait ses analysis_attempts"
        );
    }

    #[test]
    fn persist_report_writes_columns_and_marks_analysed() {
        let conn = db();
        let id = add_pending(&conn, "x.flac");
        let r = fake_report();
        persist_report(&conn, id, &r, &serde_json::to_string(&r).unwrap()).unwrap();
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
        // `verdict_ver` part dans le MÊME UPDATE que `verdict` : c'est l'invariant qui rend la
        // colonne fiable. L'oublier laisserait la ligne périmée dès l'écriture, donc reprise à
        // chaque refill — ce que l'assertion suivante attraperait aussi, mais sans le nommer.
        let ver: Option<i64> = conn
            .query_row("SELECT verdict_ver FROM tracks WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(ver, Some(analysis::verdict::VERDICT_CACHE_VERSION));
        // and it leaves select_pending empty now
        assert!(select_pending(&conn).unwrap().is_empty());
    }

    #[test]
    fn persist_failure_clears_a_stale_verdict_from_a_prior_success() {
        // Review-caught bug: a track that succeeded once (real verdict), then had its content
        // change (scanner.rs resets analyzed_at/report_json but leaves the old verdict), then
        // failed on re-analysis, used to keep displaying that now-STALE verdict as if it were
        // still current — persist_failure never touched the `verdict` column. Invariant this
        // test locks in: verdict is non-NULL iff it reflects the file's most recent successful
        // analysis, never a leftover from before a later failure.
        let conn = db();
        let id = add_pending(&conn, "x.flac");
        let r = fake_report();
        persist_report(&conn, id, &r, &serde_json::to_string(&r).unwrap()).unwrap(); // first pass: succeeds, verdict="fake"
        conn.execute(
            "UPDATE tracks SET analyzed_at=NULL, report_json=NULL WHERE id=?1", // content changed
            [id],
        )
        .unwrap();
        persist_failure(&conn, id, "decode error").unwrap(); // second pass: fails

        let verdict: Option<String> = conn
            .query_row("SELECT verdict FROM tracks WHERE id=?1", [id], |r| r.get(0))
            .unwrap();
        assert_eq!(
            verdict, None,
            "a failed re-analysis must clear the old verdict, not leave it stale"
        );
    }

    #[test]
    fn persist_failure_bumps_attempts_and_clears_stale_derived_columns() {
        // A track analysed OK once (real_quality set, drives the queue rail), then its content
        // changes and re-analysis fails: the old rail/measurements must not survive as if current,
        // and the attempt counter must climb toward the terminal MAX_ANALYSIS_ATTEMPTS.
        let conn = db();
        let id = add_pending(&conn, "x.flac");
        let r = fake_report();
        persist_report(&conn, id, &r, &serde_json::to_string(&r).unwrap()).unwrap(); // real_quality="lossless", etc.

        persist_failure(&conn, id, "decode error").unwrap();
        let (rq, attempts): (Option<String>, i64) = conn
            .query_row(
                "SELECT real_quality, analysis_attempts FROM tracks WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            rq, None,
            "stale rail (real_quality) must be cleared on failure"
        );
        assert_eq!(
            attempts, 1,
            "each failed analysis increments the attempt counter"
        );

        persist_failure(&conn, id, "decode error again").unwrap();
        let attempts2: i64 = conn
            .query_row(
                "SELECT analysis_attempts FROM tracks WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            attempts2, 2,
            "attempts accumulate across successive failures"
        );
    }

    #[test]
    fn progress_counts_done_over_total() {
        let conn = db();
        let _a = add_pending(&conn, "a.flac");
        let b = add_pending(&conn, "b.flac");
        let r = fake_report();
        persist_report(&conn, b, &r, &serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(progress(&conn).unwrap(), (1, 2));
    }

    /// « done » est le complément exact de la clause de `refill` : une piste que le pool va
    /// re-traiter (verdict_ver périmée après un bump de VERDICT_CACHE_VERSION) ne compte PAS
    /// comme faite — jusqu'au 2026-09-06 elle comptait, et `analysis_progress` disait « repos »
    /// pendant une re-analyse de masse (celle du 3e signal MDCT, #52, observée sur 846 pistes).
    #[test]
    fn progress_counts_a_stale_verdict_ver_as_not_done() {
        let conn = db();
        let a = add_pending(&conn, "a.flac");
        let r = fake_report();
        persist_report(&conn, a, &r, &serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(progress(&conn).unwrap(), (1, 1), "fraîche = faite");
        conn.execute(
            "UPDATE tracks SET verdict_ver = verdict_ver - 1 WHERE id=?1",
            rusqlite::params![a],
        )
        .unwrap();
        assert_eq!(
            progress(&conn).unwrap(),
            (0, 1),
            "verdict_ver périmée = le pool va la reprendre, elle n'est pas faite"
        );
    }
}
