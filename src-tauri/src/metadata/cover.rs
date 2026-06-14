//! Cover-art cache. Covers are downloaded into a per-app cache dir keyed by Discogs release id
//! so the same release isn't re-fetched. Download is best-effort: failures are non-fatal (the
//! caller applies metadata anyway). Only the path mapping is unit-tested (no network in CI).
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// The cache path for a release's cover. `release_id` is sanitized so it can't escape `dir`.
pub fn cover_path(dir: &Path, release_id: &str) -> PathBuf {
    let safe: String = release_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    dir.join(format!("{safe}.jpg"))
}

/// Download `url` into the cache for `release_id`, returning the path. Idempotent: if the file
/// already exists it is returned without re-downloading. Best-effort — the caller treats Err
/// as "no cover" and proceeds.
pub fn download_cover(dir: &Path, release_id: &str, url: &str) -> Result<PathBuf, String> {
    let out = cover_path(dir, release_id);
    if out.exists() {
        return Ok(out);
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let resp = ureq::get(url)
        .set("User-Agent", concat!("Sift/", env!("CARGO_PKG_VERSION")))
        .call()
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    use std::io::Read;
    resp.into_reader()
        .take(10 * 1024 * 1024) // cap at 10 MB
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    std::fs::write(&out, &bytes).map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_is_under_dir_and_keyed_by_release_id() {
        let dir = std::path::Path::new("/cache/covers");
        let p = cover_path(dir, "12345");
        assert_eq!(p, std::path::Path::new("/cache/covers/12345.jpg"));
    }

    #[test]
    fn release_id_is_sanitized() {
        let dir = std::path::Path::new("/cache/covers");
        let p = cover_path(dir, "a/b");
        assert_eq!(p, std::path::Path::new("/cache/covers/a_b.jpg"));
    }
}
