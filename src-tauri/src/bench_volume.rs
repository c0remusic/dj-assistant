//! Phase 3 measurement-only benchmark (docs/superpowers/specs/2026-07-13-architecture-evolution-design.md,
//! section 6). Measures real latency of `library::list_filed` / `queue::list_pending` at 15k and
//! 100k rows on a synthetic-but-varied dataset, plus `EXPLAIN QUERY PLAN` and JSON serialization
//! size — no pagination code, no production behaviour change. Compiled only in test builds
//! (`#[cfg(test)] mod bench_volume;` in lib.rs), never shipped in the release binary.
//!
//! Extended 2026-07-27 (tranche P1 of docs/superpowers/changes/2026-07-27-perf-fixes/PRD.md) with
//! the FILING LOOP — the gestures D3 budgets at < 50 ms (PRD.md:78: destination bins listing,
//! final-name preview, moving to the next track, and the acknowledgement of the Convertir click)
//! — measured at the D1 target volume of 15 000 tracks, on a real on-disk library root. Nothing
//! measured here belongs to the < 100 ms class (PRD.md:79 = hover, selection, filter typing,
//! returning to an already-visited screen), so no number below may be judged against 100 ms.
//! Same shape as the existing block: synthetic seed, timed repetitions, `EXPLAIN QUERY PLAN`; no
//! criterion, no production behaviour change.
//!
//! Run with: `cargo test --release -- --ignored --nocapture bench_volume`
//! (release build matters: these numbers are meaningless in an unoptimised debug build).
//!
//! Add `--test-threads=1` as soon as more than one bench is selected: since P1 there are TWO
//! `#[ignore]`d benchmarks in this file, and the default harness runs them on parallel threads —
//! their output interleaves AND they contend for CPU/disk, inflating both by a measurable margin
//! (list_pending 16 ms → 19 ms, find_duplicate 56 ms → 59 ms, observed 2026-07-27). Every number
//! quoted from this file must say which of the two modes produced it.
//! Note also that `-- --ignored` selects the OTHER ignored tests of the crate too, including the
//! `*_on_real_masterdb_copy` ones, which fail unless `SIFT_M8_REAL_COPY_DIR` points at a manual
//! copy of a real Rekordbox master.db — unrelated to these benchmarks.

use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::library::{self, LibraryFilter};
use crate::queue;

// ── synthetic dataset ────────────────────────────────────────────────────────

const FIRST_NAMES: [&str; 25] = [
    "Marcus", "Aya", "Kaito", "Solene", "Devon", "Nadia", "Ronan", "Priya", "Elias", "Yuki",
    "Camille", "Tobias", "Zara", "Idris", "Mireille", "Kenji", "Anouk", "Femi", "Leila", "Ozzy",
    "Ingrid", "Malik", "Rosa", "Viggo", "Chidi",
];
const LAST_NAMES: [&str; 20] = [
    "Reyes",
    "Okafor",
    "Lindqvist",
    "Dubois",
    "Kowalski",
    "Haddad",
    "Moreau",
    "Nakamura",
    "Ferreira",
    "Novak",
    "Adeyemi",
    "Bianchi",
    "Larsen",
    "Petrov",
    "Costa",
    "Herrera",
    "Weiss",
    "Diallo",
    "Kimura",
    "Santos",
];

/// Genre pool. "House" is index 0 so the genre-filter benchmark can target a value guaranteed
/// to exist in the dataset without depending on the assignment formula below.
const GENRES: [&str; 30] = [
    "House",
    "Deep House",
    "Tech House",
    "Techno",
    "Minimal",
    "Melodic Techno",
    "Disco",
    "Nu Disco",
    "Funk",
    "Soul",
    "Drum and Bass",
    "Jungle",
    "Breakbeat",
    "Garage",
    "UK Garage",
    "Dubstep",
    "Ambient",
    "Downtempo",
    "Trance",
    "Progressive House",
    "Electro",
    "Acid House",
    "Italo Disco",
    "Boogie",
    "Afrobeat",
    "Amapiano",
    "Jazz Fusion",
    "Trip Hop",
    "IDM",
    "Hard Techno",
];

fn artist_for(i: usize) -> String {
    let first = FIRST_NAMES[(i * 7 + 3) % FIRST_NAMES.len()];
    let last = LAST_NAMES[(i * 11 + 5) % LAST_NAMES.len()];
    format!("{first} {last}")
}

/// Format pool, weighted toward mp3 like a real catalogue: 3/5 mp3, 1/5 each aiff/flac/wav.
fn format_for(i: usize) -> &'static str {
    match i % 5 {
        0 => "aiff",
        1 => "flac",
        2 => "wav",
        _ => "mp3",
    }
}

fn bitrate_for(format: &str, i: usize) -> i64 {
    if format == "mp3" {
        [128, 192, 256, 320][i % 4]
    } else {
        1411
    }
}

/// ~80% ok, ~10% fake, ~10% grey.
fn verdict_for(i: usize) -> &'static str {
    match i % 10 {
        0 => "fake",
        1 => "grey",
        _ => "ok",
    }
}

/// 1-3 genres per track, indices spaced by 10 (mod 30) so they're always distinct — avoids a
/// PRIMARY KEY(track_id, genre) collision when g_count == 3.
fn genres_for(i: usize) -> Vec<&'static str> {
    let base = (i * 7) % GENRES.len();
    let count = 1 + (i % 3);
    (0..count)
        .map(|g| GENRES[(base + g * 10) % GENRES.len()])
        .collect()
}

