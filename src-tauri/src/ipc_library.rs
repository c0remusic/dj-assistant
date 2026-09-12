//! IPC surface for the M6b library browser: read-only listing + facets of filed tracks,
//! plus the `update_metadata` command for inline editing in the Bibliothèque.
use crate::actions;
use crate::db;
use crate::filing;
use crate::library::{self, LibraryFacets, LibraryFilter, LibraryTrack};
use crate::metadata::{self, MetadataEdit};
use rusqlite::Connection;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

/// Filed tracks joined to metadata + genres, filtered (folder / quality / genre / q).
#[tauri::command]
pub fn list_library(
    conn: State<'_, Mutex<Connection>>,
    filter: Option<LibraryFilter>,
) -> Result<Vec<LibraryTrack>, String> {
    let conn = db::lock_conn(&conn)?;
    library::list_filed(&conn, &filter.unwrap_or_default()).map_err(|e| e.to_string())
}

/// Folder + genre facet counts for the sidebar.
#[tauri::command]
pub fn library_folders(conn: State<'_, Mutex<Connection>>) -> Result<LibraryFacets, String> {
    let conn = db::lock_conn(&conn)?;
    library::folder_facets(&conn).map_err(|e| e.to_string())
}

/// Phase 1 of `update_metadata` (under the DB lock): resolve the track's path. Fast row read.
fn update_metadata_path(conn: &Connection, track_id: i64) -> Result<String, String> {
    conn.query_row(
        "SELECT path FROM tracks WHERE id=?1",
        rusqlite::params![track_id],
        |r| r.get(0),
    )
    .map_err(|_| format!("track {track_id} not found"))
}

/// Phase 2 of `update_metadata` (NO DB lock, NO DB access): snapshot the OLD tags, then rewrite
/// the audio file's tags in place. Rewriting a tag block on a lossless file rewrites the file —
/// unbounded I/O that must never run under the global connection mutex. Returns the snapshot so
/// phase 3 can journal a revertable `tag_edit`. Same shape as `apply_tags` (`ipc_filing.rs`).
fn update_metadata_write_file(
    path: &str,
    edit: &MetadataEdit,
) -> Result<crate::tagging::TagsSnapshot, String> {
    // Snapshot the OLD tags BEFORE writing — same pattern as apply_tags.
    let snapshot = crate::tagging::read_tags_full(path)?;
    // Write the file tags. If it fails we stop here — nothing journaled, DB untouched.
    crate::tagging::write_tags_full(
        path,
        &edit.artist,
        &edit.title,
        edit.label.as_deref(),
        edit.year,
        &edit.genres,
        edit.cover_path.as_deref(),
    )?;
    Ok(snapshot)
}

/// Phase 3 of `update_metadata` (under the DB lock): persist the edit, journal the revertable
/// `tag_edit`, run the read-only M8 Tier 3 detectors. Every value written here comes either from
/// `edit` (the user's input, unaffected by anything another thread may have done) or from
/// `snapshot` (the tags actually read off the file in phase 2) — nothing is a read-modify-write
/// of DB state observed in phase 1, so re-taking the lock cannot commit a stale decision.
fn update_metadata_commit(
    conn: &Connection,
    track_id: i64,
    edit: &MetadataEdit,
    path: &str,
    snapshot: &crate::tagging::TagsSnapshot,
) -> Result<String, String> {
    metadata::update_metadata_db(conn, track_id, edit).map_err(|e| e.to_string())?;

    // (5) Journal a revertable tag_edit — this is the fix for a pre-existing gap: before this,
    // Bibliothèque edits had no undo path at all (see M8 Tier 3 design, "Fix du gap").
    let meta = serde_json::to_string(snapshot).map_err(|e| e.to_string())?;
    let batch_id = filing::new_batch_id(track_id);
    let action_id = actions::record_with_meta(
        conn,
        &batch_id,
        Some(track_id),
        "tag_edit",
        Some(path),
        None,
        Some(&meta),
    )
    .map_err(|e| e.to_string())?;

    // (6) M8 Tier 3: detect (read-only) whether this track is linked to Rekordbox and needs a
    // metadata sync candidate. Never fails the edit itself.
    let (genre, label) = actions::sanitize_genre_label(&edit.genres, edit.label.as_deref());
    let values = actions::MetadataSyncValues {
        artist: Some(edit.artist.clone()),
        title: Some(edit.title.clone()),
        label,
        year: edit.year,
        genre,
    };
    // ONE decrypt for BOTH detectors. The `_if_linked` variants each resolve the index
    // themselves, so calling both meant decrypting a multi-MB SQLCipher `master.db` twice per
    // edit — exactly what `resolve_masterdb_index_if_linked`'s docs forbid (actions.rs:206).
    // Same shape as `ipc_filing::apply_tags` and `filing::commit_file`.
    if let Some(index) = actions::resolve_masterdb_index_if_linked(conn) {
        actions::detect_masterdb_metadata_sync_with_index(
            conn, &index, path, track_id, &values, action_id,
        );

        // (7) M8 Tier 3 (pochette): only when THIS edit actually changed the cover — unlike the
        // metadata detector above, which always fires.
        if let Some(cover_path) = &edit.cover_path {
            actions::detect_masterdb_artwork_sync_with_index(
                conn, &index, path, track_id, cover_path, action_id,
            );
        }
    }

    Ok(batch_id)
}

/// Test harness for `update_metadata`: composes the three phases IN THE SAME ORDER the command
/// runs them, against a single connection (the tests own a plain `Connection`, not a Tauri
/// `State<Mutex<Connection>>`). Returns the `tag_edit` batch_id — same contract as `apply_tags`
/// (`ipc_filing.rs`). `cfg(test)` because production must NOT have a path that holds one
/// connection across phase 2: that is precisely the lock-over-I/O this tranche removes.
#[cfg(test)]
fn update_metadata_inner(
    conn: &Connection,
    track_id: i64,
    edit: MetadataEdit,
) -> Result<String, String> {
    let path = update_metadata_path(conn, track_id)?;
    let snapshot = update_metadata_write_file(&path, &edit)?;
    update_metadata_commit(conn, track_id, &edit, &path, &snapshot)
}

