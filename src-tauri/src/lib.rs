mod actions;
pub mod analysis;
mod db;
mod ecartes;
mod encode;
mod ffmpeg;
mod filing;
mod ipc;
mod ipc_filing;
mod library;
mod naming;
mod queue;
mod scanner;
mod settings;
mod sources;
mod tagging;
mod watcher;
mod worker;

use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            ffmpeg::init_ffmpeg_path();
            let dir = app.path().app_data_dir().expect("no app data dir");
            std::fs::create_dir_all(&dir).ok();
            let conn = db::open(&dir.join("sift.db")).expect("db open failed");
            app.manage(Mutex::new(conn));
            watcher::init_state(app.handle());
            watcher::start_all(app.handle());
            worker::init(app.handle());
            worker::refill(app.handle());
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
            ipc::analyze_path,
            ipc::analysis_progress,
            ipc::import_paths,
            ipc::open_url,
            ipc_filing::reconcile,
            ipc_filing::file_track,
            ipc_filing::file_batch,
            ipc_filing::reject_track,
            ipc_filing::trash_track,
            ipc_filing::list_bins,
            ipc_filing::create_bin,
            ipc_filing::undo_last,
            ipc_filing::revert_batch,
            ipc_filing::list_journal,
            ipc_filing::get_setting,
            ipc_filing::set_setting,
            ipc_filing::list_ecartes,
            ipc_filing::restore_track,
            ipc_filing::purge_trash
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
