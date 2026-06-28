//! Write canonical {artist, title} onto an audio file in place, via lofty. Reused at
//! filing time: the same canonical record that renders the filename (see naming.rs) is
//! written here, so tags and name never diverge. Fields we don't own are left untouched.

use lofty::config::WriteOptions;
use lofty::file::TaggedFileExt;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::{Accessor, TagExt};
use lofty::probe::Probe;
use lofty::tag::{ItemKey, Tag};
use serde::{Deserialize, Serialize};

/// Write the full canonical+enrichment set: artist, title, and optionally label, year,
/// genres (joined as "A; B" in one Genre field — multi-item doesn't round-trip on ID3),
/// and an embedded front cover read from `cover_path`.
/// Fields left None/empty are not touched. Returns a human-readable error on any lofty failure.
pub fn write_tags_full(
    path: &str,
    artist: &str,
    title: &str,
    label: Option<&str>,
    year: Option<i64>,
    genres: &[String],
    cover_path: Option<&str>,
) -> Result<(), String> {
    let mut tagged = Probe::open(path)
        .and_then(|p| p.read())
        .map_err(|e| format!("read tags: {e}"))?;

    if tagged.primary_tag_mut().is_none() {
        let tt = tagged.primary_tag_type();
        tagged.insert_tag(Tag::new(tt));
    }
    let tag = tagged
        .primary_tag_mut()
        .ok_or_else(|| "could not create a tag for this file".to_string())?;

    tag.set_artist(artist.to_string());
    tag.set_title(title.to_string());
    if let Some(l) = label.filter(|s| !s.trim().is_empty()) {
        tag.insert_text(ItemKey::Label, l.to_string());
    }
    if let Some(y) = year {
        if y > 0 {
            tag.set_year(y as u32);
        }
    }
    // Genres are joined into one field ("Deep House; House"): multiple same-key items don't
    // round-trip on ID3, and Rekordbox/CDJ read a single genre field. The structured per-genre
    // list is kept in the DB (track_genres); the embedded tag gets the joined form.
    let joined: String = genres
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("; ");
    if !joined.is_empty() {
        tag.set_genre(joined);
    }
    if let Some(cp) = cover_path {
        if let Ok(bytes) = std::fs::read(cp) {
            let mime = if cp.to_lowercase().ends_with(".png") {
                MimeType::Png
            } else {
                MimeType::Jpeg
            };
            let pic = Picture::new_unchecked(PictureType::CoverFront, Some(mime), None, bytes);
            // Replace, don't accumulate: re-identifying a track must not leave the old cover
            // embedded alongside the new one.
            tag.remove_picture_type(PictureType::CoverFront);
            tag.push_picture(pic);
        }
    }

    tag.save_to_path(path, WriteOptions::default())
        .map_err(|e| format!("save tags: {e}"))
}

/// Read embedded artist + title (empty strings when absent or unreadable). Used by filing
/// to seed reconciliation.
pub fn read_artist_title(path: &str) -> (String, String) {
    match Probe::open(path).and_then(|p| p.read()) {
        Ok(tagged) => match tagged.primary_tag() {
            Some(tag) => (
                tag.artist().map(|s| s.to_string()).unwrap_or_default(),
                tag.title().map(|s| s.to_string()).unwrap_or_default(),
            ),
            None => (String::new(), String::new()),
        },
        Err(_) => (String::new(), String::new()),
    }
}

/// Bytes + mime of one embedded cover, captured so a revert can re-embed the exact image.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverSnap {
    pub mime: Option<String>,
    pub bytes: Vec<u8>,
}

/// A snapshot of EXACTLY the tag fields `write_tags_full` owns, captured before an Apply so the
/// edit is fully reversible. Each field is `None` when the source had no such frame, so a revert
/// can faithfully RESTORE an originally-empty field instead of leaving the applied value behind.
/// The cover bytes are embedded here (this struct is serialized to JSON into `actions.meta`)
/// rather than backed up to a side file: self-contained means a revert can never be orphaned by a
/// missing backup, at the cost of a larger journal row for the rare tag edit. `read_tags_full`
/// fills it; `restore_tags` is its exact inverse — the two MUST cover the same fields as
/// `write_tags_full` or a revert would be incomplete.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TagsSnapshot {
    pub artist: Option<String>,
    pub title: Option<String>,
    pub label: Option<String>,
    pub year: Option<i64>,
    pub genre_joined: Option<String>,
    pub cover: Option<CoverSnap>,
}

