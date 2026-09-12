//! Windows backend: enumerate removable disks via WMI — `MSFT_Disk` for the bus type,
//! `Win32_DiskDrive`'s associations for the volume description — and format via a scripted
//! `diskpart`.
//!
//! ⚠️ **`diskpart` does NOT lift the 32 GB FAT32 ceiling**, et c'est pour ça que Windows ne
//! formate pas ici. La limite vit dans le pilote de formatage de Windows, partagé à l'identique
//! par `format.com`, `diskpart` et la boîte de dialogue de l'explorateur : une clé de 64 Go
//! répond « The volume is too big for FAT32 » à `format fs=fat32`.
//!
//! La décision est prise et livrée — Sift écrit le système de fichiers lui-même, avec
//! `fat32::write_fat32` (crate `fatfs`, MIT) à travers `SectorIo` qui aligne sur le secteur.
//!
//! **L'élévation n'est PAS incontournable, et ne l'a jamais été.** La frontière de Windows ne
//! sépare pas « lecture » et « écriture brute » : elle sépare le VOLUME du DISQUE. Mesuré le
//! 2026-08-03 depuis un process non élevé — `\\.\<lettre>` rend un handle
//! `FILE_GENERIC_WRITE` et accepte `FSCTL_LOCK_VOLUME`, là où `\\.\PhysicalDrive0` répond
//! `ACCESS_DENIED`. C'est toute l'explication du fait que `fat32format` n'affiche aucun UAC :
//! il ne partitionne pas, on lui donne une lettre déjà montée.
//!
//! D'où les deux chemins de `format_large_fat32` :
//!
//! - partition existante qui couvre le disque → `try_format_in_place`, **sans aucun UAC** ;
//! - disque RAW, sans lettre, ou partition trop petite → `diskpart` dans un Sift relancé en
//!   administrateur, qui **partitionne seulement** (`privileged.rs`, `run_diskpart_script`),
//!   puis le même `write_fat32`.
//!
//! Ce bloc a porté deux affirmations fausses successives : « `diskpart` est le seul chemin CLI
//! qui formate FAT32 au-delà de 32 Go », puis « livrer FAT32 reste une décision produit ouverte »
//! alors que `fat32.rs` la tranchait déjà. Les deux ont survécu au correctif qu'elles
//! surplombaient. Celle-ci est mesurée, pas déduite.
//!
//! **Enumerates physical disks, not volumes.** It used to walk `Win32_DiskDrive` ->
//! `Win32_DiskPartition` -> `Win32_LogicalDisk` and emit one entry per *mounted volume*, dropping
//! any disk with no partition or no `VolumeSerialNumber`. That excluded a brand-new, RAW or
//! corrupted key — the exact thing a formatting tool exists for. Volumes are still looked up, but
//! only to *describe* a disk (drive letter, current filesystem); they never gate it.

use super::{RemovableDrive, RemovableDriveBackend, TargetFs, UsbFormatError};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Write;
use std::process::Command;
use wmi::{Variant, WMIConnection};

pub struct WindowsBackend;

/// `MSFT_Disk.BusType` — a numeric enum, not a string to match. 7 is USB; 11 SATA, 17 NVMe on
/// this machine.
pub(crate) const BUS_TYPE_USB: u16 = 7;

/// What the removable decision is made from. Kept free of WMI types so the decision itself is
/// unit-testable with fabricated values, on any OS.
pub(crate) struct RawDiskInfo {
    /// From `MSFT_Disk`, NOT `Win32_DiskDrive.InterfaceType` — see `is_confidently_removable`.
    pub bus_type: u16,
    pub is_boot: bool,
    pub is_system: bool,
    /// Belt and braces on top of `is_boot`/`is_system`: one of this disk's logical drives is
    /// `%SystemDrive%`.
    pub carries_system_volume: bool,
}

/// The bus is the positive signal; anything system-bearing is the veto.
///
/// Two earlier versions of this filter were wrong, both measured on real hardware 2026-07-31:
///
/// 1. `MediaType == "Removable Media"` — the card reader on this machine reports no MediaType at
///    all, and an external SSD reports `"Fixed hard disk media"`.
/// 2. `Win32_DiskDrive.InterfaceType == "USB"` — an SSK portable SSD, a real USB drive with a
///    500 GB DJ library on it, reports `InterfaceType = "SCSI"`. UASP enclosures speak SCSI over
///    USB, and `Win32_DiskDrive` reports the transport protocol rather than the bus. Its
///    `PNPDeviceID` says `SCSI\DISK&VEN_SSK_SSD…` too, and the volume's `DriveType` says `Fixed`
///    — every CIMV2 signal agrees, and all of them are wrong about the bus.
///
/// `MSFT_Disk.BusType` (root\Microsoft\Windows\Storage) is the one field that answers `USB`, and
/// it is a number rather than a locale-sensitive string. `IsBoot`/`IsSystem` come from the same
/// class and are a far stronger veto than comparing drive letters.
pub(crate) fn is_confidently_removable(disk: &RawDiskInfo) -> bool {
    disk.bus_type == BUS_TYPE_USB && !disk.is_boot && !disk.is_system && !disk.carries_system_volume
}

/// `root\Microsoft\Windows\Storage` — the namespace `Get-Disk` reads. Not the default `ROOT\CIMV2`
/// this backend's other queries use, so `list` opens two connections.
pub(crate) const STORAGE_NAMESPACE: &str = "ROOT\\Microsoft\\Windows\\Storage";

/// `rename` is load-bearing, not cosmetic: `WMIConnection::query()` builds `SELECT … FROM <name>`
/// where `<name>` is the *serde* name of this struct (wmi 0.18.4, `query.rs` → `build_query` →
/// `de::meta::struct_name_and_fields`). Without it the class queried is `MsftDisk`, which does not
/// exist, and every enumeration fails with `0x80041010` (`WBEM_E_INVALID_CLASS`) — exactly the
/// state the old `Win32DiskDrive` struct shipped in from M7 until 2026-07-31.
/// Pinned by `typed_query_targets_the_real_wmi_class`.
#[allow(non_snake_case)]
#[derive(Deserialize, Debug)]
#[serde(rename = "MSFT_Disk")]
struct MsftDisk {
    /// Same numbering as `Win32_DiskDrive.Index`, so it keys both the `\\.\PHYSICALDRIVEn` path
    /// and the CIMV2 association queries that describe the disk's volumes.
    Number: u32,
    /// `"SSK SSD Portable SSD"` — cleaner than `Win32_DiskDrive.Model`, which appends the
    /// transport (`"… SCSI Disk Device"`).
    FriendlyName: Option<String>,
    BusType: u16,
    Size: Option<u64>,
    /// `Option` rather than `bool` so a disk WMI cannot describe is vetoed instead of defaulting
    /// to "safe to erase" — see how `list` unwraps these.
    IsBoot: Option<bool>,
    IsSystem: Option<bool>,
    /// Real here where CIMV2's was junk: `"SSKPSSD0000000000012"` against `Win32_DiskDrive`'s
    /// `"+"` for the same class of device.
    SerialNumber: Option<String>,
    UniqueId: Option<String>,
}

/// WQL object paths take the DeviceID **verbatim**: `\\.\PHYSICALDRIVE2`, backslashes NOT
/// doubled. This shipped doubling them (`\\\\.\\PHYSICALDRIVE2`) from M7, and WMI answers that
/// with "objet introuvable" for every disk — so the partition lookup found nothing and the
/// error-swallowing `continue` dropped every candidate in silence. Measured on this machine
/// 2026-07-31 against `Win32_DiskDrive` disk 0: the doubled form fails, the verbatim form
/// returns `Disk #0, Partition #0`. Pinned by `partitions_query_does_not_double_backslashes`.
pub(crate) fn partitions_query(disk_index: u32) -> String {
    format!(
        "ASSOCIATORS OF {{Win32_DiskDrive.DeviceID='\\\\.\\PHYSICALDRIVE{disk_index}'}} \
         WHERE AssocClass = Win32_DiskDriveToDiskPartition"
    )
}

/// Partition DeviceIDs look like `Disk #0, Partition #0` — no backslash, so nothing to escape
/// here. Extracted next to `partitions_query` only so both query shapes are visible (and
/// testable) side by side; this one was never broken.
pub(crate) fn logical_disks_query(partition_id: &str) -> String {
    format!(
        "ASSOCIATORS OF {{Win32_DiskPartition.DeviceID='{partition_id}'}} \
         WHERE AssocClass = Win32_LogicalDiskToPartition"
    )
}

