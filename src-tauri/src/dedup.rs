//! Duplicate detection — name first (here), sound confirmation layered on later (see the M5
//! spec). The cheap name pre-filter normalizes each track's name (from its filename) into a
//! key (`naming::name_key`) and flags collisions: `name_dups` marks the queue, `find_duplicate`
//! reports the best name match for one track. The acoustic confirmation upgrades the match
//! `kind` from `name` to `both` when the sound agrees.

use crate::{fingerprint, naming};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// A candidate row loaded for matching: (id, path, status, folder, filename).
type CandRow = (i64, String, String, Option<String>, Option<String>);

/// A duplicate match for one track. `kind`: `name` (names agree) or `both` (name + sound).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DupMatch {
    pub id: i64,
    pub status: String,
    pub folder: Option<String>,
    pub filename: Option<String>,
    pub kind: String,
    pub score: f32,
}

/// One member of a duplicate group (a `filed` track acoustically identical to the others).
#[derive(Debug, Clone, Serialize)]
pub struct DupGroupMember {
    pub id: i64,
    pub path: String,
    pub filename: Option<String>,
    pub folder: Option<String>,
    pub format: Option<String>,
    pub bitrate: Option<i64>,
    pub duration: Option<f64>,
    pub truncated: bool,
    pub recommend_keep: bool,
    /// Human-readable reason, set only on the recommended member (e.g. "lossless, 1411 kbps").
    pub reason: Option<String>,
}

/// A group of 2+ `filed` tracks that are acoustically the same recording.
#[derive(Debug, Clone, Serialize)]
pub struct DupGroup {
    pub members: Vec<DupGroupMember>,
    /// Weakest pairwise similarity that linked the group together.
    pub similarity: f32,
}

/// Délègue à `tags::rail_from_ext`, l'autorité de la règle rail↔format, au lieu d'en recopier la
/// liste une quatrième fois — la copie locale avait déjà divergé (`alac` manquant).
fn is_lossless_fmt(fmt: &Option<String>) -> bool {
    fmt.as_deref()
        .map(|f| crate::analysis::tags::rail_from_ext(f) == crate::analysis::Rail::Lossless)
        .unwrap_or(false)
}

/// Index of the member to recommend keeping: lossless > lossy, then higher bitrate, then
/// longer duration, then non-truncated; ties keep the first occurrence.
fn pick_keep(members: &[DupGroupMember]) -> usize {
    let key = |m: &DupGroupMember| {
        (
            is_lossless_fmt(&m.format),
            m.bitrate.unwrap_or(-1),
            m.duration.map(|d| (d * 1000.0) as i64).unwrap_or(-1),
            !m.truncated,
        )
    };
    let mut best = 0usize;
    for i in 1..members.len() {
        if key(&members[i]) > key(&members[best]) {
            best = i;
        }
    }
    best
}

fn find_root(parent: &mut [usize], x: usize) -> usize {
    if parent[x] != x {
        parent[x] = find_root(parent, parent[x]);
    }
    parent[x]
}

fn union(parent: &mut [usize], a: usize, b: usize) {
    let (ra, rb) = (find_root(parent, a), find_root(parent, b));
    if ra != rb {
        parent[ra] = rb;
    }
}

/// Two `filed` tracks whose durations differ by more than this can't be the same recording,
/// so we skip the (expensive) fingerprint comparison. Only applied when BOTH durations are
/// known — a missing duration falls through to the full comparison (fail-open, no false skip).
const DURATION_MATCH_TOL_SEC: f64 = 2.0;

/// Énumère les paires `(i, j)`, `i < j`, que le pré-filtre de durée ne peut PAS écarter — et
/// seulement celles-là.
///
/// **C'est le point de l'issue #38.** Le pré-filtre vivait à l'intérieur de la double boucle : il
/// décidait qui payait `fingerprint::similarity`, pas qui était ÉNUMÉRÉ. `n²/2` paires étaient donc
/// parcourues quoi qu'il arrive — 5 × 10⁹ à 100 000 pistes, **3,22 s de balayage nu** avant tout
/// travail utile (mesuré, `bench_dedup::bench_dedup_bare_enumeration`).
///
/// Ici les durées connues sont triées une fois, puis balayées par fenêtre glissante : depuis une
/// piste de durée `d`, on s'arrête au premier voisin de durée `> d + DURATION_MATCH_TOL_SEC`,
/// puisque le tri garantit que tous les suivants le sont aussi. Coût : un tri `O(n log n)` plus une
/// visite par paire survivante.
///
/// **Équivalence, au bit près.** La condition d'origine est `(a - b).abs() <= tol`. Sur une suite
/// triée croissante on a `b >= a`, et l'arrondi IEEE-754 au plus proche est symétrique, donc
/// `b - a == -(a - b)` exactement, donc `(a - b).abs() == b - a`. La fenêtre ne teste pas une
/// approximation de la condition : elle teste la même expression, sans epsilon et sans marge.
///
/// **Ce que la fenêtre ne peut pas couvrir** passe par la condition d'origine appliquée telle
/// quelle, contre tout le reste :
/// - durée absente — la condition exige les DEUX durées, donc une paire dont un membre n'a pas de
///   durée n'est jamais écartée. C'est déjà le raisonnement de `load_dup_candidates`, et
///   `duplicate_scan_matches_full_scan_when_duration_is_null` le verrouille ;
/// - durée non finie (NaN, ±∞) — elle ne se trie pas, et son comportement sous la condition
///   d'origine est contre-intuitif : `NaN > tol` est faux donc une paire NaN SURVIT, `+∞` contre
///   une durée finie est écartée, mais `+∞` contre `+∞` survit puisque `∞ - ∞` vaut NaN. Rejouer
///   la condition littérale est la seule façon de ne pas se tromper, et le coût est nul : ces
///   lignes n'existent pas dans une base normale.
///
/// L'ordre de visite n'est PAS l'ordre d'index — c'est celui des durées croissantes. Les deux
/// consommateurs y sont insensibles : `link` fusionne les minimums sans dépendre de l'ordre des
/// arêtes, et `record_scanned` normalise `(a_id, b_id)` avant un `INSERT OR REPLACE`.
pub(crate) fn for_each_candidate_pair(
    durations: &[Option<f64>],
    mut visit: impl FnMut(usize, usize),
) {
    // `finite` se trie donc se fenêtre ; `loose` est tout le reste, comparé au prédicat littéral.
    let mut finite: Vec<(f64, usize)> = Vec::with_capacity(durations.len());
    let mut loose: Vec<usize> = Vec::new();
    for (i, d) in durations.iter().enumerate() {
        match *d {
            Some(v) if v.is_finite() => finite.push((v, i)),
            _ => loose.push(i),
        }
    }
    // `total_cmp` plutôt que `partial_cmp` : un ordre total, donc aucun `unwrap` à écrire sur un
    // `Option` de comparaison — et `unwrap` hors test est un interdit dur du projet.
    finite.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));

    for (p, &(dp, ip)) in finite.iter().enumerate() {
        for &(dq, iq) in &finite[p + 1..] {
            if dq - dp > DURATION_MATCH_TOL_SEC {
                break; // trié : tous les suivants sont plus loin encore
            }
            visit(ip.min(iq), ip.max(iq));
        }
    }

    for (k, &il) in loose.iter().enumerate() {
        for &(_, iq) in &finite {
            visit_if_within_tol(durations, il, iq, &mut visit);
        }
        for &io in &loose[k + 1..] {
            visit_if_within_tol(durations, il, io, &mut visit);
        }
    }
}

/// Applique la condition de durée D'ORIGINE, littéralement, puis visite la paire normalisée.
///
/// Réservée aux durées que la fenêtre ne sait pas ordonner (absentes ou non finies) : recopier la
/// condition plutôt que la réinterpréter est ce qui rend l'équivalence vraie sans raisonnement.
fn visit_if_within_tol<V: FnMut(usize, usize)>(
    durations: &[Option<f64>],
    a: usize,
    b: usize,
    visit: &mut V,
) {
    if let (Some(x), Some(y)) = (durations[a], durations[b]) {
        if (x - y).abs() > DURATION_MATCH_TOL_SEC {
            return;
        }
    }
    visit(a.min(b), a.max(b));
}

/// One row loaded from `tracks` for a duplicate scan — everything the O(n²) compare and the
/// group-building step need, so they can run without touching the connection.
pub(crate) struct DupScanRow {
    pub id: i64,
    pub path: String,
    pub filename: Option<String>,
    pub folder: Option<String>,
    pub format: Option<String>,
    pub bitrate: Option<i64>,
    pub duration: Option<f64>,
    pub truncated: bool,
    /// Empreinte en cache, **déjà passée par `fingerprint::cached`** au moment de la lecture : une
    /// version périmée arrive donc ici en `None`, indiscernable d'une absence, et se recalcule
    /// (`build_fingerprints`). Chaque chargeur qui remplit ce champ doit passer par cette fonction
    /// — sinon il ressert une empreinte produite par un autre algorithme (issue #39).
    pub fingerprint: Option<String>,
}

