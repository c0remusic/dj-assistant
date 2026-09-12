//! Phase 4 measurement-only benchmark — la déduplication `O(n²)`.
//!
//! Même méthode que la Phase 3 (`bench_volume.rs`) : **mesurer d'abord, ne rien changer sans
//! preuve**. Aucun changement de comportement de production ici, aucune dépendance `criterion`.
//! Compilé uniquement en build de test (`#[cfg(test)] mod bench_dedup;` dans `lib.rs`).
//!
//! Ce qui est mesuré, et pourquoi ce découpage :
//!
//! 1. **Le coût unitaire d'une comparaison** (`fingerprint::similarity` sur une paire). Il ne
//!    dépend d'AUCUNE hypothèse sur la forme de la bibliothèque — c'est un fait de la machine et
//!    du crate. `similarity` appelle `match_fingerprints` (rusty-chromaprint 0.3.0) : deux `Vec`
//!    alloués de `len1+len2`, un `sort_unstable`, puis un balayage à histogramme. Ce n'est pas la
//!    comparaison de bits que promet le doc-comment de `fingerprint.rs`.
//! 2. **Le taux de survie du pré-filtre de durée** (tolérance `DURATION_MATCH_TOL_SEC` = 2 s).
//!    C'est ce qui décide combien de paires paient le coût unitaire ci-dessus. Compté par
//!    arithmétique pure, sans appeler `similarity` — mesurer ne doit pas perturber ce qu'on mesure.
//! 3. **`group_duplicates` en bout à bout**, aux tailles réelles, pour confronter le produit
//!    (1) × (2) à la réalité.
//! 4. **L'empreinte mémoire** des empreintes acoustiques tenues simultanément en RAM par
//!    `build_fingerprints` — un coût que le temps CPU ne révèle pas.
//! 5. **L'énumération nue** (#38) : le balayage seul, celui qui reste quand le pré-filtre écarte
//!    tout. Jusqu'à #38, `n²/2` paires étaient parcourues quoi qu'il arrive — le pré-filtre
//!    décidait qui payait `similarity`, pas qui était énuméré. Ce bench compare l'ancienne double
//!    boucle à la fenêtre glissante qui l'a remplacée, dans la même exécution.
//!
//! Cadence d'item de `Configuration::preset_test1()` : `(4096 - 4096·2/3) / 11025` =
//! 0,1238 s/item. Une piste de 6 min pèse donc ≈ 2900 `u32`, soit ≈ 11,6 ko.
//!
//! Lancer avec :
//! `cargo test --manifest-path src-tauri/Cargo.toml --release -- --ignored --nocapture bench_dedup`
//!
//! Le `--release` n'est pas optionnel : ces chiffres n'ont aucun sens en debug. Ajouter
//! `--test-threads=1` dès qu'on sélectionne plus d'un bench (voir la même mise en garde en tête de
//! `bench_volume.rs` : les benchs se disputent le CPU et leurs sorties s'entrelacent).

use std::time::Instant;

use crate::dedup::{group_duplicates, DupScanRow};
use crate::fingerprint;

/// Secondes d'audio couvertes par un item de fingerprint sous `preset_test1`.
/// `(DEFAULT_FRAME_SIZE - DEFAULT_FRAME_OVERLAP) / DEFAULT_SAMPLE_RATE`
/// = `(4096 - (4096 - 4096/3)) / 11025`.
const ITEM_DURATION_SEC: f64 = (4096.0 / 3.0) / 11025.0;

/// Copie de la tolérance de `dedup.rs`. Volontairement dupliquée plutôt qu'exportée : la constante
/// de production reste privée, et un bench n'a pas à élargir une visibilité pour se mesurer. Si
/// elle diverge, `duration_prefilter_tolerance_still_matches_production` échoue.
const DURATION_TOL_SEC: f64 = 2.0;

// ── corpus synthétique ───────────────────────────────────────────────────────

/// Générateur déterministe (LCG Numerical Recipes). Pas de `rand` dans l'arbre, et un bench doit
/// être rejouable à l'identique d'une session à l'autre.
struct Lcg(u32);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.0
    }
}

