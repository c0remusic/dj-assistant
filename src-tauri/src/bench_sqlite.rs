//! Phase 5 — mesures préalables SQLite et budget de ressources.
//!
//! La Phase 5 de `docs/superpowers/specs/2026-07-13-architecture-evolution-design.md` (§8, fichier
//! sorti du suivi git le 2026-07-31, récupérable par `git show 6551728^:<chemin>`) est
//! CONDITIONNELLE : elle exige quatre mesures avant tout code, et se clôt sans changement si les
//! mesures ne montrent pas de problème réel. Ce fichier porte celles qu'un banc in-process peut
//! produire honnêtement :
//!
//!   (1) temps d'attente du verrou et fréquence des `SQLITE_BUSY` ;
//!   (2) latence d'une commande IPC pendant que le pool d'analyse écrit ;
//!   (4bis) coût CPU d'UNE analyse, sur de vrais fichiers — le chiffre qui dit ce que coûterait
//!          une ré-analyse de bibliothèque, et donc ce que coûte d'évincer un rapport du cache.
//!
//! La mesure (3), taille de `report_json`, ne se fait PAS ici : elle se lit sur la base de
//! production (2026-08-03 : 2 714 pistes, 2,22 Go vifs, 817 ko/rapport, dont 1,00 Go de `peaks`
//! là où le cap `MAX_PEAKS` en prévoyait 0,12). Un banc synthétique n'y mesurerait que ses
//! propres hypothèses. La mesure (4) complète — charge CPU/disque analyse ET encodage en même
//! temps — demande l'application réellement en marche.
//!
//! Lancer avec : `cargo test --release -- --ignored --nocapture bench_sqlite --test-threads=1`
//! (`--test-threads=1` obligatoire : ce banc mesure de la contention, et les autres `#[ignore]`
//! du crate tournant en parallèle la fausseraient — même mise en garde qu'en tête de
//! `bench_volume.rs`.)

use rusqlite::Connection;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Taille d'un `report_json` tel qu'il est réellement écrit aujourd'hui, mesurée sur la base de
/// production le 2026-08-03 : 817 ko de moyenne sur 2 710 rapports. Le banc écrit cette taille-là,
/// pas une taille inventée — c'est elle qui décide de la durée pendant laquelle le verrou est tenu.
const REPORT_BYTES: usize = 817 * 1024;

/// Combien de temps un worker passe à analyser AVANT d'écrire son rapport. C'est le paramètre qui
/// décide si la mesure veut dire quelque chose : à 0 ms le banc modélise 16 threads qui écrivent
/// en boucle fermée, ce que la production ne fait jamais, et il ne mesure plus que l'iniquité du
/// `std::sync::Mutex`. La médiane réelle de la bibliothèque est une piste de 394 s ; le temps
/// d'analyse correspondant sort de `bench_analysis_cost_on_real_tracks` ci-dessous.
const REALISTIC_ANALYSIS_MS: u64 = 2_000;

/// Le thread « interface » échantillonne pendant une FENÊTRE, pas jusqu'à un nombre d'échantillons.
/// La première version comptait 200 échantillons et s'arrêtait : elle finissait en 1,2 s, avant
/// que le premier écrivain ait fini ses 2 s d'analyse, et mesurait donc une base au repos en
/// croyant mesurer une base sous charge (constaté le 2026-08-03). La fenêtre doit couvrir
/// plusieurs cycles d'écriture pour que le mot « charge » veuille dire quelque chose.
const UI_WINDOW: Duration = Duration::from_secs(12);
const UI_SAMPLE_EVERY: Duration = Duration::from_millis(50);
/// Garde-fou pur : ne doit jamais mordre avant `UI_WINDOW`. S'il mord, la fenêtre est mal réglée.
const UI_MAX_SAMPLES: usize = 5_000;

fn seed(conn: &Connection, tracks: usize) {
    crate::db::run_migrations(conn).expect("migrations");
    let tx = conn.unchecked_transaction().expect("tx");
    for i in 0..tracks {
        tx.execute(
            "INSERT INTO tracks(path, status) VALUES(?1, 'pending')",
            rusqlite::params![format!("C:/musique/piste_{i:06}.mp3")],
        )
        .expect("insert");
    }
    tx.commit().expect("commit");
}

/// Quantiles d'un échantillon de durées.
fn quantiles(mut v: Vec<Duration>) -> (Duration, Duration, Duration) {
    v.sort_unstable();
    let at = |q: f64| v[((v.len() as f64 - 1.0) * q) as usize];
    (at(0.50), at(0.95), v[v.len() - 1])
}

