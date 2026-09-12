//! macOS backend: enumerate removable disks via `diskutil list -plist`, format via
//! `diskutil eraseDisk`. NOT exercised end-to-end in this repo (no Mac available) -- only the
//! pure parsing/filter functions below are unit tested here; the `RemovableDriveBackend` impl
//! (which shells out to `diskutil`) is gated `#[cfg(target_os = "macos")]` and has never run.

#[cfg(target_os = "macos")]
use super::{RemovableDrive, RemovableDriveBackend, TargetFs, UsbFormatError};
#[cfg(target_os = "macos")]
use std::process::Command;

#[cfg(target_os = "macos")]
pub struct MacBackend;

/// The two `diskutil` plist flags the removable filter needs, isolated from all other plist
/// content so the filter has zero parsing/process dependency and can be unit tested directly.
/// Only consumed by `MacBackend::list` (macOS-only) outside of tests — `allow(dead_code)` on
/// non-macOS is deliberate: this struct exists specifically so the logic below is verifiable on
/// any OS via `cargo test`, per the M7 design's cross-platform-testability requirement.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) struct RawDiskEntry {
    pub removable_media: Option<bool>,
    pub internal: Option<bool>,
}

/// Conservative removable check, matching the Windows filter's shape: BOTH flags must agree
/// removable AND external. Any missing flag excludes the disk.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn is_confidently_removable(disk: &RawDiskEntry) -> bool {
    matches!(disk.removable_media, Some(true)) && matches!(disk.internal, Some(false))
}

/// One partition entry parsed out of `diskutil list -plist` output. Same cross-platform-testing
/// rationale as `RawDiskEntry` above.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ParsedDisk {
    pub id: String,
    pub name: String,
    pub size_bytes: u64,
}

/// Minimal, dependency-free plist scanner: extracts `DeviceIdentifier`/`VolumeName`/`Size`
/// triples from `<key>..</key><string|integer>..</string|integer>` pairs inside the
/// `Partitions` arrays. Deliberately narrow -- not a general plist parser -- because only these
/// three fields are needed and pulling in a full plist crate for them isn't justified (repo
/// dependency-minimalism convention, CLAUDE.md dependency audit).
///
/// **Whole-disk vs partition identifier**: `diskutil list -plist`'s `AllDisksAndPartitions`
/// array holds one dict per *whole disk* (e.g. `disk4`), each with its own top-level
/// `DeviceIdentifier` and a nested `Partitions` array whose dicts have their *own*
/// `DeviceIdentifier` (e.g. `disk4s1`) for each partition/slice. `diskutil eraseDisk` takes the
/// whole-disk identifier, not a partition's -- so `ParsedDisk.id` must come from the outer dict's
/// `DeviceIdentifier`, captured before the nested `Partitions` array's own `DeviceIdentifier`
/// keys are seen (`whole_disk_id` tracks it separately from the per-partition `id` below).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn parse_disk_entries(plist_xml: &str) -> Vec<ParsedDisk> {
    let mut result = Vec::new();
    let mut whole_disk_id: Option<String> = None;
    let mut name: Option<String> = None;
    let mut size: Option<u64> = None;
    // `<array>` nesting depth, plus the depth at which the current `Partitions` array (if any)
    // was opened. Depth-based rather than a plain bool: with multiple whole disks in
    // `AllDisksAndPartitions`, a bool would stay stuck "true" forever after the first disk's
    // `Partitions` array, causing every later disk's own DeviceIdentifier to be mistaken for a
    // partition id.
    let mut array_depth = 0u32;
    let mut partitions_depth: Option<u32> = None;

    let mut pending_key: Option<String> = None;

    let mut idx = 0usize;
    while idx < plist_xml.len() {
        let rest = &plist_xml[idx..];
        if !rest.starts_with('<') {
            idx += 1;
            continue;
        }
        if rest.starts_with("</array>") {
            if partitions_depth == Some(array_depth) {
                partitions_depth = None;
            }
            array_depth = array_depth.saturating_sub(1);
            idx += "</array>".len();
            continue;
        } else if rest.starts_with("<array>") {
            array_depth += 1;
            idx += "<array>".len();
            continue;
        } else if rest.starts_with("<key>") {
            if let Some(end) = rest.find("</key>") {
                let key = rest[5..end].to_string();
                if key == "Partitions" {
                    // The array opening tag for this key comes right after it; record the
                    // depth it will open at.
                    partitions_depth = Some(array_depth + 1);
                }
                pending_key = Some(key);
                idx += end + "</key>".len();
                continue;
            }
        } else if rest.starts_with("<string>") {
            if let Some(end) = rest.find("</string>") {
                if let Some(key) = pending_key.take() {
                    let value = rest[8..end].to_string();
                    // Outside any `Partitions` array, DeviceIdentifier is the whole disk's own
                    // id -- update it. Inside one, it belongs to a partition and must not
                    // overwrite the whole-disk id it's nested under.
                    let in_partitions = partitions_depth.is_some_and(|d| array_depth >= d);
                    match key.as_str() {
                        "DeviceIdentifier" if !in_partitions => whole_disk_id = Some(value),
                        "VolumeName" => name = Some(value),
                        _ => {}
                    }
                }
                idx += end + "</string>".len();
                continue;
            }
        } else if rest.starts_with("<integer>") {
            if let Some(end) = rest.find("</integer>") {
                if let Some(key) = pending_key.take() {
                    let value = rest[9..end].to_string();
                    if key == "Size" {
                        size = value.parse().ok();
                    }
                }
                idx += end + "</integer>".len();
                continue;
            }
        }
        idx += 1;

        // A partition record is complete once we have the whole-disk id + name + size together
        // (VolumeName is the last of the three fields to appear in real diskutil output for a
        // given partition). whole_disk_id is intentionally NOT cleared afterwards: a disk can
        // have multiple partitions, all belonging to the same whole disk.
        if let (Some(pid), Some(pname), Some(psize)) = (&whole_disk_id, &name, size) {
            result.push(ParsedDisk {
                id: pid.clone(),
                name: pname.clone(),
                size_bytes: psize,
            });
            name = None;
            size = None;
        }
    }

    result
}

