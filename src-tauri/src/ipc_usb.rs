//! IPC surface for the M7 "Formater une clé USB" utility. Selects the platform backend via
//! `#[cfg(target_os = ...)]` at this single call site — the two backends themselves never
//! branch on OS internally (see `usb_format::windows`/`usb_format::macos`).

use crate::usb_format::{self, RemovableDrive, RemovableDriveBackend, TargetFs};

use usb_format::backend_for_this_os as backend;

/// List drives Sift is confident are removable (conservative filter — see backend docs).
#[tauri::command]
pub fn list_removable_drives() -> Result<Vec<RemovableDrive>, String> {
    backend().list().map_err(|e| e.to_string())
}

/// Étape courante du formatage privilégié, pour que l'écran montre ce qui se passe.
///
/// Lu depuis un fichier parce que le travail se fait dans un AUTRE processus, élevé, dont la
/// sortie ne peut pas être redirigée. Une chaîne vide veut dire « rien en cours ».
#[tauri::command]
pub fn format_step() -> String {
    #[cfg(target_os = "windows")]
    {
        std::fs::read_to_string(usb_format::privileged::step_file()).unwrap_or_default()
    }
    #[cfg(not(target_os = "windows"))]
    {
        String::new()
    }
}

/// Démonte `drive_id` pour qu'il puisse être débranché sans risque.
///
/// Pas de garde anti-course ici, contrairement au formatage : éjecter le mauvais disque est
/// contrariant, pas destructeur, et exiger une identité rendrait l'action impossible juste après
/// un formatage — qui change précisément cette identité.
///
/// `"EJECT_BUSY"` quand le système refuse : rien n'a été démonté, donc débrancher reste risqué.
#[tauri::command]
pub fn eject_drive(drive_id: String) -> Result<(), String> {
    let b = backend();
    let fresh = b.list().map_err(|e| e.to_string())?;
    let drive = fresh
        .into_iter()
        .find(|d| d.id == drive_id)
        // Déjà absent : c'est le résultat voulu, pas une erreur à afficher.
        .ok_or_else(|| usb_format::DRIVE_VANISHED.to_string())?;
    b.eject(&drive).map_err(|e| e.to_string())
}

/// Format `drive_id` to `fs`. `identity` must match what the frontend last saw for this drive —
/// re-checked against a fresh listing immediately before formatting (anti-race guard); fails with
/// `"IDENTITY_MISMATCH"` or `"DRIVE_VANISHED"` if a different drive now answers to the same id
/// (e.g. a USB stick was swapped between listing and confirmation).
///
/// The disk actually formatted is `confirmed` — the entry from the **fresh** listing, never the
/// caller's. `drive_id` only selects; every field `format` acts on is backend-produced.
#[tauri::command]
pub fn format_drive(
    drive_id: String,
    identity: String,
    fs: TargetFs,
    label: String,
) -> Result<(), String> {
    let b = backend();
    let fresh = b.list().map_err(|e| e.to_string())?;
    let candidate = RemovableDrive {
        id: drive_id,
        label: String::new(),
        mount: String::new(),
        size_bytes: 0,
        free_bytes: 0,
        current_fs: String::new(),
        volume_name: String::new(),
        health: String::new(),
        has_media: false,
        identity,
    };
    usb_format::verify_identity_unchanged(&candidate, &fresh).map_err(|e| e.to_string())?;
    let confirmed = fresh
        .into_iter()
        .find(|d| d.id == candidate.id)
        .ok_or_else(|| usb_format::UsbFormatError::DriveVanished.to_string())?;
    // An enumerated-but-empty card reader is listable (so the UI can say why it is useless) but
    // never formattable — `diskpart` would fail on it anyway, and failing here says why.
    if !confirmed.has_media {
        return Err("Aucun média dans ce lecteur — rien à formater.".to_string());
    }

    // Tout ce qui précède est instantané et doit échouer AVANT qu'on parte : garde anti-course,
    // disque introuvable, lecteur vide. Ce qui suit dure — attente de l'invite UAC, partitionnement,
    // écriture — et part donc sur son propre fil.
    //
    // Une commande Tauri synchrone qui attend bloque le service des AUTRES commandes : tant que
    // `format_drive` attendait la fin du processus élevé, `format_step` ne pouvait pas être servie
    // et l'interface entière était figée. L'affichage des étapes ne pouvait pas fonctionner par
    // construction — c'est le gel constaté au premier formatage réel, pas une lenteur.
    #[cfg(target_os = "windows")]
    {
        usb_format::privileged::write_step("Autorisation Windows demandée…");
        std::thread::spawn(move || {
            let outcome = backend().format(&confirmed, fs, &label);
            // L'état terminal passe par le fichier d'étape, seul canal que le frontend interroge :
            // ce fil n'a personne à qui répondre, la commande a déjà rendu la main.
            match outcome {
                Ok(()) => usb_format::privileged::write_step(usb_format::privileged::STEP_DONE),
                Err(e) => usb_format::privileged::write_step(&format!(
                    "{} : {e}",
                    usb_format::privileged::STEP_FAILED_PREFIX
                )),
            }
        });
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        b.format(&confirmed, fs, &label).map_err(|e| e.to_string())
    }
}
