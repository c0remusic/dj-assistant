//! Disk scanning + reconciliation. Pure-ish logic: given a folder and the DB,
//! computes which audio files to add / update / drop from the queue.
use std::path::Path;

/// Audio extensions Sift queues. Everything else on disk is ignored.
const AUDIO_EXTS: &[&str] = &["mp3", "flac", "wav", "aif", "aiff", "m4a", "aac", "ogg", "opus"];

/// True if `path` has a recognised audio extension (case-insensitive).
pub fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn audio_extensions_are_recognised() {
        assert!(is_audio(Path::new("a/b/track.mp3")));
        assert!(is_audio(Path::new("track.FLAC"))); // case-insensitive
        assert!(is_audio(Path::new("x.aiff")));
        assert!(!is_audio(Path::new("cover.jpg")));
        assert!(!is_audio(Path::new("notes.txt")));
        assert!(!is_audio(Path::new("no_extension")));
    }
}