/// Edit a filed track's metadata: writes the file tags first, then updates the DB, then
/// journals the edit as a revertable `tag_edit` (returns its `batch_id` for a targeted undo —
/// see `frontend/library-detail.ts`'s "Annuler" toast).
///
/// Phase 2 (the full tag rewrite of the audio file) runs with the DB lock RELEASED — same split
/// as `apply_tags` (`ipc_filing.rs:218-280`), which is the pattern this mirrors. If the write
/// fails, phase 3 is never reached and the DB is untouched.
#[tauri::command]
pub fn update_metadata(
    conn: State<'_, Mutex<Connection>>,
    track_id: i64,
    edit: MetadataEdit,
) -> Result<String, String> {
    let path = {
        let conn = db::lock_conn(&conn)?;
        update_metadata_path(&conn, track_id)?
    };
    let snapshot = update_metadata_write_file(&path, &edit)?;
    let conn = db::lock_conn(&conn)?;
    update_metadata_commit(&conn, track_id, &edit, &path, &snapshot)
}

/// Group `filed` tracks by acoustic fingerprint into duplicate clusters, each with a
/// recommended keeper.
///
/// **N'est plus en lecture seule depuis la v19** : l'appel met à jour `dup_edges` / `dup_scanned`
/// au passage. Résoudre un groupe reste un `trash_track` par perdant, et le `ON DELETE CASCADE`
/// du schéma s'occupe des arêtes.
///
/// Toute la mécanique est dans `refresh_duplicate_groups` — y compris la portée des verrous, qui
/// reste l'invariant de SYS-1 : jamais le verrou global pendant `build_fingerprints`.
#[tauri::command]
pub fn scan_library_duplicates(
    conn: State<'_, Mutex<Connection>>,
) -> Result<Vec<crate::dedup::DupGroup>, String> {
    refresh_duplicate_groups(&conn)
}

/// Met l'état de dédoublonnage à jour, puis rend les groupes courants.
///
/// **Incrémental depuis la v19** (Phase 4). Le scan complet coûte ≈ 2 min 31 s à 15 000 pistes
/// (`bench_dedup.rs`) et il était rejoué dès qu'UNE piste était rangée, parce que
/// `library::filed_signature` vaut `(COUNT(*), MAX(id))`. Ici seules les pistes jamais comparées
/// sont comparées, contre les seules candidates que le pré-filtre de durée laisse passer :
/// ~20 ms dans le cas courant. Le premier appel sur une base existante fait le travail complet
/// une fois, puis plus jamais.
///
/// **Portée des verrous**, invariant repris de SYS-1 (2026-07-28) : le verrou global n'est jamais
/// tenu pendant `build_fingerprints`, qui décode de l'audio depuis le disque. Trois sections
/// courtes — lecture, puis écriture, puis relecture — séparées par du calcul non verrouillé.
fn refresh_duplicate_groups(
    conn: &State<'_, Mutex<Connection>>,
) -> Result<Vec<crate::dedup::DupGroup>, String> {
    // ── 1. Lecture brève : ce qui reste à comparer, et contre quoi ────────────
    let (unscanned, candidates) = {
        let guard = db::lock_conn(conn)?;
        // Avant tout le reste : une piste qui n'est plus `filed` (ré-encodée donc repassée en
        // `pending`, ou dérangée) doit sortir du jeu AVEC ses arêtes, sinon elles mentiraient sur
        // une empreinte que `scanner.rs` vient d'effacer.
        crate::dedup::prune_unfiled(&guard).map_err(|e| e.to_string())?;
        let unscanned = crate::dedup::load_unscanned_rows(&guard).map_err(|e| e.to_string())?;
        let mut candidates = Vec::with_capacity(unscanned.len());
        for row in &unscanned {
            candidates
                .push(crate::dedup::load_dup_candidates(&guard, row).map_err(|e| e.to_string())?);
        }
        (unscanned, candidates)
        // `guard` relâché ici — avant le décodage disque ci-dessous.
    };

    // ── 2. Calcul non verrouillé ─────────────────────────────────────────────
    let built = crate::dedup::build_fingerprints(&unscanned);
    let mut edges = Vec::new();
    for (i, row) in unscanned.iter().enumerate() {
        let Some(fp) = built.fps[i].as_deref() else {
            // Empreinte impossible à calculer (fichier illisible). La piste est quand même
            // marquée comparée : la reprendre à chaque passage rejouerait le même échec de
            // décodage indéfiniment.
            continue;
        };
        edges.extend(crate::dedup::edges_against(row, fp, &candidates[i]));
    }
    // Les nouvelles pistes entre elles. `load_dup_candidates` ne rend que `dup_scanned`, donc sans
    // ceci deux doublons rangés dans la même fournée ne se verraient jamais.
    //
    // `for_each_candidate_pair` remplace la double boucle `i+1..` depuis #38 : elle rendait chaque
    // paire une seule fois, mais les rendait TOUTES — et le premier passage sur une base existante
    // a `unscanned == toute la bibliothèque`, donc `n²/2` (3,22 s de balayage nu à 100 000 pistes,
    // mesuré). Les paires qui atteignent `edge_between` sont exactement les mêmes.
    let new_durations: Vec<Option<f64>> = unscanned.iter().map(|r| r.duration).collect();
    crate::dedup::for_each_candidate_pair(&new_durations, |i, j| {
        let (Some(fi), Some(fj)) = (built.fps[i].as_deref(), built.fps[j].as_deref()) else {
            return;
        };
        if let Some(e) = crate::dedup::edge_between(&unscanned[i], fi, &unscanned[j], fj) {
            edges.push(e);
        }
    });

    // ── 3. Écriture brève ────────────────────────────────────────────────────
    {
        let mut guard = db::lock_conn(conn)?;
        if !built.to_persist.is_empty() {
            crate::dedup::persist_fingerprints(&guard, &built.to_persist);
        }
        let ids: Vec<i64> = unscanned.iter().map(|r| r.id).collect();
        crate::dedup::record_scanned(&mut guard, &edges, &ids).map_err(|e| e.to_string())?;
    }

    // ── 4. Relecture brève, puis assemblage non verrouillé ───────────────────
    let (rows, all_edges) = {
        let guard = db::lock_conn(conn)?;
        let rows = crate::dedup::load_dup_group_rows(&guard).map_err(|e| e.to_string())?;
        let edges = crate::dedup::load_edges(&guard).map_err(|e| e.to_string())?;
        (rows, edges)
    };
    Ok(crate::dedup::groups_from_edges(&rows, &all_edges))
}

