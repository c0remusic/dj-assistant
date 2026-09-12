//! Write canonical {artist, title} onto an audio file in place, via lofty. Reused at
//! filing time: the same canonical record that renders the filename (see naming.rs) is
//! written here, so tags and name never diverge. Fields we don't own are left untouched.

use lofty::config::WriteOptions;
use lofty::file::TaggedFileExt;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::{Accessor, TagExt};
use lofty::probe::Probe;
use lofty::tag::items::Timestamp;
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
    // ItemKey::Publisher, not ItemKey::Label: lofty writes ItemKey::Label to the same ID3v2 TPUB
    // frame, but reads that frame back as ItemKey::Publisher — Label never round-tripped under
    // its own key on any format (confirmed empirically: MP3 and WAV both lost it on reload).
    if let Some(l) = label.filter(|s| !s.trim().is_empty()) {
        tag.insert_text(ItemKey::Publisher, l.to_string());
    }
    if let Some(y) = year {
        if y > 0 {
            tag.set_date(Timestamp {
                year: y as u16,
                ..Default::default()
            });
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
            let pic = Picture::unchecked(bytes)
                .pic_type(PictureType::CoverFront)
                .mime_type(mime)
                .build();
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
    /// Serialized as a base85 RFC1924 string (see `crate::b85_bytes`) instead of serde's default
    /// array of decimal integers — a cover is the single biggest thing this repo writes to JSON
    /// (measured 2026-07-27: 4 rows of `actions.meta` at ~43 MB each for ~11 MB of real image).
    /// Deserialization stays tolerant of the historic array form ON PURPOSE: these bytes are NOT
    /// recomputable, so refusing to read an old row would not be a cache miss, it would destroy
    /// the undo of a tag edit already applied to the file.
    #[serde(with = "crate::b85_bytes")]
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
        label: tag.get_string(ItemKey::Publisher).map(|s| s.to_string()),
        year: tag.date().map(|d| d.year as i64),
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
            tag.insert_text(ItemKey::Publisher, l.clone());
        }
        None => tag.remove_key(ItemKey::Publisher),
    }
    match snap.year {
        Some(y) if y > 0 => tag.set_date(Timestamp {
            year: y as u16,
            ..Default::default()
        }),
        _ => tag.remove_date(),
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
        let mut builder = Picture::unchecked(cov.bytes.clone()).pic_type(PictureType::CoverFront);
        if let Some(m) = mime {
            builder = builder.mime_type(m);
        }
        let pic = builder.build();
        tag.push_picture(pic);
    }

    tag.save_to_path(path, WriteOptions::default())
        .map_err(|e| format!("save tags: {e}"))
}

#[cfg(test)]
mod tests {
    use super::{
        read_artist_title, read_tags_full, restore_tags, write_tags_full, CoverSnap, TagsSnapshot,
    };
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

    /// THE retro-compatibility guard for `actions.meta`. The end-to-end test
    /// (`actions::tests::revert_tag_edit_restores_tags_without_touching_status_or_metadata`)
    /// produces AND consumes in the same run, so it can never catch a regression on rows written
    /// by an older version. This one deserializes a literal in the HISTORIC format (cover bytes
    /// as an array of decimal integers) — exactly what the 26 rows already in production hold.
    /// No fixture, so it always actually runs.
    #[test]
    fn deserializes_the_historic_integer_array_cover_format() {
        let historic = r#"{"artist":null,"title":null,"label":null,"year":null,"genre_joined":null,"cover":{"mime":"image/png","bytes":[137,80,78,71,13,10,26,10]}}"#;
        let snap: TagsSnapshot = serde_json::from_str(historic).expect("historic meta must parse");
        let cover = snap.cover.expect("cover present");
        assert_eq!(cover.mime.as_deref(), Some("image/png"));
        assert_eq!(cover.bytes, vec![137u8, 80, 78, 71, 13, 10, 26, 10]);
    }