/// Inserts `n` synthetic tracks, `filed_fraction` of them 'filed' and the rest 'pending'
/// (interleaved via `i % 1000`, not a block split, so the two statuses are spread across the
/// id range like a real catalogue rather than clustered). Metadata + genres inserted alongside,
/// batched in one transaction for insert speed (irrelevant to the measured queries, but keeps
/// dataset setup itself fast).
///
/// `filed_fraction` is NOT fixed at the originally-planned 50/50: see
/// `debug_print_sqlite_variable_limit` and the "SQLite bound-parameter limit" finding in
/// docs/superpowers/plans/2026-07-14-phase3-measurement-report.md — `library::list_filed`'s
/// batched genre lookup (`genres::get_genres_batch`, one bound `?` per filed row in a single
/// `IN (...)`) hits SQLite's 32766-parameter limit and returns an `Err` once the filed set is
/// close to/above that size. The main benchmark run uses a fraction low enough to stay under it
/// at both volumes (comparable across 15k/100k); a separate block reproduces the crash directly
/// at a realistic 50% to document it with a real error message.
fn seed_dataset(conn: &mut Connection, n: usize, filed_fraction: f64) -> rusqlite::Result<()> {
    let filed_permille = (filed_fraction * 1000.0).round() as usize;
    let tx = conn.transaction()?;
    {
        // `verdict_ver` semée avec le verdict : sans elle, la base de mesure porterait 15 000
        // verdicts que `verdict::cached` efface à la lecture, et le compte « À re-sourcer » qu'on
        // vient mesurer rendrait 0 (issue #39).
        let mut ins_track = tx.prepare(
            "INSERT INTO tracks
                (id, path, format, bitrate, duration, verdict, verdict_ver, status, folder, has_cover, filename)
             VALUES (?1,?2,?3,?4,?5,?6,?11,?7,?8,?9,?10)",
        )?;
        let mut ins_meta = tx.prepare(
            "INSERT INTO metadata (track_id, artist, title, label, year, bpm)
             VALUES (?1,?2,?3,?4,?5,?6)",
        )?;
        let mut ins_genre =
            tx.prepare("INSERT INTO track_genres (track_id, genre, ord) VALUES (?1,?2,?3)")?;

        for i in 0..n {
            let id = (i + 1) as i64;
            let format = format_for(i);
            let bitrate = bitrate_for(format, i);
            let duration = 120.0 + (i % 300) as f64 * 1.3;
            let verdict = verdict_for(i);
            let status = if (i % 1000) < filed_permille {
                "filed"
            } else {
                "pending"
            };
            let folder = if status == "filed" {
                Some(format!("Folder{}", i % 15))
            } else {
                None
            };
            let has_cover = (i % 3 == 0) as i64;
            let path = format!("/lib/{status}/{id}.{format}");
            let filename = format!("{id}.{format}");

            ins_track.execute(rusqlite::params![
                id,
                path,
                format,
                bitrate,
                duration,
                verdict,
                status,
                folder,
                has_cover,
                filename,
                crate::analysis::verdict::VERDICT_CACHE_VERSION,
            ])?;

            let artist = artist_for(i);
            let title = format!("Track {id}");
            let label = format!("Label{}", i % 40);
            let year = 1985 + (i % 40) as i64;
            let bpm = 100 + (i % 45) as i64;
            ins_meta.execute(rusqlite::params![id, artist, title, label, year, bpm])?;

            for (ord, genre) in genres_for(i).into_iter().enumerate() {
                ins_genre.execute(rusqlite::params![id, genre, ord as i64])?;
            }
        }
    }
    tx.commit()
}

/// Opens a fresh temp-file SQLite DB, runs real migrations, seeds `n` tracks.
/// The `NamedTempFile` deletes its file on drop — no manual cleanup needed (Step 6).
fn build_dataset(n: usize, filed_fraction: f64) -> (tempfile::NamedTempFile, Connection) {
    let tmp = tempfile::NamedTempFile::new().expect("create temp db file");
    let mut conn = crate::db::open(tmp.path()).expect("db open + migrate");
    seed_dataset(&mut conn, n, filed_fraction).expect("seed dataset");
    (tmp, conn)
}

// ── timing helpers ───────────────────────────────────────────────────────────

fn measure<F: FnMut()>(mut f: F, iters: usize) -> Vec<Duration> {
    let mut out = Vec::with_capacity(iters);
    for _ in 0..iters {
        let start = Instant::now();
        f();
        out.push(start.elapsed());
    }
    out
}

/// Same as `measure`, but runs `setup` OUTSIDE the timed window before every repetition. Needed
/// for the cold-cache variants: putting `tracks.fingerprint` back to NULL is itself a write, and
/// charging it to the measurement would inflate the very number P4 is judged on.
fn measure_with_setup<S: FnMut(), F: FnMut()>(
    mut setup: S,
    mut f: F,
    iters: usize,
) -> Vec<Duration> {
    let mut out = Vec::with_capacity(iters);
    for _ in 0..iters {
        setup();
        let start = Instant::now();
        f();
        out.push(start.elapsed());
    }
    out
}

/// Prints min / median / p90 / max over the sample. p90 is a true nearest-rank pick on the sorted
/// sample: rank `ceil(0.9 n)`, i.e. index `ceil(9n/10) - 1` (the 45th of 50, not the 46th — the
/// earlier `9n/10` landed on index 45, which is the ~92nd centile). Added 2026-07-27 for P1, whose
/// budget check (D3) is stated on a median AND a p90, never on a single number. Kept in the one
/// shared helper so both blocks of this file report the same columns.
fn summarize(label: &str, mut durs: Vec<Duration>) {
    durs.sort();
    let n = durs.len();
    let min = durs[0];
    let max = durs[n - 1];
    let median = durs[n / 2];
    let p90 = durs[(n * 9).div_ceil(10).saturating_sub(1)];
    println!(
        "  {label:<44} n={n:<4} min={:>8.2}ms  median={:>8.2}ms  p90={:>8.2}ms  max={:>8.2}ms",
        min.as_secs_f64() * 1000.0,
        median.as_secs_f64() * 1000.0,
        p90.as_secs_f64() * 1000.0,
        max.as_secs_f64() * 1000.0,
    );
}

const ITERS: usize = 5;

