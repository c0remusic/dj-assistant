//! Metadata source abstraction. A `MetadataProvider` turns a `Query` into ranked `Candidate`s;
//! `apply_identity` persists a chosen candidate into the DB. Discogs is the first provider
//! (see discogs.rs); the trait keeps a future AcoustID/MusicBrainz provider a drop-in.

pub mod cover;
pub mod discogs;

use crate::naming::{Canonical, Confidence};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// User-edited metadata fields for a filed track (Bibliothèque inline edit).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataEdit {
    pub artist: String,
    pub title: String,
    pub label: Option<String>,
    pub year: Option<i64>,
    pub genres: Vec<String>,
    pub cover_path: Option<String>,
}

/// Whether an upsert overwrites `cover_path` unconditionally (a fresh Discogs cover download,
/// which may legitimately be `None`) or only when a new value is actually provided (a manual
/// metadata edit, where "no new cover chosen" must not erase the existing one).
enum CoverWrite {
    Always,
    OnlyIfSome,
}

/// Whether an upsert overwrites `discogs_release_id`/`source` (a fresh Discogs match) or leaves
/// them untouched (a manual edit must not wipe an existing release link).
enum ReleaseLink<'a> {
    Set {
        release_id: &'a str,
        source: &'a str,
    },
    Preserve,
}

/// The single-value columns every upsert writes, grouped so `upsert_metadata_row` takes a
/// reasonable number of arguments instead of one per column.
struct MetadataFields<'a> {
    artist: &'a str,
    title: &'a str,
    version: Option<&'a str>,
    label: Option<&'a str>,
    year: Option<i64>,
    cover_path: Option<&'a str>,
}

/// Shared upsert into `metadata` for `artist`/`title`/`version`/`label`/`year`/`cover_path`, with
/// the two axes (`cover`, `release`) that previously made `update_metadata_db` and
/// `apply_identity` diverge into two near-identical hand-written SQL statements.
fn upsert_metadata_row(
    conn: &Connection,
    track_id: i64,
    f: &MetadataFields,
    cover: CoverWrite,
    release: ReleaseLink,
) -> rusqlite::Result<()> {
    let cover_set_clause = match cover {
        CoverWrite::Always => "cover_path=excluded.cover_path",
        CoverWrite::OnlyIfSome => {
            "cover_path=CASE WHEN excluded.cover_path IS NOT NULL THEN excluded.cover_path ELSE cover_path END"
        }
    };
    match release {
        ReleaseLink::Set { release_id, source } => conn.execute(
            &format!(
                "INSERT INTO metadata(track_id, artist, title, version, label, year, cover_path, discogs_release_id, source)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)
                 ON CONFLICT(track_id) DO UPDATE SET
                    artist=excluded.artist, title=excluded.title, version=excluded.version,
                    label=excluded.label, year=excluded.year, {cover_set_clause},
                    discogs_release_id=excluded.discogs_release_id, source=excluded.source"
            ),
            params![track_id, f.artist, f.title, f.version, f.label, f.year, f.cover_path, release_id, source],
        ),
        ReleaseLink::Preserve => conn.execute(
            &format!(
                "INSERT INTO metadata(track_id, artist, title, version, label, year, cover_path)
                 VALUES(?1,?2,?3,?4,?5,?6,?7)
                 ON CONFLICT(track_id) DO UPDATE SET
                    artist=excluded.artist, title=excluded.title, version=excluded.version,
                    label=excluded.label, year=excluded.year, {cover_set_clause}"
            ),
            params![track_id, f.artist, f.title, f.version, f.label, f.year, f.cover_path],
        ),
    }?;
    Ok(())
}

