//! Removable-drive USB formatting utility (M7). Two platform backends behind a shared trait
//! (`usb_format::windows`, `usb_format::macos`), never a `cfg!(windows)` branch inside one
//! function — the macOS path cannot be exercised here (no Mac available), so isolating it
//! behind its own file/impl keeps the blast radius of an untested path to one file.
//!
//! Conservative-by-design: any backend that cannot positively confirm a disk is removable
//! must exclude it from `list()` rather than include it. Never show an internal disk, even
//! by a detection bug — see `is_confidently_removable` in each backend and its tests.

use serde::{Deserialize, Serialize};

/// Formatage FAT32 au-delà du plafond de 32 Go de Windows.
///
/// Gate `any(windows, test)` et non `windows` seul, pour tenir deux exigences à la fois :
///
/// - **macOS n'en a aucun usage.** Il formate par `diskutil eraseDisk FAT32` (`macos.rs`), qui
///   ne connaît pas le plafond de 32 Go — ce module n'existe QUE pour contourner Windows. Ses
///   seuls appelants, `privileged.rs` et `windows.rs`, sont eux-mêmes gates Windows. Sans gate,
///   le binaire macOS compilait et liait `fatfs` pour du code que rien n'y appelle.
/// - **Les tests doivent tourner sur n'importe quelle machine.** C'était la raison d'être de
///   l'absence de gate ; `test` la préserve telle quelle. `fatfs` est donc déclaré deux fois
///   dans `Cargo.toml` : en dépendance `cfg(windows)` pour la production, et en
///   dev-dépendance pour que ces tests compilent partout.
#[cfg(any(target_os = "windows", test))]
pub mod fat32;

#[cfg(target_os = "windows")]
pub mod privileged;

#[cfg(target_os = "windows")]
pub mod raw_volume;

/// Alignement secteur pour les écritures sur volume brut. Même gate que `fat32` ci-dessus, et
/// pour la même raison : son unique appelant est `privileged.rs`, gate Windows. La logique est
/// pure (`std::io` seul, aucune dépendance externe), donc `test` suffit à garder ses tests sur
/// n'importe quelle machine.
#[cfg(any(target_os = "windows", test))]
pub mod sector_io;

#[cfg(target_os = "windows")]
pub mod windows;

// NOT OS-gated at the module level (unlike `windows` above): the macOS backend's pure
// parsing/filter functions must compile and run under `cargo test` on any OS, including this
// Windows dev machine, so they're actually verified without access to a Mac. Only the
// `RemovableDriveBackend` impl inside `macos.rs` (which shells out to `diskutil`) is gated
// `#[cfg(target_os = "macos")]` — see that file's module doc.
pub mod macos;

/// A drive Sift considers safe to offer for formatting: passed the removable filter.
///
/// `id` identifies a **physical disk** (`\\.\PHYSICALDRIVE2`, `/dev/disk4`), never a mounted
/// volume — a brand-new or RAW key has no volume, and that is precisely what this tool formats.
///
/// `identity` is the anti-race anchor, re-checked immediately before formatting (see
/// `verify_identity_unchanged`) in case a different disk answers to the same id between listing
/// and confirmation. It was `volume_serial` until 2026-07-31; a volume serial cannot exist for an
/// unformatted disk, so anchoring on it excluded every disk the user most needed to format. It is
/// opaque to the frontend, which round-trips the string and never parses it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RemovableDrive {
    pub id: String,
    pub label: String,
    /// Drive letter(s)/mount point(s), for display only. Empty for a disk with no mounted volume
    /// — an empty string here is the signal that the disk is RAW, not that lookup failed.
    pub mount: String,
    pub size_bytes: u64,
    /// Octets libres sur les volumes montés, `0` si le disque n'en a aucun. Affiché tel quel, et
    /// réutilisé comme clé d'invalidation du cache d'occupation : un espace libre différent
    /// signifie un contenu différent, donc un parcours à refaire.
    pub free_bytes: u64,
    pub current_fs: String,
    /// Nom du volume actuel (`"DJERMUSIQUE"`), vide si le disque n'est pas formaté. Sert de valeur
    /// par défaut au champ de nom de la modale : reformater une clé en gardant son nom est le cas
    /// courant, le retaper à chaque fois serait une corvée.
    pub volume_name: String,
    /// État de santé du volume, déjà formulé en français par le backend. Vide quand il n'y a aucun
    /// volume monté à interroger — un disque RAW n'a pas de santé de système de fichiers.
    pub health: String,
    /// `false` for a card reader / drive bay that is enumerated but empty. Such a device is a
    /// real removable disk with a real drive letter — Windows keeps showing `E:` in Explorer with
    /// nothing inserted — but there is nothing to format in it. Listing it and saying so beats
    /// hiding it: "no drive detected" while Explorer shows a USB drive letter is exactly the
    /// contradiction that cost an evening on 2026-07-31.
    pub has_media: bool,
    pub identity: String,
}

