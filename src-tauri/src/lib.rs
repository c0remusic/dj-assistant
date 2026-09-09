mod actions;
pub mod analysis;
mod b85_bytes;
#[cfg(test)]
mod bench_cpu_budget;
#[cfg(test)]
mod bench_dedup;
#[cfg(test)]
mod bench_sqlite;
#[cfg(test)]
mod bench_volume;
mod db;
mod dedup;
mod dev_annotate;
mod dev_locate;
mod ecartes;
mod encode;
mod ffmpeg;
mod filing;
mod fingerprint;
mod genres;
mod ipc;
mod ipc_filing;
mod ipc_identify;
mod ipc_library;
mod ipc_usage;
mod ipc_usb;
mod library;
mod metadata;
mod naming;
mod queue;
mod rekordbox_masterdb;
mod rekordbox_repairs;
mod rekordbox_xml;
mod reverdict;
mod scanner;
#[cfg(test)]
mod search_corpus;
mod search_terms;
mod settings;
mod sources;
mod tagging;
mod usb_format;
mod volume_usage;
mod watcher;
mod worker;

use std::sync::Mutex;
use tauri::Manager;

/// Extends the DWM frame into the whole client area (all margins -1) so Windows treats the
/// entire window as "glass" instead of drawing its own opaque backdrop in the native resize-margin
/// strip around an undecorated, transparent, resizable window — that strip was showing as a solid
/// blue-grey rectangle instead of true transparency (visible only in windowed, not maximized,
/// mode — maximized windows have no resize margin). See docs/ressources-externes.md.
#[cfg(windows)]
fn extend_frame_into_client_area(window: &tauri::WebviewWindow) {
    use windows::Win32::Graphics::Dwm::DwmExtendFrameIntoClientArea;
    use windows::Win32::UI::Controls::MARGINS;
    let Ok(hwnd) = window.hwnd() else { return };
    let margins = MARGINS {
        cxLeftWidth: -1,
        cxRightWidth: -1,
        cyTopHeight: -1,
        cyBottomHeight: -1,
    };
    // SAFETY: `hwnd` comes from `window.hwnd()` on a live WebviewWindow owned by this
    // process (the `let Ok(hwnd) = ... else { return }` above discards the failure case),
    // so it is a valid, currently-open top-level window handle. `margins` is a local,
    // fully-initialized `MARGINS` struct passed by reference for the duration of this
    // call only. `DwmExtendFrameIntoClientArea` has no other safety preconditions beyond
    // a valid HWND and a valid MARGINS pointer.
    unsafe {
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
    }
}

/// Demande à DWM d'arrondir les coins de la fenêtre (issue #41).
///
/// La coque est dessinée de bout en bout par l'application — `tauri.conf.json` déclare
/// `"decorations": false` et `"transparent": true` — donc aucun organe système ne pose de rayon à
/// sa place, et les coins restaient carrés pendant qu'une soixantaine de surfaces internes sont
/// arrondies. Le rayon d'une fenêtre est un organe DWM, pas une peinture de l'application : il se
/// demande par `DWMWA_WINDOW_CORNER_PREFERENCE`, jamais par un `border-radius` CSS — sur une
/// fenêtre transparente, un rayon CSS découperait le contenu en laissant des coins réellement
/// transparents, sans rien dire ni à l'ombre portée ni à la marge de redimensionnement, toutes deux
/// tenues par le système.
///
/// L'appel ÉCHOUE sur Windows 10 : l'attribut (33) n'existe qu'à partir du build 22000, et DWM rend
/// `E_INVALIDARG` en dessous. C'est le comportement attendu de la plateforme, pas une erreur à
/// remonter — Windows 10 n'arrondit aucune fenêtre, celles du système comprises, donc une fenêtre
/// carrée y est le rendu juste. D'où le `let _ =` sans journalisation : cet échec est une propriété
/// de la version de l'OS, connue une fois pour toutes au lancement, et rien ni personne n'agirait
/// sur la ligne de log qu'il produirait.
#[cfg(windows)]
fn round_window_corners(window: &tauri::WebviewWindow) {
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    };
    let Ok(hwnd) = window.hwnd() else { return };
    let preference = DWMWCP_ROUND;
    // SAFETY: `hwnd` vient de `window.hwnd()` sur une WebviewWindow vivante possédée par ce
    // processus (le `let Ok(hwnd) = ... else { return }` ci-dessus écarte le cas d'échec), c'est
    // donc une poignée de fenêtre de haut niveau valide et actuellement ouverte. `preference` est
    // une variable locale entièrement initialisée, vivante pendant toute la durée de l'appel et
    // pendant elle seule : DWM lit le tampon et ne conserve pas le pointeur au retour. La paire
    // (attribut, taille) est la vraie précondition de `DwmSetWindowAttribute`, qui lit exactement
    // `cbattribute` octets derrière `pvattribute` — ici la taille est calculée sur la valeur
    // passée elle-même (`size_of_val`), donc elle ne peut pas mentir sur ce que le pointeur
    // désigne. `DWM_WINDOW_CORNER_PREFERENCE` est `#[repr(transparent)]` sur `i32`, la
    // représentation attendue par l'attribut.
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            std::ptr::from_ref(&preference).cast::<std::ffi::c_void>(),
            std::mem::size_of_val(&preference) as u32,
        );
    }
}