/// Dashboard aggregate stats for the Bibliothèque (totals, lossless/mp3 split, duplicates,
/// tracks to re-source, genre breakdown).
///
/// Ne tient PAS le verrou global pendant le comptage des doublons. Sur un cache miss, ce comptage
/// passe par `build_fingerprints`, qui décode de l'audio depuis le disque : le tenir sous le verrou
/// affamait toute autre commande IPC et le `persist_result` du pool d'analyse en tâche de fond —
/// exactement ce que `scan_library_duplicates`, 25 lignes plus haut, documente et évite déjà. Le
/// bon patron était écrit dans le même fichier et n'avait pas été appliqué ici. Audit 2026-07-28,
/// SYS-1.
///
/// Portée des verrous : bref agrégat SQL + lecture de signature, relâche, calcul non verrouillé,
/// puis brève écriture uniquement si de nouvelles empreintes ont été calculées.
#[tauri::command]
pub fn library_stats(
    conn: State<'_, Mutex<Connection>>,
) -> Result<library::DashboardStats, String> {
    // Premier verrou : agrégats SQL + signature. RIEN d'autre — surtout pas le chargement des
    // lignes de dédoublonnage : elles portent les empreintes de toute la bibliothèque filée, et
    // les charger inconditionnellement ferait payer au CAS NORMAL (cache hit, à chaque visite du
    // tableau de bord) une lecture massive pour la jeter aussitôt.
    let (mut stats, sig) = {
        let guard = db::lock_conn(&conn)?;
        let stats = library::library_stats(&guard).map_err(|e| e.to_string())?;
        let sig = library::filed_signature(&guard).map_err(|e| e.to_string())?;
        (stats, sig)
        // `guard` relâché ici.
    };

    // Le verrou global est RELÂCHÉ à ce point — invariant d'appel de `duplicate_count_or_compute`,
    // qui prend son propre jeton de single-flight et appelle `compute` par-dessus. Le cache, la
    // génération et la sérialisation des calculs concurrents vivent tous dans cette fonction ;
    // ici on ne fournit que le calcul coûteux.
    // Le cache reste utile mais n'est plus critique : depuis la v19 son défaut de granularité
    // (`filed_signature` bouge à chaque rangement) déclenche un recalcul INCRÉMENTAL de ~20 ms
    // au lieu des ~2 min 31 s du scan complet. On le garde pour éviter le travail redondant entre
    // deux visites du tableau de bord, plus pour masquer un coût qui n'existe plus.
    stats.duplicates = library::duplicate_count_or_compute(sig, || {
        Ok::<i64, String>(refresh_duplicate_groups(&conn)?.len() as i64)
    })?;
    Ok(stats)
}

// ── M7 Rekordbox XML export + playlist path repair ──────────────────────────

/// Status of the linked Rekordbox XML — surfaced to the Bibliothèque dashboard card.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RekordboxLinkStatus {
    pub path: Option<String>,
    pub linked: bool,
    pub playlist_count: usize,
    pub track_count: usize,
    /// Set (linked=false is NOT implied) when the linked file is unreadable/corrupt at last
    /// check — the card shows this and blocks further auto-repair until the user re-links.
    pub error: Option<String>,
    /// FIX-7: true when a prior filing/move's Rekordbox repair hit an AMBIGUOUS `patch_location`
    /// match (`settings::REKORDBOX_XML_DRIFT` — see `actions::repair_rekordbox_xml_if_linked`) —
    /// the linked XML's raw text no longer matches what Sift's DB expects for some track, and the
    /// repair could not safely proceed. Previously only visible in the server log. Cleared by a
    /// fresh `link_rekordbox_xml` or the next successful repair.
    pub drift_detected: bool,
    /// Impasse A13 ([issue #15](https://github.com/c0remusic/sift/issues/15)) : pourquoi les
    /// quatre cartes de synchronisation M8 ne peuvent RIEN faire, quand c'est le cas.
    ///
    /// `error` ci-dessus ne couvre que le XML lié. Les quatre détecteurs M8, eux, lisent
    /// `master.db` — un tout autre fichier, à un tout autre endroit — et transforment chacun leur
    /// échec en `None` muet. Zéro ligne détectée donnait donc un corps de carte vide, que l'écran
    /// peignait « à jour » : l'app affirmait que tout était synchronisé précisément quand rien ne
    /// pouvait l'être.
    ///
    /// **Portée exacte, à ne pas élargir en lisant ce champ** : il répond à « le fichier est-il là,
    /// à l'endroit où on le cherche ». Il ne dit PAS qu'il se déchiffre — ça coûterait le
    /// déchiffrement SQLCipher multi-Mo à chaque affichage de l'écran, et cet appel-ci est un
    /// statut. Un `master.db` présent mais illisible reste, lui, un `None` chez les détecteurs.
    pub masterdb_error: Option<String>,
}

/// Le `masterdb_error` de `RekordboxLinkStatus` : `None` si rien n'est lié (les cartes M8 ne sont
/// pas en jeu, l'écran montre la porte « lier un XML »), sinon un compte positif de ce qui manque.
///
/// Deux causes distinctes, dites séparément plutôt qu'aplaties : le dossier de configuration de
/// l'OS est introuvable, ou `master.db` n'est pas dedans. La seconde est le cas d'une machine sans
/// Rekordbox — celui de l'inventaire du premier lancement.
///
/// N'ouvre pas le fichier : `exists()` seul, pour que le coût reste celui d'un statut.
fn masterdb_error(linked: bool) -> Option<String> {
    if !linked {
        return None;
    }
    let Some(dir) = crate::actions::rekordbox_pioneer_dir() else {
        return Some(
            "dossier de configuration introuvable — impossible de localiser master.db".into(),
        );
    };
    let path = dir.join("master.db");
    if path.exists() {
        None
    } else {
        Some(format!("master.db introuvable ({})", path.display()))
    }
}

