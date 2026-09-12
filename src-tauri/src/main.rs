// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Le mode privilégié court-circuite tout : relancé en administrateur pour formater un disque,
    // Sift ne doit ouvrir aucune fenêtre ni toucher la base. Testé AVANT `run()` pour cette raison.
    if let Some(code) = sift_lib::run_privileged_if_asked() {
        std::process::exit(code);
    }
    sift_lib::run();
}