/// Target filesystem for formatting. FAT32 is the default (spec: bypasses the Windows GUI's
/// 32 GB `format.com` ceiling). exFAT is a secondary, explicit-only choice (CDJ compatibility
/// warning lives in the frontend modal, not here).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetFs {
    Fat32,
    ExFat,
}

/// Why listing or formatting could not complete. Stable string tags (mirrors the
/// `FilingError`/`"RAIL_MISMATCH"` convention) so the frontend can pattern-match distinctly.
#[derive(Debug, Clone, PartialEq)]
pub enum UsbFormatError {
    /// The drive last seen at this id/serial is no longer present at format time.
    DriveVanished,
    /// The anti-race check: the volume serial at format time doesn't match what was listed.
    IdentityMismatch,
    /// Backend-specific enumeration failure (WMI/diskutil call failed, unparseable output).
    Enumeration(String),
    /// Backend-specific format failure (diskpart/diskutil exited non-zero or errored).
    Format(String),
    /// The user dismissed the OS elevation prompt. Nothing was touched.
    ElevationDeclined,
    /// Éjection refusée : le volume est encore utilisé. Rien n'a été démonté.
    EjectBusy,
}

/// Le disque attendu n'est plus branché. Sentinelle traversant l'IPC — miroir de
/// `shared/contracts.ts`, tenu par `usb_format_sentinels_match_contracts_ts`.
pub const DRIVE_VANISHED: &str = "DRIVE_VANISHED";

/// Un AUTRE disque répond maintenant à cette lettre : le numéro de série ne correspond plus à
/// celui que l'utilisateur a confirmé. Même contrat de miroir que ci-dessus. Un formatage est
/// irréversible : c'est la sentinelle qui doit interdire le « réessaie », pas l'inviter.
pub const IDENTITY_MISMATCH: &str = "IDENTITY_MISMATCH";

/// L'utilisateur a fermé l'invite d'élévation Windows (UAC). Même contrat de miroir. Formater un
/// disque exige les droits administrateur — `diskpart` refuse même `list disk` depuis un process
/// utilisateur normal (mesuré le 2026-07-31) — donc Sift demande l'élévation pour cette seule
/// opération. Refuser est un choix délibéré, pas une panne : rien n'a été touché, et le message
/// doit le dire au lieu d'accuser le disque.
pub const ELEVATION_DECLINED: &str = "ELEVATION_DECLINED";

/// Le système a refusé de démonter le volume : un programme le tient encore ouvert. Même contrat
/// de miroir. C'est le cas FRÉQUENT d'une éjection, pas un cas limite — Rekordbox, une fenêtre de
/// l'explorateur ou un antivirus suffisent — et rien n'a été démonté, donc débrancher maintenant
/// reste risqué. Le message doit dire quoi fermer, pas « réessaie ».
pub const EJECT_BUSY: &str = "EJECT_BUSY";

impl std::fmt::Display for UsbFormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UsbFormatError::DriveVanished => write!(f, "{DRIVE_VANISHED}"),
            UsbFormatError::IdentityMismatch => write!(f, "{IDENTITY_MISMATCH}"),
            UsbFormatError::Enumeration(m) => write!(f, "enumeration: {m}"),
            UsbFormatError::Format(m) => write!(f, "format: {m}"),
            UsbFormatError::ElevationDeclined => write!(f, "{ELEVATION_DECLINED}"),
            UsbFormatError::EjectBusy => write!(f, "{EJECT_BUSY}"),
        }
    }
}

/// Per-OS enumeration + formatting. Two impls (`windows::WindowsBackend`,
/// `macos::MacBackend`), never a mixed `cfg!` branch inside one function.
pub trait RemovableDriveBackend {
    fn list(&self) -> Result<Vec<RemovableDrive>, UsbFormatError>;
    /// `label` est le nom donné au volume. Il est assaini par le backend (`fat32::volume_label`),
    /// jamais pris tel quel : FAT32 n'accepte que 11 octets majuscules.
    fn format(
        &self,
        drive: &RemovableDrive,
        fs: TargetFs,
        label: &str,
    ) -> Result<(), UsbFormatError>;
    /// Démonte le disque pour qu'il puisse être débranché sans risque.
    ///
    /// Doit VÉRIFIER que le disque a bien disparu avant de rendre `Ok` : sur les deux OS la
    /// demande d'éjection est asynchrone et réussit silencieusement même quand le système la
    /// refuse ensuite. Annoncer un succès non vérifié ici, c'est inviter à débrancher un volume
    /// encore monté.
    fn eject(&self, drive: &RemovableDrive) -> Result<(), UsbFormatError>;
}

