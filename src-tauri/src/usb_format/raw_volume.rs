//! Ouverture d'un volume Windows en écriture **brute**, pour y écrire un système de fichiers.
//!
//! C'est le code le plus dangereux du dépôt : il écrit directement dans un volume, sans filet de
//! système de fichiers. Trois protections, toutes obligatoires et aucune redondante.
//!
//! **1. Élévation.** Windows réserve l'ouverture d'un volume en écriture à l'administrateur. Sans
//! ça, `CreateFileW` échoue en « accès refusé » — donc un Sift non élevé ne peut pas se tromper de
//! disque, il ne peut rien ouvrir du tout.
//!
//! **2. Verrouillage puis démontage.** `FSCTL_LOCK_VOLUME` échoue si le moindre programme tient un
//! fichier ouvert sur ce volume, ce qui évite d'écraser un volume en cours d'utilisation.
//! `FSCTL_DISMOUNT_VOLUME` force ensuite Windows à oublier le système de fichiers, sans quoi le
//! cache réécrirait par-dessus nos structures.
//!
//! **3. Refus du disque système.** Une lettre de lecteur est une chaîne, et une chaîne se trompe.
//! `%SystemDrive%` est refusé ici, en plus des vérifications de l'appelant — cette fonction ne
//! fait aucune confiance à ce qu'on lui passe.

use std::fs::File;
use std::os::windows::io::FromRawHandle;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ,
    FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::Ioctl::{FSCTL_DISMOUNT_VOLUME, FSCTL_LOCK_VOLUME};
use windows::Win32::System::IO::DeviceIoControl;

#[derive(Debug, Clone, PartialEq)]
pub enum RawVolumeError {
    /// Refus délibéré : cette lettre porte le système.
    RefusedSystemVolume(String),
    /// `CreateFileW` a échoué — en pratique « accès refusé » quand le processus n'est pas élevé.
    Open(String),
    /// Le volume est utilisé par un autre programme.
    Locked(String),
    Dismount(String),
}

impl std::fmt::Display for RawVolumeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RawVolumeError::RefusedSystemVolume(l) => {
                write!(f, "{l} porte le système : écriture brute refusée")
            }
            RawVolumeError::Open(m) => write!(f, "ouverture du volume: {m}"),
            RawVolumeError::Locked(m) => write!(f, "volume verrouillé par un autre programme: {m}"),
            RawVolumeError::Dismount(m) => write!(f, "démontage impossible: {m}"),
        }
    }
}

impl std::error::Error for RawVolumeError {}

/// `"I:"` -> `r"\\.\I:"`, la forme que `CreateFileW` attend pour un volume.
///
/// Pure et testée : c'est la chaîne qui décide quel volume sera écrasé, elle ne doit pas se
/// construire au fil du code.
pub fn volume_path(letter: &str) -> String {
    format!(r"\\.\{}", letter.trim().trim_end_matches('\\'))
}

/// Refuse tout ce qui ressemble au disque système, insensiblement à la casse.
pub fn is_system_volume(letter: &str, system_drive: &str) -> bool {
    letter
        .trim()
        .trim_end_matches('\\')
        .eq_ignore_ascii_case(system_drive.trim())
}

/// Volume ouvert, verrouillé et démonté, prêt à recevoir un système de fichiers.
///
/// Le `File` rend `Read + Write + Seek`, exactement ce que `fatfs::format_volume` demande — donc
/// le même code écrit sur une image et sur un vrai disque.
pub struct RawVolume {
    file: File,
    handle: HANDLE,
}

impl RawVolume {
    /// Ouvre `letter` (`"I:"`), le verrouille et le démonte.
    ///
    /// Échoue plutôt que de forcer si un programme tient le volume : le `FSCTL_LOCK_VOLUME` est
    /// justement là pour ça, et le contourner reviendrait à écraser des fichiers ouverts.
    pub fn open(letter: &str) -> Result<Self, RawVolumeError> {
        let system_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
        if is_system_volume(letter, &system_drive) {
            return Err(RawVolumeError::RefusedSystemVolume(letter.to_string()));
        }

        let path = volume_path(letter);
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();

        // SAFETY: `wide` est une chaîne UTF-16 terminée par un zéro, vivante pendant tout l'appel.
        // Les autres arguments sont des constantes du crate `windows`. `CreateFileW` n'a pas
        // d'autre précondition ; son échec est signalé par la valeur de retour, testée juste après.
        let handle = unsafe {
            CreateFileW(
                PCWSTR(wide.as_ptr()),
                (FILE_GENERIC_READ | FILE_GENERIC_WRITE).0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            )
        }
        .map_err(|e| RawVolumeError::Open(e.to_string()))?;

        // SAFETY: `handle` vient d'être rendu valide par `CreateFileW` (l'erreur est propagée
        // ci-dessus) et n'est possédé par personne d'autre. `from_raw_handle` en prend la
        // propriété ; `handle` n'est plus refermé que par le `Drop` de ce type, qui ne ferme pas
        // le fichier lui-même.
        let file = unsafe { File::from_raw_handle(handle.0) };

        let vol = RawVolume { file, handle };
        vol.control(FSCTL_LOCK_VOLUME)
            .map_err(RawVolumeError::Locked)?;
        vol.control(FSCTL_DISMOUNT_VOLUME)
            .map_err(RawVolumeError::Dismount)?;
        Ok(vol)
    }

    fn control(&self, code: u32) -> Result<(), String> {
        let mut returned: u32 = 0;
        // SAFETY: `self.handle` est valide tant que ce `RawVolume` vit. Les deux tampons sont
        // absents (`None`) parce que ces deux codes de contrôle n'en prennent aucun ;
        // `returned` est une variable locale vivante pendant l'appel.
        unsafe {
            DeviceIoControl(
                self.handle,
                code,
                None,
                0,
                None,
                0,
                Some(&mut returned),
                None,
            )
        }
        .map_err(|e| e.to_string())
    }

    /// Le support à passer à `fatfs::format_volume`.
    pub fn as_file_mut(&mut self) -> &mut File {
        &mut self.file
    }
}

impl Drop for RawVolume {
    fn drop(&mut self) {
        // Le verrou tombe de lui-même à la fermeture du handle, que le `Drop` de `File` effectue —
        // `File` possède le handle depuis `from_raw_handle`. Pas de `CloseHandle` ici : le fermer
        // deux fois est un comportement indéfini.
        let _ = CloseHandle; // référence gardée pour documenter le choix, jamais appelée.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_path_uses_the_device_namespace() {
        assert_eq!(volume_path("I:"), r"\\.\I:");
        assert_eq!(volume_path("I:\\"), r"\\.\I:");
        assert_eq!(volume_path(" I: "), r"\\.\I:");
    }

    /// La lettre système est refusée quelle que soit sa casse ou sa ponctuation : c'est la
    /// dernière barrière avant une écriture brute, elle ne doit pas dépendre d'une forme exacte.
    #[test]
    fn system_volume_is_refused_in_any_form() {
        for l in ["C:", "c:", "C:\\", " c: "] {
            assert!(is_system_volume(l, "C:"), "{l} doit être refusé");
        }
    }

    #[test]
    fn other_letters_are_not_the_system_volume() {
        for l in ["I:", "D:", "E:"] {
            assert!(!is_system_volume(l, "C:"));
        }
    }

    /// `%SystemDrive%` n'est pas toujours C: — une installation sur D: doit protéger D:, pas C:.
    #[test]
    fn the_system_letter_is_read_not_assumed() {
        assert!(is_system_volume("D:", "D:"));
        assert!(!is_system_volume("C:", "D:"));
    }
}