fn measure_queries(conn: &Connection, volume: usize) {
    println!("\n=== list_filed / list_pending latency @ {volume} rows ===");

    let base = LibraryFilter::default();
    summarize(
        "list_filed (no filter)",
        measure(
            || {
                library::list_filed(conn, &base).unwrap();
            },
            ITERS,
        ),
    );

    let with_q = LibraryFilter {
        q: Some("a".to_string()),
        ..Default::default()
    };
    summarize(
        "list_filed (q LIKE, worst case)",
        measure(
            || {
                library::list_filed(conn, &with_q).unwrap();
            },
            ITERS,
        ),
    );

    let with_genre = LibraryFilter {
        genre: Some("House".to_string()),
        ..Default::default()
    };
    summarize(
        "list_filed (genre IN subquery)",
        measure(
            || {
                library::list_filed(conn, &with_genre).unwrap();
            },
            ITERS,
        ),
    );

    summarize(
        "list_pending",
        measure(
            || {
                queue::list_pending(conn).unwrap();
            },
            ITERS,
        ),
    );
}

// ── EXPLAIN QUERY PLAN ───────────────────────────────────────────────────────

/// Mirrors the exact SQL text built by `library::list_filed`/`queue::list_pending` for each
/// filter combination (those functions build SQL dynamically and don't expose it, so the
/// strings are reproduced here for `EXPLAIN QUERY PLAN` purposes only — literal filter values
/// substituted directly since EXPLAIN QUERY PLAN never binds/executes real params).
fn explain(conn: &Connection, label: &str, sql: &str) {
    println!("\n-- EXPLAIN QUERY PLAN: {label} --");
    println!("   SQL: {sql}");
    let mut stmt = conn
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
        .expect("prepare explain");
    let rows = stmt
        .query_map([], |r| {
            let detail: String = r.get(3)?;
            Ok(detail)
        })
        .expect("query explain");
    for row in rows {
        println!("   {}", row.expect("explain row"));
    }
}

fn explain_all_queries(conn: &Connection) {
    explain(
        conn,
        "list_filed (no filter)",
        "SELECT t.id, t.path, t.format, t.bitrate, t.duration, t.verdict, t.folder, t.has_cover, \
         m.artist, m.title, m.label, m.year, m.bpm, m.cover_path, m.discogs_release_id \
         FROM tracks t LEFT JOIN metadata m ON m.track_id = t.id \
         WHERE t.status = 'filed' ORDER BY m.artist, m.title, t.path",
    );
    explain(
        conn,
        "list_filed (q LIKE)",
        "SELECT t.id, t.path, t.format, t.bitrate, t.duration, t.verdict, t.folder, t.has_cover, \
         m.artist, m.title, m.label, m.year, m.bpm, m.cover_path, m.discogs_release_id \
         FROM tracks t LEFT JOIN metadata m ON m.track_id = t.id \
         WHERE t.status = 'filed' AND (m.artist LIKE '%a%' OR m.title LIKE '%a%' OR t.path LIKE '%a%') \
         ORDER BY m.artist, m.title, t.path",
    );
    explain(
        conn,
        "list_filed (genre IN subquery)",
        "SELECT t.id, t.path, t.format, t.bitrate, t.duration, t.verdict, t.folder, t.has_cover, \
         m.artist, m.title, m.label, m.year, m.bpm, m.cover_path, m.discogs_release_id \
         FROM tracks t LEFT JOIN metadata m ON m.track_id = t.id \
         WHERE t.status = 'filed' AND t.id IN (SELECT track_id FROM track_genres WHERE genre = 'House') \
         ORDER BY m.artist, m.title, t.path",
    );
    explain(
        conn,
        "list_pending",
        "SELECT t.id, t.path, t.filename, t.source_id, t.verdict, t.real_quality, m.artist, m.title \
         FROM tracks t LEFT JOIN metadata m ON m.track_id = t.id \
         WHERE t.status='pending' ORDER BY t.id",
    );
}

// ── JSON serialization proxy (IPC cost) ──────────────────────────────────────

fn measure_serialization(conn: &Connection) {
    let rows = library::list_filed(conn, &LibraryFilter::default()).unwrap();
    println!("\n=== JSON serialization proxy (list_filed, no filter, 100k dataset) ===");
    println!("  rows returned: {}", rows.len());

    let start = Instant::now();
    let json = serde_json::to_string(&rows).unwrap();
    let elapsed = start.elapsed();

    println!(
        "  serialized size: {} bytes ({:.2} MB)",
        json.len(),
        json.len() as f64 / 1_048_576.0
    );
    println!(
        "  serialization time: {:.2}ms",
        elapsed.as_secs_f64() * 1000.0
    );
    println!(
        "  NOTE: this is serde_json::to_string cost only (Tauri's actual IPC command \
         serialization), not a full IPC round-trip — that requires a running `tauri dev` \
         WebView2 process and is out of scope for this automated benchmark."
    );
}

// ── entry point ───────────────────────────────────────────────────────────────

/// Filed fraction used for the main latency run — chosen to stay under the SQLite
/// bound-parameter limit (see `seed_dataset` doc comment) at BOTH volumes, so the two are
/// comparable. Not the originally-planned 50/50; see the crash-reproduction block below for
/// that proportion.
const MAIN_RUN_FILED_FRACTION: f64 = 0.25;

#[test]
#[ignore]
fn bench_volume_list_filed_and_list_pending() {
    for &volume in &[15_000usize, 100_000usize] {
        let (_tmp, conn) = build_dataset(volume, MAIN_RUN_FILED_FRACTION);

        measure_queries(&conn, volume);
        explain_all_queries(&conn);

        if volume == 100_000 {
            measure_serialization(&conn);
        }
        // `_tmp` (NamedTempFile) drops here, deleting the temp SQLite file from disk.
    }

    reproduce_sqlite_variable_limit_crash();
}