/// Fingerprint d'une piste SANS rapport avec les autres : des `u32` pseudo-aléatoires. Les hashes
/// de `match_fingerprints` ne collisionnent alors quasiment jamais, ce qui est exactement le
/// comportement d'une paire de morceaux différents.
fn synth_fingerprint(seed: u32, items: usize) -> Vec<u32> {
    let mut lcg = Lcg(seed);
    (0..items).map(|_| lcg.next()).collect()
}

/// Fingerprint d'un DOUBLON : la même piste ré-encodée. On repart de `base` et on retourne un bit
/// dans une minorité d'items — assez pour que `similarity` dépasse `MATCH_THRESHOLD` sans être une
/// copie bit-à-bit, ce qui serait un cas trop favorable au matcher.
fn synth_reencode(base: &[u32], seed: u32) -> Vec<u32> {
    let mut lcg = Lcg(seed);
    base.iter()
        .map(|&x| {
            if lcg.next() % 8 == 0 {
                x ^ (1u32 << (lcg.next() % 32))
            } else {
                x
            }
        })
        .collect()
}

/// Durée, en secondes, de la piste d'indice `i` dans le corpus synthétique de taille `n`.
///
/// **C'est LA décision qui gouverne tout ce benchmark**, et c'est pour ça qu'elle est isolée dans
/// sa propre fonction : seules les paires dont les deux durées tiennent dans ±2 s paient le coût
/// d'un `match_fingerprints`, et depuis #38 ce sont aussi les seules énumérées. Leur nombre est
/// fixé ici, et par rien d'autre.
///
/// L'étalement uniforme ci-dessous est **provisoire et optimiste**. Une vraie bibliothèque DJ
/// n'est pas uniforme : elle s'agglutine autour des formats de production (edits radio ~3 min,
/// mixes club ~6-7 min), et chaque agglutination fait exploser le nombre de paires qui passent le
/// pré-filtre — puisqu'il ne regarde QUE la durée. Le chiffre produit par cette version est donc
/// un **plancher**, pas la réponse.
///
/// TODO(Antoine) — remplacer par la vraie forme. Ce qui compte n'est pas la moyenne mais la
/// concentration : combien de pistes partagent la même durée à 2 s près.
fn duration_for(i: usize, n: usize) -> f64 {
    // Étalement uniforme sur 2 min → 10 min. Provisoire, voir le doc-comment.
    120.0 + (i as f64 / n as f64) * 480.0
}

/// Les durées du corpus synthétique de taille `n`, doublons compris.
///
/// Extraite de `synth_rows` pour que l'énumération NUE puisse se mesurer sans construire une seule
/// empreinte : à 100 000 pistes elles pèsent ≈ 1,1 Go de RAM, et le balayage ne les regarde jamais.
/// Une seule source pour la distribution des durées — c'est elle, et rien d'autre, qui décide du
/// nombre de paires survivantes.
fn synth_durations(n: usize, dup_pairs: usize) -> Vec<Option<f64>> {
    let mut d: Vec<Option<f64>> = (0..n).map(|i| Some(duration_for(i, n))).collect();
    // Les doublons sont posés APRÈS coup, par paires voisines : la piste `2k+1` prend la durée de
    // la piste `2k`. Voisines pour que le pré-filtre les laisse passer — un doublon que le
    // pré-filtre écarte ne serait pas un doublon.
    for k in 0..dup_pairs.min(n / 2) {
        d[2 * k + 1] = d[2 * k];
    }
    d
}