#[cfg(target_os = "macos")]
impl RemovableDriveBackend for MacBackend {
    fn list(&self) -> Result<Vec<RemovableDrive>, UsbFormatError> {
        let output = Command::new("diskutil")
            .args(["list", "-plist"])
            .output()
            .map_err(|e| UsbFormatError::Enumeration(format!("spawn diskutil list: {e}")))?;
        if !output.status.success() {
            return Err(UsbFormatError::Enumeration(format!(
                "diskutil list exited with {:?}",
                output.status.code()
            )));
        }
        let plist_xml = String::from_utf8_lossy(&output.stdout);

        // Cross-check each parsed partition against `diskutil info -plist <id>` for the
        // removable/internal flags and volume UUID (not present in the `list` output's
        // partition dicts) -- conservative: any info-lookup failure excludes that disk.
        let mut drives = Vec::new();
        for disk in parse_disk_entries(&plist_xml) {
            let info_output = Command::new("diskutil")
                .args(["info", "-plist", &disk.id])
                .output()
                .map_err(|e| UsbFormatError::Enumeration(format!("spawn diskutil info: {e}")))?;
            if !info_output.status.success() {
                continue;
            }
            let info_xml = String::from_utf8_lossy(&info_output.stdout);
            let removable_media = info_xml.contains("<key>RemovableMedia</key>\n\t<true/>");
            let internal = info_xml.contains("<key>Internal</key>\n\t<true/>");
            let raw = RawDiskEntry {
                removable_media: Some(removable_media),
                internal: Some(internal),
            };
            if !is_confidently_removable(&raw) {
                continue;
            }
            // A missing VolumeUUID used to exclude the disk. That is the same defect the Windows
            // backend carried until 2026-07-31: an unformatted disk has no volume UUID, and an
            // unformatted disk is exactly what this tool formats. The UUID now only *sharpens*
            // the identity anchor; the whole-disk identifier and size carry it when there is none.
            let volume_uuid =
                extract_plist_string_value(&info_xml, "VolumeUUID").unwrap_or_default();
            let fs = extract_plist_string_value(&info_xml, "FilesystemType")
                .unwrap_or_else(|| "non formaté".to_string());
            let mount = extract_plist_string_value(&info_xml, "MountPoint").unwrap_or_default();
            drives.push(RemovableDrive {
                id: disk.id.clone(),
                label: disk.name.clone(),
                mount,
                size_bytes: disk.size_bytes,
                // diskutil expose FreeSpace ailleurs ; non câblé tant que ce backend n'a jamais tourné.
                free_bytes: 0,
                current_fs: fs,
                volume_name: String::new(),
                health: String::new(),
                has_media: disk.size_bytes > 0,
                identity: format!("{}|{}|{}", disk.id, disk.size_bytes, volume_uuid),
            });
        }
        Ok(drives)
    }

    fn eject(&self, drive: &RemovableDrive) -> Result<(), UsbFormatError> {
        let output = Command::new("diskutil")
            .args(["eject", &drive.id])
            .output()
            .map_err(|e| UsbFormatError::Enumeration(format!("spawn diskutil eject: {e}")))?;
        if output.status.success() {
            return Ok(());
        }
        // `diskutil eject` est synchrone et rapporte l'échec, contrairement au verbe shell de
        // Windows — pas de re-listage à faire ici. Un volume tenu ouvert donne "Unable to eject".
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("busy") || stderr.contains("Unable to eject") {
            return Err(UsbFormatError::EjectBusy);
        }
        Err(UsbFormatError::Enumeration(format!(
            "diskutil eject exited with {:?}: {stderr}",
            output.status.code()
        )))
    }