/// Regression guard for a crash this benchmark originally discovered (and that is now fixed,
/// see `genres::get_genres_batch`'s `GENRE_BATCH_CHUNK_SIZE` chunking, commit 50239e3): at a
/// realistic 50/50 filed/pending split and 100k total rows, `list_filed`'s unfiltered path used
/// to ask `genres::get_genres_batch` to bind ~50,000 placeholders in one `IN (...)` — above this
/// SQLite build's 32766 bound-parameter limit (see `debug_print_sqlite_variable_limit`). Fixed by
/// chunking; this now exists to catch a REGRESSION if the chunking is ever removed/broken, not to
/// reproduce an active bug.
fn reproduce_sqlite_variable_limit_crash() {
    println!(
        "\n=== Regression check: 50k-filed IN(...) clause that used to crash (fixed by genres.rs chunking) ==="
    );
    let (_tmp, conn) = build_dataset(100_000, 0.5);
    let base = LibraryFilter::default();
    match library::list_filed(&conn, &base) {
        Ok(rows) => println!(
            "  OK: list_filed succeeded with {} rows — chunking fix confirmed still effective.",
            rows.len()
        ),
        Err(e) => println!(
            "  REGRESSION: list_filed(no filter) returned Err: {e} — the chunking fix in \
             genres::get_genres_batch may have been reverted or broken, check GENRE_BATCH_CHUNK_SIZE."
        ),
    }
}

// ── Filing loop @ 15k (P1) ───────────────────────────────────────────────────
//
// Scope, decided by READING the code paths the rail actually calls, not by guessing:
//  - clic sur un bac            → `library::list_bins` (walks the library root on the FILESYSTEM)
//  - aperçu du nom final        → `ipc_filing::preview_filename` = settings read + `render_filename`
//  - passage à la piste suivante → `filing::reconcile_track` + `track_release`'s two DB reads +
//                                  `tagging::read_tags_full` + `dedup::find_duplicate` (front fires
//                                  them in parallel, `frontend/filing.ts:350-373`)
//  - clic sur Convertir         → `plan_file` / `execute_file` / `commit_file`
//                                  (`ipc_filing::file_track`, the three phases it runs)
//
// "hors encodage" is obtained WITHOUT stubbing anything: the source files are real 16-bit/44.1 kHz
// WAVs and the target is forced to `Wav1644` (the format chip the rail already exposes), so
// `encode::is_conformant` is true and `execute_file` takes the tag+move path — the same code as
// production, minus the ffmpeg spawn. Conformance is asserted before the loop rather than assumed:
// a non-conformant source would silently turn this into an encode benchmark.

/// Top-level destination bins of a DJ library root. 14 × (1 + 6) = 98 real directories: `list_bins`
/// walks the filesystem, so the tree has to exist on disk for the number to mean anything.
const TOP_BINS: [&str; 14] = [
    "House",
    "Deep House",
    "Tech House",
    "Techno",
    "Disco",
    "Nu Disco",
    "Funk",
    "Soul",
    "Drum and Bass",
    "Breakbeat",
    "Garage",
    "Ambient",
    "Electro",
    "Edits",
];
const SUB_BINS: [&str; 6] = [
    "Classics",
    "Nouveautes",
    "Peak Time",
    "Warmup",
    "Promo",
    "A trier",
];

/// Frames per synthetic source file: 1M × 2 bytes ≈ 2 MB (~23 s of 16-bit/44.1 kHz mono PCM).
/// Deliberately SMALLER than a real 30–40 MB lossless track. The two phases D3/P4 care about
/// (`plan_file`, `commit_file`) only touch the DB and file headers, so they are unaffected; but
/// `execute_file`'s tag rewrite + move DO scale with file size, so its number below is a LOWER
/// BOUND and is reported as its own line, never folded into the lock-held total.
const SOURCE_FRAMES: usize = 1_000_000;

/// Repetitions for the read-only gestures. Enough for a median and a p90 to mean something
/// (nearest-rank p90 = the 45th of 50), while keeping the whole run under a minute.
const ITERS_LOOP: usize = 50;

/// Repetitions for the ONE variant that decodes audio (`find_duplicate` on a match with an empty
/// fingerprint cache: two full decodes per call). Fewer, because each repetition costs orders of
/// magnitude more than the read-only ones — still enough for a median and a nearest-rank p90.
const ITERS_HEAVY: usize = 10;

/// Duplicate pairs seeded by `bench_filing_loop_15k`: N pending tracks whose file name collides
/// with a filed one, both sides holding a REAL WAV. Without them the expensive branch of
/// `find_duplicate` is never entered and the baseline understates the geste by ~3 orders of
/// magnitude. A DJ library without name collisions is not realistic — dedup exists because they
/// happen.
const NAME_COLLISIONS: usize = 12;

/// Creates the bin tree under `root`; returns every bin's `rel` (the exact string the rail sends
/// back as `bin_rel`), in creation order.
fn create_bin_tree(root: &Path) -> Vec<String> {
    let mut rels = Vec::with_capacity(TOP_BINS.len() * (1 + SUB_BINS.len()));
    for top in TOP_BINS {
        std::fs::create_dir_all(root.join(top)).expect("create top bin");
        rels.push(top.to_string());
        for sub in SUB_BINS {
            std::fs::create_dir_all(root.join(top).join(sub)).expect("create sub bin");
            rels.push(format!("{top}/{sub}"));
        }
    }
    rels
}

/// Writes a minimal but genuinely valid 16-bit / 44.1 kHz mono PCM WAV — lofty parses it (so
/// `is_conformant`, `read_tags_full` and `write_tags_full` all run their real code paths) without
/// depending on `src-tauri/fixtures/*`, which is gitignored and absent from a fresh checkout.
fn write_wav_16_44(path: &Path, frames: usize) {
    let data_len = frames * 2;
    let mut buf = Vec::with_capacity(44 + data_len);
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&((36 + data_len) as u32).to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
    buf.extend_from_slice(&1u16.to_le_bytes()); // format = PCM
    buf.extend_from_slice(&1u16.to_le_bytes()); // channels = mono
    buf.extend_from_slice(&44_100u32.to_le_bytes());
    buf.extend_from_slice(&88_200u32.to_le_bytes()); // byte rate = 44100 * 1 * 2
    buf.extend_from_slice(&2u16.to_le_bytes()); // block align
    buf.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&(data_len as u32).to_le_bytes());
    for i in 0..frames {
        let s = (((i % 441) as i16) - 220).wrapping_mul(64);
        buf.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(path, &buf).expect("write wav");
}