/// Read the SAME fields `write_tags_full` writes (artist, title, label, year, the joined Genre,
/// and the front cover) into a snapshot, so it fully covers what an Apply can change. Errors only
/// if the file can't be opened/parsed; a file with no tag yields an all-`None` snapshot (apply →
/// revert then returns it to "no tags").
pub fn read_tags_full(path: &str) -> Result<TagsSnapshot, String> {
    let tagged = Probe::open(path)
        .and_then(|p| p.read())
        .map_err(|e| format!("read tags: {e}"))?;
    let Some(tag) = tagged.primary_tag() else {
        return Ok(TagsSnapshot::default());
    };
    let cover = tag
        .pictures()
        .iter()
        .find(|p| p.pic_type() == PictureType::CoverFront)
        .map(|p| CoverSnap {
            mime: p.mime_type().map(|m| m.as_str().to_string()),
            bytes: p.data().to_vec(),
        });
    Ok(TagsSnapshot {
        artist: tag.artist().map(|s| s.to_string()),
        title: tag.title().map(|s| s.to_string()),
        label: tag.get_string(&ItemKey::Label).map(|s| s.to_string()),
        year: tag.year().map(|y| y as i64),
        genre_joined: tag.genre().map(|s| s.to_string()),
        cover,
    })
}

/// Faithful inverse of an Apply: make the file's tags EXACTLY match `snap`. Unlike
/// `write_tags_full` (which leaves `None`/empty fields untouched), this SETS *or* REMOVES each
/// owned field — so a field that was empty before the Apply is cleared again, not left with the
/// applied value. Used by the `tag_edit` revert branch. The save is the last step, so a failure
/// before it leaves the file unchanged.
pub fn restore_tags(path: &str, snap: &TagsSnapshot) -> Result<(), String> {
    let mut tagged = Probe::open(path)
        .and_then(|p| p.read())
        .map_err(|e| format!("read tags: {e}"))?;
    if tagged.primary_tag_mut().is_none() {
        let tt = tagged.primary_tag_type();
        tagged.insert_tag(Tag::new(tt));
    }
    let tag = tagged
        .primary_tag_mut()
        .ok_or_else(|| "could not access a tag for this file".to_string())?;

    match &snap.artist {
        Some(a) => tag.set_artist(a.clone()),
        None => tag.remove_artist(),
    }
    match &snap.title {
        Some(t) => tag.set_title(t.clone()),
        None => tag.remove_title(),
    }
    match &snap.label {
        Some(l) => {
            tag.insert_text(ItemKey::Label, l.clone());
        }
        None => tag.remove_key(&ItemKey::Label),
    }
    match snap.year {
        Some(y) if y > 0 => tag.set_year(y as u32),
        _ => tag.remove_year(),
    }
    match &snap.genre_joined {
        Some(g) => tag.set_genre(g.clone()),
        None => tag.remove_genre(),
    }
    // Cover: drop any current front cover, then re-embed the snapshot's exact bytes (if it had one).
    tag.remove_picture_type(PictureType::CoverFront);
    if let Some(cov) = &snap.cover {
        let mime = cov.mime.as_deref().map(|s| match s {
            "image/png" => MimeType::Png,
            "image/jpeg" => MimeType::Jpeg,
            other => MimeType::Unknown(other.to_string()),
        });
        let pic = Picture::new_unchecked(PictureType::CoverFront, mime, None, cov.bytes.clone());
        tag.push_picture(pic);
    }

    tag.save_to_path(path, WriteOptions::default())
        .map_err(|e| format!("save tags: {e}"))
}

#[cfg(test)]
mod tests {
    use super::{read_artist_title, read_tags_full, restore_tags, write_tags_full};
    use lofty::file::TaggedFileExt;
    use lofty::probe::Probe;
    use lofty::tag::ItemKey;