/// Everything the mounted volumes of one physical disk can tell us about it. All of it is
/// descriptive: an empty `VolumeFacts` means "RAW / not formatted", which is a perfectly
/// listable disk, not a reason to drop it.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct VolumeFacts {
    pub letters: Vec<String>,
    pub filesystems: Vec<String>,
    /// Per-volume `VolumeSerialNumber`s. Not an identity anchor by themselves (a RAW disk has
    /// none) but they change the moment a different key is formatted into the same slot, so they
    /// sharpen `disk_identity` when they exist.
    pub serials: Vec<String>,
    pub total_size: u64,
    /// État de santé du volume, déjà mis en français (`describe_health`). Vide quand aucun volume
    /// monté n'a pu être interrogé.
    pub health: String,
    /// Nom du premier volume monté, vide sinon.
    pub volume_name: String,
    /// Somme des `FreeSpace` des volumes montés. C'est aussi la clé d'invalidation du cache
    /// d'occupation (`volume_usage`) : si l'espace libre a bougé, le contenu a bougé.
    pub free_bytes: u64,
}

/// `MSFT_Volume.OperationalStatus` — code mesuré le 2026-08-01 sur un volume FAT32 réel, que
/// `Get-Volume` rend par « Full Repair Needed ».
pub(crate) const OP_STATUS_FULL_REPAIR_NEEDED: u16 = 53263;

/// Ce que l'encadré affiche pour « Santé ».
///
/// `HealthStatus` porte le niveau (0 sain, 1 avertissement, 2 défaillant) et `OperationalStatus`
/// la raison. Seul le code 53263 est traduit ici : c'est le seul dont j'aie vu le rendu de Windows.
/// Tout autre code inconnu est affiché TEL QUEL à côté du niveau, plutôt que traduit au jugé —
/// une santé disque inventée est pire qu'une santé disque brute.
pub(crate) fn describe_health(health_status: Option<u16>, operational: &[u16]) -> String {
    if operational.contains(&OP_STATUS_FULL_REPAIR_NEEDED) {
        return "Réparation complète nécessaire".to_string();
    }
    match health_status {
        Some(0) => "OK".to_string(),
        Some(1) | Some(2) => {
            let level = if health_status == Some(1) {
                "Avertissement"
            } else {
                "Défaillant"
            };
            match operational.iter().find(|c| **c != 2) {
                Some(code) => format!("{level} (code {code})"),
                None => level.to_string(),
            }
        }
        _ => "Inconnue".to_string(),
    }
}

/// What the UI shows under "actuellement …". A disk with no readable filesystem is the normal
/// state of a new key, so it gets a plain French label rather than the old `"unknown"`.
pub(crate) fn describe_filesystem(facts: &VolumeFacts) -> String {
    if facts.filesystems.is_empty() {
        "non formaté".to_string()
    } else {
        facts.filesystems.join(", ")
    }
}

/// Anti-race anchor, re-read from a fresh listing immediately before formatting. Composite on
/// purpose: no single field survives every case.
///
/// - `MSFT_Disk.UniqueId` is always present, but not always device-specific: an SSK enclosure
///   measured 2026-07-31 reports `5000000000000001`, a value its firmware clearly makes up.
/// - `SerialNumber` is often junk (`"+"` on this machine's card reader) or absent.
/// - Volume serials change whenever the disk is reformatted, but a RAW disk has none.
///
/// Together they catch every swap the old volume-serial-only anchor caught, plus the RAW case it
/// could not represent at all. Opaque to the frontend — it round-trips the string, never parses it.
pub(crate) fn disk_identity(
    unique_id: Option<&str>,
    hardware_serial: Option<&str>,
    size_bytes: u64,
    volume_serials: &[String],
) -> String {
    let uid = unique_id.unwrap_or("").trim();
    let hw = hardware_serial.unwrap_or("").trim();
    format!("{uid}|{hw}|{size_bytes}|{}", volume_serials.join(","))
}

/// `\\.\PHYSICALDRIVE2` -> `2`. Returns `None` on anything else, and the caller MUST treat that
/// as a hard error: `format` builds a `select disk N` + `clean` script, so guessing or defaulting
/// a disk number here would wipe the wrong disk. Never add a fallback.
pub(crate) fn disk_index_from_id(id: &str) -> Option<u32> {
    id.rsplit_once("PHYSICALDRIVE")
        .and_then(|(_, n)| n.trim().parse().ok())
}

/// `clean` + `create partition primary` is what makes a RAW or oddly-partitioned key formattable
/// at all — `select volume <letter>` (what this used to do) needs a mounted volume, which is
/// exactly what a new or corrupted key does not have. `assign` gives the result a drive letter so
/// the key is usable the moment the modal closes.
pub(crate) fn diskpart_script(disk_index: u32, fs: TargetFs) -> String {
    let fs_name = match fs {
        TargetFs::Fat32 => "fat32",
        TargetFs::ExFat => "exfat",
    };
    format!(
        "select disk {disk_index}\nclean\ncreate partition primary\nformat fs={fs_name} quick\nassign\nexit\n"
    )
}

/// Exécute un script `diskpart` **directement**, sans passer par le shim d'élévation.
///
/// Réservé au processus déjà élevé (`privileged::run`) : appelé depuis un Sift ordinaire, il
/// échouerait au `CreateProcess` comme tout `diskpart` non élevé. Le shim reste le chemin normal.
pub(crate) fn run_diskpart_script(script: &str) -> Result<(), String> {
    let mut tmp = tempfile::Builder::new()
        .suffix(".txt")
        .tempfile()
        .map_err(|e| format!("tempfile: {e}"))?;
    tmp.write_all(script.as_bytes())
        .map_err(|e| format!("write script: {e}"))?;
    let path = tmp.into_temp_path();

    let output = Command::new("diskpart")
        .arg("/s")
        .arg(&path)
        .output()
        .map_err(|e| format!("spawn diskpart: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "diskpart {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).trim()
    ))
}

/// Première lettre montée sur ce disque, ou `None` si Windows n'en a encore monté aucune.
pub(crate) fn first_letter_of_disk(disk_index: u32) -> Option<String> {
    let cimv2 = WMIConnection::new().ok()?;
    let facts = WindowsBackend::volume_facts(&cimv2, disk_index);
    facts.letters.first().cloned()
}

/// Taille de la PARTITION portant cette lettre, en octets.
///
/// La partition, pas le volume logique : juste après `create partition primary`, le volume est
/// encore RAW et `Win32_LogicalDisk` ne rapporte rien d'exploitable. C'est pourtant cette taille
/// qu'il faut inscrire dans le BPB.
pub(crate) fn volume_size_bytes(letter: &str) -> Option<u64> {
    let cimv2 = WMIConnection::new().ok()?;
    let query = format!(
        "ASSOCIATORS OF {{Win32_LogicalDisk.DeviceID='{}'}} \
         WHERE AssocClass = Win32_LogicalDiskToPartition",
        letter.trim().trim_end_matches('\\')
    );
    let rows: Vec<HashMap<String, Variant>> = cimv2.raw_query(query).ok()?;
    rows.iter().find_map(|r| variant_to_u64(r.get("Size")))
}

/// `ERROR_CANCELLED`. What Windows reports when the user dismisses the UAC prompt; reused as the
/// exit code of the PowerShell shim below so the two outcomes stay distinguishable.
pub(crate) const UAC_DECLINED_EXIT: i32 = 1223;

/// `diskpart` cannot run from a normal user process at all — measured 2026-07-31, even a
/// read-only `list disk` fails with "L'opération demandée nécessite une élévation" before the
/// process starts. Sift is not elevated, so `Command::new("diskpart")` was failing at
/// `CreateProcess` every single time: the format has never been able to run since M7.
///
/// Formatting a disk requires administrator rights on Windows, full stop. The app therefore asks
/// for elevation for this one operation (the way Disk Management does) rather than running
/// elevated for its whole life. `Start-Process -Verb RunAs` is what raises the UAC prompt from a
/// non-elevated parent.
///
/// The nested `cmd /c … > log 2>&1` is not decoration: `-Verb RunAs` cannot redirect the elevated
/// child's stdout, so without it diskpart's own diagnosis (the FAT32-too-big message, "no media",
/// an access error) would be lost and every failure would read the same.
///
/// Refuses paths containing a quote instead of escaping them — these are our own tempfile paths,
/// so a quote means something is wrong upstream, and a mis-escaped path here is a command
/// injection into an *elevated* shell.
pub(crate) fn elevation_powershell(script_path: &str, log_path: &str) -> Option<String> {
    if [script_path, log_path]
        .iter()
        .any(|p| p.contains('\'') || p.contains('"'))
    {
        return None;
    }
    Some(format!(
        "$ErrorActionPreference='Stop'; \
         try {{ $p = Start-Process -FilePath cmd.exe \
         -ArgumentList '/c',\"diskpart /s \\\"{script_path}\\\" > \\\"{log_path}\\\" 2>&1\" \
         -Verb RunAs -Wait -PassThru -WindowStyle Hidden; exit $p.ExitCode }} \
         catch {{ exit {UAC_DECLINED_EXIT} }}"
    ))
}