/// Read the persisted drift flag (see `settings::REKORDBOX_XML_DRIFT`) for building a
/// `RekordboxLinkStatus`. Absent/unset or any value other than "1" = no known drift.
fn drift_detected(conn: &Connection) -> bool {
    crate::settings::get(conn, crate::settings::REKORDBOX_XML_DRIFT)
        .ok()
        .flatten()
        .as_deref()
        == Some("1")
}

fn count_playlists(nodes: &[crate::rekordbox_xml::PlaylistNode]) -> usize {
    nodes
        .iter()
        .map(|n| match n {
            crate::rekordbox_xml::PlaylistNode::Playlist { .. } => 1,
            crate::rekordbox_xml::PlaylistNode::Folder { children, .. } => {
                count_playlists(children)
            }
        })
        .sum()
}

/// Plain (testable without a `State`) implementation of `link_rekordbox_xml`: parse+validate
/// `path` as a Rekordbox XML and, on success, persist it as the linked file via `conn`. Fails
/// fast (nothing persisted) if the file can't be read or parsed.
fn link_rekordbox_xml_inner(conn: &Connection, path: &str) -> Result<RekordboxLinkStatus, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("lecture impossible: {e}"))?;
    let parsed = crate::rekordbox_xml::parse(&bytes)?;
    crate::settings::set(conn, crate::settings::REKORDBOX_XML_PATH, path)
        .map_err(|e| e.to_string())?;
    // FIX-7: (re-)linking is the user's explicit "I've dealt with it" signal — clear any drift
    // flagged against the PREVIOUSLY linked file so a stale warning doesn't linger forever.
    crate::settings::set(conn, crate::settings::REKORDBOX_XML_DRIFT, "0")
        .map_err(|e| e.to_string())?;
    Ok(RekordboxLinkStatus {
        path: Some(path.to_string()),
        linked: true,
        playlist_count: count_playlists(&parsed.playlists),
        track_count: parsed.collection.len(),
        error: None,
        drift_detected: false,
        masterdb_error: masterdb_error(true),
    })
}

/// Parse+validate `path` as a Rekordbox XML and, on success, persist it as the linked file.
/// Fails fast (path NOT persisted) if the file can't be read or parsed — no silent partial link.
#[tauri::command]
pub fn link_rekordbox_xml(
    conn: State<'_, Mutex<Connection>>,
    path: String,
) -> Result<RekordboxLinkStatus, String> {
    let conn = db::lock_conn(&conn)?;
    link_rekordbox_xml_inner(&conn, &path)
}

/// Test harness for `rekordbox_status`: composes the settings read and the disk read against a
/// single connection, in the same order the command runs them. `cfg(test)` — production reads
/// the settings, releases the lock, and only then touches the file.
#[cfg(test)]
fn rekordbox_status_inner(conn: &Connection) -> Result<RekordboxLinkStatus, String> {
    let path = crate::settings::get(conn, crate::settings::REKORDBOX_XML_PATH)
        .map_err(|e| e.to_string())?;
    let drift = drift_detected(conn);
    Ok(rekordbox_status_from_disk(path, drift))
}

/// The disk half of `rekordbox_status`: re-read and re-parse the linked XML. NO DB access — the
/// two settings it needs (`path`, `drift`) are read first, under the lock, and passed in. Reading
/// and parsing a full Rekordbox XML is unbounded I/O + CPU and this command is called on every
/// visit to the Rekordbox screen, so it must not run under the global connection mutex. Nothing
/// is written anywhere, so there is no state that can go stale between the two halves.
fn rekordbox_status_from_disk(path: Option<String>, drift: bool) -> RekordboxLinkStatus {
    let Some(path) = path else {
        return RekordboxLinkStatus {
            path: None,
            linked: false,
            playlist_count: 0,
            track_count: 0,
            error: None,
            drift_detected: false,
            masterdb_error: None,
        };
    };
    match std::fs::read(&path)
        .map_err(|e| e.to_string())
        .and_then(|b| crate::rekordbox_xml::parse(&b))
    {
        Ok(parsed) => RekordboxLinkStatus {
            path: Some(path),
            linked: true,
            playlist_count: count_playlists(&parsed.playlists),
            track_count: parsed.collection.len(),
            error: None,
            drift_detected: drift,
            masterdb_error: masterdb_error(true),
        },
        // Le XML est illisible ET `master.db` est peut-être absent : ce sont deux fichiers
        // différents, donc deux champs, jamais l'un rabattu sur l'autre. C'est exactement la
        // confusion que corrige A13 — l'écran n'annonçait « indisponible » que sur le premier.
        Err(e) => RekordboxLinkStatus {
            path: Some(path),
            linked: true,
            playlist_count: 0,
            track_count: 0,
            error: Some(e),
            drift_detected: drift,
            masterdb_error: masterdb_error(true),
        },
    }
}

/// Current link status: re-reads the linked file (if any) fresh from disk. If a path is
/// persisted but the file is now unreadable/corrupt, reports `linked:true, error:Some(..)` —
/// the setting is NOT cleared automatically (the spec: block auto-rewrite, don't lose the
/// reference silently; the user must explicitly re-link).
///
/// The two settings are read under the lock; the file read + XML parse run after releasing it.
#[tauri::command]
pub fn rekordbox_status(conn: State<'_, Mutex<Connection>>) -> Result<RekordboxLinkStatus, String> {
    let (path, drift) = {
        let conn = db::lock_conn(&conn)?;
        (
            crate::settings::get(&conn, crate::settings::REKORDBOX_XML_PATH)
                .map_err(|e| e.to_string())?,
            drift_detected(&conn),
        )
    };
    Ok(rekordbox_status_from_disk(path, drift))
}

