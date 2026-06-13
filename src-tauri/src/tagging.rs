//! Write canonical {artist, title} onto an audio file in place, via lofty. Reused at
//! filing time: the same canonical record that renders the filename (see naming.rs) is
//! written here, so tags and name never diverge. Fields we don't own are left untouched.
#![allow(dead_code)]

use lofty::config::WriteOptions;
use lofty::file::TaggedFileExt;
use lofty::prelude::{Accessor, TagExt};
use lofty::probe::Probe;
use lofty::tag::Tag;

/// Set artist + title on `path`, creating a native primary tag if none exists. Returns a
/// human-readable error string on any lofty failure (read, or save).
pub fn write_tags(path: &str, artist: &str, title: &str) -> Result<(), String> {
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
    use super::{read_artist_title, write_tags};
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
}
