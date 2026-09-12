//! IPC surface for M6a identification. `identify` queries Discogs (token from settings) and
//! returns ranked candidates; `apply_identity_cmd` downloads the cover (best-effort) and
//! persists the chosen candidate. Errors are flattened to stable sentinel codes the front maps
//! to messages: NO_TOKEN, RATE_LIMITED:<s>, NETWORK, PARSE.

use crate::db;
use crate::metadata::{self, AppliedIdentity, Candidate, MetadataProvider, Query};
use crate::settings;
use rusqlite::Connection;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

/// Query Discogs for `track_id`'s best-guess artist/title; ranked candidates, best first.
#[tauri::command]
pub fn identify(
    conn: State<'_, Mutex<Connection>>,
    track_id: i64,
) -> Result<Vec<Candidate>, String> {
    // Chemin sous le verrou, lecture des tags APRÈS l'avoir relâché : une lecture disque ne doit
    // pas geler les autres utilisateurs de la base (même découpage que `ipc_filing::reconcile`).
    let (token, path) = {
        let conn = db::lock_conn(&conn)?;
        let token = settings::get(&conn, settings::DISCOGS_TOKEN)
            .map_err(|e| e.to_string())?
            .unwrap_or_default();
        let path = crate::filing::track_path(&conn, track_id).map_err(|e| e.to_string())?;
        (token, path)
    };
    if token.trim().is_empty() {
        return Err("NO_TOKEN".into());
    }
    let query = build_query(&path);
    let provider = metadata::discogs::Discogs { token };
    provider.search(&query).map_err(|e| e.code())
}

/// Demande à Discogs si le jeton enregistré est accepté. Rend les MÊMES codes que `identify`.
///
/// Impasse A11 de l'issue #15 : « Jeton enregistré. » dit l'écriture et rien d'autre, et un jeton
/// faux ne se découvrait qu'au premier Identifier — plus tard, dans un autre écran, sur un morceau
/// qu'on voulait traiter. Ce bouton déplace la découverte au moment où on colle le jeton.
///
/// Les codes sont ceux de `ProviderError::code()`, déjà traduits par `identifyErrorHtml` côté
/// front : pas de second vocabulaire d'erreur pour la même API.
#[tauri::command]
pub fn verify_discogs_token(conn: State<'_, Mutex<Connection>>) -> Result<(), String> {
    let token = {
        let conn = db::lock_conn(&conn)?;
        settings::get(&conn, settings::DISCOGS_TOKEN)
            .map_err(|e| e.to_string())?
            .unwrap_or_default()
    };
    // Même garde que `identify`, et pour la même raison : sans jeton il n'y a pas de recherche
    // anonyme à tenter, donc rien à demander au réseau.
    if token.trim().is_empty() {
        return Err("NO_TOKEN".into());
    }
    metadata::discogs::Discogs { token }
        .verify_token()
        .map_err(|e| e.code())
}