/// True only for the one specific, expected updater-plugin init failure: the WHOLE
/// `plugins.updater` key absent from the merged config, so serde tries to deserialize `null` as
/// the entire `Config` struct — the normal case for `tauri dev` and the unsigned CI build
/// (`npm run tauri build`, no `--config tauri.release.conf.json`). Deliberately narrow: matches
/// the exact captured phrase (verbatim from a real `tauri dev` run, see the test fixture) rather
/// than two independent `.contains()` checks — an earlier version of this classifier matched
/// `msg.contains("updater") && msg.contains("invalid type: null")` separately, which ALSO
/// classified a null SUB-FIELD inside an otherwise-present config (e.g. `"pubkey": null` on a
/// signed release build) as expected, silently swallowing a genuine misconfiguration (caught by
/// verify-gate before landing). Anything not matching this exact phrase — malformed pubkey, bad
/// `endpoints`, a null sub-field, any other plugin init failure — returns false and stays
/// fail-fast: a signed release build failing to register the updater for a REAL reason must crash
/// loudly, not vanish into a log line nobody reads (tauri_plugin_log itself is only registered
/// under cfg!(debug_assertions), so a release build's log::warn! is a no-op — see run()'s call
/// site). String-matched on the plugin's own error text, not a public tauri error variant — no
/// public API surfaces "config key was absent vs malformed" more precisely than this; a wording
/// change upstream would revert this to fail-fast-on-dev (loud, immediately visible), not to a
/// silent swallow, so the failure mode of drift here is the safe direction.
fn is_missing_updater_config(err: &tauri::Error) -> bool {
    err.to_string()
        .contains("'plugins.updater' within your Tauri configuration: invalid type: null, expected struct Config")
}

/// How long the journal purge waits before touching the DB at all. The retention sweep (PRD D4)
/// has no deadline of its own, whereas everything racing it at launch does: the first render is
/// bounded to under a second (PRD D3) and the analysis pool is refilling from the same single
/// `Mutex<Connection>`. Starting late costs nothing and keeps the sweep off the boot path
/// entirely, rather than merely making it short.
const JOURNAL_PURGE_START_DELAY: std::time::Duration = std::time::Duration::from_secs(10);

/// Runs the journal retention sweep once per launch, on its own thread.
///
/// A migration would be the wrong home for it: `db.rs`'s migrations run ONCE, keyed on
/// `PRAGMA user_version`, so a 30-day rolling window enforced there would be enforced exactly one
/// time and then never again. Retention is recurring work, and this `setup` is the only recurring
/// entry point the app has (the same place `worker::init`/`worker::refill` hook their background
/// work). Once per launch is deliberate: a session left open for weeks lets the window drift, but
/// the alternative — a timer thread — buys nothing for a desktop app that gets relaunched.
/// `purge_expired_journal` releases the DB lock between batches; the delay here only keeps the
/// very first query away from the launch burst.
fn spawn_journal_purge(app: &tauri::AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(JOURNAL_PURGE_START_DELAY);
        // `state()` panics when the state is absent; `setup` manages the connection before it
        // spawns this thread, so it cannot happen today — but a panic in a detached thread of a
        // maintenance task is the wrong failure mode either way (same rule that bans
        // unwrap/expect outside tests, .claude/rules/rust.md). Log and give up instead.
        let Some(state) = app.try_state::<Mutex<rusqlite::Connection>>() else {
            log::error!("journal retention: DB connection state unavailable, purge skipped");
            return;
        };
        match actions::purge_expired_journal(&state, actions::JOURNAL_RETENTION_DAYS) {
            Ok(0) => {}
            Ok(n) => log::info!(
                "journal retention: purged {n} action rows older than {} days",
                actions::JOURNAL_RETENTION_DAYS
            ),
            Err(e) => log::error!("journal retention purge failed: {e}"),
        }
    });
}