    /// New rows must be written in the compact form, and read back byte-identical.
    #[test]
    fn new_cover_format_is_base85_and_round_trips() {
        let snap = TagsSnapshot {
            artist: Some("A".into()),
            title: Some("T".into()),
            label: None,
            year: Some(1999),
            genre_joined: None,
            cover: Some(CoverSnap {
                mime: Some("image/png".into()),
                bytes: vec![137, 80, 78, 71, 13, 10, 26, 10],
            }),
        };
        let j = serde_json::to_string(&snap).unwrap();
        assert!(
            j.contains(r#""bytes":"iBL{Q4GJ0x""#),
            "cover bytes must be a base85 string: {j}"
        );
        assert!(
            !j.contains(r#""bytes":["#),
            "cover bytes regressed to an array"
        );
        let back: TagsSnapshot = serde_json::from_str(&j).unwrap();
        assert_eq!(back, snap);
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
        assert_eq!(tag.get_string(ItemKey::TrackArtist), Some("Larry Heard"));
        assert_eq!(tag.get_string(ItemKey::TrackTitle), Some("Mystery of Love"));
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
        assert_ne!(
            after_apply, before,
            "the apply must actually change the tags"
        );

        // Revert: restore the captured snapshot, then it must equal the original byte-for-byte.
        restore_tags(dst, &before).expect("restore old tags");
        let after_restore = read_tags_full(dst).expect("snapshot after restore");
        assert_eq!(
            after_restore, before,
            "restore must reproduce the original tags exactly"
        );
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
        assert_eq!(tag.get_string(ItemKey::TrackArtist), Some("Larry Heard"));
        let genre = tag.get_string(ItemKey::Genre).unwrap_or("");
        assert!(
            genre.contains("Deep House") && genre.contains("House"),
            "genre = {genre:?}"
        );
        assert!(!tag.pictures().is_empty(), "cover embedded");
    }
}

#[cfg(test)]
mod label_year_regression {
    use super::{read_tags_full, write_tags_full};

    fn fixture(name: &str) -> Option<String> {
        let p = format!("fixtures/{name}");
        if std::path::Path::new(&p).exists() {
            Some(p)
        } else {
            None
        }
    }

    // Regression test for a real bug found via annotation ("Pourquoi ça reste jaune même quand
    // j'applique ?" / "C'est TOUT LE TEMPS affiché"): ItemKey::Label writes to ID3v2's TPUB frame,
    // but lofty reads that same frame back as ItemKey::Publisher, never ItemKey::Label — so the
    // label NEVER round-tripped, on any format, permanently keeping the discrepancy marker + CDJ
    // warning banner stuck on even right after a successful Apply. Fixed by using
    // ItemKey::Publisher consistently (write_tags_full/read_tags_full/restore_tags).
    #[test]
    fn label_roundtrips_mp3() {
        let Some(src) = fixture("real_320.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let dst = dir.path().join("label_rt.mp3");
        std::fs::copy(&src, &dst).unwrap();
        let dst = dst.to_str().unwrap();
        write_tags_full(
            dst,
            "Larry Heard",
            "Mystery of Love",
            Some("Permanent Vacation"),
            Some(2008),
            &["House".to_string()],
            None,
        )
        .expect("write");
        let snap = read_tags_full(dst).expect("read");
        assert_eq!(snap.label.as_deref(), Some("Permanent Vacation"));
        assert_eq!(snap.year, Some(2008));
    }

    #[test]
    fn label_roundtrips_wav() {
        let Some(src) = fixture("dual_mono.wav") else {
            eprintln!("skip: no fixture");
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let dst = dir.path().join("label_rt.wav");
        std::fs::copy(&src, &dst).unwrap();
        let dst = dst.to_str().unwrap();
        write_tags_full(
            dst,
            "Larry Heard",
            "Mystery of Love",
            Some("Permanent Vacation"),
            Some(2008),
            &["House".to_string()],
            None,
        )
        .expect("write");
        let snap = read_tags_full(dst).expect("read");
        assert_eq!(snap.label.as_deref(), Some("Permanent Vacation"));
        assert_eq!(snap.year, Some(2008));
    }
}
