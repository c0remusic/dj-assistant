//! Formatage FAT32 **au-delà du plafond de 32 Go de Windows** — la raison d'être de l'écran
//! Clé USB, et la seule chose qu'aucun outil livré avec Windows ne sait faire.
//!
//! Le plafond est celui de Microsoft, pas celui de FAT32 : la spécification autorise jusqu'à
//! 2 To, et c'est le pilote de formatage qui refuse au-delà de 32 Go — `format.com`, l'assistant
//! de l'explorateur et `diskpart` le subissent tous les trois (vérifié 2026-07-31 : un volume de
//! 64 Go répond « The volume is too big for FAT32 »).
//!
//! On écrit donc les structures nous-mêmes, via le crate `fatfs` — MIT, comme Sift. Pas de binaire
//! tiers embarqué, pas d'obligation GPL, et pas de système de fichiers écrit à la main.
//!
//! **Le même code sert le test et la production.** `fatfs::format_volume` travaille sur n'importe
//! quel `Read + Write + Seek` : un fichier d'image et un handle de volume brut passent exactement
//! par le même chemin. Valider sur image ne teste donc pas une variante, mais la chose elle-même.

//! ⚠️ **Rien n'appelle encore ce module en production**, et c'est délibéré plutôt qu'oublié.
//! Écrire ces structures sur une vraie clé demande d'ouvrir le volume brut (`\\.\X:`) en écriture,
//! ce que Windows réserve à l'administrateur — comme `diskpart`. Le shim d'élévation existe déjà
//! (`elevation_powershell`, commit 163720e) mais il élève `diskpart`, pas notre propre code. Il
//! manque donc une étape : que Sift se relance élevé sur un drapeau dédié pour ce seul écrit, puis
//! rende la main. Câbler `format()` dessus avant d'avoir ça livrerait un chemin qui échoue
//! systématiquement en « accès refusé », ce qui serait pire que le refus explicite d'aujourd'hui.
//!
//! Ce qui EST acquis : les structures sont écrites correctement au-delà de 32 Go, vérifié sur
//! image de 40 Go — type FAT32, clusters de 32 Kio, signature de secteur, et un volume qui accepte
//! puis rend des fichiers.
#![allow(dead_code)]

use fatfs::{FatType, FormatVolumeOptions};
use std::io::{Read, Seek, Write};

/// Plafond que Windows impose à la CRÉATION d'un volume FAT32. Il ne limite ni la lecture ni
/// l'écriture : Windows monte parfaitement un FAT32 de 500 Go, il refuse seulement d'en fabriquer
/// un. C'est cette asymétrie qui rend l'écran utile.
pub const WINDOWS_FAT32_CREATE_CEILING: u64 = 32 * 1024 * 1024 * 1024;

/// Taille de secteur supposée. 512 partout sur les clés et disques USB courants ; les modèles 4Kn
/// existent mais aucun n'est passé entre les mains de ce projet, et supposer 512 sur un 4Kn
/// produirait un volume faux. À rendre dynamique le jour où un tel disque apparaît.
pub const BYTES_PER_SECTOR: u16 = 512;

#[derive(Debug, Clone, PartialEq)]
pub enum Fat32Error {
    /// Le volume dépasse ce que FAT32 sait adresser, quel que soit l'outil.
    TooLarge(u64),
    /// Échec d'écriture des structures.
    Write(String),
}

impl std::fmt::Display for Fat32Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Fat32Error::TooLarge(bytes) => write!(
                f,
                "volume de {bytes} octets : au-delà de ce que FAT32 peut adresser"
            ),
            Fat32Error::Write(m) => write!(f, "écriture FAT32: {m}"),
        }
    }
}

impl std::error::Error for Fat32Error {}