    fn format(
        &self,
        drive: &RemovableDrive,
        fs: TargetFs,
        label: &str,
    ) -> Result<(), UsbFormatError> {
        let fs_name = match fs {
            TargetFs::Fat32 => "FAT32",
            TargetFs::ExFat => "ExFAT",
        };
        let output = Command::new("diskutil")
            .args(["eraseDisk", fs_name, label, &drive.id])
            .output()
            .map_err(|e| UsbFormatError::Format(format!("spawn diskutil eraseDisk: {e}")))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(UsbFormatError::Format(format!(
                "diskutil eraseDisk exited with {:?}: {stderr}",
                output.status.code()
            )));
        }
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn extract_plist_string_value(plist_xml: &str, key: &str) -> Option<String> {
    let marker = format!("<key>{key}</key>\n\t<string>");
    let start = plist_xml.find(&marker)? + marker.len();
    let end = plist_xml[start..].find("</string>")?;
    Some(plist_xml[start..start + end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removable_and_external_is_included() {
        let disk = RawDiskEntry {
            removable_media: Some(true),
            internal: Some(false),
        };
        assert!(is_confidently_removable(&disk));
    }

    #[test]
    fn internal_disk_is_excluded() {
        let disk = RawDiskEntry {
            removable_media: Some(true),
            internal: Some(true),
        };
        assert!(!is_confidently_removable(&disk));
    }

    #[test]
    fn non_removable_media_is_excluded() {
        let disk = RawDiskEntry {
            removable_media: Some(false),
            internal: Some(false),
        };
        assert!(!is_confidently_removable(&disk));
    }

    #[test]
    fn missing_removable_media_flag_is_excluded() {
        let disk = RawDiskEntry {
            removable_media: None,
            internal: Some(false),
        };
        assert!(!is_confidently_removable(&disk));
    }

    #[test]
    fn missing_internal_flag_is_excluded() {
        let disk = RawDiskEntry {
            removable_media: Some(true),
            internal: None,
        };
        assert!(!is_confidently_removable(&disk));
    }

    #[test]
    fn parse_disk_entries_extracts_one_external_usb_disk() {
        let plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>AllDisksAndPartitions</key>
    <array>
        <dict>
            <key>DeviceIdentifier</key>
            <string>disk4</string>
            <key>Partitions</key>
            <array>
                <dict>
                    <key>DeviceIdentifier</key>
                    <string>disk4s1</string>
                    <key>VolumeName</key>
                    <string>SIFT_USB</string>
                    <key>Size</key>
                    <integer>16000000000</integer>
                </dict>
            </array>
        </dict>
    </array>
</dict>
</plist>"#;
        let entries = parse_disk_entries(plist);
        assert_eq!(entries.len(), 1);
        // Must be the WHOLE-DISK identifier ("disk4"), not the partition slice ("disk4s1") --
        // `diskutil eraseDisk` takes the whole disk. Regression test for the id-capture bug.
        assert_eq!(entries[0].id, "disk4");
        assert_eq!(entries[0].name, "SIFT_USB");
        assert_eq!(entries[0].size_bytes, 16_000_000_000);
    }

    #[test]
    fn parse_disk_entries_uses_whole_disk_id_with_multiple_partitions() {
        // Realistic case: a USB key partitioned into two slices under the same physical disk
        // (e.g. a small EFI/reserved partition + the main data partition). Both entries must
        // report the SAME whole-disk id ("disk5"), never a partition slice id ("disk5s1"/"disk5s2").
        let plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>AllDisksAndPartitions</key>
    <array>
        <dict>
            <key>DeviceIdentifier</key>
            <string>disk3</string>
            <key>Partitions</key>
            <array>
                <dict>
                    <key>DeviceIdentifier</key>
                    <string>disk3s1</string>
                    <key>VolumeName</key>
                    <string>Macintosh HD</string>
                    <key>Size</key>
                    <integer>500000000000</integer>
                </dict>
            </array>
        </dict>
        <dict>
            <key>DeviceIdentifier</key>
            <string>disk5</string>
            <key>Partitions</key>
            <array>
                <dict>
                    <key>DeviceIdentifier</key>
                    <string>disk5s1</string>
                    <key>VolumeName</key>
                    <string>EFI</string>
                    <key>Size</key>
                    <integer>209715200</integer>
                </dict>
                <dict>
                    <key>DeviceIdentifier</key>
                    <string>disk5s2</string>
                    <key>VolumeName</key>
                    <string>SIFT_USB</string>
                    <key>Size</key>
                    <integer>15790284800</integer>
                </dict>
            </array>
        </dict>
    </array>
</dict>
</plist>"#;
        let entries = parse_disk_entries(plist);
        assert_eq!(entries.len(), 3);

        assert_eq!(entries[0].id, "disk3");
        assert_eq!(entries[0].name, "Macintosh HD");

        // Both partitions on the second physical disk must resolve to "disk5", not "disk5s1"/
        // "disk5s2" -- this is the exact scenario the bug would get wrong.
        assert_eq!(entries[1].id, "disk5");
        assert_eq!(entries[1].name, "EFI");
        assert_eq!(entries[2].id, "disk5");
        assert_eq!(entries[2].name, "SIFT_USB");
    }
}
