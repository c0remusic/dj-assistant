//! FFmpeg integration via the `ffmpeg-sidecar` crate, pointed at our bundled binary.
//!
//! Binary model (no runtime auto-download):
//! - **release**: Tauri's `externalBin` places the sidecar next to the app binary as
//!   `ffmpeg(.exe)`; `ffmpeg-sidecar` resolves it by default (sibling of current exe).
//! - **dev**: the bundled binary lives at `<manifest>/binaries/ffmpeg-<triple>`; we point
//!   `ffmpeg-sidecar` at it via the `FFMPEG_BINARY` env var.

/// Wire `ffmpeg-sidecar` to our bundled binary. Call once at startup, before any ffmpeg use.
pub fn init_ffmpeg_path() {
    #[cfg(debug_assertions)]
    if let Some(p) = find_bundled_ffmpeg() {
        std::env::set_var("FFMPEG_BINARY", &p);
    }
}

/// Locate the dev-time bundled binary at `<manifest>/binaries/ffmpeg-<triple>(.exe)`.
#[cfg(debug_assertions)]
fn find_bundled_ffmpeg() -> Option<std::path::PathBuf> {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries");
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("ffmpeg-"))
                .unwrap_or(false)
        })
}

/// Returns the bundled ffmpeg version string (respects `FFMPEG_BINARY`).
pub fn version() -> Result<String, String> {
    ffmpeg_sidecar::version::ffmpeg_version().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    /// Validates the dev binary-resolution logic — the exact wiring we de-risk here.
    /// Requires `npm run fetch-ffmpeg` to have populated src-tauri/binaries/.
    #[cfg(debug_assertions)]
    #[test]
    fn finds_bundled_ffmpeg_in_dev() {
        assert!(
            super::find_bundled_ffmpeg().is_some(),
            "no binaries/ffmpeg-* found — run `npm run fetch-ffmpeg`"
        );
    }
}