/// Un corpus de `n` pistes `filed`, dont `dup_pairs` vraies paires de doublons (même durée, même
/// enregistrement ré-encodé) — sinon `group_duplicates` ne construirait jamais un seul groupe et
/// on ne mesurerait pas le chemin `union`/`min_sim`.
fn synth_rows(n: usize, dup_pairs: usize) -> (Vec<DupScanRow>, Vec<Option<Vec<u32>>>) {
    let durations = synth_durations(n, dup_pairs);
    let mut rows = Vec::with_capacity(n);
    let mut fps: Vec<Option<Vec<u32>>> = Vec::with_capacity(n);

    for (i, &duration) in durations.iter().enumerate() {
        let secs = duration.unwrap_or(0.0);
        let items = (secs / ITEM_DURATION_SEC) as usize;
        rows.push(DupScanRow {
            id: i as i64 + 1,
            path: format!("C:/library/artist{}/track{i}.aiff", i % 200),
            filename: Some(format!("track{i}.aiff")),
            folder: Some(format!("artist{}", i % 200)),
            format: Some(if i % 3 == 0 { "aiff" } else { "mp3" }.to_string()),
            bitrate: Some(if i % 3 == 0 { 1411 } else { 320 }),
            duration,
            truncated: false,
            fingerprint: None,
        });
        fps.push(Some(synth_fingerprint(i as u32 + 1, items)));
    }

    // La piste `2k+1` devient un ré-encodage de la piste `2k` ; sa durée est déjà celle de `2k`,
    // posée par `synth_durations`.
    for k in 0..dup_pairs.min(n / 2) {
        let (a, b) = (2 * k, 2 * k + 1);
        let base = fps[a].clone().unwrap_or_default();
        fps[b] = Some(synth_reencode(&base, b as u32 + 7));
    }

    (rows, fps)
}

// ── comptage du pré-filtre (arithmétique pure, aucun `similarity` appelé) ─────

struct PrefilterCount {
    pairs_total: u64,
    pairs_survived: u64,
}

/// Rejoue EXACTEMENT la condition de durée de `dedup.rs` sur la DOUBLE BOUCLE d'origine, sans rien
/// comparer d'acoustique. Donne le dénominateur et le numérateur du taux de survie.
///
/// C'est aussi l'ORACLE de l'énumération : toute forme plus maligne doit rendre exactement les
/// `pairs_survived` comptées ici, en en parcourant moins.
fn count_prefilter(durations: &[Option<f64>]) -> PrefilterCount {
    let mut pairs_total = 0u64;
    let mut pairs_survived = 0u64;
    for (i, di) in durations.iter().enumerate() {
        for dj in durations.iter().skip(i + 1) {
            pairs_total += 1;
            match (di, dj) {
                // Le `continue` de production ne s'applique QUE si les deux durées sont connues :
                // une durée NULL laisse passer la paire.
                (Some(a), Some(b)) if (a - b).abs() > DURATION_TOL_SEC => {}
                _ => pairs_survived += 1,
            }
        }
    }
    PrefilterCount {
        pairs_total,
        pairs_survived,
    }
}

// ── benchmarks ───────────────────────────────────────────────────────────────

/// (1) Coût unitaire d'une comparaison. Indépendant de toute hypothèse sur la bibliothèque.
#[test]
#[ignore]
fn bench_dedup_unit_similarity_cost() {
    println!("\n=== Phase 4 · coût unitaire de fingerprint::similarity ===");
    println!("(0,1238 s d'audio par item — une piste de 6 min ≈ 2907 items)\n");

    for &minutes in &[3.0f64, 6.0, 10.0] {
        let items = (minutes * 60.0 / ITEM_DURATION_SEC) as usize;
        let a = synth_fingerprint(1, items);
        let b = synth_fingerprint(2, items);
        let dup = synth_reencode(&a, 3);

        // Assez de répétitions pour sortir du bruit de l'horloge, peu pour que le bench reste court.
        const REPS: u32 = 200;

        let t = Instant::now();
        let mut sink = 0.0f32;
        for _ in 0..REPS {
            sink += fingerprint::similarity(&a, &b);
        }
        let unrelated = t.elapsed() / REPS;

        let t = Instant::now();
        for _ in 0..REPS {
            sink += fingerprint::similarity(&a, &dup);
        }
        let matching = t.elapsed() / REPS;

        println!(
            "  {minutes:>4.0} min ({items:>5} items, {:>6.1} ko) : \
             paire sans rapport {unrelated:>10.2?} · paire doublon {matching:>10.2?}",
            (items * 4) as f64 / 1024.0,
        );
        // `sink` empêche l'optimiseur de supprimer les appels en release.
        assert!(sink >= 0.0);
    }
}