/// Plain (testable) implementation of `export_rekordbox_xml`.
fn export_rekordbox_xml_inner(conn: &Connection) -> Result<RekordboxLinkStatus, String> {
    let path = crate::settings::get(conn, crate::settings::REKORDBOX_XML_PATH)
        .map_err(|e| e.to_string())?
        .ok_or("aucun XML Rekordbox lié — relie un fichier avant d'exporter")?;
    let filed = library::list_filed(conn, &LibraryFilter::default()).map_err(|e| e.to_string())?;

    let bytes = std::fs::read(&path).map_err(|e| format!("XML Rekordbox illisible: {e}"))?;
    let mut parsed = crate::rekordbox_xml::parse(&bytes)?;
    crate::rekordbox_xml::merge_filed_tracks(&mut parsed, &filed);
    let out = crate::rekordbox_xml::write(&parsed);
    std::fs::write(&path, &out).map_err(|e| format!("écriture impossible: {e}"))?;
    Ok(RekordboxLinkStatus {
        path: Some(path),
        linked: true,
        playlist_count: count_playlists(&parsed.playlists),
        track_count: parsed.collection.len(),
        error: None,
        drift_detected: drift_detected(conn),
        masterdb_error: masterdb_error(true),
    })
}

/// Reload the linked XML, merge every `filed` track absent from it, rewrite the file. Fails fast
/// (no write attempted) if no XML is linked, or if the linked file is unreadable/corrupt — no
/// silent recreation of an empty tree, matching the spec's fail-fast requirement.
#[tauri::command]
pub fn export_rekordbox_xml(
    conn: State<'_, Mutex<Connection>>,
) -> Result<RekordboxLinkStatus, String> {
    let conn = db::lock_conn(&conn)?;
    export_rekordbox_xml_inner(&conn)
}

// ── M8 Tier 1: master.db path-repair candidates ──────────────────────────────

/// One candidate `master.db` path repair, detected read-only on filing
/// (`actions::detect_masterdb_repair_if_linked`) and surfaced for manual, batch-confirmed
/// application. Never applied automatically.
pub use crate::rekordbox_repairs::{ApplyRepairOutcome, PendingMasterdbRepair};

/// Candidate `master.db` path repairs detected so far, excluding ones already `applied` or
/// `dismissed`.
#[tauri::command]
pub fn rekordbox_masterdb_pending_repairs(
    conn: State<'_, Mutex<Connection>>,
) -> Result<Vec<PendingMasterdbRepair>, String> {
    let conn = db::lock_conn(&conn)?;
    crate::rekordbox_repairs::pending_repairs_inner(&conn)
}

/// Mark a pending/ambiguous repair as dismissed — it stops appearing in `pending_repairs`.
/// Never applies anything.
#[tauri::command]
pub fn rekordbox_masterdb_dismiss_repair(
    conn: State<'_, Mutex<Connection>>,
    id: i64,
) -> Result<(), String> {
    let conn = db::lock_conn(&conn)?;
    crate::rekordbox_repairs::dismiss_repair_inner(&conn, id)
}

/// Resolves an ambiguous repair by manually picking the correct `master.db` candidate. The row
/// becomes an ordinary `pending` row afterwards — no other change to the `apply_repairs` flow.
#[tauri::command]
pub fn rekordbox_masterdb_resolve_ambiguous(
    conn: State<'_, Mutex<Connection>>,
    id: i64,
    chosen_track_id: String,
) -> Result<(), String> {
    let conn = db::lock_conn(&conn)?;
    crate::rekordbox_repairs::resolve_ambiguous_inner(&conn, id, &chosen_track_id)
}

/// Applies the given pending/ambiguous repair `id`s against the linked Rekordbox's `master.db`,
/// one at a time (never in parallel — one `master.db`). Never invoked automatically — this is
/// the explicit, user-confirmed write step. A failure on one `id` does not stop the rest of the
/// batch. Backups land under `app_data_dir()/rekordbox-backups/<batch timestamp>/<id>/`, one
/// subdirectory per row so a later row's backup in the same batch never overwrites an earlier
/// row's.
#[tauri::command]
pub fn rekordbox_masterdb_apply_repairs(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    ids: Vec<i64>,
) -> Result<Vec<ApplyRepairOutcome>, String> {
    let backup_root = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("rekordbox-backups");
    let conn = db::lock_conn(&conn)?;
    crate::rekordbox_repairs::apply_repairs_inner(&conn, &backup_root, &ids)
}

// ── M8 Tier 3: master.db metadata sync candidates ─────────────────────────────

pub use crate::rekordbox_repairs::{ApplyMetadataSyncOutcome, PendingMetadataSync};

/// Candidate `master.db` metadata syncs detected so far, excluding ones already `applied` or
/// `dismissed`.
#[tauri::command]
pub fn rekordbox_masterdb_pending_metadata_syncs(
    conn: State<'_, Mutex<Connection>>,
) -> Result<Vec<PendingMetadataSync>, String> {
    let conn = db::lock_conn(&conn)?;
    crate::rekordbox_repairs::pending_metadata_syncs_inner(&conn)
}

/// Mark a pending/ambiguous metadata sync as dismissed — it stops appearing in
/// `pending_metadata_syncs`. Never applies anything. A subsequent retag of the same track still
/// resurrects a fresh candidate (see `detect_masterdb_metadata_sync_if_linked`'s `ON CONFLICT`).
#[tauri::command]
pub fn rekordbox_masterdb_dismiss_metadata_sync(
    conn: State<'_, Mutex<Connection>>,
    id: i64,
) -> Result<(), String> {
    let conn = db::lock_conn(&conn)?;
    crate::rekordbox_repairs::dismiss_metadata_sync_inner(&conn, id)
}

/// Resolves an ambiguous metadata sync by manually picking the correct `master.db` candidate. The
/// row becomes an ordinary `pending` row afterwards — no other change to the
/// `apply_metadata_syncs` flow.
#[tauri::command]
pub fn rekordbox_masterdb_resolve_ambiguous_metadata_sync(
    conn: State<'_, Mutex<Connection>>,
    id: i64,
    chosen_track_id: String,
) -> Result<(), String> {
    let conn = db::lock_conn(&conn)?;
    crate::rekordbox_repairs::resolve_ambiguous_metadata_sync_inner(&conn, id, &chosen_track_id)
}