/// Le seul endroit du dépôt qui choisit un backend. Extrait de `ipc_usb` quand `ipc_usage` en a eu
/// besoin à son tour : deux `#[cfg(target_os)]` à tenir d'accord, c'est déjà un de trop.
#[cfg(target_os = "windows")]
pub fn backend_for_this_os() -> impl RemovableDriveBackend {
    windows::WindowsBackend
}

#[cfg(target_os = "macos")]
pub fn backend_for_this_os() -> impl RemovableDriveBackend {
    macos::MacBackend
}

/// Anti-race guard: re-resolve `drive` by identity from a **fresh** listing (`fresh`, passed in
/// by the caller right before formatting) and fail explicitly if it's gone or its identity
/// changed — never fall back to "the id still matches, must be the same drive".
pub fn verify_identity_unchanged(
    drive: &RemovableDrive,
    fresh: &[RemovableDrive],
) -> Result<(), UsbFormatError> {
    match fresh.iter().find(|d| d.id == drive.id) {
        None => Err(UsbFormatError::DriveVanished),
        Some(d) if d.identity != drive.identity => Err(UsbFormatError::IdentityMismatch),
        Some(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Contract test, même famille que `filing.rs` : on parse le texte source de
    /// `shared/contracts.ts` et on assert que la valeur littérale des constantes Rust y apparaît.
    /// Ces deux-là traversaient l'IPC en littéraux recopiés à la main de chaque côté, et le côté
    /// TS ne les reconnaissait même pas — la modale invitait à réessayer un formatage sur un
    /// disque que le backend venait de déclarer différent.
    const CONTRACTS_TS: &str = include_str!("../../../shared/contracts.ts");

    #[test]
    fn usb_format_sentinels_match_contracts_ts() {
        for sentinel in [
            DRIVE_VANISHED,
            IDENTITY_MISMATCH,
            ELEVATION_DECLINED,
            EJECT_BUSY,
        ] {
            let expected = format!("\"{sentinel}\"");
            assert!(
                CONTRACTS_TS.contains(&expected),
                "shared/contracts.ts must contain {expected}"
            );
        }
    }

    /// ... et le `Display` doit RÉELLEMENT les émettre : sans ça les deux constantes peuvent
    /// rester d'accord entre elles pendant que le message envoyé au front n'en porte plus aucune.
    #[test]
    fn display_emits_the_sentinels_verbatim() {
        assert_eq!(UsbFormatError::DriveVanished.to_string(), DRIVE_VANISHED);
        assert_eq!(
            UsbFormatError::IdentityMismatch.to_string(),
            IDENTITY_MISMATCH
        );
        assert_eq!(
            UsbFormatError::ElevationDeclined.to_string(),
            ELEVATION_DECLINED
        );
        assert_eq!(UsbFormatError::EjectBusy.to_string(), EJECT_BUSY);
    }

    fn drive(id: &str, identity: &str) -> RemovableDrive {
        RemovableDrive {
            id: id.to_string(),
            label: "TEST".to_string(),
            mount: "E:".to_string(),
            size_bytes: 8_000_000_000,
            free_bytes: 4_000_000_000,
            current_fs: "FAT32".to_string(),
            volume_name: "TEST".to_string(),
            health: "OK".to_string(),
            has_media: true,
            identity: identity.to_string(),
        }
    }

    #[test]
    fn verify_identity_unchanged_ok_when_serial_matches() {
        let d = drive("E:", "AAAA-1111");
        let fresh = vec![drive("E:", "AAAA-1111")];
        assert_eq!(verify_identity_unchanged(&d, &fresh), Ok(()));
    }

    #[test]
    fn verify_identity_unchanged_fails_when_serial_changed() {
        let d = drive("E:", "AAAA-1111");
        let fresh = vec![drive("E:", "BBBB-2222")]; // same letter, different key was plugged in
        assert_eq!(
            verify_identity_unchanged(&d, &fresh),
            Err(UsbFormatError::IdentityMismatch)
        );
    }

    #[test]
    fn verify_identity_unchanged_fails_when_drive_vanished() {
        let d = drive("E:", "AAAA-1111");
        let fresh: Vec<RemovableDrive> = vec![];
        assert_eq!(
            verify_identity_unchanged(&d, &fresh),
            Err(UsbFormatError::DriveVanished)
        );
    }
}
