//! Passe de re-verdict au démarrage : rejoue `analysis::verdict::verdict` sur les MESURES DÉJÀ
//! STOCKÉES (`tracks.report_json`), sans ré-analyse, pour toute ligne dont `verdict_ver` n'est plus
//! la version courante.
//!
//! **Pourquoi elle existe (2026-09-01, bump `VERDICT_CACHE_VERSION` 1 → 2, `8ac3a23`).**
//! Le pool (`worker::select_needing_analysis`, alors `select_pending`) bornait sa reprise à
//! `status='pending'` — « la bibliothèque RANGÉE n'est jamais reprise ici […] cette décision-là
//! appartient au jour du bump ». C'est ce jour-là. Sans cette passe, une piste rangée à
//! `verdict_ver` périmée n'est reprise par personne : `verdict::cached` efface son verdict à la
//! lecture, pour toujours — Bibliothèque sans badge, compte « à re-sourcer » faux. Mesuré sur la
//! base réelle au moment de l'écriture : 15 lignes rangées + 1 en re-sourcing dans cet état.
//!
//! **Ce qu'elle ne couvre pas, et qui a un autre filet depuis le 2026-09-09 (issue #59).** Le
//! filtre `report_cache_ver = courant` ci-dessous est nécessaire (un rapport d'une forme antérieure
//! rendrait un verdict sur des valeurs par défaut), mais il laisse passer toute ligne rangée
//! analysée AVANT un bump de rapport — et le bump v10 du 2026-09-02 est arrivé avec le bump de
//! verdict v3 : rien n'a été re-stampé sur ces lignes, et `cached()` les effaçait quand même.
//! Depuis #59 le pool reprend aussi `filed` et `resourcing` (après la file) : ce que cette passe
//! saute ou sort du domaine finit ré-analysé pour de vrai, au lieu de rester « — » à vie.
//!
//! **Ce n'est PAS une migration** (`db.rs::MIGRATIONS` reste intouché) : elle est idempotente,
//! versionnée par `verdict_ver`, tourne à chaque lancement et ne fait rien quand tout est courant.
//! Une migration s'exécute une fois et ne rattrape pas le bump SUIVANT.
//!
//! **Idempotence : une exception, et elle est bornée.** Les deux issues qui ÉCRIVENT sortent la
//! ligne du filtre pour de bon — `Restamp` pose la version courante, `Clear` remet `verdict` à NULL
//! et le prédicat `verdict IS NOT NULL` l'écarte. Reste `SkipParse` : une ligne dont le
//! `report_json` est illisible **à la version de rapport courante** garde son verdict périmé, donc
//! rematche à chaque lancement. C'est accepté — le coût est une désérialisation ratée par ligne, et
//! l'alternative (écraser un verdict au motif qu'on n'a pas su relire la mesure) serait pire.
//! Verrouillé par `deux_passes_de_suite_la_seconde_ne_fait_rien`.
//!
//! Coût : lecture + désérialisation de rapports en cache (~39 ko chacun depuis le retrait de la
//! grille de spectrogramme, 2026-08-03). Aucun décodage audio, aucune FFT.
//!
//! **Chiffre mesuré, pas un adjectif** — `cout_de_la_passe_sur_3386_lignes`, `--release`,
//! 2026-09-01 : **420 / 433 / 434 ms** sur trois exécutions, pour 3 386 lignes TOUTES à re-juger et
//! à réécrire, soit le pire cas (le jour d'un bump). Rapports du banc à 23 ko, en-dessous des
//! ~39 ko réels : le coût sur la vraie base est donc au-dessus de ces 430 ms, sans changer d'ordre
//! de grandeur. Le cas ordinaire — rien de périmé — ne lit aucune ligne et ne réécrit rien.

use crate::analysis::verdict::{self, HfFlatness, NotMeasured};
use crate::analysis::{AnalysisReport, Rail};
use rusqlite::Connection;

