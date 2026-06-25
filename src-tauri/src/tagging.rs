//! Write canonical {artist, title} onto an audio file in place, via lofty. Reused at
//! filing time: the same canonical record that renders the filename (see naming.rs) is
//! written here, so tags and name never diverge. Fields we don't own are left untouched.

use lofty::config::WriteOptions;
use lofty::file::TaggedFileExt;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::{Accessor, TagExt};
use lofty::probe::Probe;
use lofty::tag::{ItemKey, Tag};

/// Back-compat: artist + title only (used where no rich metadata is available).
pub fn write_tags(path: &str, artist: &str, title: &str) -> Result<(), String> {
    write_tags_full(path, artist, title, None, None, &[], None)
}

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

#[cfg(test)]
mod tests {
    use super::{read_artist_title, write_tags, write_tags_full};
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

        write_tags(dst, "Larry Heard", "Mystery of Love").expect("write tags");

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
        write_tags(dst, "Chez Damier", "Can You Feel It").unwrap();

        let (a, t) = read_artist_title(dst);
        assert_eq!(a, "Chez Damier");
        assert_eq!(t, "Can You Feel It");
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