/// Limite réelle de FAT32 avec des secteurs de 512 octets : `u32::MAX` secteurs, soit 2 Tio.
/// C'est `fatfs::format_volume` qui la fait respecter, mais on la vérifie AVANT d'ouvrir quoi que
/// ce soit — refuser tôt vaut mieux qu'échouer après avoir touché le disque.
pub const FAT32_MAX_BYTES: u64 = (u32::MAX as u64) * (BYTES_PER_SECTOR as u64);

/// Nombre de secteurs pour un volume de `bytes` octets, ou `None` s'il dépasse FAT32.
pub fn total_sectors_for(bytes: u64) -> Option<u32> {
    let sectors = bytes / u64::from(BYTES_PER_SECTOR);
    u32::try_from(sectors).ok().filter(|s| *s > 0)
}

/// Nom de volume FAT32 : 11 octets, majuscules, complété d'espaces.
///
/// Tronqué plutôt que refusé — un nom trop long est une contrariété, pas une raison de ne pas
/// formater. Les octets non-ASCII sont remplacés : la page de code d'un FAT32 lu par un CDJ n'est
/// pas la nôtre, et un accent mal encodé s'y affiche en charabia.
pub fn volume_label(label: &str) -> [u8; 11] {
    let mut out = [b' '; 11];
    for (slot, ch) in out.iter_mut().zip(label.chars()) {
        *slot = if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            ch.to_ascii_uppercase() as u8
        } else {
            b'_'
        };
    }
    out
}