fn variant_to_u64(v: Option<&Variant>) -> Option<u64> {
    match v {
        Some(Variant::UI8(n)) => Some(*n),
        Some(Variant::UI4(n)) => Some(u64::from(*n)),
        Some(Variant::String(s)) => s.parse::<u64>().ok(),
        _ => None,
    }
}

fn variant_to_string(v: Option<&Variant>) -> Option<String> {
    match v {
        Some(Variant::String(s)) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

/// Réduit un nom de volume libre à ce qui peut traverser une ligne de commande PowerShell sans
/// échappement : le champ est libre côté interface, et `privileged_elevation_powershell` refuse
/// tout guillemet plutôt que de l'échapper. On assainit donc ici, avec la MÊME règle que
/// `fat32::volume_label` — sinon l'utilisateur verrait un nom et le disque en porterait un autre.
pub(crate) fn sanitize_label_for_command(label: &str) -> String {
    let cleaned: String = label
        .chars()
        .take(11)
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        DEFAULT_VOLUME_LABEL.to_string()
    } else {
        cleaned
    }
}

/// Nom donné au volume par un formatage FAT32 de Sift, quand l'utilisateur n'en fournit aucun.
///
/// Un formatage efface tout, nom de volume compris — c'est le comportement de tous les outils, et
/// le préserver donnerait l'illusion que quelque chose a survécu.
pub(crate) const DEFAULT_VOLUME_LABEL: &str = "SIFT";

/// Relance Sift **lui-même**, élevé, sur son drapeau de formatage privilégié.
///
/// Même mécanique que `elevation_powershell`, mais la cible est notre propre exécutable au lieu de
/// `diskpart` : partitionner et écrire un volume brut exigent tous deux l'administrateur, et les
/// faire dans un seul processus élevé n'ouvre qu'une invite UAC au lieu de deux.
///
/// Refuse tout chemin ou libellé contenant un guillemet — ils viennent de notre énumération et de
/// nos constantes, donc un guillemet signale un problème en amont, et un échappement raté serait
/// une injection dans un shell sur le point d'être élevé.
pub(crate) fn privileged_elevation_powershell(
    exe: &str,
    disk_index: u32,
    fs_name: &str,
    label: &str,
) -> Option<String> {
    if [exe, fs_name, label]
        .iter()
        .any(|s| s.contains('\'') || s.contains('"'))
    {
        return None;
    }
    let flag = super::privileged::PRIVILEGED_FLAG;
    Some(format!(
        "$ErrorActionPreference='Stop'; \
         try {{ $p = Start-Process -FilePath '{exe}' \
         -ArgumentList '{flag}','{disk_index}','{fs_name}','{label}' \
         -Verb RunAs -Wait -PassThru -WindowStyle Hidden; exit $p.ExitCode }} \
         catch {{ exit {UAC_DECLINED_EXIT} }}"
    ))
}

/// Une partition déjà montée est-elle réutilisable telle quelle, ou faut-il repartitionner ?
///
/// C'est la seule décision de `try_format_in_place` qui puisse nuire en silence : réutiliser une
/// partition plus petite que le disque rendrait une clé amputée sans rien dire, alors que
/// l'utilisateur a demandé « formate ce disque », pas « formate ce qui reste d'une partition
/// d'avant ». La marge de 10 % absorbe l'écart normal entre la taille du disque et celle de la
/// partition (table de partitions, alignement sur 1 Mio, secteurs de réserve) — elle ne doit PAS
/// être élargie pour faire passer un cas : au-delà, repartitionner est la bonne réponse.
pub(crate) fn partition_covers_disk(volume_bytes: u64, disk_bytes: u64) -> bool {
    volume_bytes.saturating_mul(100) >= disk_bytes.saturating_mul(90)
}

/// Types MBR dans lesquels écrire du FAT32 est cohérent : `0x0B` (FAT32 CHS) et `0x0C` (FAT32 LBA).
pub(crate) const MBR_FAT32_CHS: u16 = 0x0B;
pub(crate) const MBR_FAT32_LBA: u16 = 0x0C;

/// Peut-on écrire du FAT32 dans une partition de ce type, sans toucher à la table de partitions ?
///
/// Ce contrôle a coûté un volume. `try_format_in_place` réutilisait la partition existante sans
/// regarder son octet de type : sur le SSD d'Antoine il valait `0x06` (FAT16, posé par le
/// `create partition primary` sans `id=` de `partition_script`, corrigé depuis). Écrire du FAT32
/// dedans donne un volume que Windows monte comme du FAT16, trouve incohérent, et déclare
/// « endommagé et illisible » — os error 1392, le 2026-08-03.
///
/// Corriger le type demande d'écrire la table de partitions, donc `\\.\PhysicalDriveN`, donc
/// l'administrateur. C'est précisément ce que le chemin sans élévation ne peut pas faire : quand
/// le type ne convient pas, la seule réponse correcte est de repartitionner.
pub(crate) fn mbr_type_accepts_fat32(mbr_type: u16) -> bool {
    mbr_type == MBR_FAT32_CHS || mbr_type == MBR_FAT32_LBA
}

/// `MSFT_Partition.MbrType` de la partition portant `letter`, `None` si illisible ou si le disque
/// est en GPT (où le type est un GUID et la question ne se pose pas de la même façon).
fn partition_mbr_type(letter: &str) -> Option<u16> {
    #[derive(Deserialize)]
    #[serde(rename = "MSFT_Partition")]
    #[serde(rename_all = "PascalCase")]
    struct MsftPartition {
        drive_letter: Option<String>,
        mbr_type: Option<u16>,
    }
    let storage = WMIConnection::with_namespace_path(STORAGE_NAMESPACE).ok()?;
    let rows: Vec<MsftPartition> = storage.query().ok()?;
    // `DriveLetter` est le caractère seul (« E »), pas « E: » — d'où la comparaison sur l'initiale.
    let wanted = letter.trim().chars().next()?.to_ascii_uppercase();
    rows.into_iter()
        .find(|p| {
            p.drive_letter
                .as_deref()
                .and_then(|d| d.chars().next())
                .map(|c| c.to_ascii_uppercase() == wanted)
                .unwrap_or(false)
        })
        .and_then(|p| p.mbr_type)
}

impl WindowsBackend {
    /// Écrit la FAT32 **sans élévation**, dans la partition déjà montée. Rend `None` quand le
    /// disque n'offre pas de partition réutilisable — c'est-à-dire quand il faut vraiment
    /// repartitionner, seul cas qui justifie un UAC.
    ///
    /// La frontière de Windows ne sépare pas « lecture » et « écriture brute », contrairement à ce
    /// que ce module supposait : elle sépare le VOLUME du DISQUE. Mesuré le 2026-08-03 sur une clé
    /// FAT32, depuis un process non élevé :
    ///   `\\.\G:`             -> handle FILE_GENERIC_WRITE obtenu, `FSCTL_LOCK_VOLUME` accepté ;
    ///   `\\.\PhysicalDrive0` -> ACCESS_DENIED (Win32 5).
    /// C'est pour ça que `fat32format` n'affiche aucun UAC : il ne partitionne pas, on lui donne
    /// une lettre déjà montée. Le UAC de Sift ne venait jamais du formatage — il venait du
    /// `clean`/`create partition` de `diskpart`, fait systématiquement même quand le disque avait
    /// déjà exactement la partition qu'il fallait.
    fn try_format_in_place(
        &self,
        drive: &RemovableDrive,
        label: &str,
    ) -> Option<Result<(), UsbFormatError>> {
        // CHAQUE sortie de cette fonction se journalise. La première version n'en traçait qu'une
        // sur trois : quand un UAC est apparu là où il ne devait pas (2026-08-03), il était
        // impossible de dire laquelle avait décidé. `write_step` n'aide pas — il ÉCRASE, donc
        // l'étape suivante efface le diagnostic. C'est `log::info!` qui garde la trace.
        let letter = drive
            .mount
            .split(',')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if letter.is_empty() {
            log::info!(
                "format {}: aucune lettre montée (mount={:?}) — élévation nécessaire",
                drive.id,
                drive.mount
            );
            return None; // disque RAW — rien à réutiliser, l'élévation est légitime
        }
        let Some(volume_bytes) = volume_size_bytes(&letter) else {
            log::info!(
                "format {}: taille de la partition {letter} illisible (WMI) — élévation nécessaire",
                drive.id
            );
            return None;
        };
        if !partition_covers_disk(volume_bytes, drive.size_bytes) {
            log::info!(
                "format {}: partition {letter} de {volume_bytes} o pour un disque de {} o \
                 ({:.1} %) — sous le seuil, repartitionnement nécessaire",
                drive.id,
                drive.size_bytes,
                100.0 * volume_bytes as f64 / (drive.size_bytes.max(1)) as f64,
            );
            return None;
        }
        // Le type MBR doit déjà accepter du FAT32 : le corriger exigerait d'écrire la table de
        // partitions, donc l'administrateur, donc précisément ce que ce chemin évite.
        match partition_mbr_type(&letter) {
            Some(t) if mbr_type_accepts_fat32(t) => {}
            Some(t) => {
                log::info!(
                    "format {}: partition {letter} de type MBR 0x{t:02X}, incompatible FAT32 \
                     — repartitionnement nécessaire",
                    drive.id
                );
                return None;
            }
            None => {
                log::info!(
                    "format {}: type MBR de {letter} illisible — repartitionnement par sécurité",
                    drive.id
                );
                return None;
            }
        }
        log::info!(
            "format {}: partition {letter} réutilisable ({volume_bytes} o / {} o) — sans élévation",
            drive.id,
            drive.size_bytes
        );

        super::privileged::write_step("Verrouillage du volume…");
        let mut volume = match super::raw_volume::RawVolume::open(&letter) {
            Ok(v) => v,
            // Le verrou refusé n'est PAS un échec silencieux : l'étape s'affiche, et l'appelant
            // enchaîne sur le chemin élevé, dont le `clean` sait forcer un volume récalcitrant.
            Err(e) => {
                log::info!("format {}: verrou refusé sur {letter}: {e}", drive.id);
                super::privileged::write_step(&format!(
                    "Volume {letter} non verrouillable ({e}) — passage par l'élévation…"
                ));
                return None;
            }
        };

        super::privileged::write_step("Écriture du système de fichiers FAT32…");
        // L'étape dit combien est parti, pas seulement qu'on écrit : sur ce SSD de 500 Go le
        // zéro-remplissage des FAT (~122 Mo) a laissé l'étape figée plusieurs minutes le
        // 2026-09-09 — « l'impression que l'écran est bloqué ». Un mot tous les 16 Mio, pas plus :
        // `write_step` recrée le fichier d'étape à chaque appel.
        let mut last_reported: u64 = 0;
        let aligned = super::sector_io::SectorIo::new(
            volume.as_file_mut(),
            u64::from(super::fat32::BYTES_PER_SECTOR),
        )
        .with_progress(Box::new(move |written| {
            if written - last_reported >= 16 << 20 {
                last_reported = written;
                super::privileged::write_step(&format!(
                    "Écriture du système de fichiers FAT32… {} Mo écrits",
                    written / 1_000_000
                ));
            }
        }));
        Some(
            super::fat32::write_fat32(aligned, volume_bytes, label)
                .map_err(|e| UsbFormatError::Format(e.to_string())),
        )
    }

    /// FAT32 au-delà du plafond de 32 Go. Sans élévation quand la partition existante convient,
    /// via un Sift relancé en administrateur seulement quand il faut repartitionner.
    fn format_large_fat32(
        &self,
        drive: &RemovableDrive,
        label: &str,
    ) -> Result<(), UsbFormatError> {
        if let Some(res) = self.try_format_in_place(drive, label) {
            return res;
        }
        let disk_index = disk_index_from_id(&drive.id).ok_or_else(|| {
            UsbFormatError::Format(format!(
                "identifiant de disque non reconnu: {} — formatage refusé",
                drive.id
            ))
        })?;
        let exe = std::env::current_exe()
            .map_err(|e| UsbFormatError::Format(format!("chemin de l'exécutable: {e}")))?;
        let exe = exe.to_string_lossy().to_string();

        // Repart d'un fichier propre : une étape restée d'un formatage précédent s'afficherait
        // comme la progression de celui-ci.
        super::privileged::write_step("Autorisation Windows demandée…");
        let safe = sanitize_label_for_command(label);
        let ps =
            privileged_elevation_powershell(&exe, disk_index, "fat32", &safe).ok_or_else(|| {
                UsbFormatError::Format(
                    "chemin d'exécutable contenant un guillemet — formatage refusé".to_string(),
                )
            })?;

        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps])
            .output()
            .map_err(|e| UsbFormatError::Format(format!("spawn powershell: {e}")))?;

        let code = output.status.code().unwrap_or(-1);
        if code == UAC_DECLINED_EXIT {
            return Err(UsbFormatError::ElevationDeclined);
        }
        if code == super::privileged::EXIT_OK {
            return Ok(());
        }
        // Chaque code dit ce qui a échoué : un « échec du formatage » générique n'aiderait
        // personne à savoir s'il faut fermer un programme ou rebrancher la clé.
        Err(UsbFormatError::Format(match code {
            c if c == super::privileged::EXIT_PARTITION_FAILED => {
                "le partitionnement a échoué — le disque est-il protégé en écriture ?".to_string()
            }
            c if c == super::privileged::EXIT_NO_LETTER => {
                "Windows n'a monté aucune lettre après le partitionnement".to_string()
            }
            c if c == super::privileged::EXIT_VOLUME_LOCKED => {
                "un programme tient encore ce disque ouvert — ferme-les et réessaie".to_string()
            }
            c if c == super::privileged::EXIT_WRITE_FAILED => {
                "l'écriture du système de fichiers a échoué".to_string()
            }
            other => format!("le formatage privilégié est sorti avec le code {other}"),
        }))
    }

    /// Santé de chaque volume monté, indexée par lettre (`"I:"`).
    ///
    /// `MSFT_Volume` vit dans l'espace de noms Storage et n'a pas d'equivalent en CIMV2 :
    /// `Win32_LogicalDisk` ne porte aucune notion de santé. `raw_query` plutôt qu'une struct
    /// typée parce que `DriveLetter` y est un `Char` WMI, pas une chaîne — mesuré le 2026-08-01.
    fn volume_health(storage: &WMIConnection) -> HashMap<String, (String, String)> {
        let mut out = HashMap::new();
        let rows: Vec<HashMap<String, Variant>> = match storage
            // `FileSystemLabel` doit figurer ici, pas seulement dans la lecture plus bas : une
            // projection WQL ne rend QUE les colonnes nommées, et un champ absent se lit comme un
            // champ vide. C'est ce qui a fait remonter un nom de volume vide alors que la santé,
            // issue de la même ligne, arrivait correctement.
            .raw_query(
                "SELECT DriveLetter, HealthStatus, OperationalStatus, FileSystemLabel \
                 FROM MSFT_Volume",
            ) {
            Ok(r) => r,
            Err(e) => {
                log::error!("usb_format: MSFT_Volume query failed: {e}");
                return out;
            }
        };
        for row in rows {
            let letter = match row.get("DriveLetter") {
                Some(Variant::String(s)) if !s.is_empty() => s.clone(),
                // Un Char WMI arrive en entier : c'est le point de code de la lettre.
                Some(Variant::UI2(n)) if *n != 0 => match char::from_u32(u32::from(*n)) {
                    Some(c) => c.to_string(),
                    None => continue,
                },
                _ => continue,
            };
            let health = match row.get("HealthStatus") {
                Some(Variant::UI2(n)) => Some(*n),
                Some(Variant::UI4(n)) => u16::try_from(*n).ok(),
                _ => None,
            };
            let ops: Vec<u16> = match row.get("OperationalStatus") {
                Some(Variant::Array(items)) => items
                    .iter()
                    .filter_map(|v| match v {
                        Variant::UI2(n) => Some(*n),
                        Variant::UI4(n) => u16::try_from(*n).ok(),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            };
            let name = match row.get("FileSystemLabel") {
                Some(Variant::String(s)) => s.clone(),
                _ => String::new(),
            };
            out.insert(format!("{letter}:"), (describe_health(health, &ops), name));
        }
        out
    }

    /// Collect the mounted volumes of one physical disk. A failed lookup logs and yields whatever
    /// was gathered so far — it must never remove the disk from the listing, since a disk we
    /// cannot describe is still a disk the user may need to format.
    fn volume_facts(con: &WMIConnection, disk_index: u32) -> VolumeFacts {
        let mut facts = VolumeFacts::default();
        let partitions: Vec<HashMap<String, Variant>> =
            match con.raw_query(partitions_query(disk_index)) {
                Ok(p) => p,
                Err(e) => {
                    // Never silent: a malformed query here used to drop the disk with no trace,
                    // which is how the escaping bug above survived from M7 to 2026-07-31.
                    log::error!("usb_format: partition lookup failed for disk {disk_index}: {e}");
                    return facts;
                }
            };
        for part in partitions {
            let Some(Variant::String(part_id)) = part.get("DeviceID") else {
                continue;
            };
            let logicals: Vec<HashMap<String, Variant>> =
                match con.raw_query(logical_disks_query(part_id)) {
                    Ok(l) => l,
                    Err(e) => {
                        log::error!(
                            "usb_format: logical-disk lookup failed for partition {part_id}: {e}"
                        );
                        continue;
                    }
                };
            for logical in logicals {
                if let Some(letter) = variant_to_string(logical.get("DeviceID")) {
                    facts.letters.push(letter);
                }
                if let Some(fs) = variant_to_string(logical.get("FileSystem")) {
                    facts.filesystems.push(fs);
                }
                if let Some(serial) = variant_to_string(logical.get("VolumeSerialNumber")) {
                    facts.serials.push(serial);
                }
                facts.total_size += variant_to_u64(logical.get("Size")).unwrap_or(0);
                facts.free_bytes += variant_to_u64(logical.get("FreeSpace")).unwrap_or(0);
            }
        }
        facts
    }
}

impl RemovableDriveBackend for WindowsBackend {
    fn list(&self) -> Result<Vec<RemovableDrive>, UsbFormatError> {
        // Two namespaces: the bus type only exists in the Storage one, the volume description only
        // in CIMV2.
        let storage = WMIConnection::with_namespace_path(STORAGE_NAMESPACE).map_err(|e| {
            UsbFormatError::Enumeration(format!("WMIConnection({STORAGE_NAMESPACE}): {e}"))
        })?;
        let cimv2 = WMIConnection::new()
            .map_err(|e| UsbFormatError::Enumeration(format!("WMIConnection::new: {e}")))?;

        let disks: Vec<MsftDisk> = storage
            .query()
            .map_err(|e| UsbFormatError::Enumeration(format!("MSFT_Disk query: {e}")))?;

        // `%SystemDrive%` rather than a hardcoded "C:" — a Windows install is not guaranteed to
        // live on C:, and these vetoes are what stand between `diskpart clean` and a disk the user
        // cannot afford to lose.
        let system_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
        // Une seule requête pour tous les volumes, pas une par disque.
        let health_by_letter = Self::volume_health(&storage);

        let mut drives = Vec::new();
        for disk in disks {
            // Volumes are read BEFORE the filter because one veto is derived from them. Cheap:
            // a few WMI calls per disk, on a list that is single digits long.
            let mut facts = Self::volume_facts(&cimv2, disk.Number);
            if let Some((health, name)) = facts
                .letters
                .iter()
                .find_map(|l| health_by_letter.get(l).cloned())
            {
                facts.health = health;
                facts.volume_name = name;
            }
            let carries_system_volume = facts
                .letters
                .iter()
                .any(|l| l.eq_ignore_ascii_case(&system_drive));

            let raw = RawDiskInfo {
                bus_type: disk.BusType,
                // A disk WMI will not vouch for is treated as system-bearing, never as free to
                // erase: the safe default for an irreversible action is to refuse.
                is_boot: disk.IsBoot.unwrap_or(true),
                is_system: disk.IsSystem.unwrap_or(true),
                carries_system_volume,
            };
            if !is_confidently_removable(&raw) {
                continue;
            }

            // Falling back to the summed volume size keeps a row from reading "0.0 Go", which the
            // UI would otherwise show as a plausible-looking lie.
            let size_bytes = disk.Size.unwrap_or(facts.total_size);
            // An empty card-reader slot reports no capacity (measured 2026-07-31, matching
            // `Get-Disk`'s `OperationalStatus: No Media`). A disk holding real media always
            // reports its capacity, formatted or not — a RAW key included.
            let has_media = size_bytes > 0;
            let id = format!(r"\\.\PHYSICALDRIVE{}", disk.Number);

            drives.push(RemovableDrive {
                label: disk.FriendlyName.clone().unwrap_or_else(|| id.clone()),
                mount: facts.letters.join(", "),
                size_bytes,
                free_bytes: facts.free_bytes,
                current_fs: describe_filesystem(&facts),
                volume_name: facts.volume_name.clone(),
                health: facts.health.clone(),
                has_media,
                identity: disk_identity(
                    disk.UniqueId.as_deref(),
                    disk.SerialNumber.as_deref(),
                    size_bytes,
                    &facts.serials,
                ),
                id,
            });
        }

        Ok(drives)
    }

    fn eject(&self, drive: &RemovableDrive) -> Result<(), UsbFormatError> {
        // Un disque sans volume monté n'a rien à démonter : il est déjà débranchable.
        let letters: Vec<&str> = drive
            .mount
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        if letters.is_empty() {
            return Ok(());
        }

        for letter in &letters {
            let ps = eject_powershell(letter).ok_or_else(|| {
                UsbFormatError::Enumeration(format!("lettre de lecteur inattendue: {letter}"))
            })?;
            let output = Command::new("powershell")
                .args(["-NoProfile", "-Command", &ps])
                .output()
                .map_err(|e| UsbFormatError::Enumeration(format!("spawn powershell: {e}")))?;
            if !output.status.success() {
                log::error!(
                    "usb_format: demande d'éjection refusée pour {letter}: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                );
            }
        }

        // La demande a abouti ou non — c'est la disparition du disque qui tranche, pas le code de
        // sortie du shell, qui vaut 0 même quand Windows refuse ensuite le démontage. Annoncer un
        // succès non vérifié inviterait à débrancher un volume encore monté.
        for _ in 0..EJECT_POLL_ATTEMPTS {
            std::thread::sleep(EJECT_POLL_INTERVAL);
            let still_there = self
                .list()
                .map(|drives| drives.iter().any(|d| d.id == drive.id))
                .unwrap_or(true);
            if !still_there {
                return Ok(());
            }
        }
        Err(UsbFormatError::EjectBusy)
    }

    fn format(
        &self,
        drive: &RemovableDrive,
        fs: TargetFs,
        label: &str,
    ) -> Result<(), UsbFormatError> {
        // FAT32 au-delà de 32 Go : Windows refuse de le CRÉER, donc `diskpart` ne peut pas servir.
        // On passe par notre propre écriture, dans un Sift relancé en administrateur — c'est la
        // raison d'être de tout ce chemin, et le cas d'usage principal d'une clé DJ moderne.
        if fs == TargetFs::Fat32 && drive.size_bytes > super::fat32::WINDOWS_FAT32_CREATE_CEILING {
            return self.format_large_fat32(drive, label);
        }

        // Hard failure, never a fallback: the script below runs `clean` on this number.
        let disk_index = disk_index_from_id(&drive.id).ok_or_else(|| {
            UsbFormatError::Format(format!(
                "identifiant de disque non reconnu: {} — formatage refusé",
                drive.id
            ))
        })?;
        let script = diskpart_script(disk_index, fs);

        let mut tmp = tempfile::Builder::new()
            .suffix(".txt")
            .tempfile()
            .map_err(|e| UsbFormatError::Format(format!("tempfile: {e}")))?;
        tmp.write_all(script.as_bytes())
            .map_err(|e| UsbFormatError::Format(format!("write script: {e}")))?;
        // `into_temp_path` closes our handle while keeping the file (and its delete-on-drop): the
        // elevated diskpart runs as a different process and must be able to open both paths.
        let script_path = tmp.into_temp_path();
        let log_path = tempfile::Builder::new()
            .suffix(".log")
            .tempfile()
            .map_err(|e| UsbFormatError::Format(format!("tempfile: {e}")))?
            .into_temp_path();

        let ps = elevation_powershell(&script_path.to_string_lossy(), &log_path.to_string_lossy())
            .ok_or_else(|| {
                UsbFormatError::Format(
                    "chemin temporaire contenant un guillemet — formatage refusé".to_string(),
                )
            })?;

        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps])
            .output()
            .map_err(|e| UsbFormatError::Format(format!("spawn powershell: {e}")))?;

        let code = output.status.code().unwrap_or(-1);
        if code == UAC_DECLINED_EXIT {
            return Err(UsbFormatError::ElevationDeclined);
        }
        // diskpart's own words, captured through the `cmd /c … > log` shim. Without them every
        // failure — volume too big for FAT32, no media, disk write-protected — reads the same.
        let diagnosis = std::fs::read_to_string(&log_path).unwrap_or_else(|e| {
            log::error!("usb_format: could not read diskpart log: {e}");
            String::new()
        });
        if code != 0 {
            return Err(UsbFormatError::Format(format!(
                "diskpart exited with {code}: {}",
                diagnosis.trim()
            )));
        }
        // diskpart can exit 0 having refused a step it printed an error for, so the log is checked
        // even on success — a silent no-op that reports success is worse than a loud failure.
        if diagnosis.contains("DiskPart has encountered an error")
            || diagnosis.contains("DiskPart a rencontré une erreur")
        {
            return Err(UsbFormatError::Format(diagnosis.trim().to_string()));
        }

        Ok(())
    }
}