/// DB-only part (unit-tested): upsert the editable metadata fields + replace genres.
/// Preserves `discogs_release_id` and `source` — a manual edit must not wipe the release link.
/// On INSERT (no prior metadata row) those columns stay NULL.
///
/// `MetadataEdit.title` is the COMPLETE title, version included — that is what the Bibliothèque
/// form shows and what the user edits (`library::list_filed` composes it). It is split back into
/// `title` + `version` here, by the SAME `split_title_version` a Discogs match goes through, so
/// the stored shape is identical whichever way a track was identified. Writing it whole would
/// make `render_filename` emit the version twice.
pub fn update_metadata_db(
    conn: &Connection,
    track_id: i64,
    e: &MetadataEdit,
) -> rusqlite::Result<()> {
    let (base_title, version) = split_title_version(&e.title);
    upsert_metadata_row(
        conn,
        track_id,
        &MetadataFields {
            artist: &e.artist,
            title: &base_title,
            version: version.as_deref(),
            label: e.label.as_deref(),
            year: e.year,
            cover_path: e.cover_path.as_deref(),
        },
        CoverWrite::OnlyIfSome,
        ReleaseLink::Preserve,
    )?;
    crate::genres::set_genres(conn, track_id, &e.genres)?;
    if e.cover_path.is_some() {
        conn.execute(
            "UPDATE tracks SET has_cover=1 WHERE id=?1",
            params![track_id],
        )?;
    }
    Ok(())
}

/// Persist ONLY the `label` column for a track, upserting a metadata row if none exists yet.
/// Used by `apply_tags` when the user edits the Label field in the Revue pane: the edit is written
/// to the file's ID3 tag (via `write_tags_full`) AND here, so a close+reopen reads back the edited
/// label from the DB (`track_release`) instead of the stale Discogs one — otherwise the reopened
/// pane would show the old label while the file holds the new one, tripping the discrepancy banner.
/// Deliberately label-only: it must NOT touch artist/title/version (those come from the file via
/// `reconcile` on reopen) nor `discogs_release_id`/`source` (an edit never wipes a release link).
/// `artist`/`title` stay NULL on a first INSERT (both are nullable) — a label-only row keeps
/// `identified` false, exactly like no row at all.
pub fn set_metadata_label(conn: &Connection, track_id: i64, label: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO metadata(track_id, label) VALUES(?1, ?2)
         ON CONFLICT(track_id) DO UPDATE SET label=excluded.label",
        params![track_id, label],
    )?;
    Ok(())
}

/// What we search for: the track's current best-guess artist/title, plus the version
/// (remix/dub) used to pick the matching release among a release's tracklist.
///
/// `artist`/`title`/`version` drive the SCORING (matching a release's tracklist); `attempts`
/// drives the QUERYING. They are deliberately distinct: the scoring wants the cleanest fields we
/// have, while the querying wants several progressively degraded shots at the same track.
pub struct Query {
    pub artist: String,
    pub title: String,
    pub version: Option<String>,
    /// Requêtes à tenter, de la plus spécifique à la plus dégradée (voir `search_terms`). Vide =
    /// l'appelant n'en fournit pas et le fournisseur retombe sur `"{artist} {title}"` — c'est le
    /// cas des tests et de tout appelant historique.
    pub attempts: Vec<String>,
}

/// A normalized identification result, ranked best-first by the provider.
// Serialize → sent to the UI; Deserialize → the UI returns the chosen one to apply_identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub artist: String,
    pub title: String,
    pub label: Option<String>,
    pub year: Option<i64>,
    pub styles: Vec<String>, // Discogs "style" (sub-genres), ordered
    pub country: Option<String>,
    pub format: Option<String>,
    pub cover_url: Option<String>,
    pub release_id: String,
    pub source: String, // "discogs"
}

/// What the UI needs after applying a candidate: the (name-driving) canonical plus the extra
/// tag fields, so it can refresh the preview, cover, and genre chips.
#[derive(Debug, Clone, Serialize)]
pub struct AppliedIdentity {
    pub canonical: Canonical,
    pub label: Option<String>,
    pub year: Option<i64>,
    pub styles: Vec<String>,
    pub cover_path: Option<String>,
}