/// Intercepte le mode privilégié AVANT que Tauri ne démarre.
///
/// Rend `Some(code)` quand le processus a été relancé en administrateur pour formater un disque :
/// aucune fenêtre ne s'ouvre, aucune base n'est touchée, on fait le travail et on sort. `None`
/// dans tous les autres cas, c'est-à-dire au lancement normal.
///
/// Ce point d'entrée existe parce qu'écrire un FAT32 au-delà de 32 Go demande d'ouvrir un volume
/// brut, réservé à l'administrateur — et faire tourner Sift entier en élevé serait hostile.
#[cfg(target_os = "windows")]
pub fn run_privileged_if_asked() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    match usb_format::privileged::parse_args(&args)? {
        Ok(job) => Some(usb_format::privileged::run(&job)),
        Err(msg) => {
            eprintln!("sift: {msg}");
            Some(usb_format::privileged::EXIT_BAD_ARGS)
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn run_privileged_if_asked() -> Option<i32> {
    None
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be the FIRST plugin. A second launch focuses the running window instead of
        // opening a rival instance — two Sift processes on one SQLite DB + file-moving
        // pipeline risks corruption.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            // plugins.updater only exists in tauri.release.conf.json, merged in by `--config` on
            // the signed release build (.github/workflows/release.yml) — NOT by `npm run tauri
            // build` (.github/workflows/build.yml, unsigned CI smoke-build) nor `tauri dev`. This
            // isn't a debug/release split: cfg!(debug_assertions) can't see whether `--config` was
            // passed, so gating on it just moves the crash from `tauri dev` (caught immediately,
            // 2026-07-24) to every unsigned release-mode build. Fail-fast stays the rule
            // (.claude/rules/rust.md) for anything this process doesn't itself control the cause
            // of: only the ONE specific, classified "config absent" failure is tolerated below
            // (is_missing_updater_config) — a genuine init failure on the signed release build
            // (malformed pubkey, bad endpoints...) still propagates and crashes setup() as before,
            // exactly where it must be loud. log::warn! is a documented best-effort here, not the
            // safety net: tauri_plugin_log itself is debug-only (this same match arm), so a
            // release build's warning is a no-op by construction — the real guardrail is the
            // classifier being narrow enough that anything unexpected still fails hard.
            if let Err(e) = app.handle().plugin(tauri_plugin_updater::Builder::new().build()) {
                if is_missing_updater_config(&e) {
                    log::warn!("tauri_plugin_updater not registered (expected without plugins.updater config, e.g. unsigned/dev builds): {e}");
                } else {
                    return Err(e.into());
                }
            }
            ffmpeg::init_ffmpeg_path();
            // Ces trois étapes conditionnent toute l'application : sans dossier de données, sans
            // base ouverte ou sans identifiant de session, rien de ce qui suit n'a de sens. Elles
            // doivent donc arrêter le démarrage — mais par le `?` de `setup`, qui remonte un
            // message, et non par un `expect` qui rend une pile d'appels à l'utilisateur.
            // `.claude/rules/rust.md` : `expect()` hors test est un interdit dur.
            let dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("dossier de données de l'application introuvable: {e}"))?;
            std::fs::create_dir_all(&dir)
                .map_err(|e| format!("création du dossier de données {} impossible: {e}", dir.display()))?;
            let db_path = dir.join("sift.db");
            let mut conn = db::open(&db_path)
                .map_err(|e| format!("ouverture de la base {} impossible: {e}", db_path.display()))?;
            // Re-verdict depuis les mesures DÉJÀ stockées, sans ré-analyse (2026-09-01, bump
            // `VERDICT_CACHE_VERSION` 1 → 2). Ici et pas ailleurs : la connexion est ouverte,
            // migrée, et encore exclusive — aucun thread du pool n'existe. Idempotente : quand tout
            // est déjà à la version courante elle ne lit aucune ligne et ne journalise rien. Coût du
            // pire cas MESURÉ (`cout_de_la_passe_sur_3386_lignes`, `--release`, 2026-09-01) :
            // 420 / 433 / 434 ms pour 3 386 lignes toutes à re-juger — un bump de version, donc.
            //
            // L'échec ne fait PAS tomber le démarrage, contrairement aux trois étapes ci-dessus :
            // une base lisible dont le re-verdict échoue reste une app utilisable — les lignes
            // concernées gardent l'état « pas de verdict courant » que toute l'UI sait afficher, et
            // la passe se rejouera au lancement suivant.
            if let Err(e) = reverdict::run_and_log(&mut conn) {
                log::warn!("passe de re-verdict au démarrage échouée (rejouée au prochain lancement): {e}");
            }
            // Ce que la passe n'a PAS pu réparer et que le pool va reprendre après la file (issue
            // #59) : dit une fois au démarrage, pour que ce silence ne dure plus une semaine.
            match worker::count_filed_needing_analysis(&conn) {
                Ok(0) => {}
                Ok(n) => log::info!(
                    "{n} piste(s) rangée(s) sans verdict courant : reprises par le pool d'analyse après la file"
                ),
                Err(e) => log::warn!("compte des pistes rangées sans verdict courant impossible: {e}"),
            }
            let session_id = format!(
                "{}-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis(),
                std::process::id()
            );
            settings::set(&conn, settings::CURRENT_SESSION_ID, &session_id)
                .map_err(|e| format!("écriture de l'identifiant de session impossible: {e}"))?;
            app.manage(Mutex::new(conn));
            app.manage(ipc_filing::FilingCancel::default());
            // Le scope du protocole `asset:` part VIDE (`tauri.conf.json`) et se remplit ici, au
            // strict nécessaire. Il valait `["**"]`, c'est-à-dire : n'importe quel fichier de la
            // machine lisible depuis la webview. Sur une app qui affiche des tags de fichiers
            // inconnus — et qui a déjà livré un XSS stocké une fois — ça transforme la moindre
            // injection en lecture de `~/.ssh/id_rsa`. Deux sources seulement :
            //   - les pochettes, écrites par Sift dans son propre cache (dossier entier, borné) ;
            //   - le fichier audio en cours de lecture, autorisé UN PAR UN par `playback_url`.
            // Le `?` est délibéré : sans pochettes lisibles l'app est visiblement cassée, ce qui
            // vaut mieux qu'un écran muet dont personne ne trouve la cause.
            let covers = app
                .path()
                .app_cache_dir()
                .map_err(|e| format!("dossier de cache de l'application introuvable: {e}"))?
                .join("covers");
            std::fs::create_dir_all(&covers).map_err(|e| {
                format!("création du cache de pochettes {} impossible: {e}", covers.display())
            })?;
            app.asset_protocol_scope()
                .allow_directory(&covers, false)
                .map_err(|e| format!("autorisation du cache de pochettes impossible: {e}"))?;
            watcher::init_state(app.handle());
            watcher::start_all(app.handle());
            worker::init(app.handle());
            worker::refill(app.handle());
            spawn_journal_purge(app.handle());
            #[cfg(windows)]
            if let Some(w) = app.get_webview_window("main") {
                extend_frame_into_client_area(&w);
                round_window_corners(&w);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::app_info,
            ipc::db_health,
            ipc::ffmpeg_version,
            ipc::report_smoke,
            ipc::add_source,
            ipc::list_sources,
            ipc::remove_source,
            ipc::list_queue,
            ipc::rescan_source,
            ipc::set_source_watched,
            ipc::set_source_color,
            ipc::analyze_path,
            ipc::reanalyze_tracks,
            ipc::analysis_progress,
            ipc::import_paths,
            ipc::open_url,
            ipc::playback_url,
            ipc_filing::reconcile,
            ipc_filing::preview_filename,
            ipc_filing::track_release,
            ipc_filing::track_file_tags,
            ipc_filing::apply_tags,
            ipc_filing::file_track,
            ipc_filing::file_batch,
            ipc_filing::file_cancel,
            ipc_filing::reject_track,
            ipc_filing::reject_batch,
            ipc_filing::trash_track,
            ipc_filing::reveal_track,
            ipc_filing::list_bins,
            ipc_filing::undo_last,
            ipc_filing::revert_batch,
            ipc_filing::list_journal,
            ipc_filing::get_session_id,
            ipc_filing::get_setting,
            ipc_filing::set_setting,
            ipc_filing::list_ecartes,
            ipc_filing::restore_track,
            ipc_filing::requeue_track,
            ipc_filing::purge_trash,
            ipc_filing::find_duplicate,
            ipc_identify::identify,
            ipc_identify::apply_identity_cmd,
            ipc_identify::verify_discogs_token,
            ipc_library::list_library,
            ipc_library::library_folders,
            ipc_library::update_metadata,
            ipc_library::scan_library_duplicates,
            ipc_library::library_stats,
            ipc_library::link_rekordbox_xml,
            ipc_library::rekordbox_status,
            ipc_library::export_rekordbox_xml,
            ipc_library::rekordbox_masterdb_pending_repairs,
            ipc_library::rekordbox_masterdb_apply_repairs,
            ipc_library::rekordbox_masterdb_scan_playlist_duplicates,
            ipc_library::rekordbox_masterdb_dedup_playlist_group,
            ipc_library::rekordbox_masterdb_dismiss_repair,
            ipc_library::rekordbox_masterdb_resolve_ambiguous,
            ipc_library::rekordbox_masterdb_pending_metadata_syncs,
            ipc_library::rekordbox_masterdb_apply_metadata_syncs,
            ipc_library::rekordbox_masterdb_dismiss_metadata_sync,
            ipc_library::rekordbox_masterdb_resolve_ambiguous_metadata_sync,
            ipc_library::rekordbox_masterdb_pending_artwork_syncs,
            ipc_library::rekordbox_masterdb_apply_artwork_syncs,
            ipc_library::rekordbox_masterdb_dismiss_artwork_sync,
            ipc_library::rekordbox_masterdb_resolve_ambiguous_artwork_sync,
            ipc_usb::list_removable_drives,
            ipc_usb::format_drive,
            ipc_usb::eject_drive,
            ipc_usb::format_step,
            ipc_usage::drive_usage,
            ipc_usage::library_usage,
            dev_locate::locate_source,
            dev_annotate::save_annotation
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod updater_config_classification_tests {
    use super::is_missing_updater_config;

    // Verbatim from a real `tauri dev` run (2026-07-24, captured in
    // scratchpad/tauri-dev2.log): "[WARN] tauri_plugin_updater not registered (...): failed to
    // initialize plugin `updater`: Error deserializing 'plugins.updater' within your Tauri
    // configuration: invalid type: null, expected struct Config" — not hand-approximated.
    const ABSENT_CONFIG_MESSAGE: &str = "Error deserializing 'plugins.updater' within your Tauri configuration: invalid type: null, expected struct Config";

    #[test]
    fn absent_config_is_recognized_as_expected() {
        let err =
            tauri::Error::PluginInitialization("updater".into(), ABSENT_CONFIG_MESSAGE.into());
        assert!(is_missing_updater_config(&err));
    }

    #[test]
    fn other_plugin_missing_config_is_not_misclassified() {
        // Same shape of error, different plugin — must not match on a loose two-part contains().
        let err = tauri::Error::PluginInitialization(
            "some-other-plugin".into(),
            "Error deserializing 'plugins.some-other-plugin' within your Tauri configuration: invalid type: null, expected struct Config".into(),
        );
        assert!(!is_missing_updater_config(&err));
    }

    #[test]
    fn genuine_init_failure_without_null_stays_fail_fast() {
        let err = tauri::Error::PluginInitialization(
            "updater".into(),
            "Error deserializing 'plugins.updater': invalid value for 'pubkey': malformed base64"
                .into(),
        );
        assert!(!is_missing_updater_config(&err));
    }

    #[test]
    fn null_subfield_on_an_otherwise_present_config_is_not_misclassified() {
        // The exact gap verify-gate caught in an earlier version of this classifier: a plugin
        // present in tauri.release.conf.json with ONE null field still contains both "updater"
        // and "invalid type: null" somewhere in its message, but is NOT the same failure as the
        // whole `plugins.updater` key being absent — a signed release build with this error is
        // genuinely misconfigured and must still crash setup(), not be silently swallowed.
        let err = tauri::Error::PluginInitialization(
            "updater".into(),
            "Error deserializing 'plugins.updater.pubkey' within your Tauri configuration: invalid type: null, expected a string".into(),
        );
        assert!(!is_missing_updater_config(&err));
    }
}