/// Applies the given pending/ambiguous metadata sync `id`s against the linked Rekordbox's
/// `master.db`, one at a time. Never invoked automatically — explicit user-confirmed write step.
/// A failure on one `id` does not stop the rest of the batch. Backups land under
/// `app_data_dir()/rekordbox-backups/<batch timestamp>/<id>/`, same convention as Tier 1/2.
#[tauri::command]
pub fn rekordbox_masterdb_apply_metadata_syncs(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    ids: Vec<i64>,
) -> Result<Vec<ApplyMetadataSyncOutcome>, String> {
    let backup_root = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("rekordbox-backups");
    let conn = db::lock_conn(&conn)?;
    crate::rekordbox_repairs::apply_metadata_syncs_inner(&conn, &backup_root, &ids)
}

// ── M8 Tier 3 (pochette): master.db artwork sync candidates ───────────────────

pub use crate::rekordbox_repairs::PendingArtworkSync;

/// Candidate `master.db` artwork syncs detected so far, excluding ones already `applied` or
/// `dismissed`.
#[tauri::command]
pub fn rekordbox_masterdb_pending_artwork_syncs(
    conn: State<'_, Mutex<Connection>>,
) -> Result<Vec<PendingArtworkSync>, String> {
    let conn = db::lock_conn(&conn)?;
    crate::rekordbox_repairs::rekordbox_masterdb_pending_artwork_syncs_inner(&conn)
}

/// Mark a pending/ambiguous artwork sync as dismissed.
#[tauri::command]
pub fn rekordbox_masterdb_dismiss_artwork_sync(
    conn: State<'_, Mutex<Connection>>,
    id: i64,
) -> Result<(), String> {
    let conn = db::lock_conn(&conn)?;
    crate::rekordbox_repairs::rekordbox_masterdb_dismiss_artwork_sync_inner(&conn, id)
}

/// Resolves an ambiguous artwork sync by manually picking the correct `master.db` candidate.
#[tauri::command]
pub fn rekordbox_masterdb_resolve_ambiguous_artwork_sync(
    conn: State<'_, Mutex<Connection>>,
    id: i64,
    chosen_track_id: String,
) -> Result<(), String> {
    let conn = db::lock_conn(&conn)?;
    crate::rekordbox_repairs::rekordbox_masterdb_resolve_ambiguous_artwork_sync_inner(
        &conn,
        id,
        &chosen_track_id,
    )
}

/// Applies the given pending/ambiguous artwork sync `id`s against the linked Rekordbox's cached
/// artwork files, one at a time. Never invoked automatically. Backups land under
/// `app_data_dir()/rekordbox-backups/<batch timestamp>/<id>/`, same convention as the other tiers.
#[tauri::command]
pub fn rekordbox_masterdb_apply_artwork_syncs(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    ids: Vec<i64>,
) -> Result<Vec<crate::rekordbox_repairs::ApplyArtworkSyncOutcome>, String> {
    let backup_root = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("rekordbox-backups");
    let conn = db::lock_conn(&conn)?;
    crate::rekordbox_repairs::rekordbox_masterdb_apply_artwork_syncs_inner(
        &conn,
        &backup_root,
        &ids,
    )
}

// ── M8 Tier 2: playlist duplicate-entry dedup ─────────────────────────────────

pub use crate::rekordbox_repairs::PlaylistDuplicateGroupDto;

/// Scans the linked Rekordbox's `master.db` for playlists containing the
/// same track more than once. Read-only — never touches `master.db`. Called
/// fresh on demand (no persistence): unlike Tier 1's candidate repairs,
/// duplicate playlist entries are a pre-existing library condition, not
/// something Sift's own actions cause, so there's nothing to detect
/// mid-filing or store until later review.
#[tauri::command]
pub fn rekordbox_masterdb_scan_playlist_duplicates(
    conn: State<'_, Mutex<Connection>>,
) -> Result<Vec<PlaylistDuplicateGroupDto>, String> {
    let conn = db::lock_conn(&conn)?;
    crate::rekordbox_repairs::scan_playlist_duplicates_inner(&conn)
}

/// Removes every extra occurrence in `group.remove` from the linked
/// Rekordbox's `master.db`, keeping `group.keep` untouched — the explicit,
/// user-confirmed write step for one duplicate group returned by
/// `rekordbox_masterdb_scan_playlist_duplicates`. Never invoked
/// automatically. `group` should be exactly what the frontend received from
/// a scan; if the library changed since then (e.g. the row was already
/// removed), the write engine's own verification catches it and this
/// returns a humanized error rather than silently doing nothing or the
/// wrong thing.
#[tauri::command]
pub fn rekordbox_masterdb_dedup_playlist_group(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    group: PlaylistDuplicateGroupDto,
) -> Result<(), String> {
    let backup_root = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("rekordbox-backups");
    let conn = db::lock_conn(&conn)?;
    crate::rekordbox_repairs::dedup_playlist_group_inner(&conn, &backup_root, group)
}

#[cfg(test)]
mod rekordbox_tests {
    use super::*;