/// (2) Taux de survie du pré-filtre + (4) empreinte mémoire. Aucun `similarity` appelé.
#[test]
#[ignore]
fn bench_dedup_prefilter_survival() {
    println!("\n=== Phase 4 · survie du pré-filtre de durée (±2 s) ===");
    println!("ATTENTION : chiffres produits par la distribution PROVISOIRE de `duration_for`");
    println!("(étalement uniforme 2→10 min). Une vraie bibliothèque est agglutinée : lire ces");
    println!("taux comme un PLANCHER.\n");

    for &n in &[1_000usize, 5_000, 15_000] {
        let (rows, fps) = synth_rows(n, n / 100);
        let durations: Vec<Option<f64>> = rows.iter().map(|r| r.duration).collect();

        let t = Instant::now();
        let c = count_prefilter(&durations);
        let counting = t.elapsed();

        let fp_bytes: usize = fps
            .iter()
            .map(|f| f.as_ref().map_or(0, |v| v.len() * 4))
            .sum();

        println!(
            "  n = {n:>6} : {:>13} paires · {:>12} survivent ({:>6.3} %) · \
             énumération seule {counting:>9.2?} · empreintes en RAM {:>7.1} Mo",
            c.pairs_total,
            c.pairs_survived,
            100.0 * c.pairs_survived as f64 / c.pairs_total as f64,
            fp_bytes as f64 / (1024.0 * 1024.0),
        );
    }
}

/// (3) `group_duplicates` en bout à bout. Volontairement borné : à 15 000 pistes ce bench peut
/// durer plusieurs minutes, et c'est en soi le résultat.
#[test]
#[ignore]
fn bench_dedup_group_duplicates_end_to_end() {
    println!("\n=== Phase 4 · group_duplicates bout à bout ===");
    println!("(mêmes réserves sur `duration_for` que le bench de survie)\n");

    for &n in &[500usize, 1_000, 2_000, 4_000] {
        let (rows, fps) = synth_rows(n, n / 100);
        let t = Instant::now();
        let groups = group_duplicates(&rows, &fps);
        let elapsed = t.elapsed();
        println!(
            "  n = {n:>6} : {elapsed:>10.2?} · {} groupes trouvés",
            groups.len()
        );
    }
    println!("\n  (extrapoler vers 15 000 avec le coût unitaire + le taux de survie —");
    println!("   pas en supposant un n² propre : le pré-filtre casse la courbe.)");
}