/// Brief read: every `filed` track plus its cached fingerprint. Intended to be called under a
/// short-held lock — the caller drops the lock before doing anything with the result.
/// **Réservé aux tests depuis la v19.** La production ne charge plus jamais TOUTES les empreintes
/// d'un coup : 166 Mo de RAM à 15 000 pistes (`bench_dedup.rs`) pour un chemin qui n'a besoin que
/// des nouvelles. Voir `load_unscanned_rows`, `load_dup_candidates` et `load_dup_group_rows`.
/// `#[cfg(test)]` fait échouer la compilation de tout futur appelant de production, au lieu de le
/// laisser réintroduire le coût en silence — même geste que `scan_library_duplicates` plus bas.
#[cfg(test)]
pub(crate) fn load_dup_scan_rows(conn: &Connection) -> rusqlite::Result<Vec<DupScanRow>> {
    // `target_format`, PAS `format` : cette dernière n'est écrite par aucun code de production (voir
    // `library::list_filed`). `is_lossless_fmt` recevait donc toujours NULL, `pick_keep` n'appliquait
    // JAMAIS son premier critère, et le membre recommandé se décidait au bitrate seul — un MP3 320
    // battait un AIFF sur tous les groupes mixtes. Le geste suivant que propose l'écran est une
    // suppression.
    let mut stmt = conn.prepare(
        "SELECT id, path, filename, folder, target_format, bitrate, duration, truncated, \
                fingerprint, fingerprint_ver \
         FROM tracks WHERE status='filed'",
    )?;
    let rows: Vec<DupScanRow> = stmt
        .query_map([], |r| {
            Ok(DupScanRow {
                id: r.get(0)?,
                path: r.get(1)?,
                filename: r.get(2)?,
                folder: r.get(3)?,
                // Ramené à une extension dès la lecture : tout l'aval (affichage ET
                // `is_lossless_fmt`) raisonne en extension, pas en clé de base.
                format: r
                    .get::<_, Option<String>>(4)?
                    .as_deref()
                    .and_then(crate::encode::Target::from_db_value)
                    .map(|t| t.ext().to_string()),
                bitrate: r.get(5)?,
                duration: r.get(6)?,
                truncated: r.get::<_, Option<i64>>(7)?.unwrap_or(0) != 0,
                fingerprint: fingerprint::cached(r.get(8)?, r.get(9)?),
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// Result of resolving every row's fingerprint (cached-decode or freshly computed from disk).
pub(crate) struct BuiltFingerprints {
    /// Aligned 1:1 with the `rows` slice passed to `build_fingerprints`.
    pub fps: Vec<Option<Vec<u32>>>,
    /// Newly-computed fingerprints (cache miss) still needing a DB write.
    pub to_persist: Vec<(i64, Vec<u32>)>,
}

/// Resolve every row's fingerprint: reuse the cached value already loaded on the row, or
/// decode/compute from disk. Pure — no connection touched, safe to run without any lock held.
pub(crate) fn build_fingerprints(rows: &[DupScanRow]) -> BuiltFingerprints {
    let mut fps = Vec::with_capacity(rows.len());
    let mut to_persist = Vec::new();
    for r in rows {
        match r.fingerprint.as_deref() {
            Some(s) if !s.is_empty() => fps.push(Some(fingerprint::decode(s))),
            _ => match fingerprint::compute_for_path(&r.path) {
                Ok(fp) => {
                    to_persist.push((r.id, fp.clone()));
                    fps.push(Some(fp));
                }
                Err(_) => fps.push(None),
            },
        }
    }
    BuiltFingerprints { fps, to_persist }
}

/// Persist newly-computed fingerprints (cache warm-up). Intended to be called under a
/// short-held lock, after the heavy compute is already done.
///
/// `fingerprint_ver` part dans le MÊME `UPDATE` que la valeur : une écriture qui l'oublierait
/// laisserait la ligne éternellement périmée aux yeux de `fingerprint::cached`, donc recalculée à
/// chaque passage — un décodage audio par piste et par appel, en silence.
pub(crate) fn persist_fingerprints(conn: &Connection, entries: &[(i64, Vec<u32>)]) {
    for (id, fp) in entries {
        let _ = conn.execute(
            "UPDATE tracks SET fingerprint=?2, fingerprint_ver=?3 WHERE id=?1",
            params![
                id,
                fingerprint::encode(fp),
                fingerprint::FINGERPRINT_CACHE_VERSION
            ],
        );
    }
}

/// Le comparateur + union-find lui-même. Pur — aucune connexion touchée, exécutable verrou
/// relâché. `fps` doit être aligné 1:1 avec `rows` (voir `build_fingerprints`).
///
/// **Plus en `O(n²)` depuis #38** : l'énumération passe par `for_each_candidate_pair`, qui trie les
/// durées puis ne parcourt que la fenêtre de tolérance. Le pré-filtre n'est plus une décision prise
/// à l'intérieur de la double boucle, c'est la boucle elle-même. L'ENSEMBLE des paires qui
/// atteignent `similarity` est inchangé, au bit près — voir la preuve dans le doc-comment de
/// `for_each_candidate_pair`, et `group_duplicates_matches_the_naive_pairwise_scan` qui la
/// verrouille contre une réimplémentation littérale de l'ancienne double boucle.
///
/// **Réservé aux tests depuis la v19.** La production passe par `groups_from_edges`, qui relit
/// des comparaisons déjà faites au lieu de les refaire (≈ 2 min 31 s à 15 000 pistes, mesuré).
/// Cette fonction reste la RÉFÉRENCE contre laquelle le chemin incrémental est vérifié — les deux
/// doivent rendre exactement les mêmes groupes, c'est ce que verrouillent les tests d'équivalence.
#[cfg(test)]
pub(crate) fn group_duplicates(rows: &[DupScanRow], fps: &[Option<Vec<u32>>]) -> Vec<DupGroup> {
    let n = rows.len();
    let mut parent: Vec<usize> = (0..n).collect();
    let mut min_sim: HashMap<usize, f32> = HashMap::new();
    let durations: Vec<Option<f64>> = rows.iter().map(|r| r.duration).collect();
    for_each_candidate_pair(&durations, |i, j| {
        let (Some(fi), Some(fj)) = (&fps[i], &fps[j]) else {
            return;
        };
        let s = fingerprint::similarity(fi, fj);
        if s >= fingerprint::MATCH_THRESHOLD {
            link(&mut parent, &mut min_sim, i, j, s);
        }
    });

    assemble_groups(rows, &mut parent, &min_sim)
}

/// Rattache `i` et `j` dans l'union-find, en tenant à jour le minimum de similarité du groupe.
///
/// Extrait pour être partagé par le scan complet (`group_duplicates`) et la reconstruction
/// depuis les arêtes persistées (`groups_from_edges`) : ce bloc porte une correction subtile, et
/// une seconde implémentation la perdrait tôt ou tard.
///
/// `min_sim` est indexé par RACINE, et une fusion change la racine : le minimum enregistré sous
/// l'ancienne racine devenait orphelin, jamais relu par le `min_sim.get(&root)` final. Sur un
/// groupe de 3 dont le lien le plus faible est la PREMIÈRE arête trouvée, `similarity`
/// sur-rapportait — un champ publié qui mentait sur la seule chose qu'il prétend dire. Fusionner
/// les minimums au moment du `union` est la correction ; ne pas dépendre du sens de `union` (on
/// relit la racine après).
fn link(parent: &mut [usize], min_sim: &mut HashMap<usize, f32>, i: usize, j: usize, s: f32) {
    let ra = find_root(parent, i);
    let rb = find_root(parent, j);
    if ra == rb {
        let e = min_sim.entry(ra).or_insert(s);
        if s < *e {
            *e = s;
        }
    } else {
        let prev_a = min_sim.remove(&ra);
        let prev_b = min_sim.remove(&rb);
        union(parent, i, j);
        let root = find_root(parent, i);
        let merged = prev_a
            .into_iter()
            .chain(prev_b)
            .chain(std::iter::once(s))
            .fold(f32::INFINITY, f32::min);
        min_sim.insert(root, merged);
    }
}

/// Assemblage final : des classes d'équivalence vers les `DupGroup` publiés, keeper recommandé
/// compris. Partagé par les deux chemins, pour la même raison que `link`.
fn assemble_groups(
    rows: &[DupScanRow],
    parent: &mut [usize],
    min_sim: &HashMap<usize, f32>,
) -> Vec<DupGroup> {
    let n = rows.len();
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        let root = find_root(parent, i);
        groups.entry(root).or_default().push(i);
    }

    let mut out = Vec::new();
    for (root, idxs) in groups {
        if idxs.len() < 2 {
            continue;
        }
        let mut members: Vec<DupGroupMember> = idxs
            .iter()
            .map(|&i| {
                let r = &rows[i];
                DupGroupMember {
                    id: r.id,
                    path: r.path.clone(),
                    filename: r.filename.clone(),
                    folder: r.folder.clone(),
                    format: r.format.clone(),
                    bitrate: r.bitrate,
                    duration: r.duration,
                    truncated: r.truncated,
                    recommend_keep: false,
                    reason: None,
                }
            })
            .collect();
        let keep = pick_keep(&members);
        members[keep].recommend_keep = true;
        let lossless = is_lossless_fmt(&members[keep].format);
        members[keep].reason = Some(match members[keep].bitrate {
            Some(b) => format!("{}, {b} kbps", if lossless { "lossless" } else { "lossy" }),
            None => (if lossless { "lossless" } else { "lossy" }).to_string(),
        });
        out.push(DupGroup {
            similarity: *min_sim.get(&root).unwrap_or(&1.0),
            members,
        });
    }
    out.sort_by(|a, b| a.members[0].id.cmp(&b.members[0].id));
    out
}

// ── Dédoublonnage incrémental (v19) ──────────────────────────────────────────
//
// Le scan complet coûte ≈ 2 min 31 s à 15 000 pistes (`bench_dedup.rs`), et il était rejoué
// dès qu'UNE piste était rangée : `library::filed_signature` vaut `(COUNT(*), MAX(id))`, donc
// tout rangement fait tomber le cache. Le travail réellement nécessaire dans ce cas est de
// comparer la nouvelle piste aux autres — ~20 ms.
//
// Les tables `dup_edges` / `dup_scanned` mémorisent le résultat des comparaisons plutôt que le
// comptage agrégé. Invariant : **toute paire de `dup_scanned` a été évaluée**.

/// Une arête persistée : deux pistes dont la similarité atteint `MATCH_THRESHOLD`.
pub(crate) struct DupEdge {
    pub a_id: i64,
    pub b_id: i64,
    pub similarity: f32,
}

/// Les pistes déjà comparées contre lesquelles `row` doit être évaluée.
///
/// Reproduit EXACTEMENT le pré-filtre de `group_duplicates` : une paire n'est écartée que si les
/// **deux** durées sont connues et distantes de plus de `DURATION_MATCH_TOL_SEC`. Une durée
/// inconnue — des deux côtés — laisse passer.
///
/// C'est le point où l'incrémental pourrait diverger du scan complet sans qu'aucune erreur ne
/// soit levée : écarter les durées inconnues serait moins cher et trouverait silencieusement
/// moins de doublons. Le coût de l'équivalence stricte est borné — il ne concerne que les pistes
/// sans durée, qui se comparent alors à toute la bibliothèque.
/// `duplicate_scan_matches_full_scan_when_duration_is_null` verrouille cette équivalence.
///
/// `status='filed'` en plus de la jointure sur `dup_scanned` : les deux devraient toujours
/// s'accorder, et si jamais ils divergent on préfère le statut réel de la piste.
pub(crate) fn load_dup_candidates(
    conn: &Connection,
    row: &DupScanRow,
) -> rusqlite::Result<Vec<DupScanRow>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.path, t.filename, t.folder, t.target_format, t.bitrate, t.duration, \
                t.truncated, t.fingerprint, t.fingerprint_ver \
         FROM tracks t \
         JOIN dup_scanned s ON s.track_id = t.id \
         WHERE t.status='filed' AND t.id <> ?1 \
           AND (?2 IS NULL OR t.duration IS NULL OR ABS(t.duration - ?2) <= ?3)",
    )?;
    let rows = stmt
        .query_map(params![row.id, row.duration, DURATION_MATCH_TOL_SEC], |r| {
            Ok(DupScanRow {
                id: r.get(0)?,
                path: r.get(1)?,
                filename: r.get(2)?,
                folder: r.get(3)?,
                format: r
                    .get::<_, Option<String>>(4)?
                    .as_deref()
                    .and_then(crate::encode::Target::from_db_value)
                    .map(|t| t.ext().to_string()),
                bitrate: r.get(5)?,
                duration: r.get(6)?,
                truncated: r.get::<_, Option<i64>>(7)?.unwrap_or(0) != 0,
                fingerprint: fingerprint::cached(r.get(8)?, r.get(9)?),
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// Les pistes `filed`, SANS leur empreinte.
///
/// `groups_from_edges` n'a besoin que des métadonnées : les comparaisons sont déjà faites et
/// vivent dans `dup_edges`. Charger les empreintes ici coûterait 166 Mo de RAM à 15 000 pistes
/// (mesuré, `bench_dedup.rs`) et ~250 Mo de TEXT lus depuis SQLite, pour les jeter aussitôt.
/// C'est le chemin NORMAL — celui qu'emprunte chaque visite du tableau de bord.
pub(crate) fn load_dup_group_rows(conn: &Connection) -> rusqlite::Result<Vec<DupScanRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, path, filename, folder, target_format, bitrate, duration, truncated \
         FROM tracks WHERE status='filed'",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(DupScanRow {
                id: r.get(0)?,
                path: r.get(1)?,
                filename: r.get(2)?,
                folder: r.get(3)?,
                format: r
                    .get::<_, Option<String>>(4)?
                    .as_deref()
                    .and_then(crate::encode::Target::from_db_value)
                    .map(|t| t.ext().to_string()),
                bitrate: r.get(5)?,
                duration: r.get(6)?,
                truncated: r.get::<_, Option<i64>>(7)?.unwrap_or(0) != 0,
                // Délibérément absente : voir le doc-comment. Ne PAS la remplir « au cas où ».
                fingerprint: None,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// Les pistes `filed` qui n'ont encore jamais été comparées.
pub(crate) fn load_unscanned_rows(conn: &Connection) -> rusqlite::Result<Vec<DupScanRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, path, filename, folder, target_format, bitrate, duration, truncated, \
                fingerprint, fingerprint_ver \
         FROM tracks \
         WHERE status='filed' AND id NOT IN (SELECT track_id FROM dup_scanned)",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(DupScanRow {
                id: r.get(0)?,
                path: r.get(1)?,
                filename: r.get(2)?,
                folder: r.get(3)?,
                format: r
                    .get::<_, Option<String>>(4)?
                    .as_deref()
                    .and_then(crate::encode::Target::from_db_value)
                    .map(|t| t.ext().to_string()),
                bitrate: r.get(5)?,
                duration: r.get(6)?,
                truncated: r.get::<_, Option<i64>>(7)?.unwrap_or(0) != 0,
                fingerprint: fingerprint::cached(r.get(8)?, r.get(9)?),
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// Retire de `dup_scanned` les pistes qui ne sont plus `filed`, et leurs arêtes avec.
///
/// `ON DELETE CASCADE` ne couvre que la suppression d'une LIGNE `tracks` ; une piste qui repasse
/// en `pending` (ré-encodage détecté par `scanner.rs`, dérangement) garde sa ligne et sortirait
/// donc du jeu sans que ses arêtes disparaissent. Elles mentiraient alors sur une empreinte que
/// `scanner.rs` vient justement d'effacer.
pub(crate) fn prune_unfiled(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM dup_edges WHERE a_id IN (SELECT track_id FROM dup_scanned \
            WHERE track_id NOT IN (SELECT id FROM tracks WHERE status='filed')) \
            OR b_id IN (SELECT track_id FROM dup_scanned \
            WHERE track_id NOT IN (SELECT id FROM tracks WHERE status='filed'))",
        [],
    )?;
    conn.execute(
        "DELETE FROM dup_scanned \
         WHERE track_id NOT IN (SELECT id FROM tracks WHERE status='filed')",
        [],
    )
}

/// Enregistre les arêtes trouvées et marque les pistes comme comparées, en une transaction.
///
/// L'ordre compte : les arêtes d'abord, `dup_scanned` ensuite. Une interruption entre les deux
/// laisse des arêtes pour une piste non marquée — elle sera recomparée au prochain passage, et
/// `INSERT OR REPLACE` réécrira les mêmes valeurs. L'ordre inverse laisserait une piste marquée
/// « comparée » sans ses arêtes, ce qui casserait l'invariant en silence et pour de bon.
pub(crate) fn record_scanned(
    conn: &mut Connection,
    edges: &[DupEdge],
    scanned_ids: &[i64],
) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    {
        let mut ins = tx.prepare(
            "INSERT OR REPLACE INTO dup_edges (a_id, b_id, similarity) VALUES (?1, ?2, ?3)",
        )?;
        for e in edges {
            // `a_id < b_id` : invariant tenu ICI, pas par le schéma. Sans lui la même paire
            // pourrait exister dans les deux sens et la PRIMARY KEY ne l'attraperait pas.
            let (a, b) = if e.a_id <= e.b_id {
                (e.a_id, e.b_id)
            } else {
                (e.b_id, e.a_id)
            };
            ins.execute(params![a, b, e.similarity])?;
        }
        let mut mark = tx.prepare("INSERT OR IGNORE INTO dup_scanned (track_id) VALUES (?1)")?;
        for id in scanned_ids {
            mark.execute(params![id])?;
        }
    }
    tx.commit()
}

/// Toutes les arêtes persistées.
pub(crate) fn load_edges(conn: &Connection) -> rusqlite::Result<Vec<DupEdge>> {
    let mut stmt = conn.prepare("SELECT a_id, b_id, similarity FROM dup_edges")?;
    let rows = stmt
        .query_map([], |r| {
            Ok(DupEdge {
                a_id: r.get(0)?,
                b_id: r.get(1)?,
                similarity: r.get::<_, f64>(2)? as f32,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// Compare `row` à `candidates` et rend les arêtes trouvées. Pur : aucune connexion touchée,
/// donc appelable sans tenir le verrou global — c'est ici que passe le temps CPU.
///
/// `fp` est l'empreinte de `row` (déjà résolue par `build_fingerprints`). Les empreintes des
/// candidats viennent de leur colonne en cache : un candidat est dans `dup_scanned`, donc il a
/// déjà été comparé une fois, donc son empreinte a déjà été calculée. Aucun décodage disque ici.
pub(crate) fn edges_against(
    row: &DupScanRow,
    fp: &[u32],
    candidates: &[DupScanRow],
) -> Vec<DupEdge> {
    let mut out = Vec::new();
    for cand in candidates {
        // Le pré-filtre de durée est déjà appliqué en SQL par `load_dup_candidates`, mais il est
        // rejoué ici parce que cette fonction sert AUSSI à comparer les nouvelles pistes entre
        // elles, où aucun SQL n'est passé. Les deux formulations doivent rester d'accord.
        if let (Some(a), Some(b)) = (row.duration, cand.duration) {
            if (a - b).abs() > DURATION_MATCH_TOL_SEC {
                continue;
            }
        }
        let Some(raw) = cand.fingerprint.as_deref() else {
            continue;
        };
        if raw.is_empty() {
            continue;
        }
        let s = fingerprint::similarity(fp, &fingerprint::decode(raw));
        if s >= fingerprint::MATCH_THRESHOLD {
            out.push(DupEdge {
                a_id: row.id,
                b_id: cand.id,
                similarity: s,
            });
        }
    }
    out
}

/// Compare deux pistes dont les empreintes sont DÉJÀ décodées, et rend l'arête si elles matchent.
///
/// Sert au cas que `edges_against` ne couvre pas : deux pistes nouvellement rangées, comparées
/// entre elles. Aucune des deux n'est dans `dup_scanned`, et leur empreinte vient d'être calculée
/// en mémoire — la relire depuis la colonne en cache serait au mieux inutile, au pire fausse
/// (elle n'y est pas encore écrite).
pub(crate) fn edge_between(
    a: &DupScanRow,
    fa: &[u32],
    b: &DupScanRow,
    fb: &[u32],
) -> Option<DupEdge> {
    if let (Some(x), Some(y)) = (a.duration, b.duration) {
        if (x - y).abs() > DURATION_MATCH_TOL_SEC {
            return None;
        }
    }
    let s = fingerprint::similarity(fa, fb);
    (s >= fingerprint::MATCH_THRESHOLD).then_some(DupEdge {
        a_id: a.id,
        b_id: b.id,
        similarity: s,
    })
}

/// Enchaîne le rafraîchissement incrémental complet sur une connexion tenue du début à la fin.
///
/// **Réservé aux tests**, exactement comme `scan_library_duplicates` : la production ne peut PAS
/// faire ça, parce que `build_fingerprints` décode de l'audio depuis le disque et que tenir le
/// verrou global pendant ce temps affamerait tout le reste de l'IPC (audit SYS-1, 2026-07-28).
/// `ipc_library::refresh_duplicate_groups` refait donc ces étapes en trois sections de verrou
/// courtes. **Les deux doivent rester d'accord** — c'est le prix d'un test lisible, et
/// `#[cfg(test)]` garantit au moins qu'aucun code de production ne prendra ce raccourci.
#[cfg(test)]
pub(crate) fn refresh_incremental(conn: &mut Connection) -> rusqlite::Result<Vec<DupGroup>> {
    prune_unfiled(conn)?;
    let unscanned = load_unscanned_rows(conn)?;
    let mut candidates = Vec::with_capacity(unscanned.len());
    for row in &unscanned {
        candidates.push(load_dup_candidates(conn, row)?);
    }

    let built = build_fingerprints(&unscanned);
    let mut edges = Vec::new();
    for (i, row) in unscanned.iter().enumerate() {
        let Some(fp) = built.fps[i].as_deref() else {
            continue;
        };
        edges.extend(edges_against(row, fp, &candidates[i]));
    }
    // Les nouvelles pistes entre elles, par fenêtre de durée (#38). Miroir exact de
    // `ipc_library::refresh_duplicate_groups` — les deux doivent rester d'accord.
    let new_durations: Vec<Option<f64>> = unscanned.iter().map(|r| r.duration).collect();
    for_each_candidate_pair(&new_durations, |i, j| {
        let (Some(fi), Some(fj)) = (built.fps[i].as_deref(), built.fps[j].as_deref()) else {
            return;
        };
        if let Some(e) = edge_between(&unscanned[i], fi, &unscanned[j], fj) {
            edges.push(e);
        }
    });

    if !built.to_persist.is_empty() {
        persist_fingerprints(conn, &built.to_persist);
    }
    let ids: Vec<i64> = unscanned.iter().map(|r| r.id).collect();
    record_scanned(conn, &edges, &ids)?;

    let rows = load_dup_group_rows(conn)?;
    let all_edges = load_edges(conn)?;
    Ok(groups_from_edges(&rows, &all_edges))
}

/// Reconstruit les groupes depuis les arêtes persistées.
///
/// Passe par `link` et `assemble_groups`, les mêmes que le scan complet : c'est ce qui garantit
/// que les deux chemins produisent des groupes identiques, y compris le `similarity` du groupe
/// (le minimum des arêtes, dont le calcul avait déjà été faux une fois).
///
/// Une arête dont l'une des extrémités n'est pas dans `rows` est ignorée — `rows` est le jeu
/// `filed` courant, et `prune_unfiled` doit avoir tourné avant.
pub(crate) fn groups_from_edges(rows: &[DupScanRow], edges: &[DupEdge]) -> Vec<DupGroup> {
    let index: HashMap<i64, usize> = rows.iter().enumerate().map(|(i, r)| (r.id, i)).collect();
    let mut parent: Vec<usize> = (0..rows.len()).collect();
    let mut min_sim: HashMap<usize, f32> = HashMap::new();
    for e in edges {
        let (Some(&i), Some(&j)) = (index.get(&e.a_id), index.get(&e.b_id)) else {
            continue;
        };
        link(&mut parent, &mut min_sim, i, j, e.similarity);
    }
    assemble_groups(rows, &mut parent, &min_sim)
}

/// Group every `filed` track into duplicate clusters by acoustic fingerprint similarity
/// (reuses the same cache + threshold as `find_duplicate`). Still O(n²) in the worst case,
/// but the initial SELECT now also reads the cached `fingerprint` (no per-track N+1 SELECT)
/// and a cheap duration pre-filter skips comparisons that can't possibly match — enough for
/// a full 15k-track library dashboard scan.
///
/// Enchaîne lecture + calcul + persistance sous un `conn` tenu du début à la fin.
///
/// **Réservé aux tests depuis le 2026-07-28 (audit SYS-1).** Son dernier appelant de production,
/// `library::library_stats`, tenait le verrou global pendant tout l'appel — donc pendant le
/// décodage disque de `build_fingerprints`. Les deux commandes IPC concernées
/// (`ipc_library::scan_library_duplicates` et `ipc_library::library_stats`) enchaînent désormais
/// `load_dup_scan_rows` / `build_fingerprints` / `group_duplicates` / `persist_fingerprints`
/// elles-mêmes, de façon à ne tenir le verrou que sur la brève lecture et la brève écriture.
///
/// Garder ce raccourci hors production est délibéré : il rend les tests de `dedup` lisibles
/// (un appel au lieu de quatre) sans laisser un chemin qui reprendrait la mauvaise habitude.
/// `#[cfg(test)]` fait échouer la compilation de tout futur appelant de production, au lieu de le
/// laisser passer.
#[cfg(test)]
pub fn scan_library_duplicates(conn: &Connection) -> rusqlite::Result<Vec<DupGroup>> {
    let rows = load_dup_scan_rows(conn)?;
    let built = build_fingerprints(&rows);
    if !built.to_persist.is_empty() {
        persist_fingerprints(conn, &built.to_persist);
    }
    Ok(group_duplicates(&rows, &built.fps))
}

/// Name key for a track derived from its FILENAME only (no tag read — cheap). Uses the
/// filename parser when the name is clean, else normalizes the whole stem.
fn key_for_path(path: &str) -> String {
    let stem = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    match naming::parse_filename(stem) {
        Some((a, t, _)) => naming::name_key(&a, &t),
        None => naming::name_key("", stem),
    }
}

/// Pending track ids whose name key collides with another pending or filed track. Pure
/// string work over the `tracks` table — no file I/O, no migration. Drives the queue badge.
/// Convenance de test : enchaîne les deux moitiés sous une seule connexion. La production ne DOIT
/// pas avoir ce chemin — c'est précisément le verrou-tenu-pendant-le-calcul que cette tranche
/// retire. Même forme que `filing::file_track`.
#[cfg(test)]
pub fn name_dups(conn: &Connection) -> rusqlite::Result<HashSet<i64>> {
    Ok(group_name_dups(&load_name_dup_rows(conn)?))
}

/// Une ligne du pré-filtre par nom : `(id, path, is_pending)`.
pub(crate) type NameDupRow = (i64, String, bool);

/// Lecture brève : chaque piste `pending`/`filed`. Destinée à être appelée sous un verrou COURT —
/// l'appelant le relâche avant `group_name_dups`.
pub(crate) fn load_name_dup_rows(conn: &Connection) -> rusqlite::Result<Vec<NameDupRow>> {
    let mut stmt =
        conn.prepare("SELECT id, path, status FROM tracks WHERE status IN ('pending','filed')")?;
    let rows: Vec<NameDupRow> = stmt
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get::<_, String>(2)? == "pending"))
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// Le regroupement lui-même. Pur — aucune connexion touchée, donc exécutable verrou relâché.
///
/// C'est la moitié coûteuse : une normalisation de nom (`naming::name_key`, minuscules, pliage
/// des accents, ponctuation retirée) PAR PISTE de la bibliothèque entière. Elle tournait sous le
/// verrou global de `list_queue`, c'est-à-dire à chaque ouverture de la file d'attente, pendant
/// que le pool d'analyse attendait. Même découpage que `load_dup_scan_rows` /
/// `build_fingerprints` / `group_duplicates` juste au-dessus.
pub(crate) fn group_name_dups(rows: &[NameDupRow]) -> HashSet<i64> {
    // key -> list of (id, is_pending)
    let mut groups: HashMap<String, Vec<(i64, bool)>> = HashMap::new();
    for (id, path, is_pending) in rows {
        groups
            .entry(key_for_path(path))
            .or_default()
            .push((*id, *is_pending));
    }
    let mut dups = HashSet::new();
    for (_key, group) in groups {
        if group.len() >= 2 {
            for (id, is_pending) in group {
                if is_pending {
                    dups.insert(id);
                }
            }
        }
    }
    dups
}

/// The best duplicate match for `track_id` by name (other pending or filed track sharing its
/// name key). `None` if no name collides. Slice A returns `kind = "name"`; the acoustic layer
/// (slice B) upgrades to `both` when the sound confirms.
pub fn find_duplicate(conn: &Connection, track_id: i64) -> rusqlite::Result<Option<DupMatch>> {
    let path: String = match conn.query_row(
        "SELECT path FROM tracks WHERE id=?1",
        params![track_id],
        |r| r.get(0),
    ) {
        Ok(p) => p,
        Err(_) => return Ok(None),
    };
    let key = key_for_path(&path);

    let mut stmt = conn.prepare(
        "SELECT id, path, status, folder, filename FROM tracks
         WHERE status IN ('pending','filed') AND id<>?1",
    )?;
    let rows: Vec<CandRow> = stmt
        .query_map(params![track_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<rusqlite::Result<_>>()?;

    // Prefer a filed match (it's "already in your library") over another pending one.
    let mut best: Option<CandRow> = None;
    for (id, cand_path, status, folder, filename) in rows {
        if key_for_path(&cand_path) != key {
            continue;
        }
        let is_filed = status == "filed";
        let take = match &best {
            None => true,
            Some((_, _, bstatus, _, _)) => is_filed && bstatus != "filed",
        };
        if take {
            best = Some((id, cand_path, status, folder, filename));
            if is_filed {
                break; // strongest by-name signal
            }
        }
    }

    let Some((id, cand_path, status, folder, filename)) = best else {
        return Ok(None);
    };

    // Confirm by sound: compare cached/lazy fingerprints. `both` if the sound agrees, else
    // `name` (names match but the recording differs, or audio unreadable — "à vérifier").
    let (kind, score) = match (
        get_or_compute_fp(conn, track_id, &path),
        get_or_compute_fp(conn, id, &cand_path),
    ) {
        (Some(fa), Some(fb)) => {
            let s = fingerprint::similarity(&fa, &fb);
            if s >= fingerprint::MATCH_THRESHOLD {
                ("both", s)
            } else {
                ("name", s)
            }
        }
        _ => ("name", 1.0),
    };

    Ok(Some(DupMatch {
        id,
        status,
        folder,
        filename,
        kind: kind.to_string(),
        score,
    }))
}

/// Fetch a track's fingerprint from the `tracks.fingerprint` cache, or compute it from the
/// file and cache it. `None` if the audio can't be fingerprinted (short/corrupt/missing).
///
/// Une empreinte dont `fingerprint_ver` n'est pas la version courante est traitée comme absente
/// (`fingerprint::cached`) : elle se recalcule et s'écrase, elle ne remonte jamais une erreur.
fn get_or_compute_fp(conn: &Connection, track_id: i64, path: &str) -> Option<Vec<u32>> {
    let cached: Option<String> = conn
        .query_row(
            "SELECT fingerprint, fingerprint_ver FROM tracks WHERE id=?1",
            params![track_id],
            |r| Ok(fingerprint::cached(r.get(0)?, r.get(1)?)),
        )
        .ok()
        .flatten();
    if let Some(s) = cached {
        if !s.is_empty() {
            return Some(fingerprint::decode(&s));
        }
    }
    match fingerprint::compute_for_path(path) {
        Ok(fp) => {
            let _ = conn.execute(
                "UPDATE tracks SET fingerprint=?2, fingerprint_ver=?3 WHERE id=?1",
                params![
                    track_id,
                    fingerprint::encode(&fp),
                    fingerprint::FINGERPRINT_CACHE_VERSION
                ],
            );
            Some(fp)
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// Mirrors shared/contracts.ts's `DupGroupMember`. Exhaustive destructure (no `..`): fails
    /// to compile if a field is added/removed/renamed on the Rust struct — the forcing function
    /// to also update contracts.ts. Phase 2 — docs/superpowers/plans/2026-07-13-phase2-ipc-contract-tests.md.
    #[test]
    fn dup_group_member_shape_matches_contracts_ts() {
        let v = DupGroupMember {
            id: 0,
            path: String::new(),
            filename: None,
            folder: None,
            format: None,
            bitrate: None,
            duration: None,
            truncated: false,
            recommend_keep: false,
            reason: None,
        };
        let DupGroupMember {
            id,
            path,
            filename,
            folder,
            format,
            bitrate,
            duration,
            truncated,
            recommend_keep,
            reason,
        } = v;
        let _ = (
            id,
            path,
            filename,
            folder,
            format,
            bitrate,
            duration,
            truncated,
            recommend_keep,
            reason,
        );
    }

    /// Mirrors shared/contracts.ts's `DupGroup`. Phase 2 —
    /// docs/superpowers/plans/2026-07-13-phase2-ipc-contract-tests.md.
    #[test]
    fn dup_group_shape_matches_contracts_ts() {
        let v = DupGroup {
            members: Vec::new(),
            similarity: 0.0,
        };
        let DupGroup {
            members,
            similarity,
        } = v;
        let _ = (members, similarity);
    }

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::run_migrations(&conn).unwrap();
        conn
    }

    fn add(conn: &Connection, path: &str, status: &str) -> i64 {
        conn.execute(
            "INSERT INTO tracks(path, filename, status) VALUES(?1, ?2, ?3)",
            params![
                path,
                Path::new(path).file_name().and_then(|n| n.to_str()),
                status
            ],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    /// Sème une empreinte en cache **par le chemin de production** (`persist_fingerprints`), donc
    /// avec son `fingerprint_ver`. Un `UPDATE tracks SET fingerprint=…` écrit à la main laisserait
    /// la version NULL : `fingerprint::cached` rendrait `None`, `build_fingerprints` tenterait un
    /// décodage disque, et le test échouerait pour une raison qui n'est pas la sienne. C'est
    /// exactement ce qui est arrivé aux dix tests de ce module à l'ajout de la v22 — un seed qui
    /// contourne la production ne prouve rien sur la production.
    fn seed_fingerprint(conn: &Connection, ids: &[i64], fp: &[u32]) {
        let entries: Vec<(i64, Vec<u32>)> = ids.iter().map(|&id| (id, fp.to_vec())).collect();
        persist_fingerprints(conn, &entries);
    }

    #[test]
    fn name_dups_flags_pending_homonyms() {
        let conn = db();
        let a = add(&conn, "/dl/Larry Heard - Mystery of Love.mp3", "pending");
        let b = add(&conn, "/dl/larry_heard mystery of love.flac", "pending");
        let _c = add(&conn, "/dl/Chez Damier - Can You Feel It.aiff", "pending");
        let dups = name_dups(&conn).unwrap();
        assert!(dups.contains(&a) && dups.contains(&b));
        assert_eq!(dups.len(), 2); // c is unique
    }

    #[test]
    fn name_dups_flags_pending_against_filed() {
        let conn = db();
        let p = add(&conn, "/dl/Theo Parrish - Falling Up.mp3", "pending");
        let _f = add(&conn, "/lib/Theo Parrish - Falling Up.aiff", "filed");
        let dups = name_dups(&conn).unwrap();
        assert!(dups.contains(&p));
        assert_eq!(dups.len(), 1); // only the pending one is flagged
    }

    #[test]
    fn find_duplicate_prefers_filed_match() {
        let conn = db();
        let cur = add(&conn, "/dl/Theo Parrish - Falling Up.mp3", "pending");
        let _other_pending = add(&conn, "/dl2/theo parrish falling up.wav", "pending");
        conn.execute(
            "UPDATE tracks SET folder='House' WHERE path='/lib/x.aiff'",
            [],
        )
        .ok();
        let filed = add(&conn, "/lib/Theo Parrish - Falling Up.aiff", "filed");
        conn.execute(
            "UPDATE tracks SET folder='House' WHERE id=?1",
            params![filed],
        )
        .unwrap();

        let m = find_duplicate(&conn, cur).unwrap().unwrap();
        assert_eq!(m.id, filed);
        assert_eq!(m.status, "filed");
        assert_eq!(m.folder.as_deref(), Some("House"));
        assert_eq!(m.kind, "name");
    }

    fn fixture(name: &str) -> Option<String> {
        let p = format!("fixtures/{name}");
        std::path::Path::new(&p).exists().then_some(p)
    }

    #[test]
    fn find_duplicate_confirms_by_sound() {
        // Two encodings of the same recording, named to share a name key → name match AND
        // sound match → kind "both".
        let (Some(mp3), Some(flac)) = (fixture("real_320.mp3"), fixture("real_lossless.flac"))
        else {
            eprintln!("skip: no fixtures");
            return;
        };
        crate::ffmpeg::init_ffmpeg_path();
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("Sweep Test - Tone.mp3");
        let b = dir.path().join("Sweep Test - Tone.flac");
        std::fs::copy(&mp3, &a).unwrap();
        std::fs::copy(&flac, &b).unwrap();
        let id_a = add(&conn, a.to_str().unwrap(), "pending");
        let _id_b = add(&conn, b.to_str().unwrap(), "pending");

        let m = find_duplicate(&conn, id_a).unwrap().unwrap();
        assert_eq!(
            m.kind, "both",
            "same recording, same name → sound-confirmed"
        );
        // fingerprint cached on both after the comparison
        let cached: Option<String> = conn
            .query_row(
                "SELECT fingerprint FROM tracks WHERE id=?1",
                params![id_a],
                |r| r.get(0),
            )
            .unwrap();
        assert!(cached.is_some_and(|s| !s.is_empty()));
    }

    #[test]
    fn find_duplicate_none_when_unique() {
        let conn = db();
        let cur = add(&conn, "/dl/Unique Artist - Unique Title.mp3", "pending");
        add(&conn, "/dl/Someone Else - Other Song.mp3", "pending");
        assert!(find_duplicate(&conn, cur).unwrap().is_none());
    }

    #[test]
    fn scan_library_duplicates_groups_filed_tracks_by_sound() {
        let (Some(mp3), Some(flac)) = (fixture("real_320.mp3"), fixture("real_lossless.flac"))
        else {
            eprintln!("skip: no fixtures");
            return;
        };
        crate::ffmpeg::init_ffmpeg_path();
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.mp3");
        let b = dir.path().join("b.flac");
        std::fs::copy(&mp3, &a).unwrap();
        std::fs::copy(&flac, &b).unwrap();
        // L'extension du fixture ne sert qu'au décodage : ce qui décrit une piste RANGÉE, c'est son
        // `target_format`, seule colonne que `filing.rs:730` écrit.
        conn.execute(
            "INSERT INTO tracks(path, filename, status, target_format, bitrate, duration) \
             VALUES(?1, 'a.mp3', 'filed', 'mp3_320', 320, 30.0)",
            params![a.to_str().unwrap()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks(path, filename, status, target_format, bitrate, duration) \
             VALUES(?1, 'b.flac', 'filed', 'aiff_16_44', 1411, 30.0)",
            params![b.to_str().unwrap()],
        )
        .unwrap();
        // a lone unrelated filed track must not be grouped
        conn.execute(
            "INSERT INTO tracks(path, filename, status, format) \
             VALUES('/lib/lone.wav', 'lone.wav', 'filed', 'wav')",
            [],
        )
        .unwrap();

        let groups = scan_library_duplicates(&conn).unwrap();

        assert_eq!(groups.len(), 1, "only the a/b pair forms a group");
        let g = &groups[0];
        assert_eq!(g.members.len(), 2);
        assert!(g.similarity >= fingerprint::MATCH_THRESHOLD);
        let keep = g.members.iter().find(|m| m.recommend_keep).unwrap();
        assert_eq!(
            keep.format.as_deref(),
            Some("aiff"),
            "lossless wins over lossy"
        );
        assert!(keep.reason.is_some());
        assert_eq!(g.members.iter().filter(|m| m.recommend_keep).count(), 1);
    }

    /// Une empreinte dont la version n'est plus la courante doit se lire comme ABSENTE, sur les
    /// trois chargeurs qui remontent la colonne — c'est la lecture qui donne son effet à
    /// `fingerprint_ver` (issue #39).
    ///
    /// Le test ne peut pas muter une `const`, alors il fait l'inverse, ce qui est équivalent du
    /// point de vue de la ligne : il écrit `FINGERPRINT_CACHE_VERSION + 1`, exactement l'état
    /// qu'aurait une empreinte existante le jour où la constante est incrémentée. Relatif et pas
    /// absolu, donc il reste vrai après un bump. Sans cette mutation, un test ne prouverait rien —
    /// il passerait aussi avec la lecture d'avant.
    #[test]
    fn une_empreinte_d_une_autre_version_se_lit_comme_absente() {
        let conn = db();
        let id = add_filed(&conn, "/lib/versionnee.flac", Some(120.0), &[1, 2, 3, 4]);

        // Version courante : les trois chargeurs servent l'empreinte.
        assert!(
            load_unscanned_rows(&conn).unwrap()[0].fingerprint.is_some(),
            "empreinte à la version courante : elle doit être servie"
        );

        // Une seule colonne bouge — la valeur reste identique, seule sa version diverge.
        conn.execute(
            "UPDATE tracks SET fingerprint_ver=?2 WHERE id=?1",
            params![id, fingerprint::FINGERPRINT_CACHE_VERSION + 1],
        )
        .unwrap();

        let rows = load_unscanned_rows(&conn).unwrap();
        assert!(
            rows[0].fingerprint.is_none(),
            "load_unscanned_rows resservait une empreinte d'un autre algorithme"
        );
        assert!(
            load_dup_scan_rows(&conn).unwrap()[0].fingerprint.is_none(),
            "load_dup_scan_rows resservait une empreinte d'un autre algorithme"
        );
        // `load_dup_candidates` ne rend que des pistes déjà dans `dup_scanned` : on l'y met, puis
        // on interroge depuis une AUTRE piste, ce qui est sa vraie forme d'appel.
        conn.execute(
            "INSERT INTO dup_scanned (track_id) VALUES (?1)",
            params![id],
        )
        .unwrap();
        let autre = add_filed(&conn, "/lib/autre.flac", Some(120.0), &[9, 9, 9, 9]);
        let sonde = DupScanRow {
            id: autre,
            path: "/lib/autre.flac".into(),
            filename: None,
            folder: None,
            format: None,
            bitrate: None,
            duration: Some(120.0),
            truncated: false,
            fingerprint: None,
        };
        let cands = load_dup_candidates(&conn, &sonde).unwrap();
        assert_eq!(cands.len(), 1);
        assert!(
            cands[0].fingerprint.is_none(),
            "load_dup_candidates resservait une empreinte d'un autre algorithme"
        );

        // Et le défaut de cache est bien traité comme une ABSENCE, pas comme une erreur :
        // `build_fingerprints` retombe sur un décodage disque, qui échoue ici (chemin fictif) et
        // rend `None` sans rien faire tomber.
        let built = build_fingerprints(&rows);
        assert!(
            built.fps[0].is_none() && built.to_persist.is_empty(),
            "un défaut de cache doit mener au recalcul, jamais à une erreur"
        );
    }

    /// Régression : le scan lisait `tracks.format`, colonne qu'aucun code de production n'écrit.
    /// `is_lossless_fmt` recevait donc toujours NULL et `pick_keep` n'appliquait jamais son premier
    /// critère — le membre recommandé se décidait au bitrate seul, donc un MP3 320 battait un AIFF
    /// sur tout groupe mixte. Et le geste suivant que propose l'écran est une suppression.
    ///
    /// Semé par `target_format`, EXACTEMENT comme `filing.rs:730` le fait en production : un test
    /// qui sème `format` à la main passe sans jamais toucher le chemin réel.
    #[test]
    fn scan_library_duplicates_recommend_keep_prefers_the_lossless_target() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(id, path, filename, status, target_format, bitrate, duration, truncated) \
             VALUES(1, '/lib/a.mp3', 'a.mp3', 'filed', 'mp3_320', 320, 30.0, 0)",
            [],
        )
        .unwrap();
        // `bitrate` NULL sur l'AIFF, 320 sur le MP3 : c'est DÉLIBÉRÉ. Le critère de rang est
        // (lossless, bitrate, durée, non tronqué) dans cet ordre — avec un AIFF mieux doté en
        // bitrate, le test passerait grâce au bitrate même avec le bug, et ne prouverait rien.
        // Ici seul le premier critère peut faire gagner l'AIFF.
        conn.execute(
            "INSERT INTO tracks(id, path, filename, status, target_format, duration, truncated) \
             VALUES(2, '/lib/a.aiff', 'a.aiff', 'filed', 'aiff_16_44', 30.0, 0)",
            [],
        )
        .unwrap();
        seed_fingerprint(&conn, &[1, 2], &[1, 2, 3, 4, 5, 6, 7, 8]);

        let groups = scan_library_duplicates(&conn).unwrap();
        assert_eq!(groups.len(), 1);
        let keep = groups[0].members.iter().find(|m| m.recommend_keep).unwrap();
        assert_eq!(keep.id, 2, "l'AIFF est lossless, il doit gagner sur le MP3");
        assert_eq!(
            keep.format.as_deref(),
            Some("aiff"),
            "l'écran doit montrer une extension, pas 'aiff_16_44'"
        );
    }

    #[test]
    fn scan_library_duplicates_recommend_keep_prefers_higher_bitrate_when_same_lossiness() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(id, path, filename, status, format, bitrate, duration, truncated) \
             VALUES(1, '/lib/low.mp3', 'low.mp3', 'filed', 'mp3', 128, 30.0, 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks(id, path, filename, status, format, bitrate, duration, truncated) \
             VALUES(2, '/lib/high.mp3', 'high.mp3', 'filed', 'mp3', 320, 30.0, 0)",
            [],
        )
        .unwrap();
        // Fake-match the pair directly via a shared cached fingerprint (bypasses real decode).
        seed_fingerprint(&conn, &[1, 2], &[1, 2, 3, 4, 5, 6, 7, 8]);

        let groups = scan_library_duplicates(&conn).unwrap();

        assert_eq!(groups.len(), 1);
        let keep = groups[0].members.iter().find(|m| m.recommend_keep).unwrap();
        assert_eq!(keep.id, 2, "same lossiness → higher bitrate wins");
    }

    /// `DupGroup.similarity` se documente comme « la similarité par paire la plus FAIBLE qui a lié
    /// le groupe ». `min_sim` étant indexé par racine et une fusion changeant la racine, le minimum
    /// enregistré sous l'ancienne racine devenait orphelin : sur ce groupe de trois, dont le lien le
    /// plus faible est la PREMIÈRE arête trouvée, le champ sur-rapportait.
    ///
    /// Les trois empreintes sont construites, pas décodées, et leurs similarités par paire ont été
    /// MESURÉES avant d'écrire le test : A↔B 0.675 (le minimum réel), A↔C 0.85, B↔C 0.817 —
    /// toutes au-dessus du seuil de 0.6, donc les trois forment bien un seul groupe.
    #[test]
    fn group_duplicates_reports_the_weakest_link_across_a_merge() {
        let base: Vec<u32> = (0..120u32).map(|i| i.wrapping_mul(2_654_435_761)).collect();
        let with_flips = |flips: usize| {
            let mut v = base.clone();
            for k in 0..flips {
                let n = v.len();
                v[k * 2 % n] ^= 0xFFFF_FFFF;
            }
            v
        };
        let b = with_flips(20);
        let c = with_flips(10);

        // Le test ne vaut que si l'ordre des similarités est bien celui qu'on croit — sinon il
        // passerait pour une raison sans rapport avec le bug.
        let (ab, ac, bc) = (
            fingerprint::similarity(&base, &b),
            fingerprint::similarity(&base, &c),
            fingerprint::similarity(&b, &c),
        );
        assert!(
            ab < ac && ab < bc && ab >= fingerprint::MATCH_THRESHOLD,
            "prémisse du test cassée: ab={ab} ac={ac} bc={bc}"
        );

        let row = |id: i64, path: &str| DupScanRow {
            id,
            path: path.to_string(),
            filename: None,
            folder: None,
            format: Some("aiff".to_string()),
            bitrate: Some(1411),
            duration: Some(30.0),
            truncated: false,
            fingerprint: None,
        };
        let rows = vec![
            row(1, "/lib/a.aiff"),
            row(2, "/lib/b.aiff"),
            row(3, "/lib/c.aiff"),
        ];
        let fps = vec![Some(base.clone()), Some(b), Some(c)];

        let groups = group_duplicates(&rows, &fps);
        assert_eq!(groups.len(), 1, "les trois doivent former un seul groupe");
        assert_eq!(groups[0].members.len(), 3);
        assert!(
            (groups[0].similarity - ab).abs() < 1e-6,
            "similarity doit être le lien le plus FAIBLE ({ab}), pas {}",
            groups[0].similarity
        );
    }

    #[test]
    fn scan_library_duplicates_duration_prefilter_skips_far_apart() {
        // Same cached fingerprint on both, but durations 30s vs 200s (> 2s tol) → the pre-filter
        // must skip the comparison so they are NOT grouped. Guards against a wrong match by
        // fingerprint alone when the recordings are plainly different lengths.
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(id, path, filename, status, format, bitrate, duration, truncated) \
             VALUES(1, '/lib/short.mp3', 'short.mp3', 'filed', 'mp3', 320, 30.0, 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks(id, path, filename, status, format, bitrate, duration, truncated) \
             VALUES(2, '/lib/long.mp3', 'long.mp3', 'filed', 'mp3', 320, 200.0, 0)",
            [],
        )
        .unwrap();
        seed_fingerprint(&conn, &[1, 2], &[1, 2, 3, 4, 5, 6, 7, 8]);

        let groups = scan_library_duplicates(&conn).unwrap();
        assert!(
            groups.is_empty(),
            "durations 170s apart must not be grouped"
        );
    }

    #[test]
    fn scan_library_duplicates_duration_prefilter_allows_close() {
        // Same cached fingerprint, durations within tolerance (30.0 vs 31.5) → still grouped.
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(id, path, filename, status, format, bitrate, duration, truncated) \
             VALUES(1, '/lib/a.mp3', 'a.mp3', 'filed', 'mp3', 320, 30.0, 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks(id, path, filename, status, format, bitrate, duration, truncated) \
             VALUES(2, '/lib/b.mp3', 'b.mp3', 'filed', 'mp3', 320, 31.5, 0)",
            [],
        )
        .unwrap();
        seed_fingerprint(&conn, &[1, 2], &[1, 2, 3, 4, 5, 6, 7, 8]);

        let groups = scan_library_duplicates(&conn).unwrap();
        assert_eq!(
            groups.len(),
            1,
            "durations within 2s tolerance stay grouped"
        );
        assert_eq!(groups[0].members.len(), 2);
    }

    #[test]
    fn scan_library_duplicates_ignores_pending_and_lone_tracks() {
        let conn = db();
        add(&conn, "/dl/pending.mp3", "pending");
        add(&conn, "/lib/lone.flac", "filed");
        let groups = scan_library_duplicates(&conn).unwrap();
        assert!(groups.is_empty());
    }

    // ── Phase 4 : le chemin incrémental doit être ÉQUIVALENT au scan complet ──
    //
    // L'empreinte est écrite directement en base : `build_fingerprints` réutilise alors la valeur
    // en cache et ne touche jamais au disque. Les tests restent donc rapides ET indépendants de
    // `src-tauri/fixtures/`, qui est gitignoré et absent d'un checkout frais.

    /// Piste `filed` avec une durée et une empreinte donnée.
    fn add_filed(conn: &Connection, path: &str, duration: Option<f64>, fp: &[u32]) -> i64 {
        let id = add(conn, path, "filed");
        conn.execute(
            "UPDATE tracks SET duration=?2 WHERE id=?1",
            params![id, duration],
        )
        .unwrap();
        seed_fingerprint(conn, &[id], fp);
        id
    }

    /// Empreinte pseudo-aléatoire déterministe, assez LONGUE pour que `similarity` discrimine.
    ///
    /// Une empreinte de 8 items ne discrimine pas : `similarity` vaut `matched / n`, et avec
    /// `n = 8` un seul alignement fortuit du matcher suffit à dépasser le seuil de 0,6. Deux
    /// pistes sans rapport se retrouvaient alors dans le même groupe. 200 items, c'est ~25 s
    /// d'audio à la cadence de `preset_test1` — assez pour que le hasard ne décide plus.
    fn synth_fp(seed: u32) -> Vec<u32> {
        let mut x = seed;
        (0..200)
            .map(|_| {
                x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                x
            })
            .collect()
    }

    /// Forme comparable d'un jeu de groupes : (ids triés) par groupe, groupes triés.
    fn shape(groups: &[DupGroup]) -> Vec<Vec<i64>> {
        let mut out: Vec<Vec<i64>> = groups
            .iter()
            .map(|g| {
                let mut ids: Vec<i64> = g.members.iter().map(|m| m.id).collect();
                ids.sort_unstable();
                ids
            })
            .collect();
        out.sort();
        out
    }

    #[test]
    fn incremental_scan_matches_full_scan() {
        let fp_a = synth_fp(1);
        let fp_b = synth_fp(999);
        // Garde-fou du jeu de données lui-même : si ces deux empreintes matchaient, le test
        // passerait pour une raison sans rapport avec ce qu'il prétend vérifier.
        assert!(
            fingerprint::similarity(&fp_a, &fp_b) < fingerprint::MATCH_THRESHOLD,
            "les deux empreintes de test doivent être distinctes"
        );

        let mut inc = db();
        add_filed(&inc, "/lib/a1.flac", Some(300.0), &fp_a);
        add_filed(&inc, "/lib/a2.mp3", Some(300.5), &fp_a);
        add_filed(&inc, "/lib/b1.flac", Some(300.2), &fp_b);
        let incremental = refresh_incremental(&mut inc).unwrap();

        // Même jeu de données, chemin de référence.
        let full_conn = db();
        add_filed(&full_conn, "/lib/a1.flac", Some(300.0), &fp_a);
        add_filed(&full_conn, "/lib/a2.mp3", Some(300.5), &fp_a);
        add_filed(&full_conn, "/lib/b1.flac", Some(300.2), &fp_b);
        let full = scan_library_duplicates(&full_conn).unwrap();

        assert_eq!(shape(&incremental), shape(&full));
        assert_eq!(shape(&incremental), vec![vec![1, 2]]);
    }

    /// LE cas qui décide si l'incrémental ment. Le pré-filtre de `group_duplicates` n'écarte une
    /// paire que si les DEUX durées sont connues ; `load_dup_candidates` doit faire pareil, sinon
    /// une piste sans durée serait silencieusement comparée à moins de monde que dans le scan
    /// complet — sans qu'aucune erreur ne soit levée.
    #[test]
    fn duplicate_scan_matches_full_scan_when_duration_is_null() {
        let fp = [11u32, 22, 33, 44, 55, 66, 77, 88];

        // 300 s contre durée inconnue : très au-delà de la tolérance de 2 s, mais l'une des deux
        // manque, donc la paire DOIT être évaluée.
        let mut inc = db();
        add_filed(&inc, "/lib/known.flac", Some(300.0), &fp);
        add_filed(&inc, "/lib/unknown.mp3", None, &fp);
        let incremental = refresh_incremental(&mut inc).unwrap();

        let full_conn = db();
        add_filed(&full_conn, "/lib/known.flac", Some(300.0), &fp);
        add_filed(&full_conn, "/lib/unknown.mp3", None, &fp);
        let full = scan_library_duplicates(&full_conn).unwrap();

        assert_eq!(shape(&incremental), shape(&full));
        assert_eq!(
            shape(&incremental),
            vec![vec![1, 2]],
            "une durée inconnue ne doit PAS faire écarter la paire"
        );
    }

    /// Le gain de la Phase 4 : ranger une piste ne doit comparer que celle-là, tout en trouvant
    /// le même groupe qu'un scan complet refait de zéro.
    #[test]
    fn incremental_scan_picks_up_a_newly_filed_track() {
        let fp = [7u32, 7, 7, 7, 7, 7, 7, 7];
        let mut conn = db();
        add_filed(&conn, "/lib/a1.flac", Some(200.0), &fp);
        add_filed(&conn, "/lib/a2.mp3", Some(200.0), &fp);

        let first = refresh_incremental(&mut conn).unwrap();
        assert_eq!(shape(&first), vec![vec![1, 2]]);

        // Troisième exemplaire rangé après coup.
        add_filed(&conn, "/lib/a3.aiff", Some(200.0), &fp);
        let second = refresh_incremental(&mut conn).unwrap();
        assert_eq!(
            shape(&second),
            vec![vec![1, 2, 3]],
            "la nouvelle piste doit rejoindre le groupe existant"
        );
    }

    /// Deux doublons rangés dans la MÊME fournée : aucun des deux n'est dans `dup_scanned`, donc
    /// `load_dup_candidates` ne les rend pas l'un pour l'autre. Sans la comparaison des nouvelles
    /// pistes entre elles, ils ne se verraient jamais.
    #[test]
    fn incremental_scan_compares_new_tracks_against_each_other() {
        let fp = [3u32, 1, 4, 1, 5, 9, 2, 6];
        let mut conn = db();
        add_filed(&conn, "/lib/x1.flac", Some(180.0), &fp);
        add_filed(&conn, "/lib/x2.mp3", Some(180.0), &fp);
        let groups = refresh_incremental(&mut conn).unwrap();
        assert_eq!(shape(&groups), vec![vec![1, 2]]);
    }

    /// Une piste qui quitte `filed` — ré-encodée sur place, donc remise en `pending` par
    /// `scanner.rs`, qui efface AUSSI son empreinte — doit sortir du jeu avec ses arêtes. Sinon
    /// elles mentiraient sur une empreinte qui n'existe plus.
    #[test]
    fn unfiling_a_track_drops_its_edges() {
        let fp = [5u32, 5, 5, 5, 5, 5, 5, 5];
        let mut conn = db();
        let a = add_filed(&conn, "/lib/y1.flac", Some(240.0), &fp);
        add_filed(&conn, "/lib/y2.mp3", Some(240.0), &fp);
        assert_eq!(
            shape(&refresh_incremental(&mut conn).unwrap()),
            vec![vec![1, 2]]
        );

        // Mêmes colonnes que `scanner::upsert_file` remet à zéro sur un changement de contenu —
        // `fingerprint_ver` comprise : une version ne survit pas à la donnée qu'elle date.
        conn.execute(
            "UPDATE tracks SET status='pending', fingerprint=NULL, fingerprint_ver=NULL \
             WHERE id=?1",
            params![a],
        )
        .unwrap();

        let groups = refresh_incremental(&mut conn).unwrap();
        assert!(
            groups.is_empty(),
            "il ne reste qu'une piste filed, donc plus aucun groupe"
        );
        let edges: i64 = conn
            .query_row("SELECT COUNT(*) FROM dup_edges", [], |r| r.get(0))
            .unwrap();
        assert_eq!(edges, 0, "l'arête devait partir avec la piste dérangée");
        let scanned: i64 = conn
            .query_row("SELECT COUNT(*) FROM dup_scanned", [], |r| r.get(0))
            .unwrap();
        assert_eq!(scanned, 1, "seule la piste encore filed reste marquée");
    }

    /// Supprimer la LIGNE `tracks` doit emporter arêtes et marquage par `ON DELETE CASCADE` —
    /// c'est ce qui rend la résolution d'un doublon (suppression du perdant) exacte et immédiate,
    /// là où un union-find en RAM aurait imposé un scan complet.
    #[test]
    fn deleting_a_track_cascades_to_dup_tables() {
        let fp = [2u32, 7, 1, 8, 2, 8, 1, 8];
        let mut conn = db();
        // `db()` n'active pas les clés étrangères — seul `db::open` le fait. On les active ici,
        // sinon ce test passerait sans rien prouver du CASCADE.
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        let a = add_filed(&conn, "/lib/z1.flac", Some(150.0), &fp);
        add_filed(&conn, "/lib/z2.mp3", Some(150.0), &fp);
        refresh_incremental(&mut conn).unwrap();
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM dup_edges", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );

        conn.execute("DELETE FROM tracks WHERE id=?1", params![a])
            .unwrap();
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM dup_edges", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0,
            "ON DELETE CASCADE devait retirer l'arête"
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM dup_scanned", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1,
            "ON DELETE CASCADE devait retirer le marquage de la piste supprimée"
        );
    }

    // ── #38 : la fenêtre de durée doit énumérer EXACTEMENT ce que la double boucle énumérait ──
    //
    // Les tests d'équivalence de la Phase 4 ci-dessus comparent l'incrémental au scan complet. Ils
    // ne suffisent PLUS : les deux chemins partagent maintenant `for_each_candidate_pair`, donc
    // leur accord ne prouve que leur accord. Il faut un oracle extérieur — la double boucle
    // d'origine, recopiée littéralement, qui n'a aucune raison de se tromper de la même façon.

    /// L'ensemble EXACT des paires qui atteignaient `fingerprint::similarity` avant #38 : le
    /// pré-filtre à l'intérieur de la double boucle, `n²/2` paires parcourues.
    fn naive_candidate_pairs(durations: &[Option<f64>]) -> BTreeSet<(usize, usize)> {
        let mut out = BTreeSet::new();
        for i in 0..durations.len() {
            for j in (i + 1)..durations.len() {
                if let (Some(a), Some(b)) = (durations[i], durations[j]) {
                    if (a - b).abs() > DURATION_MATCH_TOL_SEC {
                        continue;
                    }
                }
                out.insert((i, j));
            }
        }
        out
    }

    /// Le même ensemble, vu par la fenêtre glissante. Vérifie au passage deux propriétés que
    /// l'égalité d'ensembles masquerait : les paires sortent normalisées (`i < j`), et aucune n'est
    /// visitée deux fois — sans quoi l'énumération ne serait pas une partition et `similarity`
    /// serait payé plusieurs fois pour rien.
    fn windowed_candidate_pairs(durations: &[Option<f64>]) -> BTreeSet<(usize, usize)> {
        let mut out = BTreeSet::new();
        for_each_candidate_pair(durations, |i, j| {
            assert!(i < j, "paire non normalisée: ({i}, {j})");
            assert!(out.insert((i, j)), "paire ({i}, {j}) visitée deux fois");
        });
        out
    }

    /// Cas limites choisis un par un, pas une moyenne.
    #[test]
    fn for_each_candidate_pair_matches_the_naive_double_loop_on_edge_cases() {
        let durations = vec![
            Some(300.0),
            Some(100.0),
            None,
            Some(102.0),       // exactement 2,0 de 100,0 → DOIT passer
            Some(104.0),       // exactement 2,0 de 102,0 → DOIT passer
            Some(102.0),       // durée strictement égale à une autre
            Some(104.000_001), // 2,000001 de 102,0 → DOIT être écartée
            Some(0.0),
            None,
            Some(-5.0),
            Some(300.000_5),
            Some(99.999_999),
            Some(1_000_000.0),
            Some(1_000_001.5),
        ];
        assert_eq!(
            windowed_candidate_pairs(&durations),
            naive_candidate_pairs(&durations)
        );
        let pairs = windowed_candidate_pairs(&durations);
        assert!(
            pairs.contains(&(1, 3)),
            "une paire EXACTEMENT à la tolérance doit être énumérée"
        );
        assert!(
            !pairs.contains(&(3, 6)),
            "une paire juste au-delà de la tolérance ne doit pas l'être"
        );
    }

    /// La forme qui MORD, d'après #38 : une bibliothèque de DJ s'agglutine autour de quelques
    /// durées de production, donc beaucoup de paires survivent et les bords de fenêtre sont
    /// franchis dans tous les sens. L'ordre d'entrée est délibérément non trié.
    #[test]
    fn for_each_candidate_pair_matches_the_naive_double_loop_on_a_clustered_corpus() {
        let mut x: u32 = 12_345;
        let mut next = || {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            x
        };
        let bands = [180.0f64, 240.0, 390.0, 420.0];
        let durations: Vec<Option<f64>> = (0..400)
            .map(|i| {
                if i % 37 == 0 {
                    return None; // des durées inconnues, qui se comparent à tout le monde
                }
                let band = bands[(next() % 4) as usize];
                // ±3 s autour de la bande : de part et d'autre de la tolérance de 2 s.
                Some(band + (next() % 6_001) as f64 / 1000.0 - 3.0)
            })
            .collect();

        let naive = naive_candidate_pairs(&durations);
        assert_eq!(windowed_candidate_pairs(&durations), naive);

        // Le test ne vaut que si le corpus fait vraiment travailler les deux côtés : ni un jeu si
        // clairsemé que rien ne survit, ni un jeu si dense que le pré-filtre n'écarte rien.
        let total = 400 * 399 / 2;
        assert!(
            naive.len() > 1_000 && naive.len() < total,
            "prémisse du test cassée: {} paires survivantes sur {total}",
            naive.len()
        );
    }

    /// Les durées non finies ne se trient pas : elles sortent de la fenêtre et repassent par la
    /// condition littérale. Leur comportement d'origine est contre-intuitif, donc facile à casser
    /// en « simplifiant » — `NaN > tol` est FAUX donc une paire NaN SURVIT ; `+∞` contre une durée
    /// finie est écartée ; `+∞` contre `+∞` survit, parce que `∞ - ∞` vaut NaN.
    #[test]
    fn for_each_candidate_pair_reproduces_the_non_finite_corner_cases() {
        let durations = vec![
            Some(100.0),
            Some(f64::NAN),
            Some(f64::INFINITY),
            Some(f64::INFINITY),
            Some(f64::NEG_INFINITY),
            None,
            Some(101.0),
        ];
        let pairs = windowed_candidate_pairs(&durations);
        assert_eq!(pairs, naive_candidate_pairs(&durations));
        assert!(
            pairs.contains(&(0, 1)),
            "NaN contre une durée finie doit SURVIVRE"
        );
        assert!(
            pairs.contains(&(2, 3)),
            "+inf contre +inf doit survivre (inf - inf = NaN)"
        );
        assert!(
            !pairs.contains(&(0, 2)),
            "+inf contre 100 s doit être écartée"
        );
        assert!(
            !pairs.contains(&(2, 4)),
            "+inf contre -inf doit être écartée"
        );
    }

    /// Réimplémentation LITTÉRALE de la double boucle d'avant #38, pré-filtre à l'intérieur. C'est
    /// l'oracle de `group_duplicates` : sans elle, la fonction serait sa propre référence.
    fn group_duplicates_naive(rows: &[DupScanRow], fps: &[Option<Vec<u32>>]) -> Vec<DupGroup> {
        let n = rows.len();
        let mut parent: Vec<usize> = (0..n).collect();
        let mut min_sim: HashMap<usize, f32> = HashMap::new();
        for i in 0..n {
            let Some(fi) = &fps[i] else { continue };
            let di = rows[i].duration;
            for (j, fj) in fps.iter().enumerate().skip(i + 1) {
                let Some(fj) = fj else { continue };
                if let (Some(a), Some(b)) = (di, rows[j].duration) {
                    if (a - b).abs() > DURATION_MATCH_TOL_SEC {
                        continue;
                    }
                }
                let s = fingerprint::similarity(fi, fj);
                if s >= fingerprint::MATCH_THRESHOLD {
                    link(&mut parent, &mut min_sim, i, j, s);
                }
            }
        }
        assemble_groups(rows, &mut parent, &min_sim)
    }

    fn row_at(id: i64, duration: Option<f64>) -> DupScanRow {
        DupScanRow {
            id,
            path: format!("/lib/t{id}.aiff"),
            filename: Some(format!("t{id}.aiff")),
            folder: Some("House".to_string()),
            format: Some("aiff".to_string()),
            bitrate: Some(1411),
            duration,
            truncated: false,
            fingerprint: None,
        }
    }

    /// Le résultat PUBLIÉ doit être inchangé, pas seulement l'ensemble des paires.
    ///
    /// Toutes les pistes partagent la même empreinte, donc toute paire évaluée matche : ce qui
    /// décide des groupes est alors la durée, et rien d'autre. Le jeu est construit autour des
    /// bords — deux liens EXACTEMENT à la tolérance, un juste au-delà — et le dernier membre ne
    /// rejoint le groupe que par TRANSITIVITÉ, jamais par un lien direct avec le premier.
    #[test]
    fn group_duplicates_matches_the_naive_pairwise_scan_across_window_edges() {
        let fp = synth_fp(4_242);
        // 199,0 · 200,5 · 201,0 · 203,0 · 205,000001
        //   (1,3) = 2,0 exactement → lie · (3,4) = 2,0 exactement → lie
        //   (4,5) = 2,000001 → n'a PAS le droit de lier
        let durations = [199.0, 200.5, 201.0, 203.0, 205.000_001];
        let rows: Vec<DupScanRow> = durations
            .iter()
            .enumerate()
            .map(|(i, &d)| row_at(i as i64 + 1, Some(d)))
            .collect();
        let fps: Vec<Option<Vec<u32>>> = (0..rows.len()).map(|_| Some(fp.clone())).collect();

        let windowed = group_duplicates(&rows, &fps);
        assert_eq!(
            shape(&windowed),
            shape(&group_duplicates_naive(&rows, &fps))
        );
        assert_eq!(
            shape(&windowed),
            vec![vec![1, 2, 3, 4]],
            "la 5e est a 2,000001 s de la 4e — au-delà de la tolérance, donc dehors"
        );

        // Un seul micro-pas ramène la 5e EXACTEMENT à la tolérance : elle doit entrer.
        let mut rows = rows;
        rows[4].duration = Some(205.0);
        let windowed = group_duplicates(&rows, &fps);
        assert_eq!(
            shape(&windowed),
            shape(&group_duplicates_naive(&rows, &fps))
        );
        assert_eq!(
            shape(&windowed),
            vec![vec![1, 2, 3, 4, 5]],
            "exactement a la tolerance = dedans, la condition est `>` et pas `>=`"
        );
    }

    /// Même exigence sur le chemin de PRODUCTION (`refresh_incremental` est son miroir de test) :
    /// une piste sans durée doit continuer de se comparer à toute la bibliothèque, et les liens de
    /// bord doivent survivre au passage par `dup_edges`.
    #[test]
    fn incremental_scan_matches_the_naive_pairwise_scan_across_window_edges() {
        let fp = synth_fp(777);
        let durations = [Some(240.0), Some(242.0), Some(244.0), None, Some(400.0)];

        let mut conn = db();
        for (i, d) in durations.iter().enumerate() {
            add_filed(&conn, &format!("/lib/t{}.aiff", i + 1), *d, &fp);
        }
        let incremental = refresh_incremental(&mut conn).unwrap();

        let rows: Vec<DupScanRow> = durations
            .iter()
            .enumerate()
            .map(|(i, &d)| row_at(i as i64 + 1, d))
            .collect();
        let fps: Vec<Option<Vec<u32>>> = (0..rows.len()).map(|_| Some(fp.clone())).collect();

        assert_eq!(
            shape(&incremental),
            shape(&group_duplicates_naive(&rows, &fps))
        );
        assert_eq!(
            shape(&incremental),
            vec![vec![1, 2, 3, 4, 5]],
            "chaîne 240-242-244 par bords exacts, puis la 4e sans durée agrège la 5e"
        );
    }
}