    fn fixture(name: &str) -> Option<String> {
        let p = format!("fixtures/{name}");
        if std::path::Path::new(&p).exists() {
            Some(p)
        } else {
            None
        }
    }

    #[test]
    fn writes_and_reads_back_artist_title() {
        let Some(src) = fixture("real_320.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let dst = dir.path().join("tagged.mp3");
        std::fs::copy(&src, &dst).unwrap();
        let dst = dst.to_str().unwrap();

        write_tags_full(dst, "Larry Heard", "Mystery of Love", None, None, &[], None)
            .expect("write tags");

        let tagged = Probe::open(dst).unwrap().read().unwrap();
        let tag = tagged.primary_tag().expect("has tag");
        assert_eq!(tag.get_string(&ItemKey::TrackArtist), Some("Larry Heard"));
        assert_eq!(tag.get_string(&ItemKey::TrackTitle), Some("Mystery of Love"));
    }

    #[test]
    fn read_artist_title_after_write() {
        let Some(src) = fixture("real_320.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let dst = dir.path().join("rt.mp3");
        std::fs::copy(&src, &dst).unwrap();
        let dst = dst.to_str().unwrap();
        write_tags_full(dst, "Chez Damier", "Can You Feel It", None, None, &[], None).unwrap();

        let (a, t) = read_artist_title(dst);
        assert_eq!(a, "Chez Damier");
        assert_eq!(t, "Can You Feel It");
    }

    #[test]
    fn apply_then_restore_round_trips_to_original_tags() {
        let Some(src) = fixture("real_320.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let dst = dir.path().join("rt_full.mp3");
        std::fs::copy(&src, &dst).unwrap();
        let dst = dst.to_str().unwrap();

        // The state we must come back to, captured exactly like apply_tags does.
        let before = read_tags_full(dst).expect("snapshot original");

        // Apply a full set of NEW tags (incl. a cover), overwriting whatever was there.
        let cover = dir.path().join("c.jpg");
        std::fs::write(&cover, b"\xFF\xD8\xFFnewcover").unwrap();
        write_tags_full(
            dst,
            "NEW Artist",
            "NEW Title",
            Some("NEW Label"),
            Some(2024),
            &["Acid".to_string(), "Techno".to_string()],
            Some(cover.to_str().unwrap()),
        )
        .expect("apply new tags");
        let after_apply = read_tags_full(dst).expect("snapshot after apply");
        assert_ne!(after_apply, before, "the apply must actually change the tags");

        // Revert: restore the captured snapshot, then it must equal the original byte-for-byte.
        restore_tags(dst, &before).expect("restore old tags");
        let after_restore = read_tags_full(dst).expect("snapshot after restore");
        assert_eq!(after_restore, before, "restore must reproduce the original tags exactly");
    }

    #[test]
    fn writes_label_year_genres_and_cover() {
        let Some(src) = fixture("real_320.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let dst = dir.path().join("full.mp3");
        std::fs::copy(&src, &dst).unwrap();
        let dst = dst.to_str().unwrap();

        let cover = dir.path().join("c.jpg");
        std::fs::write(&cover, b"\xFF\xD8\xFFimagedata").unwrap();

        write_tags_full(
            dst,
            "Larry Heard",
            "Mystery of Love",
            Some("Alleviated"),
            Some(1986),
            &["Deep House".to_string(), "House".to_string()],
            Some(cover.to_str().unwrap()),
        )
        .expect("write full tags");

        use lofty::file::TaggedFileExt;
        use lofty::probe::Probe;
        use lofty::tag::ItemKey;
        let tagged = Probe::open(dst).unwrap().read().unwrap();
        let tag = tagged.primary_tag().expect("has tag");
        assert_eq!(tag.get_string(&ItemKey::TrackArtist), Some("Larry Heard"));
        let genre = tag.get_string(&ItemKey::Genre).unwrap_or("");
        assert!(genre.contains("Deep House") && genre.contains("House"), "genre = {genre:?}");
        assert!(!tag.pictures().is_empty(), "cover embedded");
    }
}
