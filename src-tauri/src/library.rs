//! The destination bins: every subdirectory (recursive) under the configured library
//! root. Walks the tree with `walkdir`, skipping hidden dirs (e.g. the `.sift-trash`
//! corbeille). Also creates new bins and resolves collision-free destination paths. Pure
//! filesystem work; the root path comes from `settings::LIBRARY_ROOT`.
//!
//! Also exposes `list_filed` / `folder_facets` for the M6b library browser (read-only
//! DB queries over the `filed` tracks).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

// ── M6b library browser ──────────────────────────────────────────────────────

/// A filed track for the library browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryTrack {
    pub id: i64,
    pub path: String,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub format: Option<String>,
    pub bitrate: Option<i64>,
    pub duration: Option<f64>,
    pub bpm: Option<i64>,
    pub year: Option<i64>,
    pub label: Option<String>,
    pub genres: Vec<String>,
    pub discogs_release_id: Option<String>,
    pub cover_path: Option<String>,
    pub has_cover: bool,
    pub verdict: Option<String>,
    pub folder: Option<String>,
}

/// Recompose the complete title the Bibliothèque shows and edits, from the split `metadata` stores.
/// Routes through `naming::tag_title` so the list, the file's Title tag and the rendered filename
/// are all produced by one function. `None` title (no metadata row yet) stays `None`: a version
/// without a title has nothing to suffix.
fn full_title(title: Option<String>, version: Option<String>) -> Option<String> {
    let title = title?;
    Some(crate::naming::tag_title(&crate::naming::Canonical {
        artist: String::new(), // unused by `tag_title`
        title,
        version,
        label: None, // unused by `tag_title`
        confidence: crate::naming::Confidence::Green,
    }))
}

/// Server-side filters for the library list.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibraryFilter {
    /// Restrict to one folder (exact match on `tracks.folder`).
    pub folder: Option<String>,
    /// `lossless` (aiff/wav/flac/aif) or `mp3`; `None`/other = all.
    pub quality: Option<String>,
    /// Restrict by genre (exact, via track_genres).
    pub genre: Option<String>,
    /// Free text over artist/title/path (case-insensitive contains).
    pub q: Option<String>,
    /// Restrict to a verdict (currently only "fake" is used, by the dashboard's "À re-sourcer" card).
    pub verdict: Option<String>,
    /// Restrict by artist (exact match on `metadata.artist`).
    pub artist: Option<String>,
}

/// A facet bucket (folder or genre) with its filed-track count.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibraryFolder {
    pub name: String,
    pub count: i64,
}

/// Both facet lists for the library sidebar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryFacets {
    pub folders: Vec<LibraryFolder>,
    pub genres: Vec<LibraryFolder>,
    pub artists: Vec<LibraryFolder>,
}

/// One genre with its `filed`-track count, ordered by count desc then name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenreCount {
    pub genre: String,
    pub count: i64,
}

/// Aggregate stats for the Bibliothèque dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub total: i64,
    pub lossless: i64,
    pub mp3: i64,
    /// Nombre de groupes de doublons non résolus. Rempli par `ipc_library::library_stats`, HORS
    /// du verrou global — `library::library_stats` le laisse à 0 (SYS-1, 2026-07-28).
    pub duplicates: i64,
    /// Tracks with verdict = 'fake', i.e. to re-source.
    pub fake: i64,
    pub genres: Vec<GenreCount>,
}

/// Cheap signature of the `filed` set: (count, max id). Any filing commit, revert, or purge
/// changes the filed count and/or the max id, so a mismatch means the cached duplicate scan is
/// stale. Fingerprint recomputes don't change grouping outcomes, so they're intentionally not
/// part of the key. Cheap enough to recheck on every dashboard load.
pub fn filed_signature(conn: &rusqlite::Connection) -> rusqlite::Result<(i64, i64)> {
    conn.query_row(
        "SELECT COUNT(*), COALESCE(MAX(id), 0) FROM tracks WHERE status='filed'",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
}

/// Comptage mémoïsé des groupes de doublons, indexé sur `filed_signature`. Le scan complet
/// (chargement des lignes, empreintes, comparaison O(n²)) est le coût dominant du tableau de bord ;
/// celui-ci est rechargé à chaque visite alors que le jeu `filed` bouge rarement entre deux, d'où
/// le recalcul sur changement de signature seulement. `invalidate_duplicate_count_cache()` force
/// un recalcul pour les appelants qui mutent le jeu `filed` par un chemin que la signature ne voit
/// pas.
///
/// Le calcul lui-même ne vit plus ici : il est fourni par l'appelant et exécuté hors du verrou
/// global (SYS-1, 2026-07-28). Ce module n'expose qu'UNE porte d'entrée,
/// `duplicate_count_or_compute`, plus `filed_signature` et `invalidate_duplicate_count_cache`.
///
/// `(filed_signature, duplicate-group count)` — the cache slot's single entry.
type DupCountEntry = ((i64, i64), i64);

static DUP_COUNT_CACHE: std::sync::OnceLock<std::sync::Mutex<Option<DupCountEntry>>> =
    std::sync::OnceLock::new();

fn dup_count_cache() -> &'static std::sync::Mutex<Option<DupCountEntry>> {
    DUP_COUNT_CACHE.get_or_init(|| std::sync::Mutex::new(None))
}

/// Compteur de génération du cache, incrémenté par `invalidate_duplicate_count_cache`.
///
/// Nécessaire depuis que le calcul se fait HORS du verrou global (SYS-1) : entre le moment où on
/// constate un cache miss et celui où on mémorise le résultat, une invalidation concurrente peut
/// survenir. Sans ce compteur, `store_duplicate_count` l'écraserait et réinstallerait un comptage
/// périmé sous une signature INCHANGÉE — donc durablement, puisque c'est justement le cas que la
/// signature ne sait pas voir et que `invalidate_duplicate_count_cache` existe pour couvrir.
/// Trouvé par le crosscheck de la gate pre-commit sur la première version de ce correctif.
static DUP_COUNT_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Comptage en cache pour cette signature, ou `None` s'il faut recalculer.
/// Mutex empoisonné → traité comme une absence de cache : on ne sert jamais une valeur
/// potentiellement périmée.
fn cached_duplicate_count(sig: (i64, i64)) -> Option<i64> {
    let guard = dup_count_cache().lock().ok()?;
    match *guard {
        Some((cached_sig, count)) if cached_sig == sig => Some(count),
        _ => None,
    }
}