/// Assemble la requête depuis les DEUX sources, chacune dans son domaine de compétence.
///
/// Les tags embarqués priment quand ils sont propres : ils peuvent avoir été corrigés par
/// l'utilisateur, et aucune analyse du nom de fichier ne peut battre une donnée saisie. Sinon —
/// et c'est le cas de 79 % des fichiers dont le nom est sale, mesuré le 2026-07-28 — on s'appuie
/// sur `search_terms`, qui lit le nom ET le dossier parent.
///
/// La version vient TOUJOURS de `search_terms` en priorité : elle est extraite du nom de fichier,
/// où elle figure presque toujours, alors que les tags la portent rarement séparément.
///
/// Note délibérée : cette fonction n'appelle PAS `filing::reconcile_track`. `Canonical` est
/// l'identité qu'on écrit sur le disque et son portail de rejet la rend volontairement timide ;
/// s'en servir comme requête est précisément le défaut que ce chantier corrige. Les deux chemins
/// restent séparés — `ipc_filing::reconcile` continue d'alimenter les champs éditables.
fn build_query(path: &str) -> Query {
    let p = std::path::Path::new(path);
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    let folder = p
        .parent()
        .and_then(|d| d.file_name())
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();

    let terms = crate::search_terms::build(&stem, &folder);
    let (tag_artist, tag_title) = crate::tagging::read_artist_title(path);
    let tags_clean = crate::naming::is_clean(&tag_artist, &tag_title);

    let (artist, title) = if tags_clean {
        (tag_artist.trim().to_string(), tag_title.trim().to_string())
    } else {
        (terms.artist.clone(), terms.title.clone())
    };

    let mut attempts: Vec<String> = Vec::new();
    if tags_clean {
        attempts.push(format!("{artist} {title}"));
    }
    attempts.extend(terms.ladder.iter().map(|a| a.q.clone()));

    Query {
        artist,
        title,
        version: terms.version,
        attempts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une fixture audio réelle, ou `None`. `src-tauri/fixtures/*` est gitignoré (CLAUDE.md) : les
    /// tests qui en dépendent se sautent au lieu d'échouer sur un checkout frais.
    fn fixture(name: &str) -> Option<std::path::PathBuf> {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join(name);
        p.exists().then_some(p)
    }

    /// Le cas majoritaire de la bibliothèque mesurée : nom sale, AUCUN tag (79,3 % des fichiers
    /// dont le nom échoue à `parse_filename`, mesuré le 2026-07-28 sur 300 tirés au hasard).
    /// C'est exactement la population que l'ancienne implémentation envoyait à Discogs avec un
    /// artiste vide et sans repli possible.
    #[test]
    fn dirty_name_without_tags_falls_back_to_search_terms() {
        let q =
            build_query("/dl/complete/01_infunktuation_-_feel_real_good_(club_version)-idc.mp3");
        assert_eq!(q.artist, "infunktuation");
        assert_eq!(q.title, "feel real good");
        assert_eq!(q.version.as_deref(), Some("club version"));
        assert!(
            !q.attempts.is_empty(),
            "un titre non vide doit toujours produire au moins un essai"
        );
    }

    /// Le dossier parent est la seule source d'artiste pour 243 pistes de la bibliothèque mesurée.
    /// Sans ce chemin, `A1-Stepback` partait en requête titre-seul.
    #[test]
    fn parent_folder_supplies_the_artist_when_the_name_has_none() {
        let q = build_query("/rips/(SOMA 21) Slam-Snapshots/A1-Stepback.aiff");
        assert_eq!(q.artist, "Slam");
        assert_eq!(q.title, "Stepback");
    }

    /// Garde-fou inverse, et le plus important des deux : un dossier fourre-tout ne doit JAMAIS
    /// injecter son nom comme artiste. `2_040924` porte 524 pistes — s'y tromper, c'est envoyer
    /// 524 requêtes fausses d'un coup.
    #[test]
    fn a_meaningless_folder_never_becomes_the_artist() {
        let q = build_query("/dl/2_040924/[BU 002] DJ Gregory - Freeze.mp3");
        assert_eq!(q.artist, "DJ Gregory");
        assert_eq!(q.title, "Freeze");
        let q2 = build_query("/dl/complete/01 Awaken Abyss.mp3");
        assert_eq!(
            q2.artist, "",
            "aucun artiste dérivable: le vide est correct"
        );
        assert_eq!(q2.title, "Awaken Abyss");
    }

    /// La cascade doit rester non vide même sans artiste — c'est tout l'objet du chantier : la
    /// garde retirée de `discogs.rs` excluait précisément ce cas de tout repli.
    #[test]
    fn ladder_is_never_empty_when_a_title_exists() {
        let q = build_query("/dl/complete/01 Give U Love (Deep Mix).mp3");
        assert_eq!(q.artist, "");
        assert!(
            q.attempts.len() >= 2,
            "sans artiste, il faut au moins titre+version puis titre: {:?}",
            q.attempts
        );
    }

    /// Entrées hostiles : `build_query` reçoit des chemins venant du disque de l'utilisateur, elle
    /// ne doit jamais paniquer ni produire une requête vide mais présente.
    #[test]
    fn hostile_paths_never_panic_and_never_emit_a_blank_attempt() {
        for p in [
            "",
            "/",
            "/a",
            "/dl//.mp3",
            "/dl/02 [2015]/001_Untitled.mp3",
            "/dl/The Tracking System/A8.wav",
        ] {
            let q = build_query(p);
            assert!(
                q.attempts.iter().all(|a| !a.trim().is_empty()),
                "essai vide produit pour {p:?}: {:?}",
                q.attempts
            );
        }
    }

    /// Des tags PROPRES priment sur le nom de fichier : ils ont pu être corrigés à la main, et
    /// aucune analyse de nom ne bat une donnée saisie. La version, elle, continue de venir du nom
    /// — les tags la portent rarement dans un champ séparé.
    #[test]
    fn clean_tags_win_over_the_filename_but_the_version_still_comes_from_it() {
        let Some(src) = fixture("real_320.mp3") else {
            eprintln!("skip: fixture real_320.mp3 absente (gitignoree, cf. CLAUDE.md)");
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("complete");
        std::fs::create_dir_all(&sub).unwrap();
        // Nom de fichier volontairement DIVERGENT des tags, et porteur d'une version.
        let dst = sub.join("01_wrong_artist_-_wrong_title_(Club Mix).mp3");
        std::fs::copy(&src, &dst).unwrap();
        let dst_s = dst.to_str().unwrap();
        crate::tagging::write_tags_full(
            dst_s,
            "Larry Heard",
            "Mystery of Love",
            None,
            None,
            &[],
            None,
        )
        .expect("write tags");

        let q = build_query(dst_s);
        assert_eq!(q.artist, "Larry Heard", "les tags propres priment");
        assert_eq!(q.title, "Mystery of Love");
        assert_eq!(
            q.version.as_deref(),
            Some("Club Mix"),
            "la version vient du nom de fichier même quand les tags gagnent"
        );
        assert_eq!(
            q.attempts.first().map(|s| s.as_str()),
            Some("Larry Heard Mystery of Love"),
            "le premier essai est celui des tags"
        );
    }
}

/// Persist a chosen candidate for `track_id`: download its cover (best-effort) then write the
/// metadata + genres. Emits `queue:changed` so the front refreshes.
#[tauri::command]
pub fn apply_identity_cmd(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    track_id: i64,
    candidate: Candidate,
) -> Result<AppliedIdentity, String> {
    // Gate to a known track before doing any work (network download / DB writes) — mirrors the
    // implicit gate `identify` gets from reconcile_track, so a bogus id can't drive a fetch.
    {
        let conn = db::lock_conn(&conn)?;
        let known = conn
            .query_row(
                "SELECT 1 FROM tracks WHERE id=?1",
                rusqlite::params![track_id],
                |_| Ok(()),
            )
            .is_ok();
        if !known {
            return Err("unknown track id".into());
        }
    }
    let cover_path = candidate.cover_url.as_ref().and_then(|url| {
        let dir = app.path().app_cache_dir().ok()?.join("covers");
        metadata::cover::download_cover(&dir, &candidate.release_id, url)
            .ok()
            .map(|p| p.to_string_lossy().to_string())
    });
    let applied = {
        let conn = db::lock_conn(&conn)?;
        metadata::apply_identity(&conn, track_id, &candidate, cover_path)
            .map_err(|e| e.to_string())?
    };
    app.emit("queue:changed", ()).ok();
    Ok(applied)
}