/// Split a trailing "(Version)" group off a Discogs title:
/// `"Love Foolosophy (Knee Deep Remix)"` → `("Love Foolosophy", Some("Knee Deep Remix"))`.
/// Returns `(trimmed title, None)` when there is no clean trailing parenthetical (no group, nested
/// parens, or an empty base). Mirrors the front's display-time split so a stored title and a
/// freshly-fetched one render the same base + version — this is what makes the chosen remix survive
/// a close+reopen (the file tags still hold the old name until filing).
fn split_title_version(title: &str) -> (String, Option<String>) {
    let t = title.trim();
    if t.ends_with(')') {
        if let Some(open) = t.rfind('(') {
            let inner = &t[open + 1..t.len() - 1];
            let base = t[..open].trim();
            if !inner.is_empty() && !inner.contains('(') && !inner.contains(')') && !base.is_empty()
            {
                return (base.to_string(), Some(inner.trim().to_string()));
            }
        }
    }
    (t.to_string(), None)
}

/// Persist a chosen candidate for `track_id`: upsert the single-value fields into `metadata`,
/// replace the track's sub-genres, and (when provided) record the downloaded cover path. The
/// cover download itself happens in the command layer (network); this fn is pure DB so it is
/// unit-tested. Returns the payload the UI refreshes from.
pub fn apply_identity(
    conn: &Connection,
    track_id: i64,
    c: &Candidate,
    cover_path: Option<String>,
) -> rusqlite::Result<AppliedIdentity> {
    // Store the clean base title + the extracted remix/dub in the `version` column, so a reopen
    // reads back the chosen identity (the file tags still hold the old name until filing). The
    // returned canonical keeps the FULL title — the front splits it for its live display.
    let (base_title, version) = split_title_version(&c.title);
    upsert_metadata_row(
        conn,
        track_id,
        &MetadataFields {
            artist: &c.artist,
            title: &base_title,
            version: version.as_deref(),
            label: c.label.as_deref(),
            year: c.year,
            cover_path: cover_path.as_deref(),
        },
        CoverWrite::Always,
        ReleaseLink::Set {
            release_id: &c.release_id,
            source: &c.source,
        },
    )?;
    crate::genres::set_genres(conn, track_id, &c.styles)?;
    if cover_path.is_some() {
        conn.execute(
            "UPDATE tracks SET has_cover=1 WHERE id=?1",
            params![track_id],
        )?;
    }
    Ok(AppliedIdentity {
        canonical: Canonical {
            artist: c.artist.clone(),
            title: c.title.clone(),
            version: None,
            label: c.label.clone(),
            confidence: Confidence::Green, // a Discogs match is a high-confidence rename
        },
        label: c.label.clone(),
        year: c.year,
        styles: c.styles.clone(),
        cover_path,
    })
}

/// Why a provider call failed — mapped to stable IPC error codes by the command layer.
#[derive(Debug)]
pub enum ProviderError {
    NoToken,
    /// Le jeton EXISTE mais Discogs le refuse — invalide, expiré, révoqué, ou mal collé.
    ///
    /// Impasse A10 ([issue #15](https://github.com/c0remusic/sift/issues/15)) : 401 et 403
    /// tombaient dans `Network` avec tous les autres statuts, et l'écran disait « Discogs
    /// injoignable ». L'utilisateur partait vérifier sa connexion, qui allait très bien, pendant
    /// que la cause était à un clic de là, dans Réglages. C'est un cas d'authentification, pas de
    /// réseau : il ne se résout pas en réessayant.
    BadToken(String),
    RateLimited {
        retry_after_s: u64,
    },
    Network(String),
    Parse(String),
}

impl ProviderError {
    /// Stable code string the UI maps to a message (NO_TOKEN, RATE_LIMITED:<s>, NETWORK:…, PARSE:…).
    pub fn code(&self) -> String {
        match self {
            ProviderError::NoToken => "NO_TOKEN".into(),
            ProviderError::BadToken(m) => format!("BAD_TOKEN:{m}"),
            ProviderError::RateLimited { retry_after_s } => format!("RATE_LIMITED:{retry_after_s}"),
            ProviderError::Network(m) => format!("NETWORK:{m}"),
            ProviderError::Parse(m) => format!("PARSE:{m}"),
        }
    }
}