/// Écrit un système de fichiers FAT32 sur `target`, qui doit être positionné au début du volume.
///
/// `total_bytes` est la taille du VOLUME, pas celle du support : en production le support est un
/// handle de volume brut dont la fin est celle de la partition, en test un fichier bien plus
/// petit que le volume déclaré. Le passer explicitement évite que `fatfs` déduise la taille en
/// cherchant la fin du fichier, ce qui rendrait le test dépendant d'une allocation réelle.
///
/// La taille de cluster n'est PAS imposée : `fatfs` la choisit selon la taille du volume et
/// plafonne à 32 Kio, exactement la valeur que porte déjà un disque DJ formaté par un outil tiers.
/// La forcer serait plus fragile que la laisser faire.
pub fn write_fat32<T: Read + Write + Seek>(
    target: T,
    total_bytes: u64,
    label: &str,
) -> Result<(), Fat32Error> {
    let sectors = total_sectors_for(total_bytes).ok_or(Fat32Error::TooLarge(total_bytes))?;
    let options = FormatVolumeOptions::new()
        .fat_type(FatType::Fat32)
        .bytes_per_sector(BYTES_PER_SECTOR)
        .total_sectors(sectors)
        .volume_label(volume_label(label));
    fatfs::format_volume(target, options).map_err(|e| Fat32Error::Write(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Seek, SeekFrom};

    /// Un volume de 40 Go : au-delà du plafond de 32 Go que Windows oppose, donc le cas que cet
    /// écran existe pour traiter. Adossé à un fichier temporaire qui ne grandit qu'à la mesure de
    /// ce qui est réellement écrit (~10 Mo de FAT), pas à 40 Go.
    fn format_and_reopen(total_bytes: u64, label: &str) -> fatfs::FileSystem<std::fs::File> {
        let tmp = tempfile::NamedTempFile::new().expect("tempfile");
        let path = tmp.path().to_path_buf();
        {
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
                .expect("open rw");
            write_fat32(file, total_bytes, label).expect("format");
        }
        let reopened = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .expect("reopen");
        // La NamedTempFile est volontairement gardée vivante par le retour : `into_file` la
        // detacherait de la suppression automatique.
        std::mem::forget(tmp);
        fatfs::FileSystem::new(reopened, fatfs::FsOptions::new()).expect("mount")
    }

    /// LE test de cette fonctionnalité. Windows refuse de créer ce volume ; nous le créons.
    #[test]
    fn formats_beyond_the_windows_ceiling() {
        let forty_gb = 40 * 1024 * 1024 * 1024u64;
        assert!(
            forty_gb > WINDOWS_FAT32_CREATE_CEILING,
            "le test doit porter sur un volume que Windows refuse"
        );
        let fs = format_and_reopen(forty_gb, "SIFT_TEST");
        assert_eq!(fs.fat_type(), FatType::Fat32);
    }

    /// 32 Kio est la valeur de compatibilité maximale, et celle que porte déjà un disque DJ
    /// formaté par un outil tiers. Au-delà, `fatfs` lui-même avertit d'incompatibilité.
    #[test]
    fn picks_a_cdj_compatible_cluster_size() {
        let fs = format_and_reopen(40 * 1024 * 1024 * 1024u64, "SIFT_TEST");
        let cluster = fs.cluster_size();
        assert_eq!(cluster, 32 * 1024, "taille de cluster obtenue: {cluster}");
    }

    /// Formater n'est pas écrire un secteur d'amorçage : le volume doit réellement accepter des
    /// fichiers et les rendre. C'est ce qu'un CDJ fera.
    #[test]
    fn the_formatted_volume_actually_holds_files() {
        let fs = format_and_reopen(40 * 1024 * 1024 * 1024u64, "SIFT_TEST");
        let root = fs.root_dir();
        {
            let mut f = root.create_file("PISTE.TXT").expect("create");
            f.write_all(b"contenu").expect("write");
        }
        let mut back = Vec::new();
        root.open_file("PISTE.TXT")
            .expect("open")
            .read_to_end(&mut back)
            .expect("read");
        assert_eq!(back, b"contenu");
    }

    #[test]
    fn volume_label_is_written_and_read_back() {
        let fs = format_and_reopen(40 * 1024 * 1024 * 1024u64, "SIFT_TEST");
        assert_eq!(fs.volume_label().trim_end(), "SIFT_TEST");
    }

    /// Le secteur d'amorçage doit porter la signature 0x55AA, sans quoi aucun système ne
    /// reconnaît le volume — vérification indépendante de `fatfs`, qui vient de l'écrire.
    #[test]
    fn boot_sector_carries_the_signature() {
        let tmp = tempfile::NamedTempFile::new().expect("tempfile");
        let path = tmp.path().to_path_buf();
        {
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
                .expect("open");
            write_fat32(file, 40 * 1024 * 1024 * 1024u64, "SIFT").expect("format");
        }
        let mut file = std::fs::File::open(&path).expect("read");
        let mut boot = [0u8; 512];
        file.seek(SeekFrom::Start(0)).expect("seek");
        file.read_exact(&mut boot).expect("read boot");
        assert_eq!(
            &boot[510..512],
            &[0x55, 0xAA],
            "signature de secteur absente"
        );
        // "FAT32   " à l'offset 82 : le type déclaré par le BPB lui-même.
        assert_eq!(&boot[82..87], b"FAT32");
    }

    /// Accents ET espaces deviennent `_`. L'espace est autorisé par la spécification, mais un nom
    /// de volume a espaces se retape mal sur un CDJ et complique toute commande qui le manipule.
    /// La règle est donc sans exception à retenir : alphanumérique ASCII, `_` et `-` passent, tout
    /// le reste devient `_`.
    #[test]
    fn label_is_uppercased_padded_and_stripped_of_accents() {
        assert_eq!(&volume_label("sift"), b"SIFT       ");
        assert_eq!(&volume_label("Cl\u{e9} DJ"), b"CL__DJ     ");
        assert_eq!(&volume_label("UN_NOM_BEAUCOUP_TROP_LONG"), b"UN_NOM_BEAU");
    }

    #[test]
    fn refuses_a_volume_beyond_what_fat32_addresses() {
        let too_big = FAT32_MAX_BYTES + u64::from(BYTES_PER_SECTOR);
        assert_eq!(total_sectors_for(too_big), None);
    }

    #[test]
    fn two_tebibytes_is_still_addressable() {
        assert!(total_sectors_for(FAT32_MAX_BYTES).is_some());
    }
}