    fn db() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        crate::db::run_migrations(&c).unwrap();
        c
    }

    fn fixture(name: &str) -> Option<String> {
        let p = format!("fixtures/{name}");
        if std::path::Path::new(&p).exists() {
            Some(p)
        } else {
            None
        }
    }

    #[test]
    fn update_metadata_journals_a_revertable_tag_edit() {
        let Some(src) = fixture("real_320.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("track.mp3");
        std::fs::copy(&src, &path).unwrap();
        crate::tagging::write_tags_full(
            path.to_str().unwrap(),
            "OLD Artist",
            "OLD Title",
            None,
            None,
            &[],
            None,
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks(path, status) VALUES(?1, 'filed')",
            rusqlite::params![path.to_str().unwrap()],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();

        let edit = crate::metadata::MetadataEdit {
            artist: "NEW Artist".to_string(),
            title: "NEW Title".to_string(),
            label: None,
            year: None,
            genres: vec![],
            cover_path: None,
        };
        let batch_id = update_metadata_inner(&conn, track_id, edit).unwrap();
        assert!(!batch_id.is_empty());

        let after = crate::tagging::read_tags_full(path.to_str().unwrap()).unwrap();
        assert_eq!(after.artist.as_deref(), Some("NEW Artist"));

        crate::actions::revert_batch(&conn, &batch_id).unwrap();
        let reverted = crate::tagging::read_tags_full(path.to_str().unwrap()).unwrap();
        assert_eq!(
            reverted.artist.as_deref(),
            Some("OLD Artist"),
            "revert_batch must restore the pre-edit tags"
        );
    }

    #[test]
    fn update_metadata_calls_masterdb_metadata_sync_detection_when_linked() {
        let Some(src) = fixture("real_320.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        let conn = db();
        let tmp = tempfile::tempdir().unwrap();
        let pioneer_dir = tmp.path().join("pioneer");
        std::fs::create_dir_all(&pioneer_dir).unwrap();
        std::fs::copy(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/rekordbox_master.db"
            ),
            pioneer_dir.join("master.db"),
        )
        .unwrap();
        crate::actions::set_pioneer_dir_override_for_test(pioneer_dir.clone());
        let xml_path = pioneer_dir.join("masterPlaylists6.xml");
        std::fs::write(&xml_path, b"<DJ_PLAYLISTS/>").unwrap();
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("track1.mp3");
        std::fs::copy(&src, &path).unwrap();
        crate::tagging::write_tags_full(
            path.to_str().unwrap(),
            "Old",
            "Old Title",
            None,
            None,
            &[],
            None,
        )
        .unwrap();

        // Patch the fixture's track_id "40000001" FolderPath to this real temp path — same
        // decrypt/re-encrypt-for-test technique as actions.rs's ambiguous-match test (Task 2) —
        // so tracks.path (below) and master.db's FolderPath refer to the exact same string.
        let raw = std::fs::read(pioneer_dir.join("master.db")).unwrap();
        let plaintext = crate::rekordbox_masterdb::decrypt_masterdb_for_test(&raw);
        let len = plaintext.len();
        let mut conn2 = rusqlite::Connection::open_in_memory().unwrap();
        conn2
            .deserialize_read_exact(
                rusqlite::MAIN_DB,
                std::io::Cursor::new(plaintext),
                len,
                false,
            )
            .unwrap();
        conn2
            .execute(
                "UPDATE djmdContent SET FolderPath=?1 WHERE ID='40000001'",
                rusqlite::params![path.to_str().unwrap()],
            )
            .unwrap();
        let plaintext2 = conn2.serialize(rusqlite::MAIN_DB).unwrap().to_vec();
        let raw2 = crate::rekordbox_masterdb::encrypt_masterdb_for_test(&plaintext2);
        std::fs::write(pioneer_dir.join("master.db"), raw2).unwrap();

        conn.execute(
            "INSERT INTO tracks(path, status) VALUES(?1, 'filed')",
            rusqlite::params![path.to_str().unwrap()],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();

        let edit = crate::metadata::MetadataEdit {
            artist: "New Artist".to_string(),
            title: "New Title".to_string(),
            label: None,
            year: None,
            genres: vec![],
            cover_path: None,
        };
        update_metadata_inner(&conn, track_id, edit).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM rekordbox_masterdb_metadata_syncs WHERE track_id=?1",
                rusqlite::params![track_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn update_metadata_calls_masterdb_artwork_sync_detection_only_when_cover_edited() {
        let Some(src) = fixture("real_320.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        let conn = db();
        let tmp = tempfile::tempdir().unwrap();
        let pioneer_dir = tmp.path().join("pioneer");
        std::fs::create_dir_all(&pioneer_dir).unwrap();
        std::fs::copy(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/rekordbox_master.db"
            ),
            pioneer_dir.join("master.db"),
        )
        .unwrap();
        crate::actions::set_pioneer_dir_override_for_test(pioneer_dir.clone());
        let xml_path = pioneer_dir.join("masterPlaylists6.xml");
        std::fs::write(&xml_path, b"<DJ_PLAYLISTS/>").unwrap();
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("track1.mp3");
        std::fs::copy(&src, &path).unwrap();
        crate::tagging::write_tags_full(
            path.to_str().unwrap(),
            "Old",
            "Old Title",
            None,
            None,
            &[],
            None,
        )
        .unwrap();

        let raw = std::fs::read(pioneer_dir.join("master.db")).unwrap();
        let plaintext = crate::rekordbox_masterdb::decrypt_masterdb_for_test(&raw);
        let len = plaintext.len();
        let mut conn2 = rusqlite::Connection::open_in_memory().unwrap();
        conn2
            .deserialize_read_exact(
                rusqlite::MAIN_DB,
                std::io::Cursor::new(plaintext),
                len,
                false,
            )
            .unwrap();
        conn2
            .execute(
                "UPDATE djmdContent SET FolderPath=?1 WHERE ID='40000001'",
                rusqlite::params![path.to_str().unwrap()],
            )
            .unwrap();
        let plaintext2 = conn2.serialize(rusqlite::MAIN_DB).unwrap().to_vec();
        let raw2 = crate::rekordbox_masterdb::encrypt_masterdb_for_test(&plaintext2);
        std::fs::write(pioneer_dir.join("master.db"), raw2).unwrap();

        conn.execute(
            "INSERT INTO tracks(path, status) VALUES(?1, 'filed')",
            rusqlite::params![path.to_str().unwrap()],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();

        // (1) Edit WITHOUT touching the cover — no artwork candidate expected.
        let edit_no_cover = crate::metadata::MetadataEdit {
            artist: "New Artist".to_string(),
            title: "New Title".to_string(),
            label: None,
            year: None,
            genres: vec![],
            cover_path: None,
        };
        update_metadata_inner(&conn, track_id, edit_no_cover).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM rekordbox_masterdb_artwork_syncs WHERE track_id=?1",
                rusqlite::params![track_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 0,
            "no cover_path in this edit — must not create an artwork sync candidate"
        );

        // (2) Edit WITH a new cover — artwork candidate expected.
        let edit_with_cover = crate::metadata::MetadataEdit {
            artist: "New Artist".to_string(),
            title: "New Title".to_string(),
            label: None,
            year: None,
            genres: vec![],
            cover_path: Some("/cache/covers/999.jpg".to_string()),
        };
        update_metadata_inner(&conn, track_id, edit_with_cover).unwrap();
        let cover_path: String = conn
            .query_row(
                "SELECT cover_path FROM rekordbox_masterdb_artwork_syncs WHERE track_id=?1",
                rusqlite::params![track_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cover_path, "/cache/covers/999.jpg");
    }

    #[test]
    fn link_rekordbox_xml_persists_path_on_valid_file() {
        let dir = tempfile::tempdir().unwrap();
        let xml_path = dir.path().join("export.xml");
        std::fs::write(&xml_path, crate::rekordbox_xml::SAMPLE_XML).unwrap();
        let conn = db();
        let status = link_rekordbox_xml_inner(&conn, xml_path.to_str().unwrap()).unwrap();
        assert!(status.linked);
        assert_eq!(status.track_count, 3);
        assert!(status.error.is_none());
        assert_eq!(
            crate::settings::get(&conn, crate::settings::REKORDBOX_XML_PATH).unwrap(),
            Some(xml_path.to_str().unwrap().to_string())
        );
    }

    #[test]
    fn link_rekordbox_xml_reports_error_on_corrupt_file_and_does_not_persist() {
        let dir = tempfile::tempdir().unwrap();
        let xml_path = dir.path().join("bad.xml");
        std::fs::write(&xml_path, b"<not-even-xml").unwrap();
        let conn = db();
        let result = link_rekordbox_xml_inner(&conn, xml_path.to_str().unwrap());
        assert!(
            result.is_err(),
            "corrupt XML must be rejected, not silently linked"
        );
        let saved = crate::settings::get(&conn, crate::settings::REKORDBOX_XML_PATH).unwrap();
        assert_eq!(saved, None, "no path persisted on a failed link");
    }

    #[test]
    fn rekordbox_status_reports_unlinked_when_no_setting() {
        let conn = db();
        let status = rekordbox_status_inner(&conn).unwrap();
        assert!(!status.linked);
        assert_eq!(status.path, None);
    }

    #[test]
    fn export_rekordbox_xml_fails_fast_when_nothing_linked() {
        let conn = db();
        let result = export_rekordbox_xml_inner(&conn);
        assert!(
            result.is_err(),
            "export with no linked XML must fail, not create one silently"
        );
    }

    #[test]
    fn export_rekordbox_xml_merges_filed_tracks_and_rewrites_file() {
        let dir = tempfile::tempdir().unwrap();
        let xml_path = dir.path().join("export.xml");
        std::fs::write(&xml_path, crate::rekordbox_xml::SAMPLE_XML).unwrap();
        let conn = db();
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks(path, status, folder) VALUES('C:/Music/Disco/new.mp3', 'filed', 'Disco')",
            [],
        )
        .unwrap();

        let status = export_rekordbox_xml_inner(&conn).unwrap();
        assert_eq!(status.track_count, 4, "3 original + 1 newly filed");

        let rewritten = std::fs::read_to_string(&xml_path).unwrap();
        assert!(rewritten.contains("Disco/new.mp3") || rewritten.contains("Disco%2Fnew.mp3"));
    }

    /// FIX-7 regression: `RekordboxLinkStatus.drift_detected` reflects the persisted
    /// `settings::REKORDBOX_XML_DRIFT` flag — false by default, true once set (as
    /// `actions::repair_rekordbox_xml_if_linked` would on an ambiguous `patch_location` match).
    #[test]
    fn rekordbox_status_reports_drift_detected_flag() {
        let dir = tempfile::tempdir().unwrap();
        let xml_path = dir.path().join("export.xml");
        std::fs::write(&xml_path, crate::rekordbox_xml::SAMPLE_XML).unwrap();
        let conn = db();
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        let status = rekordbox_status_inner(&conn).unwrap();
        assert!(!status.drift_detected, "no drift by default");

        crate::settings::set(&conn, crate::settings::REKORDBOX_XML_DRIFT, "1").unwrap();
        let status = rekordbox_status_inner(&conn).unwrap();
        assert!(status.drift_detected, "drift flag surfaced once set");
    }

    /// FIX-7 regression: re-linking (the user's explicit "I've dealt with it" signal) clears a
    /// previously-set drift flag.
    #[test]
    fn link_rekordbox_xml_clears_a_previously_set_drift_flag() {
        let dir = tempfile::tempdir().unwrap();
        let xml_path = dir.path().join("export.xml");
        std::fs::write(&xml_path, crate::rekordbox_xml::SAMPLE_XML).unwrap();
        let conn = db();
        crate::settings::set(&conn, crate::settings::REKORDBOX_XML_DRIFT, "1").unwrap();

        let status = link_rekordbox_xml_inner(&conn, xml_path.to_str().unwrap()).unwrap();
        assert!(!status.drift_detected, "re-linking clears prior drift");
        assert!(!rekordbox_status_inner(&conn).unwrap().drift_detected);
    }

    /// Impasse A13 ([issue #15](https://github.com/c0remusic/sift/issues/15)) : le statut doit
    /// distinguer « XML lié et master.db là » de « XML lié et master.db absent », faute de quoi
    /// les quatre cartes de synchronisation s'affichent « à jour » sur une machine sans Rekordbox.
    ///
    /// Les trois branches sont couvertes dans le même test parce qu'elles ne valent que
    /// L'UNE PAR RAPPORT AUX AUTRES : un `Some` seul ne prouve pas que le champ discrimine quoi
    /// que ce soit — il pourrait être `Some` en permanence.
    #[test]
    fn masterdb_error_distinguishes_a_missing_master_db_from_a_present_one() {
        let dir = tempfile::tempdir().unwrap();
        crate::actions::set_pioneer_dir_override_for_test(dir.path().to_path_buf());

        assert_eq!(
            masterdb_error(false),
            None,
            "rien de lié : les cartes M8 ne sont pas en jeu, pas d'erreur à signaler"
        );

        let absent = masterdb_error(true).expect("master.db absent doit produire un message");
        assert!(
            absent.contains("master.db introuvable"),
            "le message nomme le fichier manquant, pas un échec générique : {absent}"
        );

        std::fs::write(dir.path().join("master.db"), b"pas un vrai SQLCipher").unwrap();
        assert_eq!(
            masterdb_error(true),
            None,
            "fichier présent : ce champ ne parle QUE de présence, pas de lisibilité"
        );
    }
}