/// A library root that exists on disk (bins + one file per filed track), a sources folder holding
/// the real WAVs about to be filed, and the matching SQLite rows. Every `TempDir`/`NamedTempFile`
/// is kept alive as a field: they delete their tree on drop, so nothing is left behind.
struct FilingDataset {
    conn: Connection,
    /// Library root holding `filed_files` real (empty) files spread across the bin tree.
    root: PathBuf,
    /// Same bin tree, no files — isolates the walk's fixed cost from the per-file cost.
    empty_root: PathBuf,
    bins: Vec<String>,
    /// `(track_id, path)` of the pending tracks whose real WAV is on disk, ready to be filed.
    sources: Vec<(i64, String)>,
    /// `(pending_id, filed_id)` pairs sharing a file-name key, each side holding a real WAV —
    /// the only rows for which `dedup::find_duplicate` reaches its fingerprint branch.
    collisions: Vec<(i64, i64)>,
    filed_files: usize,
    total: usize,
    _db: tempfile::NamedTempFile,
    _root_dir: tempfile::TempDir,
    _empty_dir: tempfile::TempDir,
    _src_dir: tempfile::TempDir,
}

/// Seeds `total` tracks: the first `filed` are `filed`, each with a real (empty) file inside the
/// bin tree — that is what makes `list_bins` realistic, since its `WalkDir` visits every FILE too,
/// not just directories. The rest are `pending`; only the first `real_sources` of them get an
/// actual WAV written (the others are DB rows only — nothing in the measured paths ever opens
/// them, and writing 3 000 more files would only slow the seed down).
///
/// `name_collisions` seeds the DUPLICATE case on top of that: the first `name_collisions` filed
/// tracks become "anchors" holding a real WAV instead of an empty file, and just after the real
/// sources come `name_collisions` pending tracks whose file name is the anchor's, byte for byte.
/// `dedup::key_for_path` (dedup.rs:217) keys on the file stem, so those pairs — and only those —
/// make `find_duplicate` go past its `Ok(None)` early return (dedup.rs:300-302) into the
/// fingerprint branch.
fn build_filing_dataset(
    total: usize,
    filed: usize,
    real_sources: usize,
    name_collisions: usize,
) -> FilingDataset {
    assert!(filed < total, "need some pending tracks to file");
    assert!(
        real_sources + name_collisions <= total - filed,
        "not enough pending tracks for that many real sources + collisions"
    );
    assert!(
        name_collisions <= filed,
        "not enough filed tracks to anchor that many collisions"
    );
    let root_dir = tempfile::TempDir::new().expect("temp library root");
    let empty_dir = tempfile::TempDir::new().expect("temp empty root");
    let src_dir = tempfile::TempDir::new().expect("temp sources dir");
    let bins = create_bin_tree(root_dir.path());
    create_bin_tree(empty_dir.path());

    let tmp_db = tempfile::NamedTempFile::new().expect("create temp db file");
    let mut conn = crate::db::open(tmp_db.path()).expect("db open + migrate");
    let mut sources = Vec::with_capacity(real_sources);
    let mut collisions: Vec<(i64, i64)> = Vec::with_capacity(name_collisions);

    {
        let tx = conn.transaction().expect("begin seed tx");
        {
            let mut ins_track = tx
                .prepare(
                    // `verdict_ver` avec le verdict — même raison que le seed de `bench_volume`
                    // plus haut : un verdict sans version est effacé à la lecture.
                    "INSERT INTO tracks
                        (id, path, format, bitrate, duration, verdict, verdict_ver, status, folder, has_cover, filename)
                     VALUES (?1,?2,?3,?4,?5,?6,?11,?7,?8,?9,?10)",
                )
                .expect("prepare track insert");
            let mut ins_meta = tx
                .prepare(
                    "INSERT INTO metadata (track_id, artist, title, label, year, bpm)
                     VALUES (?1,?2,?3,?4,?5,?6)",
                )
                .expect("prepare metadata insert");
            let mut ins_genre = tx
                .prepare("INSERT INTO track_genres (track_id, genre, ord) VALUES (?1,?2,?3)")
                .expect("prepare genre insert");

            for i in 0..total {
                let id = (i + 1) as i64;
                let artist = artist_for(i);
                let title = format!("Track {id}");
                let is_filed = i < filed;
                let (path, format, folder) = if is_filed {
                    let bin = &bins[i % bins.len()];
                    // Collision anchors carry a real WAV: `get_or_compute_fp` decodes BOTH sides
                    // of a match, so an empty file on the filed side would cut the branch short.
                    let is_anchor = i < name_collisions;
                    let format = if is_anchor { "wav" } else { format_for(i) };
                    let name = format!("{artist} - {title}.{format}");
                    // `rel` uses '/' (the rail's wire format) — push component by component rather
                    // than joining the raw string, so the path is built the same way on any OS.
                    let mut abs = root_dir.path().to_path_buf();
                    for comp in bin.split('/') {
                        abs.push(comp);
                    }
                    abs.push(&name);
                    if is_anchor {
                        write_wav_16_44(&abs, SOURCE_FRAMES);
                    } else {
                        std::fs::File::create(&abs).expect("create filed file");
                    }
                    (abs.to_string_lossy().to_string(), format, Some(bin.clone()))
                } else {
                    // Pending tracks in seeding order: real sources to be filed, then the
                    // collision sources, then DB-only rows.
                    let anchor = (i - filed)
                        .checked_sub(real_sources)
                        .filter(|j| *j < name_collisions);
                    let abs = match anchor {
                        Some(j) => {
                            // Byte-for-byte the anchor's file name → same `key_for_path`.
                            let abs = src_dir.path().join(format!(
                                "{} - Track {}.wav",
                                artist_for(j),
                                j + 1
                            ));
                            write_wav_16_44(&abs, SOURCE_FRAMES);
                            collisions.push((id, (j + 1) as i64));
                            abs
                        }
                        None => {
                            let abs = src_dir.path().join(format!("{artist} - {title}.wav"));
                            if sources.len() < real_sources {
                                write_wav_16_44(&abs, SOURCE_FRAMES);
                                sources.push((id, abs.to_string_lossy().to_string()));
                            }
                            abs
                        }
                    };
                    (abs.to_string_lossy().to_string(), "wav", None)
                };
                let filename = Path::new(&path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file")
                    .to_string();
                ins_track
                    .execute(rusqlite::params![
                        id,
                        path,
                        format,
                        bitrate_for(format, i),
                        120.0 + (i % 300) as f64 * 1.3,
                        verdict_for(i),
                        if is_filed { "filed" } else { "pending" },
                        folder,
                        (i % 3 == 0) as i64,
                        filename,
                        crate::analysis::verdict::VERDICT_CACHE_VERSION,
                    ])
                    .expect("insert track");
                ins_meta
                    .execute(rusqlite::params![
                        id,
                        artist,
                        title,
                        format!("Label{}", i % 40),
                        1985 + (i % 40) as i64,
                        100 + (i % 45) as i64,
                    ])
                    .expect("insert metadata");
                for (ord, genre) in genres_for(i).into_iter().enumerate() {
                    ins_genre
                        .execute(rusqlite::params![id, genre, ord as i64])
                        .expect("insert genre");
                }
            }
        }
        tx.commit().expect("commit seed tx");
    }

    // Phase 1 of `file_track` reads BOTH settings under the DB lock before planning
    // (ipc_filing.rs:304-305) — seed them so the benchmark can read them for real instead of
    // passing a constant, and so `library_root`'s non-empty guard is satisfied.
    crate::settings::set(
        &conn,
        crate::settings::LIBRARY_ROOT,
        &root_dir.path().to_string_lossy(),
    )
    .expect("seed library_root setting");
    crate::settings::set(
        &conn,
        crate::settings::FILENAME_TEMPLATE,
        crate::settings::DEFAULT_TEMPLATE,
    )
    .expect("seed filename_template setting");

    FilingDataset {
        conn,
        root: root_dir.path().to_path_buf(),
        empty_root: empty_dir.path().to_path_buf(),
        bins,
        sources,
        collisions,
        filed_files: filed,
        total,
        _db: tmp_db,
        _root_dir: root_dir,
        _empty_dir: empty_dir,
        _src_dir: src_dir,
    }
}

/// "Clic sur un bac" — the rail reloads its destination tree from `list_bins`. Measured on the
/// populated root AND on the same tree with no files, because `list_bins`'s `WalkDir` enumerates
/// every entry and only then filters out non-directories: the gap between the two lines IS the
/// cost of the filed files, which grows with the library while the bin tree does not.
fn measure_dest_bins(ds: &FilingDataset) {
    println!(
        "\n=== Boucle de rangement: listage des bacs ({} bacs) ===",
        ds.bins.len()
    );
    summarize(
        "list_bins (racine peuplee)",
        measure(
            || {
                library::list_bins(&ds.root);
            },
            ITERS_LOOP,
        ),
    );
    summarize(
        "list_bins (même arbre, 0 fichier)",
        measure(
            || {
                library::list_bins(&ds.empty_root);
            },
            ITERS_LOOP,
        ),
    );
    println!(
        "  NOTE: les deux lignes ci-dessus sont un régime de CACHE CHAUD — le même arbre est\n\
         \x20       reparcouru 50 fois d'affilée, les métadonnées NTFS restent en cache. Le premier\n\
         \x20       parcours après démarrage (à plus forte raison sur 15 000 fichiers réels ou sur un\n\
         \x20       disque externe) est materiellement plus cher et n'est PAS couvert ici: ces deux\n\
         \x20       chiffres sont un PLANCHER, comme celui d'execute_file plus bas."
    );
}

/// "Aperçu du nom final" — exactly what `ipc_filing::preview_filename` does under the DB lock
/// (settings read + `naming::render_filename`), plus the settings read alone so the two costs are
/// separable. The front debounces this at 150 ms per keystroke (`frontend/filing-preview.ts:79`).
fn measure_final_name(ds: &FilingDataset) {
    println!("\n=== Boucle de rangement: résolution du nom final ===");
    let (id, _) = ds.sources[0];
    let canonical = crate::filing::reconcile_track(&ds.conn, id).expect("reconcile");
    summarize(
        "preview_filename (settings + render)",
        measure(
            || {
                let tmpl = crate::settings::get_or(
                    &ds.conn,
                    crate::settings::FILENAME_TEMPLATE,
                    crate::settings::DEFAULT_TEMPLATE,
                )
                .unwrap_or_else(|_| crate::settings::DEFAULT_TEMPLATE.to_string());
                let _ = crate::naming::render_filename(&tmpl, &canonical, "wav");
            },
            ITERS_LOOP,
        ),
    );
    summarize(
        "  dont lecture settings seule",
        measure(
            || {
                let _ = crate::settings::get_or(
                    &ds.conn,
                    crate::settings::FILENAME_TEMPLATE,
                    crate::settings::DEFAULT_TEMPLATE,
                );
            },
            ITERS_LOOP,
        ),
    );
}

/// "Passage à la piste suivante" — the four backend reads `openFilingInto` fires in parallel on
/// every track open (`frontend/filing.ts:324` and `:350-373`). The analysis report is NOT here: it
/// is the "fil de pensée" class (< 1 s), already cached, and has its own measured cost in the PRD.
///
/// `track_release`'s body is reproduced rather than called: it is a `#[tauri::command]` taking a
/// Tauri `State`, which cannot be built in a plain test — same reason the `EXPLAIN` block below
/// reproduces SQL text. The SQL is copied verbatim from `ipc_filing::track_release`.
fn measure_track_open(ds: &FilingDataset) {
    println!(
        "\n=== Boucle de rangement: passage a la piste suivante (fan-out du front, budget < 50 ms) ==="
    );
    let (id, path) = ds.sources[0].clone();
    summarize(
        "reconcile_track (DB + tags fichier)",
        measure(
            || {
                crate::filing::reconcile_track(&ds.conn, id).expect("reconcile");
            },
            ITERS_LOOP,
        ),
    );
    summarize(
        "track_release (metadata + genres)",
        measure(
            || {
                let _ = crate::genres::get_genres(&ds.conn, id).unwrap_or_default();
                let _: Option<(Option<String>, Option<String>)> = ds
                    .conn
                    .query_row(
                        "SELECT artist, title, version, label, year, cover_path, discogs_release_id FROM metadata WHERE track_id=?1",
                        rusqlite::params![id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .ok();
            },
            ITERS_LOOP,
        ),
    );
    summarize(
        "track_file_tags (read_tags_full)",
        measure(
            || {
                crate::tagging::read_tags_full(&path).expect("read tags");
            },
            ITERS_LOOP,
        ),
    );
    summarize(
        "find_duplicate (aucun match)",
        measure(
            || {
                crate::dedup::find_duplicate(&ds.conn, id).expect("find_duplicate");
            },
            ITERS_LOOP,
        ),
    );

    // The expensive branch, seeded on purpose (see `build_filing_dataset`'s `name_collisions`).
    // `kind == "both"` is asserted, not assumed: it is the only proof from outside that BOTH
    // fingerprints were actually obtained (dedup.rs:310-317) — a failed decode would silently
    // fall back to `("name", 1.0)` and turn this into a second copy of the no-match line.
    let (dup_id, anchor_id) = ds.collisions[0];
    summarize(
        "find_duplicate (match, empreinte NON en cache)",
        measure_with_setup(
            || clear_fingerprints(&ds.conn, &[dup_id, anchor_id]),
            || assert_sound_confirmed_match(&ds.conn, dup_id),
            ITERS_HEAVY,
        ),
    );
    // The previous loop leaves both fingerprints cached; take one more call to be explicit about
    // it rather than relying on that side effect.
    crate::dedup::find_duplicate(&ds.conn, dup_id).expect("warm fingerprint cache");
    summarize(
        "find_duplicate (match, empreinte en cache)",
        measure(
            || assert_sound_confirmed_match(&ds.conn, dup_id),
            ITERS_LOOP,
        ),
    );
    println!(
        "  NOTE: un match n'ajoute PAS \"une comparaison d'empreintes\". dedup.rs:306-308 appelle\n\
         \x20       get_or_compute_fp DEUX fois; si la colonne `fingerprint` est vide (dedup.rs:342-346),\n\
         \x20       dedup.rs:347 lance fingerprint::compute_for_path, qui DECODE l'audio complet\n\
         \x20       (fingerprint.rs:20-27 -> analysis::decode::decode_pcm) puis ecrit un UPDATE\n\
         \x20       (dedup.rs:349-352) — deux fichiers, sous le verrou que ipc_filing.rs:771 tient sur\n\
         \x20       toute la commande. Sources synthétiques de ~{:.1} Mo: sur un lossless réel de\n\
         \x20       30-40 Mo la ligne \"NON en cache\" est encore un plancher. C'est le patron de P4.",
        (SOURCE_FRAMES * 2 + 44) as f64 / 1_048_576.0
    );
}

/// Runs `find_duplicate` and fails the bench unless the sound-confirmed branch really ran.
fn assert_sound_confirmed_match(conn: &Connection, track_id: i64) {
    let m = crate::dedup::find_duplicate(conn, track_id).expect("find_duplicate");
    let kind = m.map(|d| d.kind).unwrap_or_default();
    assert_eq!(
        kind, "both",
        "expected a fingerprint-confirmed duplicate, got {kind:?} — the seeded collision pair is \
         not exercising dedup.rs:306-317, the number below would be meaningless"
    );
}

/// Empties the `tracks.fingerprint` cache for those ids — puts the rows back in the state a
/// freshly-scanned library is in, where `find_duplicate` has to decode the audio itself.
fn clear_fingerprints(conn: &Connection, ids: &[i64]) {
    for id in ids {
        conn.execute(
            // La version tombe avec la valeur, comme `scanner::upsert_file` le fait en production.
            "UPDATE tracks SET fingerprint=NULL, fingerprint_ver=NULL WHERE id=?1",
            rusqlite::params![id],
        )
        .expect("clear fingerprint cache");
    }
}

/// "Clic sur Convertir", hors encodage: the three phases of `ipc_filing::file_track` on a
/// conformant source (tag write + move, no ffmpeg spawn). One distinct track per repetition — a
/// filing is destructive, the same track cannot be filed twice.
///
/// Phases 1 and 3 hold the DB lock in production; phase 2 does not (`ipc_filing.rs:301-327`). They
/// are reported separately for that reason: P4 targets the two lock-held ones, P5 moves phase 2
/// off the click's critical path.
///
/// Phase 1's timer starts BEFORE the two settings reads `file_track` does under the same lock
/// (`ipc_filing.rs`: `library_root_for` then `template`) — they are small but systematic, and
/// leaving them out would understate exactly the quantity P4 has to bring down.
fn measure_filing(ds: &FilingDataset, bin_rel: &str) {
    use crate::encode::Target;
    println!("\n=== Boucle de rangement: rangement d'une piste (hors encodage) ===");
    let reserved = std::collections::HashSet::new();
    let mut plan_d = Vec::with_capacity(ds.sources.len());
    let mut exec_d = Vec::with_capacity(ds.sources.len());
    let mut commit_d = Vec::with_capacity(ds.sources.len());
    let mut locked_d = Vec::with_capacity(ds.sources.len());
    let mut total_d = Vec::with_capacity(ds.sources.len());

    for (id, path) in &ds.sources {
        // Fail loudly rather than silently benchmarking an ffmpeg encode: a non-conformant source
        // would send execute_file down the transcode path and make this number meaningless.
        assert!(
            crate::encode::is_conformant(path, Target::Wav1644),
            "source {path} is not conformant for Wav1644 — execute_file would spawn ffmpeg"
        );
        let canonical = crate::filing::reconcile_track(&ds.conn, *id).expect("reconcile");
        let t0 = Instant::now();
        // Reproduces `library_root_for(&conn, bin_rel)` + `template(&conn)` (ipc_filing.rs), called
        // under the lock just before `plan_file`. Depuis #54 la prod ne lit la racine que si
        // `bin_rel` vise l'arbre ; ce banc mesure justement ce cas-là (un bac de l'arbre), donc la
        // lecture reste dans la fenêtre chronométrée — c'est bien le coût réel du chemin mesuré.
        let root = match crate::settings::get(&ds.conn, crate::settings::LIBRARY_ROOT) {
            Ok(Some(p)) if !p.trim().is_empty() => PathBuf::from(p),
            other => panic!("library_root setting not seeded: {other:?}"),
        };
        let tmpl = crate::settings::get_or(
            &ds.conn,
            crate::settings::FILENAME_TEMPLATE,
            crate::settings::DEFAULT_TEMPLATE,
        )
        .unwrap_or_else(|_| crate::settings::DEFAULT_TEMPLATE.to_string());
        let plan = crate::filing::plan_file(
            &ds.conn,
            Some(&root),
            &tmpl,
            *id,
            bin_rel,
            Some(Target::Wav1644),
            Some(canonical),
            false,
            &reserved,
        )
        .expect("plan_file");
        let t1 = Instant::now();
        let log = crate::filing::execute_file(&plan).expect("execute_file");
        let t2 = Instant::now();
        crate::filing::commit_file(&ds.conn, &plan, log, None, None).expect("commit_file");
        let t3 = Instant::now();
        plan_d.push(t1.duration_since(t0));
        exec_d.push(t2.duration_since(t1));
        commit_d.push(t3.duration_since(t2));
        locked_d.push(t1.duration_since(t0) + t3.duration_since(t2));
        total_d.push(t3.duration_since(t0));
    }

    summarize("plan_file (phase 1 + 2 lectures settings)", plan_d);
    summarize("execute_file (phase 2, HORS verrou)", exec_d);
    summarize("commit_file (phase 3, SOUS le verrou)", commit_d);
    summarize("phases sous verrou (1+3)", locked_d);
    summarize("file_track complet (hors encodage)", total_d);
    println!(
        "  NOTE: execute_file mesure une source de ~{:.1} Mo (tag write + rename intra-volume). Un vrai\n\
         \x20       lossless de 30-40 Mo, ou un move cross-disque (copy_verify_delete), coûte davantage —\n\
         \x20       ce chiffre est un PLANCHER. Aucun Rekordbox XML/master.db lié ici: les détections de\n\
         \x20       commit_file sortent immédiatement, un utilisateur lié paie plus (cible de P4).",
        (SOURCE_FRAMES * 2 + 44) as f64 / 1_048_576.0
    );
}

/// `EXPLAIN QUERY PLAN` for the filing-loop queries, same rationale as `explain_all_queries`: the
/// production code builds/binds these statements, so their text is reproduced here (literal values
/// substituted — EXPLAIN never binds or executes).
fn explain_filing_queries(conn: &Connection) {
    explain(
        conn,
        "track_path / reconcile_track / find_duplicate (étape 1)",
        "SELECT path FROM tracks WHERE id=1",
    );
    explain(
        conn,
        "track_release (metadata)",
        "SELECT artist, title, version, label, year, cover_path, discogs_release_id \
         FROM metadata WHERE track_id=1",
    );
    explain(
        conn,
        "get_genres (load_tag_extras + track_release)",
        "SELECT genre FROM track_genres WHERE track_id=1 ORDER BY ord",
    );
    explain(
        conn,
        "find_duplicate (candidats)",
        "SELECT id, path, status, folder, filename FROM tracks \
         WHERE status IN ('pending','filed') AND id<>1",
    );
    explain(
        conn,
        "commit_file (UPDATE tracks)",
        "UPDATE tracks SET status='filed', folder='House/Classics', target_format='wav_16_44', \
         confidence='yellow' WHERE id=1",
    );
}

/// P1 — baseline of the filing loop at the D1 target volume (15 000 tracks). These numbers are the
/// "before" P4 and P5 will be judged against (docs/superpowers/changes/2026-07-27-perf-fixes/PRD.md).
#[test]
#[ignore]
fn bench_filing_loop_15k() {
    let ds = build_filing_dataset(15_000, 12_000, 30, NAME_COLLISIONS);
    println!(
        "\n########## Boucle de rangement @ {} pistes ({} filed sur disque, {} bacs, {} sources réelles, {} paires de doublons) ##########",
        ds.total,
        ds.filed_files,
        ds.bins.len(),
        ds.sources.len(),
        ds.collisions.len()
    );
    measure_dest_bins(&ds);
    measure_final_name(&ds);
    measure_track_open(&ds);
    measure_filing(&ds, "House/Classics");
    explain_filing_queries(&ds.conn);
    // Every TempDir/NamedTempFile drops here, deleting the seeded library root + sources + db.
}

#[test]
#[ignore]
fn debug_print_sqlite_variable_limit() {
    let conn = Connection::open_in_memory().unwrap();
    // Binary search the largest N for which `SELECT 1 WHERE 1 IN (?,?,...N times)` still
    // prepares, to find this build's actual SQLITE_MAX_VARIABLE_NUMBER (the `limits` rusqlite
    // feature isn't enabled, so we can't just read it via the API).
    let fails = |n: usize| -> bool {
        let placeholders = vec!["?"; n].join(",");
        let sql = format!("SELECT 1 WHERE 1 IN ({placeholders})");
        conn.prepare(&sql).is_err()
    };
    let mut lo = 1usize;
    let mut hi = 100_000usize;
    assert!(!fails(lo), "even 1 variable fails to prepare");
    assert!(fails(hi), "100000 variables unexpectedly succeeded");
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if fails(mid) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    println!("largest working variable count = {lo}, fails at {hi}");
}