/// Sérialise les calculs de comptage entre eux. NE protège aucune donnée — c'est un jeton de
/// « un seul en vol à la fois ».
static DUP_COMPUTE_FLIGHT: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();

fn dup_compute_flight() -> &'static std::sync::Mutex<()> {
    DUP_COMPUTE_FLIGHT.get_or_init(|| std::sync::Mutex::new(()))
}

/// Rend le comptage de groupes de doublons pour `sig` : depuis le cache s'il est valide, sinon en
/// appelant `compute` — **au plus un calcul en vol à la fois**.
///
/// Interface unique de ce cache, délibérément. Une version antérieure exposait quatre primitives
/// (signature, génération, lecture, écriture) et laissait l'appelant les enchaîner correctement :
/// il ne l'a pas fait, et deux défauts sont passés (voir plus bas). Ici l'appelant ne fournit QUE
/// le calcul coûteux ; l'ordre des étapes n'est plus son affaire.
///
/// Trois propriétés, chacune payée par un défaut réel trouvé au crosscheck de la gate :
///
/// 1. **Single-flight.** Avant SYS-1, le verrou global `Mutex<Connection>` sérialisait de fait ces
///    calculs : deux rendus concurrents du tableau de bord ne pouvaient pas décoder la
///    bibliothèque en même temps. Sortir le calcul du verrou a supprimé cette garantie sans la
///    remplacer — N appels concurrents auraient refait N décodages disque complets en parallèle.
///    Le jeton ci-dessous la rétablit, et le second arrivant trouve le résultat en cache plutôt
///    que de recalculer.
/// 2. **Pas de réinstallation d'un comptage périmé.** Une invalidation survenue PENDANT le calcul
///    non verrouillé ne doit pas être écrasée par le résultat en vol : c'est justement le cas que
///    la signature ne sait pas voir et que `invalidate_duplicate_count_cache` existe pour couvrir.
///    D'où le compteur de génération, lu avant le calcul et revérifié sous le verrou du cache.
/// 3. **Rien n'est calculé sur un cache hit.** Le hit est le cas NORMAL — le tableau de bord est
///    rechargé à chaque visite alors que le jeu `filed` bouge rarement.
///
/// **Invariant d'appel** : ne jamais appeler en tenant le `Mutex<Connection>` global. `compute` le
/// prend brièvement ; le tenir déjà inverserait l'ordre des verrous entre ce jeton et celui de la
/// connexion, donc interblocage. Le seul appelant, `ipc_library::library_stats`, le relâche avant.
pub fn duplicate_count_or_compute<F, E>(sig: (i64, i64), compute: F) -> Result<i64, E>
where
    F: FnOnce() -> Result<i64, E>,
{
    if let Some(count) = cached_duplicate_count(sig) {
        return Ok(count);
    }
    // Un mutex empoisonné ici n'a rien corrompu : il ne garde aucune donnée, seulement le droit de
    // calculer. On reprend le jeton plutôt que de refuser le service.
    let _flight = match dup_compute_flight().lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    // Deuxième lecture, après l'attente : celui qui nous précédait vient peut-être de remplir le
    // cache pour cette même signature. C'est ce qui rend le single-flight utile plutôt que
    // seulement sérialisant.
    if let Some(count) = cached_duplicate_count(sig) {
        return Ok(count);
    }

    let generation = DUP_COUNT_GEN.load(std::sync::atomic::Ordering::SeqCst);
    let count = compute()?;

    if DUP_COUNT_GEN.load(std::sync::atomic::Ordering::SeqCst) == generation {
        if let Ok(mut guard) = dup_count_cache().lock() {
            // Revérifié SOUS le verrou du cache : sans cela, une invalidation glissée entre le
            // test ci-dessus et la prise du verrou passerait encore.
            if DUP_COUNT_GEN.load(std::sync::atomic::Ordering::SeqCst) == generation {
                *guard = Some((sig, count));
            }
        }
    }
    // Le comptage reste correct pour CE rendu même s'il n'a pas été archivé : seule la prochaine
    // visite paiera un recalcul.
    Ok(count)
}

/// Drops the cached duplicate-group count so the next `library_stats` recomputes it. Call after
/// any change to the `filed` set that `filed_signature` might not observe (e.g. an in-place
/// re-filing that leaves the filed count and max id unchanged). Safe to call from any thread.
pub fn invalidate_duplicate_count_cache() {
    // La génération est incrémentée AVANT de vider, pour qu'un calcul non verrouillé déjà en vol
    // voie forcément un écart et refuse de mémoriser son résultat.
    DUP_COUNT_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    if let Ok(mut guard) = dup_count_cache().lock() {
        *guard = None;
    }
}