/// (1) + (2) : pendant que N threads écrivent des rapports à travers le `Mutex<Connection>`
/// partagé, on mesure ce que coûte à une commande IPC de lecture — la forme exacte de
/// `queue::list_pending` — d'obtenir le verrou, puis de répondre.
///
/// Les deux variantes ne se lisent PAS pareil. La saturée est un cas limite qui n'existe pas en
/// production ; elle sert uniquement à montrer ce que le `Mutex` fait quand la pression est
/// continue. La réaliste est celle dont les chiffres ont un sens.
#[test]
#[ignore]
fn bench_sqlite_lock_wait_under_analysis_load() {
    for (label, analysis_ms) in [
        (
            "cycle réaliste (analyse 2 s, puis écriture)",
            REALISTIC_ANALYSIS_MS,
        ),
        ("saturé (écriture en boucle fermée — cas limite)", 0),
    ] {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("bench.db");
        let conn = crate::db::open(&path).expect("open");
        seed(&conn, 15_000);
        let shared = Arc::new(Mutex::new(conn));

        let stop = Arc::new(AtomicBool::new(false));
        let writes = Arc::new(AtomicU64::new(0));
        let busy = Arc::new(AtomicU64::new(0));
        let held = Arc::new(AtomicU64::new(0)); // µs cumulées de verrou tenu par les écrivains

        // Le pool réel est dimensionné sur available_parallelism() (worker.rs:156).
        let workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        let payload = "x".repeat(REPORT_BYTES);

        let started = Instant::now();
        let mut handles = Vec::new();
        for w in 0..workers {
            let shared = Arc::clone(&shared);
            let stop = Arc::clone(&stop);
            let writes = Arc::clone(&writes);
            let busy = Arc::clone(&busy);
            let held = Arc::clone(&held);
            let payload = payload.clone();
            handles.push(std::thread::spawn(move || {
                let mut i = w;
                while !stop.load(Ordering::Relaxed) {
                    // Le travail d'analyse se fait HORS verrou — c'est ce que fait worker.rs, et
                    // c'est précisément ce que la variante saturée supprime.
                    if analysis_ms > 0 {
                        std::thread::sleep(Duration::from_millis(analysis_ms));
                        if stop.load(Ordering::Relaxed) {
                            return;
                        }
                    }
                    let c = match shared.lock() {
                        Ok(c) => c,
                        Err(_) => return,
                    };
                    let t = Instant::now();
                    let r = c.execute(
                        "UPDATE tracks SET report_json=?1, report_cache_ver=6 WHERE id=?2",
                        rusqlite::params![payload, (i % 15_000) as i64 + 1],
                    );
                    held.fetch_add(t.elapsed().as_micros() as u64, Ordering::Relaxed);
                    match r {
                        Ok(_) => {
                            writes.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(rusqlite::Error::SqliteFailure(e, _))
                            if e.code == rusqlite::ErrorCode::DatabaseBusy =>
                        {
                            busy.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(e) => panic!("écriture inattendue en échec: {e}"),
                    }
                    drop(c);
                    i += workers;
                }
            }));
        }

        // Le thread « interface » : la forme de queue::list_pending, chronométrée en deux temps —
        // attente du verrou, puis requête. Séparer les deux est tout l'intérêt : une commande
        // lente parce qu'elle attend n'appelle pas le même correctif qu'une commande lente parce
        // qu'elle lit trop.
        let mut waits = Vec::new();
        let mut queries = Vec::new();
        let deadline = Instant::now() + UI_WINDOW;
        while waits.len() < UI_MAX_SAMPLES && Instant::now() < deadline {
            let t0 = Instant::now();
            let c = shared.lock().expect("lock ui");
            let waited = t0.elapsed();
            let t1 = Instant::now();
            let n: i64 = c
                .query_row(
                    "SELECT count(*) FROM tracks WHERE status='pending'",
                    [],
                    |r| r.get(0),
                )
                .expect("query");
            queries.push(t1.elapsed());
            waits.push(waited);
            drop(c);
            assert!(n >= 0);
            std::thread::sleep(UI_SAMPLE_EVERY);
        }
        let ui_elapsed = started.elapsed();

        stop.store(true, Ordering::Relaxed);
        for h in handles {
            let _ = h.join();
        }

        println!("\n=== Phase 5 · {label} ===");
        let n_writes = writes.load(Ordering::Relaxed);
        println!(
            "  {workers} threads · {n_writes} écritures de {} ko en {:.1?} · {} SQLITE_BUSY \
             · verrou tenu {:.1} % du temps",
            REPORT_BYTES / 1024,
            ui_elapsed,
            busy.load(Ordering::Relaxed),
            100.0 * held.load(Ordering::Relaxed) as f64
                / (ui_elapsed.as_micros() as f64 * workers as f64),
        );
        if n_writes == 0 {
            println!(
                "  !! AUCUNE ÉCRITURE pendant la fenêtre — la mesure ci-dessous est celle d'une \
                 base au repos, pas sous charge"
            );
        }
        // Attendu : UI_WINDOW / UI_SAMPLE_EVERY. Nettement moins = l'interface est affamee.
        let expected = (UI_WINDOW.as_millis() / UI_SAMPLE_EVERY.as_millis().max(1)) as usize;
        if waits.len() * 4 < expected {
            println!(
                "  !! FAMINE : {} échantillon(s) au lieu de ~{expected} en {:.1?} — l'interface \
                 n'obtient pas le verrou, le chiffre ci-dessous n'est pas une latence mais une attente",
                waits.len(),
                ui_elapsed,
            );
        }
        let (w50, w95, wmax) = quantiles(waits.clone());
        let (q50, q95, qmax) = quantiles(queries);
        println!(
            "  attente du verrou (n={})  p50 {w50:>9.2?}  p95 {w95:>9.2?}  max {wmax:>9.2?}",
            waits.len()
        );
        println!("  requête elle-même         p50 {q50:>9.2?}  p95 {q95:>9.2?}  max {qmax:>9.2?}");
    }
}

/// (4bis) Ce que coûte UNE analyse, sur de vrais fichiers. Aucun chemin n'est écrit en dur : le
/// banc lit `SIFT_BENCH_TRACKS_DIR`, un dossier de la machine de développement. Sans lui le test
/// s'annonce et sort — la bibliothèque d'un utilisateur n'a rien à faire dans le dépôt.
///
/// À quoi sert ce chiffre : il donne le prix de la ré-analyse (le coût d'une migration qui vide
/// le cache) ET le prix d'une éviction (le coût de rouvrir une piste dont le rapport a été
/// évincé). Les deux décisions de Phase 5 se jouent sur le même nombre.
#[test]
#[ignore]
fn bench_analysis_cost_on_real_tracks() {
    let Ok(dir) = std::env::var("SIFT_BENCH_TRACKS_DIR") else {
        println!("\n=== Phase 5 · coût d'analyse : IGNORÉ ===");
        println!(
            "  définir SIFT_BENCH_TRACKS_DIR sur un dossier contenant de vrais fichiers audio"
        );
        return;
    };
    const EXTS: [&str; 5] = ["mp3", "flac", "wav", "aif", "aiff"];
    const MAX_FILES: usize = 8;

    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .expect("lecture du dossier")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| EXTS.contains(&e.to_ascii_lowercase().as_str()))
                .unwrap_or(false)
        })
        .collect();
    files.sort();
    files.truncate(MAX_FILES);

    println!("\n=== Phase 5 · coût d'une analyse (avec spectrogramme) ===");
    if files.is_empty() {
        println!("  aucun fichier audio dans {dir}");
        return;
    }
    let mut total_audio = 0.0f64;
    let mut total_cpu = Duration::ZERO;
    let mut total_cpu_light = Duration::ZERO;
    for f in &files {
        let name = f.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        // Sans spectrogramme d'abord : c'est ce que le pool mettrait en cache si le spectrogramme
        // passait à la demande. L'écart entre les deux colonnes est ce que coûte l'ouverture du
        // collapse — et il n'est PAS l'écart de calcul FFT seul : `analyze` redécode le fichier
        // entier dans les deux cas, donc la seconde passe repaie aussi le décodage.
        let t_light = Instant::now();
        let _ = crate::analysis::analyze(&f.to_string_lossy(), false);
        let cpu_light = t_light.elapsed();
        total_cpu_light += cpu_light;
        let t = Instant::now();
        match crate::analysis::analyze(&f.to_string_lossy(), true) {
            Ok(r) => {
                let cpu = t.elapsed();
                let secs = r.duration_sec as f64;
                total_audio += secs;
                total_cpu += cpu;
                let ratio = if cpu.as_secs_f64() > 0.0 {
                    secs / cpu.as_secs_f64()
                } else {
                    0.0
                };
                println!(
                    "  {:>7.1} s d'audio · sans spectro {:>8.2?} · avec {:>8.2?}  \
                     ({ratio:>5.1}x)  {} pics  {}",
                    r.duration_sec,
                    cpu_light,
                    cpu,
                    r.peaks.len(),
                    &name[..name.len().min(40)],
                );
            }
            Err(e) => println!("  échec sur {name} : {e}"),
        }
    }
    if total_cpu.as_secs_f64() > 0.0 {
        let ratio = total_audio / total_cpu.as_secs_f64();
        println!(
            "\n  cumul : {:.0} s d'audio en {:.1?} = {ratio:.1}x temps réel, un seul thread",
            total_audio, total_cpu
        );
        // 297,2 h = la durée totale des 2 690 pistes de plus de 60 s de la base de production
        // (mesurée le 2026-08-03). C'est ce qu'une migration qui vide le cache ferait redécoder.
        let library_hours = 297.2f64;
        let single = library_hours * 3600.0 / ratio / 3600.0;
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4) as f64;
        println!(
            "  extrapolation bibliothèque réelle (297,2 h d'audio) : {single:.1} h sur 1 thread, \
             soit ~{:.1} h sur {threads:.0} threads",
            single / threads
        );
        println!(
            "  cache sans spectrogramme : {:.1?} par piste au pool, et {:.1?} d'attente a \
             l'ouverture du collapse (moyenne sur {} pistes)",
            total_cpu_light / files.len() as u32,
            total_cpu / files.len() as u32,
            files.len(),
        );
    }
}