/// Comptes de fin de passe. Rendus plutôt que seulement journalisés pour que les tests mesurent la
/// même chose que le log.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    /// Lignes re-jugées et re-stampées à la version courante.
    pub restamped: usize,
    /// Lignes sorties du domaine (`NotMeasured`) : verdict et version remis à NULL.
    pub out_of_domain: usize,
    /// Lignes dont le `report_json` n'a pas pu être désérialisé — SAUTÉES, laissées intactes.
    pub skipped_parse: usize,
}

/// Ce que la relecture d'une ligne a décidé. Trois cas, calculés HORS écriture pour que la boucle
/// SQL qui suit ne fasse plus que du SQL.
enum Outcome {
    /// Nouveau verdict, sous sa forme stockée (`worker::verdict_str`).
    Restamp(&'static str),
    /// `verdict()` a refusé de trancher : c'est l'état « non analysé » standard, pas une erreur.
    Clear,
    /// Rapport illisible (forme antérieure, écriture tronquée) : on ne touche à rien.
    SkipParse,
}

/// Rejoue le verdict pour une ligne, à partir de son seul rapport.
///
/// ⚠️ **Sémantique reproduite de `analysis::analyze` (analysis/mod.rs:311-320), pas réinventée.**
/// Là-bas, les cinq arguments sont `cutoff_hz`, `tag.declared_rail`, `tag.declared_bitrate`,
/// `content_rail`, et `HfFlatness { fixed_db: spec_res.hf_flatness_db, top_db:
/// spec_res.hf_flatness_top_db }` — tous présents à l'identique dans `AnalysisReport`, à une
/// exception : `content_rail`.
///
/// `content_rail` n'est pas sérialisé. Ce qui l'est, c'est `container_mismatch`, dont `analyze`
/// pose la définition exacte (mod.rs:305) : `tag.declared_rail == Rail::Lossless && content_rail ==
/// Rail::Lossy`. Or `verdict()` ne lit `content_rail` que par le test `content_rail == Rail::Lossy`
/// SOUS le bras `Rail::Lossless` (verdict.rs:243-247) — le seul court-circuit, et il passe bien
/// AVANT tout le reste, y compris avant l'absence de mesure. Rendre `Rail::Lossy` quand
/// `container_mismatch` est vrai et `Rail::Unknown` sinon reproduit donc ce test à l'identique :
/// `Unknown` « never triggers this short-circuit » (doc de `verdict()`), et aucun autre bras ne
/// regarde `content_rail`. Reconstruction exacte, pas approchée.
///
/// ⚠️ **Le troisième signal, lui, ne se reconstruit PAS — et c'est délibéré (issue #52,
/// 2026-09-02).** `quant_likelihood` se calcule sur le PCM DÉCODÉ, que cette passe n'a pas : elle
/// rejoue le verdict depuis `report_json`, sans ouvrir un seul fichier. Deux options auraient
/// existé — lire la valeur stockée dans le rapport, ou passer `None`. C'est `None` :
///
/// - un rapport écrit AVANT #52 (`REPORT_CACHE_VERSION` 9) n'a pas le champ, donc il vaudrait
///   `None` de toute façon — et le filtre `report_cache_ver = ?2` de [`run`] écarte déjà ces
///   lignes ;
/// - un rapport écrit APRÈS porte une valeur qui a DÉJÀ produit son verdict à l'analyse, à la même
///   version de λ : la rejouer ne changerait rien tant que λ ne bouge pas, et le jour où λ bouge
///   c'est le seuil qu'il faut re-mesurer, pas une valeur en cache qu'il faut re-seuiller.
///
/// Conséquence assumée, écrite aussi sur `VERDICT_CACHE_VERSION` : **une piste rangée garde le
/// verdict que la platitude et la falaise lui donnent, jusqu'à une ré-analyse RÉELLE**. La passe la
/// re-stampe à la version courante ; le troisième signal ne l'atteindra qu'à l'ouverture en Revue
/// (`ipc::analyze_path`) ou par une reprise du pool. L'alternative serait de re-décoder toute la
/// bibliothèque au démarrage — plusieurs dizaines de minutes de CPU.
fn replay(r: &AnalysisReport) -> Result<crate::analysis::Verdict, NotMeasured> {
    let content_rail = if r.container_mismatch {
        Rail::Lossy
    } else {
        Rail::Unknown
    };
    verdict::verdict(
        r.cutoff_hz,
        r.declared_rail,
        r.declared_bitrate,
        content_rail,
        HfFlatness {
            fixed_db: r.hf_flatness_db,
            top_db: r.hf_flatness_top_db,
        },
        // Pas de PCM ici : voir le ⚠️ ci-dessus. `None` ne dégrade jamais un verdict.
        None,
    )
}

/// Passe complète. À appeler UNE fois au démarrage, avant que la connexion parte dans le `State`.
///
/// `&mut Connection` parce que toutes les écritures tiennent dans une transaction unique : la passe
/// est soit entièrement appliquée, soit pas du tout — un arrêt au milieu ne laisse pas la moitié de
/// la bibliothèque à une version et l'autre moitié à l'autre.
pub fn run(conn: &mut Connection) -> rusqlite::Result<Stats> {
    // Le filtre reproduit celui que `verdict::cached` applique à la lecture : version absente
    // (ligne d'avant la v22) ou différente = pas de verdict courant. Les trois gardes sur
    // `report_json` écartent respectivement l'absence, la sentinelle d'échec de `persist_failure`
    // (`''`) et le NULL — `typeof(...)='null'` plutôt que `IS NULL` pour la même raison qu'ailleurs
    // (voir `worker::select_needing_analysis`) : ne pas charger la valeur pour découvrir qu'elle est absente.
    //
    // `verdict IS NOT NULL` : la MÊME borne que `worker::select_needing_analysis`, et pour le même
    // invariant (`worker.rs:161-163`) — « `verdict` est non-NULL si et seulement s'il reflète
    // l'analyse réussie la plus récente du fichier COURANT ». Une ligne sans verdict n'est pas
    // périmée, elle est non analysée : lui en poser un depuis un rapport qu'aucune analyse réussie
    // n'a validé casserait l'invariant, et c'est exactement l'état que `persist_failure` laisse
    // derrière lui. Effet second, et il est ce qui rend la passe idempotente : une ligne que la
    // passe précédente a remise à NULL (`Outcome::Clear`) sort du filtre au lancement suivant.
    //
    // `report_cache_ver = ?2` : un rapport d'une forme ANTÉRIEURE se désérialise quand même — les
    // champs neufs portent `#[serde(default)]` (`default_peaks_step`, mod.rs:216) — et rendrait
    // donc un verdict calculé sur des valeurs par défaut, stampé à la version courante. Le
    // désaccord de version est précisément ce qui dit « ces mesures ne sont plus celles du moteur
    // courant » ; la ligne se répare par les chemins existants (`ipc::analyze_path`, le pool), qui
    // eux ont le fichier sous la main.
    let mut stmt = conn.prepare(
        "SELECT id, report_json FROM tracks \
         WHERE report_json IS NOT NULL AND report_json != '' AND typeof(report_json) != 'null' \
           AND verdict IS NOT NULL \
           AND report_cache_ver = ?2 \
           AND (verdict_ver IS NULL OR verdict_ver != ?1)",
    )?;
    let decided: Vec<(i64, Outcome)> = stmt
        .query_map(
            rusqlite::params![
                verdict::VERDICT_CACHE_VERSION,
                crate::analysis::REPORT_CACHE_VERSION
            ],
            |row| {
                let id: i64 = row.get(0)?;
                let json: String = row.get(1)?;
                // La désérialisation se fait DANS la boucle de lecture pour ne jamais garder les
                // milliers de rapports en mémoire en même temps : seul le verdict sort d'ici.
                let outcome = match serde_json::from_str::<AnalysisReport>(&json) {
                    Ok(r) => match replay(&r) {
                        Ok(v) => Outcome::Restamp(crate::worker::verdict_str(v)),
                        Err(_) => Outcome::Clear,
                    },
                    // Un rapport d'une forme antérieure se répare par les chemins existants
                    // (`ipc::analyze_path`, le pool) — pas ici, où l'on n'a aucune mesure fiable.
                    Err(_) => Outcome::SkipParse,
                };
                Ok((id, outcome))
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);

    let mut stats = Stats::default();
    let tx = conn.transaction()?;
    for (id, outcome) in &decided {
        match outcome {
            Outcome::Restamp(v) => {
                tx.execute(
                    "UPDATE tracks SET verdict=?2, verdict_ver=?3 WHERE id=?1",
                    rusqlite::params![id, v, verdict::VERDICT_CACHE_VERSION],
                )?;
                stats.restamped += 1;
            }
            Outcome::Clear => {
                tx.execute(
                    "UPDATE tracks SET verdict=NULL, verdict_ver=NULL WHERE id=?1",
                    rusqlite::params![id],
                )?;
                stats.out_of_domain += 1;
            }
            Outcome::SkipParse => stats.skipped_parse += 1,
        }
    }
    tx.commit()?;
    Ok(stats)
}

/// Passe + journal, en un seul appel pour le démarrage. Un SEUL log en fin de passe : une ligne par
/// piste noierait le démarrage sur une bibliothèque de plusieurs milliers d'entrées.
pub fn run_and_log(conn: &mut Connection) -> rusqlite::Result<Stats> {
    let stats = run(conn)?;
    if stats != Stats::default() {
        log::info!(
            "re-verdict au démarrage (version {}) : {} re-jugées, {} sorties de domaine, {} sautées (rapport illisible)",
            verdict::VERDICT_CACHE_VERSION,
            stats.restamped,
            stats.out_of_domain,
            stats.skipped_parse
        );
    }
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worker::tests::fake_report;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::run_migrations(&conn).unwrap();
        conn
    }

    /// Seed par la MÊME forme que la production : un vrai `AnalysisReport` sérialisé par serde,
    /// jamais un JSON écrit à la main (CLAUDE.md § Méthode). Le paramètre `json` reste ouvert pour
    /// le seul cas qui a besoin d'autre chose : le rapport illisible.
    ///
    /// `report_cache_ver` est stampé à la version COURANTE — c'est ce que `worker::persist_report`
    /// écrit, donc l'état d'une ligne normale. Le seul test qui a besoin d'autre chose le change
    /// après coup, sur le modèle de `cached_report_ne_sert_que_la_version_courante`.
    fn seed(
        conn: &Connection,
        status: &str,
        verdict: Option<&str>,
        ver: Option<i64>,
        json: &str,
    ) -> i64 {
        conn.execute(
            "INSERT INTO tracks (path, filename, status, verdict, verdict_ver, report_json,
                                 report_cache_ver, analyzed_at)
             VALUES (?1, 'x.flac', ?2, ?3, ?4, ?5, ?6, datetime('now'))",
            rusqlite::params![
                format!("t{}.flac", rand_path()),
                status,
                verdict,
                ver,
                json,
                crate::analysis::REPORT_CACHE_VERSION
            ],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    /// Chemin unique — la colonne `path` est UNIQUE et plusieurs lignes cohabitent dans un test.
    fn rand_path() -> String {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        N.fetch_add(1, Ordering::Relaxed).to_string()
    }

    fn read(conn: &Connection, id: i64) -> (Option<String>, Option<i64>) {
        conn.query_row(
            "SELECT verdict, verdict_ver FROM tracks WHERE id=?1",
            rusqlite::params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
    }

    /// (a) Le cas qui motive toute la passe : une piste RANGÉE à version périmée et rapport valide —
    /// re-jugée ICI, sans décodage (le pool la reprendrait aussi depuis #59, mais en la re-décodant).
    #[test]
    fn ligne_rangee_perimee_est_rejugee_et_restampee() {
        let mut conn = db();
        // `fake_report()` : Lossless déclaré, coupure 16 kHz → sous la falaise → Fake.
        let json = serde_json::to_string(&fake_report()).unwrap();
        let id = seed(&conn, "filed", Some("ok"), Some(1), &json);

        let stats = run(&mut conn).unwrap();

        assert_eq!(
            read(&conn, id),
            (
                Some("fake".to_string()),
                Some(verdict::VERDICT_CACHE_VERSION)
            )
        );
        assert_eq!(stats.restamped, 1);
    }

    /// (b) Le rapport ne porte aucune mesure exploitable : `verdict()` rend `NotMeasured`, et l'état
    /// à écrire est celui, existant, du « non analysé » — verdict ET version à NULL.
    #[test]
    fn rapport_hors_domaine_remet_verdict_et_version_a_null() {
        let mut conn = db();
        let mut r = fake_report();
        r.cutoff_hz = verdict::NO_MEASUREMENT_HZ; // aucune trame décodée
        let json = serde_json::to_string(&r).unwrap();
        let id = seed(&conn, "filed", Some("fake"), Some(1), &json);

        let stats = run(&mut conn).unwrap();

        assert_eq!(read(&conn, id), (None, None));
        assert_eq!(stats.out_of_domain, 1);
        assert_eq!(stats.restamped, 0);
    }

    /// (c) La sentinelle d'échec de `persist_failure`. Elle existe pour que rien ne reprenne la
    /// ligne en boucle : la passe doit la laisser exactement où elle est.
    #[test]
    fn sentinelle_report_json_vide_reste_intacte() {
        let mut conn = db();
        let id = seed(&conn, "filed", Some("ok"), Some(1), "");

        let stats = run(&mut conn).unwrap();

        assert_eq!(read(&conn, id), (Some("ok".to_string()), Some(1)));
        assert_eq!(stats, Stats::default());
    }

    /// (d) Idempotence : une ligne déjà à la version courante n'est pas touchée, même si son verdict
    /// stocké désaccorde avec ce que le moteur rendrait. C'est ce qui rend la passe rejouable à
    /// chaque lancement sans coût.
    #[test]
    fn version_courante_non_touchee() {
        let mut conn = db();
        let json = serde_json::to_string(&fake_report()).unwrap(); // rendrait "fake"
        let id = seed(
            &conn,
            "filed",
            Some("ok"),
            Some(verdict::VERDICT_CACHE_VERSION),
            &json,
        );

        let stats = run(&mut conn).unwrap();

        assert_eq!(
            read(&conn, id),
            (Some("ok".to_string()), Some(verdict::VERDICT_CACHE_VERSION))
        );
        assert_eq!(stats, Stats::default());
    }

    /// (e) Rapport illisible : sauté et COMPTÉ, ligne intacte. Écrire NULL ici reviendrait à
    /// détruire un verdict au motif qu'on n'a pas su relire la mesure.
    #[test]
    fn json_malforme_est_saute_et_la_ligne_reste_intacte() {
        let mut conn = db();
        let id = seed(&conn, "filed", Some("ok"), Some(1), "{pas du json");

        let stats = run(&mut conn).unwrap();

        assert_eq!(read(&conn, id), (Some("ok".to_string()), Some(1)));
        assert_eq!(stats.skipped_parse, 1);
        assert_eq!(stats.restamped, 0);
        assert_eq!(stats.out_of_domain, 0);
    }

    /// Le court-circuit de désaccord de conteneur passe AVANT l'absence de mesure dans `verdict()`.
    /// Comme `content_rail` n'est pas sérialisé, c'est la reconstruction depuis `container_mismatch`
    /// qui le porte : sans elle, ce rapport sortirait « hors domaine » au lieu de rester Fake.
    ///
    /// Le verdict de départ est PÉRIMÉ mais non-NULL (`"ok"`, version 1) : depuis le prédicat
    /// `verdict IS NOT NULL`, une ligne à verdict NULL ne serait plus reprise du tout, et ce test
    /// mesurerait alors le filtre au lieu du court-circuit.
    #[test]
    fn container_mismatch_reconstruit_le_court_circuit_de_fraude() {
        let mut conn = db();
        let mut r = fake_report();
        r.container_mismatch = true;
        r.cutoff_hz = verdict::NO_MEASUREMENT_HZ;
        let json = serde_json::to_string(&r).unwrap();
        let id = seed(&conn, "filed", Some("ok"), Some(1), &json);

        let stats = run(&mut conn).unwrap();

        assert_eq!(
            read(&conn, id),
            (
                Some("fake".to_string()),
                Some(verdict::VERDICT_CACHE_VERSION)
            )
        );
        assert_eq!(stats.restamped, 1);
    }

    /// (f) L'invariant de `worker.rs:161-163` : `verdict` non-NULL SI ET SEULEMENT SI il reflète la
    /// dernière analyse RÉUSSIE du fichier courant. Une ligne sans verdict n'est pas périmée, elle
    /// est non analysée — c'est très exactement l'état que `persist_failure` laisse derrière lui
    /// (verdict NULL, `verdict_ver` NULL, mais un `report_json` d'une analyse ANTÉRIEURE peut
    /// subsister sur d'autres chemins). Lui poser un verdict depuis ce seul rapport reviendrait à
    /// annoncer FAKE sur un fichier dont aucune analyse réussie ne dit rien.
    #[test]
    fn ligne_sans_verdict_n_en_recoit_jamais_un() {
        let mut conn = db();
        let json = serde_json::to_string(&fake_report()).unwrap(); // rendrait "fake"
        let id = seed(&conn, "filed", None, None, &json);

        let stats = run(&mut conn).unwrap();

        assert_eq!(read(&conn, id), (None, None));
        assert_eq!(stats, Stats::default());
    }

    /// (g) Rapport d'une forme ANTÉRIEURE. Il se désérialise quand même — les champs neufs portent
    /// `#[serde(default)]` — donc sans ce garde la passe rendrait un verdict calculé sur des
    /// valeurs par défaut, et le stamperait à la version courante : un faux verdict présenté comme
    /// frais. La ligne doit rester intacte et se réparer par les chemins qui ont le FICHIER.
    ///
    /// La mutation se porte sur la ligne, pas sur la `const` (modèle
    /// `cached_report_ne_sert_que_la_version_courante`) : `REPORT_CACHE_VERSION - 1` est l'état
    /// d'une ligne écrite avant le bump, quelle que soit la valeur courante de la constante.
    #[test]
    fn report_cache_ver_perime_laisse_la_ligne_intacte() {
        let mut conn = db();
        let json = serde_json::to_string(&fake_report()).unwrap();
        let id = seed(&conn, "filed", Some("ok"), Some(1), &json);
        conn.execute(
            "UPDATE tracks SET report_cache_ver=?2 WHERE id=?1",
            rusqlite::params![id, crate::analysis::REPORT_CACHE_VERSION - 1],
        )
        .unwrap();

        let stats = run(&mut conn).unwrap();

        assert_eq!(read(&conn, id), (Some("ok".to_string()), Some(1)));
        assert_eq!(stats, Stats::default());
    }

    /// (h) Idempotence, mesurée et pas seulement affirmée : la passe tourne à CHAQUE lancement, et
    /// c'est la seconde exécution qui dit si elle se calme. Les deux issues écrivantes doivent
    /// sortir leur ligne du filtre — `Restamp` par la version posée, `Clear` par `verdict IS NOT
    /// NULL`. Le seul rematch admis est le cas pathologique documenté en tête de module (rapport
    /// illisible à version courante), tenu hors de ce test pour que la seconde passe soit
    /// exactement `Stats::default()`.
    #[test]
    fn deux_passes_de_suite_la_seconde_ne_fait_rien() {
        let mut conn = db();
        let restampe = seed(
            &conn,
            "filed",
            Some("ok"),
            Some(1),
            &serde_json::to_string(&fake_report()).unwrap(),
        );
        let mut hors_domaine = fake_report();
        hors_domaine.cutoff_hz = verdict::NO_MEASUREMENT_HZ;
        let efface = seed(
            &conn,
            "filed",
            Some("fake"),
            Some(1),
            &serde_json::to_string(&hors_domaine).unwrap(),
        );

        let premiere = run(&mut conn).unwrap();
        assert_eq!(premiere.restamped, 1);
        assert_eq!(premiere.out_of_domain, 1);

        let seconde = run(&mut conn).unwrap();

        assert_eq!(
            seconde,
            Stats::default(),
            "la passe tourne a chaque lancement : une seconde passe qui reecrit signale un filtre qui ne se ferme pas"
        );
        // Et l'état laissé par la première passe n'a pas bougé.
        assert_eq!(
            read(&conn, restampe),
            (
                Some("fake".to_string()),
                Some(verdict::VERDICT_CACHE_VERSION)
            )
        );
        assert_eq!(read(&conn, efface), (None, None));
    }

    /// (i) Jumelage avec `worker::select_needing_analysis` : les deux lisent la même colonne pour la même
    /// raison, et une ligne PENDING à verdict périmé est le point où ils se croisent. Fige la paire
    /// (modèle `queue.rs::un_verdict_perime_vaut_besoin_d_analyse`) : sélectionnable AVANT la
    /// passe, restampée et donc plus sélectionnable APRÈS. Ce qu'aucun des deux ne doit produire :
    /// une ligne que la passe restampe et que le pool ré-analyse quand même.
    #[test]
    fn une_ligne_pending_perimee_est_restampee_donc_plus_selectionnee() {
        let mut conn = db();
        let json = serde_json::to_string(&fake_report()).unwrap();
        let id = seed(&conn, "pending", Some("ok"), Some(1), &json);

        assert_eq!(
            crate::worker::select_needing_analysis(&conn).unwrap(),
            vec![id],
            "avant la passe, le verdict perime vaut besoin d'analyse"
        );

        let stats = run(&mut conn).unwrap();

        assert_eq!(stats.restamped, 1);
        assert_eq!(
            read(&conn, id),
            (
                Some("fake".to_string()),
                Some(verdict::VERDICT_CACHE_VERSION)
            )
        );
        assert!(
            crate::worker::select_needing_analysis(&conn)
                .unwrap()
                .is_empty(),
            "restampee sans re-analyse : le pool n'a plus rien a reprendre sur cette ligne"
        );
    }

    /// (j) Le second bras de `NotMeasured`, `Rail` : conteneur non reconnu (`Rail::Unknown`). Il
    /// tombe par un chemin tout autre que `Cutoff` (verdict.rs:278 contre :250/:271) et doit sortir
    /// au même endroit — verdict et version à NULL, compté hors domaine.
    #[test]
    fn rail_indetermine_sort_du_domaine() {
        let mut conn = db();
        let mut r = fake_report();
        r.declared_rail = Rail::Unknown;
        let json = serde_json::to_string(&r).unwrap();
        let id = seed(&conn, "filed", Some("ok"), Some(1), &json);

        let stats = run(&mut conn).unwrap();

        assert_eq!(read(&conn, id), (None, None));
        assert_eq!(stats.out_of_domain, 1);
        assert_eq!(stats.restamped, 0);
    }

    /// (k) Le cas de loin le plus fréquent en vrai : le moteur rend le MÊME verdict qu'avant. La
    /// ligne doit quand même être restampée — sans quoi elle resterait à une version périmée, donc
    /// sans verdict courant aux yeux de `verdict::cached`, et rematcherait à chaque lancement. Elle
    /// est aussi COMPTÉE : le log dirait « 0 re-jugées » sur une passe qui a bien écrit.
    #[test]
    fn verdict_identique_est_quand_meme_restampe_et_compte() {
        let mut conn = db();
        let json = serde_json::to_string(&fake_report()).unwrap(); // rend "fake", déjà stocké
        let id = seed(&conn, "filed", Some("fake"), Some(1), &json);

        let stats = run(&mut conn).unwrap();

        assert_eq!(
            read(&conn, id),
            (
                Some("fake".to_string()),
                Some(verdict::VERDICT_CACHE_VERSION)
            ),
            "meme verdict, version neuve : sans le restamp la ligne n'a toujours pas de verdict courant"
        );
        assert_eq!(stats.restamped, 1);
    }

    /// (l) **Le troisième signal ne repasse PAS par ici, et le comportement est figé (#52).**
    ///
    /// Un rapport ambigu — bande pleine, aigu sous la plage des masters — portant une
    /// vraisemblance de quantification ÉCRASANTE en cache doit quand même ressortir `grey` de
    /// cette passe : `replay` passe `None`. Ce n'est pas une omission, c'est la décision écrite
    /// sur `replay` et sur `VERDICT_CACHE_VERSION` — une ligne rangée attend une ré-analyse
    /// réelle.
    ///
    /// Sans ce test, le jour où quelqu'un « complète » `replay` avec `r.quant_likelihood`, rien
    /// ne tomberait : la passe se mettrait à prononcer Faux sur des lignes rangées, depuis un
    /// champ que le filtre de version ne garantit qu'à la version courante. Le contrôle est dans
    /// le même corps — la MÊME entrée passée directement à `verdict()` avec la mesure rend Fake,
    /// donc le test mesure bien le chemin et pas une entrée inerte.
    #[test]
    fn la_vraisemblance_en_cache_ne_rejuge_rien_dans_cette_passe() {
        let mut conn = db();
        let mut r = fake_report();
        r.cutoff_hz = 21_000.0; // bande pleine
        r.hf_flatness_db = Some(-40.0); // aigu sous la plage des masters → Douteux
        r.hf_flatness_top_db = Some(-4.0);
        r.quant_likelihood = Some(0.45); // très au-dessus de QUANT_LAMBDA
        let json = serde_json::to_string(&r).unwrap();
        let id = seed(&conn, "filed", Some("ok"), Some(1), &json);

        let stats = run(&mut conn).unwrap();

        assert_eq!(
            read(&conn, id),
            (
                Some("grey".to_string()),
                Some(verdict::VERDICT_CACHE_VERSION)
            ),
            "la passe n'a pas de PCM : elle re-stampe le doute, elle ne prononce pas Faux"
        );
        assert_eq!(stats.restamped, 1);

        // Contrôle : la MÊME mesure, passée au juge, tranche bien. Sans lui on ne saurait pas si
        // l'entrée était simplement inerte.
        assert_eq!(
            verdict::verdict(
                r.cutoff_hz,
                r.declared_rail,
                r.declared_bitrate,
                Rail::Unknown,
                HfFlatness {
                    fixed_db: r.hf_flatness_db,
                    top_db: r.hf_flatness_top_db,
                },
                r.quant_likelihood,
            ),
            Ok(crate::analysis::Verdict::Fake)
        );
    }

    /// Coût de la passe à l'échelle de la base réelle (3 386 pistes au 2026-09-01). `#[ignore]`
    /// comme les autres mesures du dépôt (`bench_*.rs`) : elle n'a pas sa place dans la suite
    /// normale. Rapports gonflés par l'enveloppe de pics, seul champ volumineux qui survit au
    /// retrait de la grille de spectrogramme — 23 ko chacun À LA MESURE, en-dessous des ~39 ko du
    /// cache réel (le banc annonçait « ~39 ko » sans que sa propre sortie le dise).
    ///
    /// Résultat 2026-09-01, `--release` : 420 / 433 / 434 ms sur trois exécutions.
    ///
    /// Lancer : `cargo test --release --lib cout_de_la_passe -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn cout_de_la_passe_sur_3386_lignes() {
        const N: usize = 3386;
        let mut conn = db();
        let mut r = fake_report();
        // ~4 800 f32 sérialisés en décimal ≈ 39 ko de JSON, l'ordre de grandeur mesuré sur la base.
        r.peaks = (0..4800).map(|i| (i % 100) as f32 / 100.0).collect();
        let json = serde_json::to_string(&r).unwrap();
        for _ in 0..N {
            seed(&conn, "filed", Some("ok"), Some(1), &json);
        }
        let t = std::time::Instant::now();
        let stats = run(&mut conn).unwrap();
        let ms = t.elapsed().as_millis();
        println!(
            "re-verdict de {N} lignes ({} ko/rapport) : {ms} ms",
            json.len() / 1024
        );
        assert_eq!(stats.restamped, N);
    }
}