/// Combien de fois, et à quel rythme, on re-liste pour savoir si le disque est réellement parti.
/// Le shell rend la main immédiatement et démonte en arrière-plan ; sans attente on conclurait
/// « échec » sur une éjection qui aboutit une demi-seconde plus tard.
const EJECT_POLL_ATTEMPTS: u32 = 12;
const EJECT_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(300);

/// Corps PowerShell de l'éjection, séparé pour ne PAS passer par `format!` : il contient des
/// accolades C# à chaque ligne, qu'il faudrait toutes doubler. Seule la lettre est interpolée,
/// dans `eject_powershell` juste en dessous.
///
/// Attend `$sift_letter` déjà défini par l'appelant.
const EJECT_PS_BODY: &str = r#"
$ErrorActionPreference = 'Stop'
$part = Get-Partition -DriveLetter ($sift_letter.TrimEnd(':'))
$pnpId = (Get-CimInstance Win32_DiskDrive | Where-Object { $_.Index -eq $part.DiskNumber }).PNPDeviceID
if (-not $pnpId) { Write-Error "aucun PNPDeviceID pour le disque $($part.DiskNumber)"; exit 1 }
Add-Type -TypeDefinition @'
using System;using System.Runtime.InteropServices;using System.Text;
public static class SiftEject {
 [DllImport("cfgmgr32.dll",CharSet=CharSet.Unicode)] public static extern int CM_Locate_DevNodeW(out uint p,string id,uint f);
 [DllImport("cfgmgr32.dll")] public static extern int CM_Get_Parent(out uint p,uint d,uint f);
 [DllImport("cfgmgr32.dll",CharSet=CharSet.Unicode)] public static extern int CM_Request_Device_EjectW(uint d,out int veto,StringBuilder name,int len,uint f);
}
'@
$dev = 0
if ([SiftEject]::CM_Locate_DevNodeW([ref]$dev, $pnpId, 0) -ne 0) { Write-Error "CM_Locate_DevNode a echoue"; exit 1 }
$parent = 0
if ([SiftEject]::CM_Get_Parent([ref]$parent, $dev, 0) -ne 0) { Write-Error "CM_Get_Parent a echoue"; exit 1 }
$veto = 0
$vetoName = New-Object System.Text.StringBuilder 260
$cr = [SiftEject]::CM_Request_Device_EjectW($parent, [ref]$veto, $vetoName, $vetoName.Capacity, 0)
if ($cr -ne 0 -or $veto -ne 0) { Write-Error "VETO cr=$cr type=$veto name=$($vetoName.ToString())"; exit 1 }
"#;