/// (5) **L'ÉNUMÉRATION NUE** — le balayage seul, celui qui reste quand le pré-filtre écarte tout.
///
/// C'est le chiffre que l'issue #38 réclame comme PREMIER geste, avant tout correctif : les quatre
/// benchs ci-dessus mesurent le coût unitaire, le taux de survie, le bout à bout et la RAM, mais
/// aucun ne dit ce que coûte le simple fait de PARCOURIR `n²/2` paires. Aucune empreinte n'est
/// construite ici — seules les durées, ce qui rend le `n = 100 000` tenable en RAM (≈ 1,6 Mo au
/// lieu de ≈ 1,1 Go).
///
/// Ce que disait le chiffre : mesuré le 2026-08-20 sur cette machine, la double boucle seule
/// coûtait **75,77 ms à 15 000 pistes** et **3,22 s à 100 000** — ni « quelques centaines de
/// millisecondes » (ce qui aurait rendu #38 opportuniste), ni la minute (ce qui en aurait fait un
/// mur), mais un plancher bien réel qui grandit en `n²` : ×6,7 sur `n`, ×42 sur le temps.
///
/// Les deux formes sont mesurées dans la MÊME exécution, sur le MÊME corpus, pour que l'écart soit
/// lisible sans comparer deux machines ou deux jours :
/// - la double boucle d'origine (`count_prefilter`), conservée exprès comme référence ;
/// - `dedup::for_each_candidate_pair`, qui trie puis ne parcourt que la fenêtre de tolérance.
///
/// Le bench ASSERTE que les deux rendent le même nombre de paires survivantes. Un gain de vitesse
/// obtenu en oubliant des paires serait un faux gain, et un bench qui se contente d'imprimer deux
/// durées ne le verrait pas.
#[test]
#[ignore]
fn bench_dedup_bare_enumeration() {
    println!("\n=== #38 · énumération nue (durées seules, aucun `similarity` appelé) ===");
    println!("(mêmes réserves sur `duration_for` que les benchs ci-dessus : étalement uniforme");
    println!(" 2→10 min, donc un taux de survie PLANCHER — une vraie bibliothèque s'agglutine.)\n");

    for &n in &[15_000usize, 100_000] {
        let durations = synth_durations(n, n / 100);

        let t = Instant::now();
        let naive = count_prefilter(&durations);
        let naive_elapsed = t.elapsed();

        let t = Instant::now();
        let mut visited = 0u64;
        crate::dedup::for_each_candidate_pair(&durations, |_, _| visited += 1);
        let windowed_elapsed = t.elapsed();

        assert_eq!(
            visited, naive.pairs_survived,
            "la fenêtre doit rendre EXACTEMENT les paires que le pré-filtre laissait passer"
        );

        println!(
            "  n = {n:>7} · double boucle      : {naive_elapsed:>10.2?} · {:>13} paires parcourues",
            naive.pairs_total,
        );
        println!(
            "  {:>11}   fenêtre glissante : {windowed_elapsed:>10.2?} · {visited:>13} paires \
             parcourues ({:>6.3} % du total) · ×{:.1} plus rapide",
            "",
            100.0 * visited as f64 / naive.pairs_total as f64,
            naive_elapsed.as_secs_f64() / windowed_elapsed.as_secs_f64().max(f64::MIN_POSITIVE),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le corpus doit produire de VRAIS doublons, sinon le bench bout à bout mesure un chemin mort
    /// (aucun `union`, aucun `min_sim`).
    #[test]
    fn synth_reencode_is_recognised_as_a_duplicate() {
        let items = (360.0 / ITEM_DURATION_SEC) as usize;
        let base = synth_fingerprint(42, items);
        let dup = synth_reencode(&base, 43);
        let other = synth_fingerprint(99, items);

        let s_dup = fingerprint::similarity(&base, &dup);
        let s_other = fingerprint::similarity(&base, &other);
        assert!(
            s_dup >= fingerprint::MATCH_THRESHOLD,
            "un ré-encodage synthétique doit passer le seuil, obtenu {s_dup}"
        );
        assert!(
            s_other < fingerprint::MATCH_THRESHOLD,
            "deux pistes sans rapport ne doivent pas matcher, obtenu {s_other}"
        );
    }

    /// La tolérance dupliquée ici doit rester celle de la production, sinon tous les taux de survie
    /// mesurés sont faux sans le dire.
    #[test]
    fn duration_prefilter_tolerance_still_matches_production() {
        let src = include_str!("dedup.rs");
        let line = src
            .lines()
            .find(|l| l.contains("DURATION_MATCH_TOL_SEC") && l.contains("f64 ="))
            .expect("constante DURATION_MATCH_TOL_SEC introuvable dans dedup.rs");
        assert!(
            line.contains(&format!("{DURATION_TOL_SEC:.1}")),
            "dedup.rs a changé de tolérance ({line}) — mettre DURATION_TOL_SEC à jour ici"
        );
    }

    /// Le comptage doit refléter la règle réelle : une durée inconnue NE fait PAS écarter la paire.
    #[test]
    fn unknown_duration_survives_the_prefilter() {
        // 100 s vs 400 s : très au-delà de la tolérance, mais l'une des trois est inconnue.
        let durations = vec![Some(100.0), None, Some(400.0)];
        let c = count_prefilter(&durations);
        assert_eq!(c.pairs_total, 3);
        // (1,2) et (2,3) survivent par durée inconnue ; (1,3) est écartée.
        assert_eq!(c.pairs_survived, 2);
    }

    /// `synth_durations` doit poser les doublons EXACTEMENT là où `synth_rows` les attend, sinon
    /// les deux corpus divergeraient en silence et les benchs ne mesureraient plus la même chose.
    #[test]
    fn synth_durations_matches_the_rows_it_feeds() {
        let (rows, _fps) = synth_rows(40, 4);
        let durations = synth_durations(40, 4);
        let from_rows: Vec<Option<f64>> = rows.iter().map(|r| r.duration).collect();
        assert_eq!(from_rows, durations);
        // Les 4 premières paires voisines partagent leur durée — c'est ce qui les rend doublons.
        for k in 0..4 {
            assert_eq!(durations[2 * k + 1], durations[2 * k]);
        }
        assert_ne!(
            durations[9], durations[8],
            "au-delà de dup_pairs, plus de partage"
        );
    }
}