pub trait MetadataProvider {
    /// Ranked candidates (best first). Empty vec = no results (not an error).
    fn search(&self, q: &Query) -> Result<Vec<Candidate>, ProviderError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::run_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO tracks(id, path, status) VALUES(1,'/x.flac','pending')",
            [],
        )
        .unwrap();
        conn
    }

    fn sample() -> Candidate {
        Candidate {
            artist: "Larry Heard".into(),
            title: "Mystery of Love".into(),
            label: Some("Alleviated".into()),
            year: Some(1986),
            styles: vec!["Deep House".into(), "House".into()],
            country: Some("US".into()),
            format: None,
            cover_url: Some("https://img/x.jpg".into()),
            release_id: "12345".into(),
            source: "discogs".into(),
        }
    }

    #[test]
    fn apply_writes_metadata_and_genres() {
        let conn = db();
        let applied = apply_identity(&conn, 1, &sample(), Some("/cache/12345.jpg".into())).unwrap();

        type MetaRow = (
            String,
            Option<String>,
            Option<i64>,
            Option<String>,
            Option<String>,
            Option<String>,
        );
        let (artist, label, year, cover, rel, src): MetaRow =
            conn.query_row(
                "SELECT artist, label, year, cover_path, discogs_release_id, source FROM metadata WHERE track_id=1",
                [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            ).unwrap();
        assert_eq!(artist, "Larry Heard");
        assert_eq!(label.as_deref(), Some("Alleviated"));
        assert_eq!(year, Some(1986));
        assert_eq!(cover.as_deref(), Some("/cache/12345.jpg"));
        assert_eq!(rel.as_deref(), Some("12345"));
        assert_eq!(src.as_deref(), Some("discogs"));

        assert_eq!(
            crate::genres::get_genres(&conn, 1).unwrap(),
            vec!["Deep House".to_string(), "House".to_string()]
        );

        assert_eq!(applied.canonical.artist, "Larry Heard");
        assert_eq!(
            applied.styles,
            vec!["Deep House".to_string(), "House".to_string()]
        );
        assert_eq!(applied.cover_path.as_deref(), Some("/cache/12345.jpg"));
    }

    #[test]
    fn re_apply_replaces_genres() {
        let conn = db();
        apply_identity(&conn, 1, &sample(), None).unwrap();
        let mut other = sample();
        other.styles = vec!["Techno".into()];
        apply_identity(&conn, 1, &other, None).unwrap();
        assert_eq!(
            crate::genres::get_genres(&conn, 1).unwrap(),
            vec!["Techno".to_string()]
        );
    }

    #[test]
    fn set_metadata_label_inserts_then_updates_label_only() {
        let conn = db();

        // INSERT path (no prior metadata row): a label-only row is created — artist/title stay NULL
        // (so `identified` reads false, same as no row) and no release link is invented.
        set_metadata_label(&conn, 1, "Trax").unwrap();
        type Row = (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        );
        let (artist, title, label, rel): Row = conn
            .query_row(
                "SELECT artist, title, label, discogs_release_id FROM metadata WHERE track_id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(label.as_deref(), Some("Trax"));
        assert!(
            artist.is_none() && title.is_none(),
            "a label-only upsert must not fabricate an identity"
        );
        assert!(
            rel.is_none(),
            "a label edit must never invent a release link"
        );

        // UPDATE path over an identified row: only the label column changes — the identity and the
        // Discogs release link are preserved (mutating this to INSERT OR REPLACE would wipe them).
        apply_identity(&conn, 1, &sample(), None).unwrap();
        set_metadata_label(&conn, 1, "Alleviated Records").unwrap();
        let (artist, label, rel): (Option<String>, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT artist, label, discogs_release_id FROM metadata WHERE track_id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            label.as_deref(),
            Some("Alleviated Records"),
            "label updated in place"
        );
        assert_eq!(artist.as_deref(), Some("Larry Heard"), "identity untouched");
        assert_eq!(rel.as_deref(), Some("12345"), "release link preserved");
    }

    #[test]
    fn split_title_version_extracts_trailing_paren() {
        assert_eq!(
            split_title_version("Love Foolosophy (Knee Deep Remix)"),
            (
                "Love Foolosophy".to_string(),
                Some("Knee Deep Remix".to_string())
            ),
        );
        assert_eq!(
            split_title_version("Mystery of Love"),
            ("Mystery of Love".to_string(), None)
        );
        // Empty base (the whole title is the parenthetical) → no split.
        assert_eq!(
            split_title_version("(Instrumental)"),
            ("(Instrumental)".to_string(), None)
        );
    }

    #[test]
    fn apply_persists_base_title_and_version() {
        let conn = db();
        let mut c = sample();
        c.title = "Can You Feel It (Larry Heard Remix)".into();
        apply_identity(&conn, 1, &c, None).unwrap();
        let (title, version): (String, Option<String>) = conn
            .query_row(
                "SELECT title, version FROM metadata WHERE track_id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(title, "Can You Feel It");
        assert_eq!(version.as_deref(), Some("Larry Heard Remix"));
    }

    /// L'axe `VersionWrite::Always` d'`apply_identity` ne vaut que sur le chemin CONFLIT — le test
    /// ci-dessus passe par l'INSERT. Une correspondance Discogs sans version doit bien EFFACER la
    /// précédente : c'est une identité de remplacement, pas une édition partielle.
    #[test]
    fn apply_overwrites_an_existing_version_including_with_none() {
        let conn = db();
        let mut c = sample();
        c.title = "Can You Feel It (Larry Heard Remix)".into();
        apply_identity(&conn, 1, &c, None).unwrap();

        c.title = "Can You Feel It".into();
        apply_identity(&conn, 1, &c, None).unwrap();
        let version: Option<String> = conn
            .query_row("SELECT version FROM metadata WHERE track_id=1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(version, None);
    }

    /// Régression : `update_metadata_db` écrivait `version: None` en dur. Une édition Bibliothèque
    /// — même une simple correction d'année — effaçait donc le nom du remix de la base, pendant que
    /// le nom du fichier sur le disque le gardait. Le titre reçu du formulaire est COMPLET (voir
    /// `library::list_filed`) : il se redécoupe ici, exactement comme une identité Discogs.
    #[test]
    fn update_metadata_db_splits_the_complete_title_and_keeps_the_version() {
        let conn = db();
        conn.execute(
            "INSERT INTO metadata(track_id, artist, title, version, year)
             VALUES(1,'Chez Damier','Can You Feel It','Fluent Remix',1992)",
            [],
        )
        .unwrap();

        // Ce que le formulaire affiche, et ce que l'utilisateur renvoie sans y toucher : il n'a
        // corrigé que l'année.
        let edit = MetadataEdit {
            artist: "Chez Damier".into(),
            title: "Can You Feel It (Fluent Remix)".into(),
            label: None,
            year: Some(1993),
            genres: vec![],
            cover_path: None,
        };
        update_metadata_db(&conn, 1, &edit).unwrap();

        type Row = (String, Option<String>, Option<i64>);
        let (title, version, year): Row = conn
            .query_row(
                "SELECT title, version, year FROM metadata WHERE track_id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            title, "Can You Feel It",
            "pas de version dans le titre de base"
        );
        assert_eq!(version.as_deref(), Some("Fluent Remix"));
        assert_eq!(year, Some(1993), "l'edit doit quand même s'appliquer");
    }

    /// L'autre moitié : retirer la parenthèse du champ Titre EST la façon de supprimer la version
    /// dans un formulaire qui n'a pas de champ dédié. Elle doit donc bien partir.
    #[test]
    fn update_metadata_db_drops_the_version_the_user_removed_from_the_title() {
        let conn = db();
        conn.execute(
            "INSERT INTO metadata(track_id, artist, title, version)
             VALUES(1,'Chez Damier','Can You Feel It','Fluent Remix')",
            [],
        )
        .unwrap();

        let edit = MetadataEdit {
            artist: "Chez Damier".into(),
            title: "Can You Feel It".into(),
            label: None,
            year: None,
            genres: vec![],
            cover_path: None,
        };
        update_metadata_db(&conn, 1, &edit).unwrap();

        let version: Option<String> = conn
            .query_row("SELECT version FROM metadata WHERE track_id=1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(version, None);
    }

    #[test]
    fn candidate_serde_round_trips() {
        let c = Candidate {
            artist: "Larry Heard".into(),
            title: "Mystery of Love".into(),
            label: Some("Alleviated".into()),
            year: Some(1986),
            styles: vec!["Deep House".into(), "House".into()],
            country: Some("US".into()),
            format: Some("Vinyl, 12\"".into()),
            cover_url: Some("https://img/x.jpg".into()),
            release_id: "12345".into(),
            source: "discogs".into(),
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: Candidate = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn update_metadata_db_preserves_release_link_and_replaces_genres() {
        let conn = db();
        // Seed a metadata row that already has a Discogs release link.
        conn.execute(
            "INSERT INTO metadata(track_id, artist, title, discogs_release_id, source)
             VALUES(1,'Old Artist','Old Title','999','discogs')",
            [],
        )
        .unwrap();
        crate::genres::set_genres(&conn, 1, &["Old".into()]).unwrap();

        let edit = MetadataEdit {
            artist: "New Artist".into(),
            title: "New Title".into(),
            label: Some("Trax".into()),
            year: Some(1990),
            genres: vec!["House".into(), "Deep House".into()],
            cover_path: None,
        };
        update_metadata_db(&conn, 1, &edit).unwrap();

        type Row = (
            String,
            String,
            Option<String>,
            Option<i64>,
            Option<String>,
            Option<String>,
        );
        let (artist, title, label, year, rel_id, source): Row = conn
            .query_row(
                "SELECT artist, title, label, year, discogs_release_id, source FROM metadata WHERE track_id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            )
            .unwrap();

        assert_eq!(artist, "New Artist");
        assert_eq!(title, "New Title");
        assert_eq!(label.as_deref(), Some("Trax"));
        assert_eq!(year, Some(1990));
        // Release link must survive the manual edit.
        assert_eq!(
            rel_id.as_deref(),
            Some("999"),
            "discogs_release_id must be preserved"
        );
        assert_eq!(
            source.as_deref(),
            Some("discogs"),
            "source must be preserved"
        );

        let genres = crate::genres::get_genres(&conn, 1).unwrap();
        assert_eq!(genres, vec!["House".to_string(), "Deep House".to_string()]);
    }

    #[test]
    fn update_metadata_db_insert_path_leaves_release_null() {
        let conn = db();
        // No prior metadata row — INSERT path.
        let edit = MetadataEdit {
            artist: "Chez Damier".into(),
            title: "Can You Feel It".into(),
            label: Some("KMS".into()),
            year: Some(1992),
            genres: vec!["House".into()],
            cover_path: None,
        };
        update_metadata_db(&conn, 1, &edit).unwrap();

        type Row = (
            String,
            String,
            Option<String>,
            Option<i64>,
            Option<String>,
            Option<String>,
        );
        let (artist, title, label, year, rel_id, source): Row = conn
            .query_row(
                "SELECT artist, title, label, year, discogs_release_id, source FROM metadata WHERE track_id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            )
            .unwrap();

        assert_eq!(artist, "Chez Damier");
        assert_eq!(title, "Can You Feel It");
        assert_eq!(label.as_deref(), Some("KMS"));
        assert_eq!(year, Some(1992));
        // No prior Discogs data → these stay NULL on INSERT.
        assert!(
            rel_id.is_none(),
            "discogs_release_id should be NULL on first insert"
        );
        assert!(source.is_none(), "source should be NULL on first insert");

        let genres = crate::genres::get_genres(&conn, 1).unwrap();
        assert_eq!(genres, vec!["House".to_string()]);
    }
}