/// Demande l'éjection du PÉRIPHÉRIQUE portant cette lettre, par `CM_Request_Device_Eject` — le
/// chemin qu'emprunte l'icône « Retirer le périphérique en toute sécurité » de la barre des
/// tâches.
///
/// **Pourquoi pas le verbe shell du volume, qui était ici jusqu'au 2026-08-05.** Il n'existe pas
/// sur un disque que Windows classe en FIXE, et un SSD USB en est un : mesuré sur un SSK 500 Go,
/// `Shell.Application.NameSpace(17).ParseName('E:').Verbs()` ne contient aucun verbe d'éjection
/// (`Win32_LogicalDisk` le donne en `DriveType=3`, `MediaType=12`, alors que `Get-Disk` sait
/// pourtant que son `BusType` est USB — même famille de mensonge que `InterfaceType` sur les
/// boîtiers UASP). `InvokeVerb` sur un verbe absent est un **no-op silencieux** : PowerShell sortait
/// 0 sans que rien n'ait été demandé, la sonde de disparition ne voyait rien partir, et
/// l'utilisateur recevait `EJECT_BUSY` — « ferme Rekordbox » — alors que rien ne tenait le disque.
/// Le bug est invisible sur une petite clé USB, qui est bien vue comme amovible ; il ne se
/// manifeste que sur le matériel des vrais utilisateurs.
///
/// Toujours pas d'élévation : `CM_Request_Device_Eject` tourne en utilisateur normal, comme
/// l'icône de la barre des tâches. `mountvol /P` et `DeviceIoControl(FSCTL_DISMOUNT_VOLUME)`
/// restent écartés pour cette raison — faire surgir une UAC pour débrancher une clé serait absurde.
///
/// Contrairement au verbe shell, cette API rend un **veto** : `vetoType` et `vetoName` disent QUI
/// bloque. Le script les remonte en stderr, ce qui rend enfin possible un message d'erreur qui
/// nomme le vrai coupable au lieu de deviner.
///
/// La vérification par re-listage chez l'appelant reste le juge final : la demande est asynchrone.
/// Refuse une lettre contenant un guillemet plutôt que d'échapper : elle vient de notre propre
/// énumération, donc un guillemet signale un problème en amont.
pub(crate) fn eject_powershell(letter: &str) -> Option<String> {
    if letter.contains('\'') || letter.contains('"') {
        return None;
    }
    Some(format!("$sift_letter = '{letter}'{EJECT_PS_BODY}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le contrôle qui manquait, et qui a rendu un volume illisible (os error 1392, 2026-08-03) :
    /// écrire du FAT32 dans une partition typée FAT16 produit un volume que Windows monte avec le
    /// mauvais pilote. Seuls 0x0B et 0x0C sont cohérents ; tout le reste impose de repartitionner.
    #[test]
    fn seuls_les_types_mbr_fat32_acceptent_une_ecriture_en_place() {
        assert!(mbr_type_accepts_fat32(MBR_FAT32_CHS)); // 0x0B
        assert!(mbr_type_accepts_fat32(MBR_FAT32_LBA)); // 0x0C
                                                        // 0x06 = FAT16, le défaut de `create partition primary` sans `id=`. C'est CE type qui a
                                                        // ete trouve sur le SSD, et FAT16 plafonne a 2 Go — 465 Go typees ainsi sont hors spec.
        assert!(!mbr_type_accepts_fat32(0x06));
        assert!(!mbr_type_accepts_fat32(0x07)); // NTFS/exFAT
        assert!(!mbr_type_accepts_fat32(0x0E)); // FAT16 LBA
        assert!(!mbr_type_accepts_fat32(0x00)); // vide
        assert!(!mbr_type_accepts_fat32(0xEE)); // protective MBR d'un disque GPT
    }

    /// Ce seuil décide s'il y aura un UAC ou non, ET s'il peut y avoir une clé amputée. Les deux
    /// erreurs coûtent, dans des directions opposées : trop strict et on élève pour rien, trop
    /// laxiste et on formate une partition qui ne couvre pas le disque en le passant sous silence.
    #[test]
    fn partition_reutilisable_seulement_si_elle_couvre_le_disque() {
        const DISQUE: u64 = 465_600_000_000; // le SSD DJ réel, 465,6 Go

        // Cas courant : une partition unique, à quelques Mio près de la taille du disque.
        assert!(partition_covers_disk(DISQUE - 4_194_304, DISQUE));
        assert!(partition_covers_disk(DISQUE, DISQUE));
        // Une partition à 89 % : formater là-dedans rendrait 51 Go inaccessibles sans le dire.
        assert!(!partition_covers_disk(DISQUE / 100 * 89, DISQUE));
        assert!(!partition_covers_disk(DISQUE / 2, DISQUE));
        // Un disque RAW n'a pas de volume : 0 octet ne couvre jamais rien.
        assert!(!partition_covers_disk(0, DISQUE));
        // Pas de débordement sur des valeurs absurdes — la décision doit rester une décision,
        // jamais une panique dans le chemin de formatage.
        assert!(partition_covers_disk(u64::MAX, u64::MAX));
        assert!(partition_covers_disk(u64::MAX, 1));
    }

    /// The bug this pins was invisible to every other test here: they all feed
    /// `is_confidently_removable` fabricated values, so the filter was covered and the query that
    /// produces the values never was. `build_query` is the same code path `query()` takes, minus
    /// the WMI call — so this asserts the real class name without touching the machine's WMI, and
    /// fails on any CI box.
    #[test]
    fn typed_query_targets_the_real_wmi_class() {
        let q = wmi::build_query::<MsftDisk>(None).expect("build_query");
        assert!(
            q.contains("FROM MSFT_Disk"),
            "typed query must name the real WMI class, got: {q}"
        );
    }

    /// The escaping bug was invisible to every other test: the query string was built inline at
    /// its single call site, so nothing could read it without a live WMI connection. Extracting
    /// the builder is what makes it assertable on any machine.
    #[test]
    fn partitions_query_does_not_double_backslashes() {
        let q = partitions_query(2);
        assert!(
            q.contains(r"'\\.\PHYSICALDRIVE2'"),
            "device path must be verbatim, got: {q}"
        );
        assert!(
            !q.contains(r"\\\\"),
            "doubling the backslashes makes WMI answer 'object not found', got: {q}"
        );
    }

    fn usb_disk() -> RawDiskInfo {
        RawDiskInfo {
            bus_type: BUS_TYPE_USB,
            is_boot: false,
            is_system: false,
            carries_system_volume: false,
        }
    }

    #[test]
    fn usb_bus_disk_is_included() {
        assert!(is_confidently_removable(&usb_disk()));
    }

    /// The exact device this filter was rewritten for: an SSK portable SSD holding a 500 GB DJ
    /// library. Every CIMV2 signal says it is not removable — `InterfaceType = "SCSI"`,
    /// `PNPDeviceID` starting `SCSI\`, volume `DriveType = Fixed`, `MediaType = "External hard
    /// disk media"` — because a UASP enclosure speaks SCSI over USB. `MSFT_Disk.BusType` is the
    /// only field that answers USB, and this test exists so nobody "simplifies" back to the
    /// CIMV2 ones.
    #[test]
    fn uasp_enclosure_is_included_even_though_cimv2_calls_it_scsi() {
        assert!(is_confidently_removable(&RawDiskInfo {
            bus_type: BUS_TYPE_USB,
            ..usb_disk()
        }));
    }

    /// SATA (11) and NVMe (17) as measured on this machine, plus the neighbours most likely to be
    /// confused with USB.
    #[test]
    fn internal_bus_disk_is_excluded() {
        for bus in [0u16, 1, 3, 8, 10, 11, 17] {
            assert!(
                !is_confidently_removable(&RawDiskInfo {
                    bus_type: bus,
                    ..usb_disk()
                }),
                "bus type {bus} must never be offered"
            );
        }
    }

    /// A Windows install running from a USB disk is on the USB bus like any key. These vetoes are
    /// what stand between `diskpart clean` and a disk the user cannot afford to lose.
    #[test]
    fn usb_disk_that_carries_the_system_is_excluded_by_every_veto() {
        for veto in [
            RawDiskInfo {
                is_boot: true,
                ..usb_disk()
            },
            RawDiskInfo {
                is_system: true,
                ..usb_disk()
            },
            RawDiskInfo {
                carries_system_volume: true,
                ..usb_disk()
            },
        ] {
            assert!(!is_confidently_removable(&veto));
        }
    }

    /// Le code 53263 est le seul dont j'aie vu le rendu de Windows — mesure sur le volume FAT32
    /// réel de cette machine, que `Get-Volume` annonce "Full Repair Needed".
    /// Le champ de nom est libre côté interface : ce qui en sort doit être sûr à poser dans une
    /// commande, et IDENTIQUE à ce que le disque portera — sinon l'utilisateur voit un nom et la
    /// cle en porte un autre.
    #[test]
    fn free_text_label_is_made_safe_for_a_command() {
        assert_eq!(sanitize_label_for_command("Cle DJ"), "CLE_DJ");
        assert_eq!(sanitize_label_for_command("djermusique"), "DJERMUSIQUE");
        // Le guillemet, le point-virgule, les espaces et la barre oblique tombent tous sur `_` ;
        // seul le tiret survit. Rien de ce qui sort d'ici ne peut refermer une chaîne PowerShell.
        assert_eq!(sanitize_label_for_command("a'; rm -r /"), "A___RM_-R__");
        assert_eq!(sanitize_label_for_command(""), DEFAULT_VOLUME_LABEL);
        assert_eq!(sanitize_label_for_command("!!!"), "___");
        assert_eq!(sanitize_label_for_command("BEAUCOUP_TROP_LONG").len(), 11);
    }

    #[test]
    fn measured_repair_code_is_named() {
        assert_eq!(
            describe_health(Some(1), &[OP_STATUS_FULL_REPAIR_NEEDED]),
            "Réparation complète nécessaire"
        );
    }

    #[test]
    fn healthy_volume_says_ok() {
        assert_eq!(describe_health(Some(0), &[2]), "OK");
    }

    /// Un code inconnu s'affiche TEL QUEL à côté du niveau. Le traduire au juge inventerait un
    /// diagnostic de disque, ce qui est pire que de montrer un nombre.
    #[test]
    fn unknown_codes_are_shown_not_invented() {
        assert_eq!(
            describe_health(Some(1), &[41234]),
            "Avertissement (code 41234)"
        );
        assert_eq!(describe_health(Some(2), &[]), "Défaillant");
    }

    #[test]
    fn absent_health_is_unknown_not_ok() {
        assert_eq!(describe_health(None, &[]), "Inconnue");
    }

    #[test]
    fn raw_disk_is_described_as_not_formatted() {
        assert_eq!(describe_filesystem(&VolumeFacts::default()), "non formaté");
    }

    #[test]
    fn formatted_disk_reports_its_filesystems() {
        let facts = VolumeFacts {
            filesystems: vec!["FAT32".to_string()],
            ..Default::default()
        };
        assert_eq!(describe_filesystem(&facts), "FAT32");
    }

    #[test]
    fn disk_index_is_parsed_from_the_device_path() {
        assert_eq!(disk_index_from_id(r"\\.\PHYSICALDRIVE2"), Some(2));
        assert_eq!(disk_index_from_id(r"\\.\PHYSICALDRIVE11"), Some(11));
    }

    /// `format` turns `None` into a refusal. Anything that could make this return `Some(0)` for a
    /// malformed id would point `clean` at the first disk in the machine.
    #[test]
    fn unrecognised_disk_id_yields_no_index() {
        for bogus in ["E:", "", "PHYSICALDRIVE", r"\\.\PHYSICALDRIVEx", "disk2"] {
            assert_eq!(disk_index_from_id(bogus), None, "must refuse {bogus:?}");
        }
    }

    #[test]
    fn diskpart_script_cleans_and_recreates_the_partition() {
        let s = diskpart_script(3, TargetFs::Fat32);
        assert!(s.starts_with("select disk 3\n"), "got: {s}");
        assert!(s.contains("\nclean\n"), "a RAW key needs clean: {s}");
        assert!(
            s.contains("\ncreate partition primary\n"),
            "a cleaned disk has no partition to format: {s}"
        );
        assert!(s.contains("format fs=fat32 quick"), "got: {s}");
        assert!(
            s.contains("\nassign\n"),
            "the key must come back mounted: {s}"
        );
    }

    #[test]
    fn diskpart_script_never_targets_a_volume_letter() {
        let s = diskpart_script(2, TargetFs::ExFat);
        assert!(
            !s.contains("select volume"),
            "selecting a volume is what made RAW keys unformattable: {s}"
        );
        assert!(s.contains("format fs=exfat quick"), "got: {s}");
    }

    /// Two serial-less keys swapped in the same USB port share a `PNPDeviceID`; their volume
    /// serials are what tells them apart. A RAW disk has none — hence the composite.
    #[test]
    fn identity_changes_when_the_volume_serial_changes() {
        let a = disk_identity(
            Some("USBSTOR\\X"),
            Some("+"),
            16_000_000_000,
            &["AAAA-1111".to_string()],
        );
        let b = disk_identity(
            Some("USBSTOR\\X"),
            Some("+"),
            16_000_000_000,
            &["BBBB-2222".to_string()],
        );
        assert_ne!(a, b);
    }

    #[test]
    fn identity_is_stable_for_the_same_disk() {
        let serials = vec!["AAAA-1111".to_string()];
        assert_eq!(
            disk_identity(Some("USBSTOR\\X"), Some("SN123"), 16_000_000_000, &serials),
            disk_identity(Some("USBSTOR\\X"), Some("SN123"), 16_000_000_000, &serials)
        );
    }

    /// A RAW key has no volume serial at all — the anchor must still be non-empty and still
    /// distinguish two different devices, or the anti-race guard silently degrades to "always OK".
    #[test]
    fn identity_still_distinguishes_two_raw_disks() {
        let a = disk_identity(Some("USBSTOR\\DISK&A\\7&1"), None, 8_000_000_000, &[]);
        let b = disk_identity(Some("USBSTOR\\DISK&B\\7&2"), None, 8_000_000_000, &[]);
        assert_ne!(a, b);
        assert!(!a.trim_matches('|').is_empty());
    }

    /// `CM_Request_Device_Eject` tourne sans élévation, comme l'icône « Retirer le périphérique
    /// en toute sécurité » de la barre des tâches. `mountvol /P` et `FSCTL_DISMOUNT_VOLUME`
    /// exigent l'administrateur : faire surgir une UAC pour débrancher une clé serait absurde.
    #[test]
    fn eject_uses_the_device_level_api_that_needs_no_elevation() {
        let ps = eject_powershell("I:").expect("shim");
        assert!(ps.contains("CM_Request_Device_EjectW"), "{ps}");
        assert!(ps.contains("$sift_letter = 'I:'"), "{ps}");
        assert!(
            !ps.contains("RunAs") && !ps.contains("mountvol"),
            "aucune élévation ne doit être demandée: {ps}"
        );
    }

    /// Le PARENT, pas le disque. L'icône de la barre des tâches éjecte le périphérique de stockage
    /// USB ; viser le devnode du disque ne démonte rien. Mesuré le 2026-08-05 : disque = devnode 1,
    /// parent = devnode 2 (`USB\VID_090C&PID_2320\…`), et seul le second a rendu `CR=0 veto=0`.
    #[test]
    fn eject_targets_the_parent_devnode_not_the_disk() {
        let ps = eject_powershell("I:").expect("shim");
        assert!(ps.contains("CM_Get_Parent"), "{ps}");
        let parent = ps.find("CM_Get_Parent").expect("CM_Get_Parent");
        let eject = ps
            .find("CM_Request_Device_EjectW($parent")
            .expect("appel sur $parent");
        assert!(
            parent < eject,
            "le parent doit être résolu AVANT l'éjection: {ps}"
        );
    }

    /// Le verbe shell du VOLUME n'existe pas sur un disque que Windows classe en fixe — et un SSD
    /// USB en est un. Ce test empêche le retour d'un chemin mesuré inopérant le 2026-08-05, dont
    /// le mode de panne est silencieux : `InvokeVerb` sur un verbe absent sort 0 sans rien faire.
    #[test]
    fn eject_never_falls_back_to_the_volume_shell_verb() {
        let ps = eject_powershell("I:").expect("shim");
        assert!(
            !ps.contains("InvokeVerb"),
            "no-op silencieux sur un disque vu comme fixe: {ps}"
        );
    }

    /// L'API rend un veto qui nomme le bloqueur. Le remonter est ce qui permettra un message
    /// d'erreur honnête, là où l'ancien chemin ne rendait rien et faisait deviner « Rekordbox ».
    #[test]
    fn eject_surfaces_the_veto_so_the_message_can_name_the_blocker() {
        let ps = eject_powershell("I:").expect("shim");
        assert!(ps.contains("$veto"), "{ps}");
        assert!(ps.contains("vetoName"), "{ps}");
    }

    #[test]
    fn eject_refuses_a_quoted_drive_letter() {
        assert_eq!(eject_powershell("I'; rm -r /"), None);
        assert_eq!(eject_powershell("I\":"), None);
    }

    #[test]
    fn elevation_shim_asks_for_uac_and_keeps_diskpart_output() {
        let ps = elevation_powershell(r"C:\tmp\s.txt", r"C:\tmp\o.log").expect("shim");
        assert!(
            ps.contains("-Verb RunAs"),
            "must raise the UAC prompt: {ps}"
        );
        assert!(
            ps.contains("-Wait"),
            "returning before diskpart finished would report success too early: {ps}"
        );
        assert!(
            ps.contains(r"diskpart /s"),
            "diskpart must still run the script: {ps}"
        );
        assert!(
            ps.contains("o.log") && ps.contains("2>&1"),
            "-Verb RunAs cannot redirect the child; the cmd shim is what keeps the diagnosis: {ps}"
        );
        assert!(
            ps.contains(&UAC_DECLINED_EXIT.to_string()),
            "a declined prompt must be distinguishable from a failure: {ps}"
        );
    }

    /// These are our own tempfile paths, so a quote means something is wrong upstream — and a
    /// mis-escaped one is a command injection into a shell that is about to be *elevated*.
    #[test]
    fn elevation_shim_refuses_quoted_paths() {
        assert_eq!(
            elevation_powershell(r"C:\tmp\a'b.txt", r"C:\tmp\o.log"),
            None
        );
        assert_eq!(
            elevation_powershell(r"C:\tmp\s.txt", "C:\\tmp\\o\".log"),
            None
        );
    }

    /// Live probe against this machine's real WMI. `--ignored` — it reports whatever is
    /// physically plugged in, so it asserts nothing; it exists so that "aucun disque amovible
    /// détecté" can be traced to the stage that dropped the disk instead of guessed at.
    /// `cargo test --manifest-path src-tauri/Cargo.toml usb_format::windows -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_dump_what_this_machine_reports() {
        let storage = WMIConnection::with_namespace_path(STORAGE_NAMESPACE).expect("storage ns");
        let cimv2 = WMIConnection::new().expect("WMIConnection::new");
        let disks: Vec<MsftDisk> = storage.query().expect("MSFT_Disk query");
        let system_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
        println!(
            "--- {} disque(s), SystemDrive={system_drive} ---",
            disks.len()
        );
        for disk in &disks {
            let facts = WindowsBackend::volume_facts(&cimv2, disk.Number);
            let carries_system_volume = facts
                .letters
                .iter()
                .any(|l| l.eq_ignore_ascii_case(&system_drive));
            let raw = RawDiskInfo {
                bus_type: disk.BusType,
                is_boot: disk.IsBoot.unwrap_or(true),
                is_system: disk.IsSystem.unwrap_or(true),
                carries_system_volume,
            };
            println!(
                "disk {} name={:?} bus={} size={:?} boot={:?} system={:?} sn={:?} uid={:?}",
                disk.Number,
                disk.FriendlyName,
                disk.BusType,
                disk.Size,
                disk.IsBoot,
                disk.IsSystem,
                disk.SerialNumber,
                disk.UniqueId
            );
            // The CIMV2 view of the same disk, side by side — this is where a UASP enclosure
            // reveals itself as "SCSI" and where the old filter went wrong.
            let cimv2_view: Vec<HashMap<String, Variant>> = cimv2
                .raw_query(format!(
                    "SELECT InterfaceType, MediaType, PNPDeviceID FROM Win32_DiskDrive \
                     WHERE Index = {}",
                    disk.Number
                ))
                .unwrap_or_default();
            for v in &cimv2_view {
                println!(
                    "    CIMV2: iface={:?} media={:?} pnp={:?}",
                    v.get("InterfaceType"),
                    v.get("MediaType"),
                    v.get("PNPDeviceID")
                );
            }
            println!(
                "    volumes={:?} fs={:?} serials={:?} => offert={}",
                facts.letters,
                facts.filesystems,
                facts.serials,
                is_confidently_removable(&raw)
            );
        }
        let listed = WindowsBackend.list().expect("list()");
        println!("--- list() renvoie {} disque(s) ---", listed.len());
        for d in &listed {
            println!("    {d:?}");
        }
    }
}