/// Aggregate counts for the Bibliothèque dashboard. Read-only.
///
/// Ne calcule PAS le nombre de doublons : `duplicates` est renvoyé à 0 et l'appelant doit le
/// remplir. C'est délibéré — sur un cache miss ce comptage décode de l'audio depuis le disque, et
/// il tenait le verrou global pendant tout ce temps (audit 2026-07-28, SYS-1). Le seul appelant,
/// `ipc_library::library_stats`, le calcule maintenant HORS verrou. Les six requêtes ci-dessous
/// sont, elles, des agrégats SQL brefs qui restent sous le verrou sans dommage.
pub fn library_stats(conn: &rusqlite::Connection) -> rusqlite::Result<DashboardStats> {
    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tracks WHERE status='filed'",
        [],
        |r| r.get(0),
    )?;
    // `target_format`, PAS `format` : `tracks.format` n'est écrit par aucun code de production
    // (voir le commentaire de `list_filed`). Ces deux compteurs rendaient donc 0 quelle que soit la
    // bibliothèque, sur un écran qui affiche « Lossless » et « MP3 » en gros.
    let lossless: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM tracks WHERE status='filed' AND target_format IN {}",
            crate::encode::TARGET_LOSSLESS_SQL_IN
        ),
        [],
        |r| r.get(0),
    )?;
    let mp3: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM tracks WHERE status='filed' AND target_format IN {}",
            crate::encode::TARGET_LOSSY_SQL_IN
        ),
        [],
        |r| r.get(0),
    )?;
    // `verdict_ver` en plus de `verdict` : la carte « À re-sourcer » propose une action sur des
    // fichiers achetés, elle ne doit compter que des verdicts que le moteur COURANT produirait
    // (issue #39). Un FAKE d'une génération précédente sort du compte au lieu d'y rester
    // indéfiniment — la piste réapparaîtra dès qu'un verdict courant sera écrit.
    let fake: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tracks WHERE status='filed' AND verdict='fake' AND verdict_ver=?1",
        rusqlite::params![crate::analysis::verdict::VERDICT_CACHE_VERSION],
        |r| r.get(0),
    )?;
    // Rempli par l'appelant, hors verrou — voir la doc de cette fonction.
    let duplicates = 0i64;

    let mut stmt = conn.prepare(
        "SELECT g.genre, COUNT(*) FROM track_genres g \
         JOIN tracks t ON t.id = g.track_id AND t.status='filed' \
         GROUP BY g.genre ORDER BY COUNT(*) DESC, g.genre",
    )?;
    let genres = stmt
        .query_map([], |r| {
            Ok(GenreCount {
                genre: r.get(0)?,
                count: r.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(DashboardStats {
        total,
        lossless,
        mp3,
        duplicates,
        fake,
        genres,
    })
}

/// All `filed` tracks joined to their metadata + genres, filtered. Read-only.
pub fn list_filed(
    conn: &rusqlite::Connection,
    f: &LibraryFilter,
) -> rusqlite::Result<Vec<LibraryTrack>> {
    // `t.verdict_ver` voyage avec `t.verdict` (dernière colonne, pour ne pas décaler les index
    // existants) et la valeur brute ne sort jamais sans `verdict::cached` : un badge FAKE produit
    // par un moteur périmé s'efface au lieu de rester affiché (issue #39). La version elle-même
    // n'est pas publiée — `LibraryTrack` et `shared/contracts.ts` sont inchangés.
    let mut sql = String::from(
        "SELECT t.id, t.path, t.target_format, t.bitrate, t.duration, t.verdict, t.folder, t.has_cover, \
                m.artist, m.title, m.label, m.year, m.bpm, m.cover_path, m.discogs_release_id, \
                m.version, t.verdict_ver \
         FROM tracks t LEFT JOIN metadata m ON m.track_id = t.id \
         WHERE t.status = 'filed'",
    );
    if f.folder.is_some() {
        sql.push_str(" AND t.folder = :folder");
    }
    if let Some(q) = &f.quality {
        match q.as_str() {
            "lossless" => sql.push_str(&format!(
                " AND t.target_format IN {}",
                crate::encode::TARGET_LOSSLESS_SQL_IN
            )),
            "mp3" => sql.push_str(&format!(
                " AND t.target_format IN {}",
                crate::encode::TARGET_LOSSY_SQL_IN
            )),
            _ => {}
        }
    }
    if f.verdict.is_some() {
        // Filtrer sur le verdict SANS sa version rendrait des lignes que la colonne Verdict de la
        // même table affiche vides (`verdict::cached` les efface) — un filtre qui trouve ce que
        // l'écran ne montre pas.
        sql.push_str(" AND t.verdict = :verdict AND t.verdict_ver = :verdict_ver");
    }
    if f.q.is_some() {
        // `m.version` fait partie du titre affiché (voir la composition plus bas) : sans elle,
        // chercher « Fluent Remix » ne rendrait rien sur une ligne qui l'affiche.
        sql.push_str(
            " AND (m.artist LIKE :like OR m.title LIKE :like OR m.version LIKE :like OR t.path LIKE :like)",
        );
    }
    if f.genre.is_some() {
        sql.push_str(" AND t.id IN (SELECT track_id FROM track_genres WHERE genre = :genre)");
    }
    if f.artist.is_some() {
        sql.push_str(" AND m.artist = :artist");
    }
    sql.push_str(" ORDER BY m.artist, m.title, t.path");

    let like = f.q.as_ref().map(|q| format!("%{q}%"));
    let verdict_ver = crate::analysis::verdict::VERDICT_CACHE_VERSION;
    let mut stmt = conn.prepare(&sql)?;
    let params: Vec<(&str, &dyn rusqlite::ToSql)> = {
        let mut p: Vec<(&str, &dyn rusqlite::ToSql)> = Vec::new();
        if let Some(folder) = &f.folder {
            p.push((":folder", folder));
        }
        if let Some(v) = &f.verdict {
            p.push((":verdict", v));
            p.push((":verdict_ver", &verdict_ver));
        }
        if let Some(l) = &like {
            p.push((":like", l));
        }
        if let Some(g) = &f.genre {
            p.push((":genre", g));
        }
        if let Some(artist) = &f.artist {
            p.push((":artist", artist as &dyn rusqlite::ToSql));
        }
        p
    };
    let rows = stmt
        .query_map(params.as_slice(), |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<i64>>(3)?,
                r.get::<_, Option<f64>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<i64>>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, Option<String>>(9)?,
                r.get::<_, Option<String>>(10)?,
                r.get::<_, Option<i64>>(11)?,
                r.get::<_, Option<i64>>(12)?,
                r.get::<_, Option<String>>(13)?,
                r.get::<_, Option<String>>(14)?,
                r.get::<_, Option<String>>(15)?,
                r.get::<_, Option<i64>>(16)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    // `format` sort de `tracks.target_format` — le format que Sift a RÉELLEMENT écrit en rangeant —
    // et non de `tracks.format`, colonne déclarée en v1 du schéma (`db.rs:24`) qu'aucun code de
    // production n'a jamais renseignée : ni `scanner::upsert_file`, ni `worker::persist_report` (qui
    // écrit `declared_fmt`), ni `filing`. Elle était NULL sur toute vraie base, donc la colonne
    // Format de la Bibliothèque affichait « ? » partout, les deux compteurs du tableau de bord
    // rendaient 0, et le filtre Lossless/MP3 ne renvoyait jamais rien. Seuls les tests, qui la
    // semaient à la main, la voyaient remplie. `declared_fmt` ne conviendrait pas non plus : il
    // garde l'extension SOURCE, celle d'avant conversion.
    // FIX-22: one batched genres query for every row instead of one query per row.
    let ids: Vec<i64> = rows.iter().map(|r| r.0).collect();
    // `metadata` stores the identity SPLIT (base title + version), because that is what
    // `render_filename` needs. La Bibliothèque n'a qu'un champ Titre : elle doit donc recevoir le
    // titre COMPLET, sinon elle affiche une piste amputée de son remix et la réécrit amputée au
    // premier enregistrement. Recomposé par `naming::tag_title` — la fonction qui rend déjà le nom
    // de fichier et le tag Titre, donc les trois ne peuvent pas diverger.
    let mut genres_by_track = crate::genres::get_genres_batch(conn, &ids)?;

    let mut out = Vec::with_capacity(rows.len());
    for (
        id,
        path,
        format,
        bitrate,
        duration,
        verdict,
        folder,
        has_cover,
        artist,
        title,
        label,
        year,
        bpm,
        cover_path,
        rel,
        version,
        verdict_ver_row,
    ) in rows
    {
        out.push(LibraryTrack {
            id,
            path,
            artist,
            title: full_title(title, version),
            // 'aiff_16_44' → "aiff" : l'écran montre un format, pas une clé de base.
            format: format
                .as_deref()
                .and_then(crate::encode::Target::from_db_value)
                .map(|t| t.ext().to_string()),
            bitrate,
            duration,
            bpm,
            year,
            label,
            genres: genres_by_track.remove(&id).unwrap_or_default(),
            discogs_release_id: rel,
            cover_path,
            has_cover: has_cover.unwrap_or(0) != 0,
            verdict: crate::analysis::verdict::cached(verdict, verdict_ver_row),
            folder,
        });
    }
    Ok(out)
}

/// Counts of `filed` tracks grouped by folder and by genre. Read-only.
pub fn folder_facets(conn: &rusqlite::Connection) -> rusqlite::Result<LibraryFacets> {
    let folders = query_facets(
        conn,
        "SELECT folder, COUNT(*) FROM tracks \
         WHERE status='filed' AND folder IS NOT NULL AND folder <> '' \
         GROUP BY folder ORDER BY folder",
    )?;
    let genres = query_facets(
        conn,
        "SELECT g.genre, COUNT(*) FROM track_genres g \
         JOIN tracks t ON t.id = g.track_id AND t.status='filed' \
         GROUP BY g.genre ORDER BY g.genre",
    )?;
    let artists = query_facets(
        conn,
        "SELECT m.artist, COUNT(*) FROM metadata m \
         JOIN tracks t ON t.id = m.track_id AND t.status='filed' \
         WHERE m.artist IS NOT NULL AND m.artist <> '' \
         GROUP BY m.artist ORDER BY m.artist",
    )?;
    Ok(LibraryFacets {
        folders,
        genres,
        artists,
    })
}

fn query_facets(conn: &rusqlite::Connection, sql: &str) -> rusqlite::Result<Vec<LibraryFolder>> {
    let mut stmt = conn.prepare(sql)?;
    let mapped = stmt.query_map([], |r| {
        Ok(LibraryFolder {
            name: r.get(0)?,
            count: r.get(1)?,
        })
    })?;
    mapped.collect()
}

/// One destination folder under the library root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Bin {
    /// Path relative to the root, forward-slash separated (e.g. "House/Deep").
    pub rel: String,
    /// Display name = last path component (e.g. "Deep").
    pub name: String,
    /// Nesting depth under root (1 = direct child).
    pub depth: usize,
}

/// Whether a directory name is hidden (leading dot) — excluded from bins.
fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

/// List all bins (recursive subdirectories) under `root`, sorted by relative path. Returns
/// an empty list if root doesn't exist. Hidden directories and their subtrees are skipped.
pub fn list_bins(root: &Path) -> Vec<Bin> {
    let mut bins = Vec::new();
    let walker = WalkDir::new(root)
        .min_depth(1)
        .into_iter()
        .filter_entry(|e| {
            // skip hidden dirs entirely (prunes their subtree too)
            !e.file_name().to_str().map(is_hidden).unwrap_or(false)
        });
    for entry in walker.flatten() {
        if !entry.file_type().is_dir() {
            continue;
        }
        let rel_path = match entry.path().strip_prefix(root) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let rel = rel_path
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect::<Vec<_>>()
            .join("/");
        if rel.is_empty() {
            continue;
        }
        let name = entry.file_name().to_str().unwrap_or_default().to_string();
        let depth = entry.depth();
        bins.push(Bin { rel, name, depth });
    }
    bins.sort_by(|a, b| a.rel.cmp(&b.rel));
    bins
}

/// Join `rel` under `root`, sanitizing every path segment and refusing anything that would
/// escape the root: `..`, absolute paths, or drive prefixes (both `/` and `\` separators
/// are accepted from the UI). Returns the contained absolute path. This is the single
/// containment guard every filesystem-mutating command must funnel destinations through —
/// `bin_rel` / `parent_rel` arrive from the (untrusted) webview and are otherwise free to
/// point anywhere (`..\..\Startup`, `C:\Windows\…`), which `Path::join` would honour.
pub fn safe_join(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let mut out = root.to_path_buf();
    for raw in rel.split(['/', '\\']) {
        if raw.is_empty() || raw == "." {
            continue;
        }
        if raw == ".." {
            return Err("path escapes the library root".into());
        }
        let safe = crate::naming::sanitize(raw);
        if safe.is_empty() {
            return Err("invalid path component".into());
        }
        out.push(safe);
    }
    Ok(out)
}

/// Create a new bin folder named `name` (sanitized) under `root/parent_rel`. `parent_rel`
/// "" means directly under root. Both `name` and every component of `parent_rel` are
/// sanitized and contained under `root` (see `safe_join`). Returns the created Bin.
pub fn create_bin(root: &Path, parent_rel: &str, name: &str) -> Result<Bin, String> {
    let safe = crate::naming::sanitize(name);
    if safe.is_empty() {
        return Err("empty bin name".into());
    }
    let abs = safe_join(root, parent_rel)?.join(&safe);
    std::fs::create_dir_all(&abs).map_err(|e| format!("create bin: {e}"))?;
    let rel = abs
        .strip_prefix(root)
        .map_err(|_| "bin outside root".to_string())?
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("/");
    let depth = rel.split('/').count();
    Ok(Bin {
        rel,
        name: safe,
        depth,
    })
}

/// True when `a` and `b` denote the same on-disk file. Prefers `canonicalize` (resolves
/// case/`.`/`..`/symlinks — needed on Windows where paths are case-insensitive), and falls
/// back to a plain `PathBuf` compare when either side can't be canonicalized (doesn't exist).
fn same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

/// Return a path that does not already exist, appending " (N)" before the extension when
/// the given path is taken. Used so filing never overwrites an existing file. `ignore` is an
/// optional "self" path that does NOT count as a collision — pass the source file when filing
/// in place so a conformant track keeps its own name instead of gaining a parasitic " (2)".
pub fn ensure_unique(path: &Path, ignore: Option<&Path>) -> PathBuf {
    let is_self = |p: &Path| ignore.is_some_and(|ig| same_path(p, ig));
    if !path.exists() || is_self(path) {
        return path.to_path_buf();
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|e| e.to_str());
    for n in 2..10_000 {
        let candidate = match ext {
            Some(e) => parent.join(format!("{stem} ({n}).{e}")),
            None => parent.join(format!("{stem} ({n})")),
        };
        if !candidate.exists() || is_self(&candidate) {
            return candidate;
        }
    }
    // pathological fallback: timestamped name
    parent.join(format!("{stem} ({}).bak", std::process::id()))
}

#[cfg(test)]
mod dup_count_cache_tests {
    use super::*;
    use std::sync::atomic::Ordering;

    /// Le cache est un `static` de processus : deux tests qui le manipulent en parallèle se
    /// marcheraient dessus (`cargo test` est multi-thread par défaut). Ce verrou les sérialise.
    static TEST_SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn fresh() -> std::sync::MutexGuard<'static, ()> {
        let g = match TEST_SERIAL.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        invalidate_duplicate_count_cache();
        g
    }

    /// Le premier appel calcule, le second sert le cache — c'est la raison d'être du memo.
    #[test]
    fn second_call_with_same_signature_does_not_recompute() {
        let _serial = fresh();
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let compute = || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok::<i64, String>(7)
        };

        assert_eq!(duplicate_count_or_compute((3, 42), compute).unwrap(), 7);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(duplicate_count_or_compute((3, 42), compute).unwrap(), 7);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "le second appel doit venir du cache"
        );
    }

    /// Une signature différente est un jeu `filed` différent : le cache ne doit pas répondre.
    #[test]
    fn a_different_signature_recomputes() {
        let _serial = fresh();
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let compute = || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok::<i64, String>(1)
        };

        let _ = duplicate_count_or_compute((3, 42), compute).unwrap();
        let _ = duplicate_count_or_compute((4, 42), compute).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    /// LE cas trouvé par le crosscheck de la gate. Une invalidation qui tombe PENDANT le calcul
    /// non verrouillé ne doit pas être écrasée par le résultat en vol : sans le compteur de
    /// génération, l'appel suivant servirait un comptage périmé sous une signature INCHANGÉE —
    /// donc durablement, puisque c'est exactement ce que la signature ne sait pas voir.
    #[test]
    fn an_invalidation_during_the_compute_is_not_overwritten() {
        let _serial = fresh();
        let sig = (3, 42);

        // Le calcul invalide pendant qu'il tourne — c'est ce que fait une écriture concurrente.
        let first = duplicate_count_or_compute(sig, || {
            invalidate_duplicate_count_cache();
            Ok::<i64, String>(7)
        })
        .unwrap();
        assert_eq!(first, 7, "le comptage reste correct pour CE rendu");

        // Le résultat ne doit PAS avoir été archivé : l'appel suivant recalcule.
        let recomputed = std::sync::atomic::AtomicUsize::new(0);
        let second = duplicate_count_or_compute(sig, || {
            recomputed.fetch_add(1, Ordering::SeqCst);
            Ok::<i64, String>(9)
        })
        .unwrap();
        assert_eq!(
            recomputed.load(Ordering::SeqCst),
            1,
            "un comptage calculé par-dessus une invalidation ne doit jamais être mémorisé"
        );
        assert_eq!(second, 9);
    }

    /// Sans invalidation concurrente, le résultat DOIT être archivé — sinon le garde de génération
    /// serait trop strict et le cache ne servirait jamais. Témoin symétrique du test précédent.
    #[test]
    fn without_concurrent_invalidation_the_result_is_stored() {
        let _serial = fresh();
        let sig = (5, 99);
        assert_eq!(
            duplicate_count_or_compute(sig, || Ok::<i64, String>(4)).unwrap(),
            4
        );
        let recomputed = std::sync::atomic::AtomicUsize::new(0);
        let again = duplicate_count_or_compute(sig, || {
            recomputed.fetch_add(1, Ordering::SeqCst);
            Ok::<i64, String>(999)
        })
        .unwrap();
        assert_eq!(recomputed.load(Ordering::SeqCst), 0);
        assert_eq!(again, 4);
    }

    /// Une erreur du calcul remonte telle quelle et ne pollue pas le cache.
    #[test]
    fn a_failed_compute_caches_nothing() {
        let _serial = fresh();
        let sig = (1, 1);
        let err = duplicate_count_or_compute(sig, || Err::<i64, String>("boom".into()));
        assert_eq!(err, Err("boom".to_string()));

        let calls = std::sync::atomic::AtomicUsize::new(0);
        let _ = duplicate_count_or_compute(sig, || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok::<i64, String>(2)
        })
        .unwrap();
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "un échec ne doit rien mémoriser"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mirrors shared/contracts.ts's `LibraryTrack`. Exhaustive destructure (no `..`): fails to
    /// compile if a field is added/removed/renamed on the Rust struct — the forcing function to
    /// also update contracts.ts. Phase 2 — docs/superpowers/plans/2026-07-13-phase2-ipc-contract-tests.md.
    #[test]
    fn library_track_shape_matches_contracts_ts() {
        let v = LibraryTrack {
            id: 0,
            path: String::new(),
            artist: None,
            title: None,
            format: None,
            bitrate: None,
            duration: None,
            bpm: None,
            year: None,
            label: None,
            genres: Vec::new(),
            discogs_release_id: None,
            cover_path: None,
            has_cover: false,
            verdict: None,
            folder: None,
        };
        let LibraryTrack {
            id,
            path,
            artist,
            title,
            format,
            bitrate,
            duration,
            bpm,
            year,
            label,
            genres,
            discogs_release_id,
            cover_path,
            has_cover,
            verdict,
            folder,
        } = v;
        let _ = (
            id,
            path,
            artist,
            title,
            format,
            bitrate,
            duration,
            bpm,
            year,
            label,
            genres,
            discogs_release_id,
            cover_path,
            has_cover,
            verdict,
            folder,
        );
    }

    #[test]
    fn lists_recursive_bins_sorted_skipping_hidden() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("House/Deep")).unwrap();
        std::fs::create_dir_all(root.join("House/Acid")).unwrap();
        std::fs::create_dir_all(root.join("Techno")).unwrap();
        std::fs::create_dir_all(root.join(".sift-trash/42")).unwrap();

        let bins = list_bins(root);
        let rels: Vec<&str> = bins.iter().map(|b| b.rel.as_str()).collect();
        assert_eq!(rels, vec!["House", "House/Acid", "House/Deep", "Techno"]);
        // hidden subtree excluded
        assert!(!rels.iter().any(|r| r.contains("sift-trash")));
        // depth + name sane
        let deep = bins.iter().find(|b| b.rel == "House/Deep").unwrap();
        assert_eq!(deep.name, "Deep");
        assert_eq!(deep.depth, 2);
    }

    #[test]
    fn missing_root_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("nope");
        assert!(list_bins(&root).is_empty());
    }

    #[test]
    fn create_bin_makes_sanitized_subfolder() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("House")).unwrap();

        let bin = create_bin(root, "House", "Deep/Soulful?").unwrap();
        assert_eq!(bin.rel, "House/Deep Soulful"); // "/" and "?" sanitized to spaces→collapsed
        assert!(root.join("House/Deep Soulful").is_dir());
    }

    #[test]
    fn create_bin_at_root_level() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let bin = create_bin(root, "", "Disco").unwrap();
        assert_eq!(bin.rel, "Disco");
        assert_eq!(bin.depth, 1);
        assert!(root.join("Disco").is_dir());
    }

    #[test]
    fn safe_join_contains_under_root() {
        let root = Path::new("C:/lib");
        // traversal (either separator) is refused
        assert!(safe_join(root, "../evil").is_err());
        assert!(safe_join(root, "House/../../x").is_err());
        assert!(safe_join(root, "..\\evil").is_err());
        // normal nested path is contained
        let j = safe_join(root, "House/Deep").unwrap();
        assert!(j.ends_with("Deep") && j.starts_with("C:/lib"));
        // an absolute/drive-prefixed rel is sanitized into components under root, not honoured
        let a = safe_join(root, "C:/Windows/System32").unwrap();
        assert!(a.starts_with("C:/lib"));
        // "" and "." resolve to the root itself
        assert_eq!(safe_join(root, "").unwrap(), root.to_path_buf());
    }

    #[test]
    fn create_bin_rejects_parent_traversal() {
        let dir = tempfile::tempdir().unwrap();
        assert!(create_bin(dir.path(), "../../etc", "evil").is_err());
    }

    #[test]
    fn ensure_unique_appends_suffix_on_collision() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("Track.mp3");
        // free → unchanged
        assert_eq!(ensure_unique(&base, None), base);
        // occupied → " (2)"
        std::fs::write(&base, b"x").unwrap();
        assert_eq!(ensure_unique(&base, None), dir.path().join("Track (2).mp3"));
    }

    #[test]
    fn ensure_unique_keeps_name_when_collision_is_the_ignored_self() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("Track.aiff");
        std::fs::write(&base, b"x").unwrap();
        // the file exists, but it IS the source we're filing in place → keep the name, no " (2)"
        assert_eq!(ensure_unique(&base, Some(&base)), base);
    }

    // ── M6b library browser tests ────────────────────────────────────────────

    fn db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::run_migrations(&conn).unwrap();
        conn
    }

    /// La version de cache que `worker::persist_report` stampe dans le MÊME `UPDATE` que le
    /// verdict. Tout seed de test qui écrit `verdict` doit l'écrire aussi : sans elle la ligne
    /// décrit un état que la production ne produit plus (un verdict d'avant la v22), et
    /// `verdict::cached` l'efface à la lecture — le test mesurerait alors l'invalidation au lieu
    /// de ce qu'il vise.
    const VER: i64 = crate::analysis::verdict::VERDICT_CACHE_VERSION;

    #[test]
    fn list_filed_joins_metadata_and_genres() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(id, path, target_format, bitrate, duration, verdict, verdict_ver, status, folder, has_cover) \
             VALUES(1, '/lib/House/a.aiff', 'aiff_16_44', 1411, 360.0, 'ok', ?1, 'filed', 'House', 1)",
            rusqlite::params![VER],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks(id, path, format, status) VALUES(2, '/in/pending.mp3', 'mp3', 'pending')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO metadata(track_id, artist, title, label, year, bpm, cover_path, discogs_release_id) \
             VALUES(1, 'Mr Fingers', 'Can You Feel It', 'Trax', 1986, 120, '/cache/1.jpg', '12345')",
            [],
        )
        .unwrap();
        crate::genres::set_genres(&conn, 1, &["House".into(), "Deep House".into()]).unwrap();

        let rows = list_filed(&conn, &LibraryFilter::default()).unwrap();

        assert_eq!(rows.len(), 1, "only filed tracks");
        let t = &rows[0];
        assert_eq!(t.id, 1);
        assert_eq!(t.artist.as_deref(), Some("Mr Fingers"));
        assert_eq!(t.title.as_deref(), Some("Can You Feel It"));
        assert_eq!(t.format.as_deref(), Some("aiff"));
        assert_eq!(t.bitrate, Some(1411));
        assert_eq!(t.verdict.as_deref(), Some("ok"));
        assert_eq!(t.folder.as_deref(), Some("House"));
        assert_eq!(t.discogs_release_id.as_deref(), Some("12345"));
        assert_eq!(
            t.genres,
            vec!["House".to_string(), "Deep House".to_string()]
        );
    }

    /// Régression : la Bibliothèque n'a qu'un champ Titre et `metadata` range l'identité en deux
    /// colonnes. En ne renvoyant que `m.title`, la liste affichait une piste amputée de son remix,
    /// et l'éditeur la réécrivait amputée au premier enregistrement — pendant que le nom du fichier
    /// sur le disque gardait la version.
    #[test]
    fn list_filed_returns_the_complete_title_version_included() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(id, path, format, status) VALUES(1, '/lib/a.aiff', 'aiff', 'filed')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO metadata(track_id, artist, title, version) \
             VALUES(1, 'Chez Damier', 'Can You Feel It', 'Fluent Remix')",
            [],
        )
        .unwrap();

        let rows = list_filed(&conn, &LibraryFilter::default()).unwrap();
        assert_eq!(
            rows[0].title.as_deref(),
            Some("Can You Feel It (Fluent Remix)")
        );

        // Et ce que la liste AFFICHE doit être cherchable : sinon la recherche dément l'écran.
        let f = LibraryFilter {
            q: Some("Fluent".into()),
            ..Default::default()
        };
        assert_eq!(list_filed(&conn, &f).unwrap().len(), 1);
    }

    /// Une piste sans version ne gagne pas de parenthèses vides, et une ligne sans metadata reste
    /// sans titre — le `?` de `full_title` ne doit pas fabriquer un titre à partir d'une version.
    #[test]
    fn list_filed_leaves_a_versionless_title_alone() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(id, path, format, status) VALUES(1, '/lib/a.aiff', 'aiff', 'filed')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks(id, path, format, status) VALUES(2, '/lib/b.aiff', 'aiff', 'filed')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO metadata(track_id, artist, title, version) VALUES(1, 'Mr Fingers', 'Mystery of Love', '')",
            [],
        )
        .unwrap();

        let rows = list_filed(&conn, &LibraryFilter::default()).unwrap();
        let by_id = |id: i64| rows.iter().find(|r| r.id == id).unwrap();
        assert_eq!(by_id(1).title.as_deref(), Some("Mystery of Love"));
        assert_eq!(by_id(2).title, None, "aucune ligne metadata, aucun titre");
    }

    #[test]
    fn list_filed_filters_by_verdict() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(id, path, status, verdict, verdict_ver) \
             VALUES(1, '/lib/a.mp3', 'filed', 'fake', ?1)",
            rusqlite::params![VER],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks(id, path, status, verdict, verdict_ver) \
             VALUES(2, '/lib/b.mp3', 'filed', 'ok', ?1)",
            rusqlite::params![VER],
        )
        .unwrap();
        let f = LibraryFilter {
            verdict: Some("fake".into()),
            ..Default::default()
        };
        let rows = list_filed(&conn, &f).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, 1);
    }

    #[test]
    fn list_filed_filters_by_artist() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(id, path, status) VALUES(1, '/lib/a.mp3', 'filed')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO metadata(track_id, artist) VALUES(1, 'Aya')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks(id, path, status) VALUES(2, '/lib/b.mp3', 'filed')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO metadata(track_id, artist) VALUES(2, 'Rob & Si')",
            [],
        )
        .unwrap();

        let f = LibraryFilter {
            artist: Some("Aya".into()),
            ..Default::default()
        };
        let tracks = list_filed(&conn, &f).unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].artist.as_deref(), Some("Aya"));
    }

    #[test]
    fn folder_facets_counts_filed_by_folder_and_genre() {
        let conn = db();
        for (id, folder) in [(1, "House"), (2, "House"), (3, "Techno")] {
            conn.execute(
                "INSERT INTO tracks(id, path, status, folder) VALUES(?1, ?2, 'filed', ?3)",
                rusqlite::params![id, format!("/lib/{folder}/{id}.aiff"), folder],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO tracks(id, path, status, folder) VALUES(9, '/in/p.mp3', 'pending', 'House')",
            [],
        )
        .unwrap();
        crate::genres::set_genres(&conn, 1, &["House".into()]).unwrap();
        crate::genres::set_genres(&conn, 2, &["House".into()]).unwrap();
        crate::genres::set_genres(&conn, 3, &["Techno".into()]).unwrap();

        let f = folder_facets(&conn).unwrap();

        let house = f.folders.iter().find(|x| x.name == "House").unwrap();
        assert_eq!(house.count, 2, "only filed House tracks");
        assert!(
            f.folders
                .iter()
                .find(|x| x.name == "Techno")
                .map(|x| x.count)
                == Some(1)
        );
        let g_house = f.genres.iter().find(|x| x.name == "House").unwrap();
        assert_eq!(g_house.count, 2);
    }

    #[test]
    fn folder_facets_counts_filed_by_artist() {
        let conn = db();
        for (id, artist) in [(1, "Aya"), (2, "Aya"), (3, "Rob & Si")] {
            conn.execute(
                "INSERT INTO tracks(id, path, status) VALUES(?1, ?2, 'filed')",
                rusqlite::params![id, format!("/lib/{id}.aiff")],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO metadata(track_id, artist) VALUES(?1, ?2)",
                rusqlite::params![id, artist],
            )
            .unwrap();
        }
        // A pending (non-filed) track with an artist must NOT be counted.
        conn.execute(
            "INSERT INTO tracks(id, path, status) VALUES(4, '/lib/4.aiff', 'pending')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO metadata(track_id, artist) VALUES(4, 'Aya')",
            [],
        )
        .unwrap();

        let facets = folder_facets(&conn).unwrap();
        assert_eq!(
            facets.artists,
            vec![
                LibraryFolder {
                    name: "Aya".into(),
                    count: 2
                },
                LibraryFolder {
                    name: "Rob & Si".into(),
                    count: 1
                },
            ]
        );
    }

    /// Régression : les compteurs, le filtre et la colonne Format lisaient `tracks.format`,
    /// colonne qu'aucun code de production n'écrit. Sur une vraie bibliothèque, le tableau de bord
    /// affichait « Lossless 0 / MP3 0 » à côté d'un total juste, le filtre Lossless/MP3 ne rendait
    /// jamais rien, et la colonne Format montrait « ? » sur chaque ligne.
    ///
    /// Semé par `target_format`, EXACTEMENT comme `filing.rs:730` le fait en production. Les tests
    /// précédents semaient `format` à la main : ils passaient sans jamais toucher le chemin réel,
    /// ce qui est précisément pourquoi le trou a survécu.
    #[test]
    fn stats_filter_and_format_column_read_what_filing_actually_writes() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(id, path, status, target_format, verdict, verdict_ver) \
             VALUES(1, '/lib/a.aiff', 'filed', 'aiff_16_44', 'ok', ?1)",
            rusqlite::params![VER],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks(id, path, status, target_format, verdict, verdict_ver) \
             VALUES(2, '/lib/b.mp3', 'filed', 'mp3_320', 'ok', ?1)",
            rusqlite::params![VER],
        )
        .unwrap();

        let s = library_stats(&conn).unwrap();
        assert_eq!(s.lossless, 1);
        assert_eq!(s.mp3, 1);

        let lossless = LibraryFilter {
            quality: Some("lossless".into()),
            ..Default::default()
        };
        let rows = list_filed(&conn, &lossless).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].format.as_deref(),
            Some("aiff"),
            "l'écran doit montrer une extension, pas 'aiff_16_44'"
        );

        let mp3 = LibraryFilter {
            quality: Some("mp3".into()),
            ..Default::default()
        };
        assert_eq!(list_filed(&conn, &mp3).unwrap().len(), 1);
    }

    /// Les trois lectures du verdict en Bibliothèque doivent tomber d'accord quand sa version est
    /// distancée : la colonne se vide, le compte « À re-sourcer » l'exclut, et le filtre ne le
    /// trouve plus (issue #39).
    ///
    /// Les trois dans le même test parce que leur désaccord serait invisible autrement : un compte
    /// qui annonce « 1 à re-sourcer » au-dessus d'une table dont la colonne Verdict est vide, ou un
    /// filtre qui rend une ligne que l'écran affiche sans badge. C'est ce que coûterait d'oublier
    /// `verdict_ver` sur une seule des trois requêtes.
    ///
    /// Mutation portée par la ligne, pas par la `const` : `VER + 1` place la ligne dans l'état
    /// qu'elle aurait le lendemain d'un bump, donc le test reste vrai après.
    #[test]
    fn un_verdict_perime_disparait_du_compte_du_filtre_et_de_la_colonne() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(id, path, status, target_format, verdict, verdict_ver) \
             VALUES(1, '/lib/a.mp3', 'filed', 'mp3_320', 'fake', ?1)",
            rusqlite::params![VER],
        )
        .unwrap();
        let seul_fake = LibraryFilter {
            verdict: Some("fake".into()),
            ..Default::default()
        };

        assert_eq!(library_stats(&conn).unwrap().fake, 1);
        assert_eq!(list_filed(&conn, &seul_fake).unwrap().len(), 1);
        assert_eq!(
            list_filed(&conn, &LibraryFilter::default()).unwrap()[0]
                .verdict
                .as_deref(),
            Some("fake")
        );

        conn.execute(
            "UPDATE tracks SET verdict_ver=?1 WHERE id=1",
            rusqlite::params![VER + 1],
        )
        .unwrap();

        assert_eq!(
            library_stats(&conn).unwrap().fake,
            0,
            "la carte « A re-sourcer » proposait une action sur un verdict d'un autre moteur"
        );
        assert_eq!(
            list_filed(&conn, &seul_fake).unwrap().len(),
            0,
            "le filtre trouvait une ligne que la colonne Verdict affiche vide"
        );
        assert_eq!(
            list_filed(&conn, &LibraryFilter::default()).unwrap()[0].verdict,
            None,
            "un badge FAKE périmé restait affiché"
        );
    }

    #[test]
    fn library_stats_aggregates_counts() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(id, path, status, target_format, verdict, verdict_ver) \
             VALUES(1, '/lib/a.wav', 'filed', 'wav_16_44', 'ok', ?1)",
            rusqlite::params![VER],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks(id, path, status, target_format, verdict, verdict_ver) \
             VALUES(2, '/lib/b.mp3', 'filed', 'mp3_320', 'ok', ?1)",
            rusqlite::params![VER],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks(id, path, status, target_format, verdict, verdict_ver) \
             VALUES(3, '/lib/c.mp3', 'filed', 'mp3_320', 'fake', ?1)",
            rusqlite::params![VER],
        )
        .unwrap();
        // Une piste en attente n'a pas encore de `target_format` — c'est l'état réel, et elle ne
        // doit compter dans aucun des trois chiffres.
        conn.execute(
            "INSERT INTO tracks(id, path, status) VALUES(9, '/in/p.mp3', 'pending')",
            [],
        )
        .unwrap();
        crate::genres::set_genres(&conn, 1, &["House".into()]).unwrap();
        crate::genres::set_genres(&conn, 2, &["House".into()]).unwrap();
        crate::genres::set_genres(&conn, 3, &["Techno".into()]).unwrap();

        let stats = library_stats(&conn).unwrap();

        assert_eq!(stats.total, 3, "only filed tracks count");
        assert_eq!(stats.lossless, 1);
        assert_eq!(stats.mp3, 2);
        assert_eq!(stats.fake, 1);
        // Tautologique DEPUIS SYS-1 (2026-07-28), et dit comme tel plutôt que laissé en place à
        // faire croire qu'il vérifie quelque chose : `library_stats` ne calcule plus le comptage
        // et renvoie `duplicates: 0` en dur, c'est `ipc_library::library_stats` qui le remplit
        // hors verrou. Conservé comme garde de CONTRAT — si un jour cette fonction se remet à
        // calculer le comptage elle-même, l'assertion casse et signale le retour en arrière.
        assert_eq!(
            stats.duplicates, 0,
            "library_stats ne doit PAS calculer le comptage de doublons: il décode du disque et \
             son appelant le fait hors du verrou global (SYS-1)"
        );
        let house = stats.genres.iter().find(|g| g.genre == "House").unwrap();
        assert_eq!(house.count, 2);
    }
}
