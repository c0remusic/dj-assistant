# M4-2 — Encoder + tagging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Two Rust modules — `encode` (decide the rail-based target, test conformance, transcode to MP3 320 / AIFF 16-bit 44.1 via ffmpeg, never upscale) and `tagging` (write `{artist, title}` onto a file via lofty).

**Architecture:** `encode.rs` wraps `ffmpeg-sidecar` exactly like `analysis/decode.rs` (spawn `FfmpegCommand`, iterate `child.iter()`, surface `Log(Error)`/`Error` events). Conformance and tagging read/write properties via `lofty` (the same crate `analysis/tags.rs` already uses for reading). Both modules take the source rail as a parameter, so they have no dependency on the analysis pipeline. Pure decisions (`target_for`, `guard_no_upscale`, `Target::ext`) are inline-unit-tested; the ffmpeg/lofty paths are integration-tested against the `fixtures/` files, skipping gracefully when absent (the project convention, see `tests/characterization.rs`).

**Tech Stack:** Rust (edition 2021, MSRV 1.77.2), `ffmpeg-sidecar` 2.5, `lofty` 0.22, `serde`, `tempfile` (dev-dep, already present). No new dependencies.

**This is plan 2 of 4 for M4** (naming ✓ → **encode/tagging** → filing/library/actions → frontend). M4-3 consumes `target_for`, `is_conformant`, `guard_no_upscale`, `encode`, and `write_tags`.

---

## Conventions for this plan

- cargo is **not** on PATH: `& "$env:USERPROFILE\.cargo\bin\cargo.exe"` (PowerShell), run from `src-tauri/`.
- `cargo test --lib encode` / `--lib tagging` runs just one module's tests and avoids
  linking `sift.exe` (locked while the app runs → "Accès refusé os error 5"; if hit,
  `Stop-Process -Name sift`).
- Tests are **inline** `#[cfg(test)] mod tests`. ffmpeg/lofty tests are guarded by a
  `fixture()` helper and skip (with `eprintln!`) when the file is missing — matching
  `tests/characterization.rs`. Outputs go in a `tempfile::tempdir()`.
- Both new modules carry `#![allow(dead_code)]` (consumed by M4-3, like `naming.rs`).
- Commit after each task. No `Co-Authored-By` trailer.

---

## File structure

- Create: `src-tauri/src/encode.rs` — `Target`, `EncodeError`, `target_for`,
  `is_conformant`, `guard_no_upscale`, `encode`.
- Create: `src-tauri/src/tagging.rs` — `write_tags`.
- Modify: `src-tauri/src/lib.rs` — add `mod encode;` and `mod tagging;`.

Fixtures already present and used: `fixtures/real_lossless.flac`, `fixtures/real_320.mp3`.

---

## Task 1: `encode` module — target + guard (pure logic)

**Files:**
- Create: `src-tauri/src/encode.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Register the module**

In `src-tauri/src/lib.rs`, add `mod encode;` after `mod db;` (keep the list tidy):

```rust
pub mod analysis;
mod db;
mod encode;
mod ffmpeg;
mod ipc;
mod naming;
mod queue;
mod scanner;
mod sources;
mod watcher;
mod worker;
```

- [ ] **Step 2: Create `encode.rs` with the pure types/functions + their tests**

```rust
//! Transcoding to the two CDJ rails (MP3 320 CBR / AIFF 16-bit 44.1 kHz) via the bundled
//! ffmpeg, plus the rail-based target choice, a conformance test (skip re-encoding files
//! already in target shape), and a hard no-upscale guard. The caller passes the source
//! rail (it already has it from the analysis report), so this module is independent of
//! the analysis pipeline. ffmpeg is driven exactly like `analysis/decode.rs`.
#![allow(dead_code)]

use crate::analysis::Rail;
use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::{FfmpegEvent, LogLevel};
use lofty::file::AudioFile;
use lofty::probe::Probe;
use serde::Serialize;

/// The two output shapes. Lossless rail → AIFF 16-bit/44.1; lossy rail → MP3 320 CBR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Target {
    #[serde(rename = "mp3_320")]
    Mp3320,
    #[serde(rename = "aiff_16_44")]
    Aiff1644,
}

impl Target {
    /// Output file extension for this target.
    pub fn ext(self) -> &'static str {
        match self {
            Target::Mp3320 => "mp3",
            Target::Aiff1644 => "aiff",
        }
    }

    /// The rail this target belongs to (used by the no-upscale guard).
    pub fn rail(self) -> Rail {
        match self {
            Target::Mp3320 => Rail::Lossy,
            Target::Aiff1644 => Rail::Lossless,
        }
    }
}

/// Why an encode could not proceed.
#[derive(Debug, Clone, PartialEq)]
pub enum EncodeError {
    /// Refused: would fabricate lossless from a lossy source.
    Upscale,
    /// ffmpeg failed (spawn, terminal log error, or empty output).
    Ffmpeg(String),
}

impl std::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EncodeError::Upscale => write!(f, "refused: cannot upscale lossy to lossless"),
            EncodeError::Ffmpeg(m) => write!(f, "ffmpeg: {m}"),
        }
    }
}

/// Rail-based default target. Lossless → AIFF 16/44.1; everything else (lossy/unknown) →
/// MP3 320 (never crosses up into lossless on its own).
pub fn target_for(rail: Rail) -> Target {
    match rail {
        Rail::Lossless => Target::Aiff1644,
        _ => Target::Mp3320,
    }
}

/// Reject a target that would upscale a lossy source into a lossless container.
pub fn guard_no_upscale(source_rail: Rail, target: Target) -> Result<(), EncodeError> {
    if source_rail == Rail::Lossy && target.rail() == Rail::Lossless {
        return Err(EncodeError::Upscale);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_follows_rail() {
        assert_eq!(target_for(Rail::Lossless), Target::Aiff1644);
        assert_eq!(target_for(Rail::Lossy), Target::Mp3320);
        assert_eq!(target_for(Rail::Unknown), Target::Mp3320);
    }

    #[test]
    fn target_ext_matches() {
        assert_eq!(Target::Mp3320.ext(), "mp3");
        assert_eq!(Target::Aiff1644.ext(), "aiff");
    }

    #[test]
    fn guard_blocks_lossy_to_lossless_only() {
        assert_eq!(guard_no_upscale(Rail::Lossy, Target::Aiff1644), Err(EncodeError::Upscale));
        assert!(guard_no_upscale(Rail::Lossy, Target::Mp3320).is_ok());
        assert!(guard_no_upscale(Rail::Lossless, Target::Aiff1644).is_ok());
        assert!(guard_no_upscale(Rail::Lossless, Target::Mp3320).is_ok()); // downscale allowed
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --lib encode`
Expected: PASS (3 tests).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/encode.rs src-tauri/src/lib.rs
git commit -m "feat(m4-2): encode target choice + no-upscale guard"
```

---

## Task 2: `is_conformant` (lofty properties)

**Files:**
- Modify: `src-tauri/src/encode.rs`

Conformance decides whether to skip re-encoding. **MP3 target:** conformant if the file
is already MP3 (we never re-encode MP3→MP3 — that only loses quality, never gains). **AIFF
target:** conformant only if the file is `.aif`/`.aiff` **and** 44.1 kHz **and** 16-bit.

- [ ] **Step 1: Write the failing tests**

Add a fixture helper + tests to `encode.rs`'s `mod tests`:

```rust
    fn fixture(name: &str) -> Option<String> {
        let p = format!("fixtures/{name}");
        if std::path::Path::new(&p).exists() { Some(p) } else { None }
    }

    #[test]
    fn mp3_is_conformant_to_mp3_target() {
        let Some(p) = fixture("real_320.mp3") else { eprintln!("skip: no fixture"); return; };
        assert!(is_conformant(&p, Target::Mp3320));
    }

    #[test]
    fn flac_is_not_conformant_to_either_target() {
        let Some(p) = fixture("real_lossless.flac") else { eprintln!("skip: no fixture"); return; };
        assert!(!is_conformant(&p, Target::Mp3320));   // wrong codec
        assert!(!is_conformant(&p, Target::Aiff1644));  // wrong container
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --lib encode`
Expected: FAIL — `cannot find function is_conformant`.

- [ ] **Step 3: Implement `is_conformant` + `ext_of`**

Add above the test module:

```rust
/// Lowercased file extension (no dot), or "" when absent.
fn ext_of(path: &str) -> String {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// True when `path` is already in `target` shape, so filing can tag+move without
/// re-encoding. MP3 target = already MP3 (any bitrate; never re-encode MP3→MP3). AIFF
/// target = .aif/.aiff at 44.1 kHz / 16-bit.
pub fn is_conformant(path: &str, target: Target) -> bool {
    let ext = ext_of(path);
    match target {
        Target::Mp3320 => ext == "mp3",
        Target::Aiff1644 => {
            if ext != "aif" && ext != "aiff" {
                return false;
            }
            match Probe::open(path).and_then(|p| p.read()) {
                Ok(t) => {
                    let props = t.properties();
                    props.sample_rate() == Some(44100) && props.bit_depth() == Some(16)
                }
                Err(_) => false,
            }
        }
    }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --lib encode`
Expected: PASS (5 tests; the two new ones skip only if fixtures are absent).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/encode.rs
git commit -m "feat(m4-2): conformance test (is_conformant)"
```

---

## Task 3: `encode` (ffmpeg transcode) + equivalence test

**Files:**
- Modify: `src-tauri/src/encode.rs`

Transcode `src` → `dst` for the target codec, mirroring `decode.rs`'s ffmpeg event
handling. MP3 = `libmp3lame -b:a 320k`; AIFF = `pcm_s16be` (AIFF is big-endian) at 44.1
kHz. Channels are left as the source's (CDJs play mono and stereo). Verify a non-empty
output file was produced.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests`:

```rust
    #[test]
    fn encodes_flac_to_conformant_aiff() {
        let Some(src) = fixture("real_lossless.flac") else { eprintln!("skip: no fixture"); return; };
        crate::ffmpeg::init_ffmpeg_path(); // point ffmpeg-sidecar at the bundled dev binary
        let dir = tempfile::tempdir().unwrap();
        let dst = dir.path().join("out.aiff");
        let dst = dst.to_str().unwrap();
        encode(&src, dst, Target::Aiff1644).expect("encode aiff");
        // equivalence: the output is exactly the target shape
        assert!(is_conformant(dst, Target::Aiff1644), "encoded AIFF must be 16-bit/44.1");
    }

    #[test]
    fn encodes_flac_to_mp3_320() {
        let Some(src) = fixture("real_lossless.flac") else { eprintln!("skip: no fixture"); return; };
        crate::ffmpeg::init_ffmpeg_path(); // point ffmpeg-sidecar at the bundled dev binary
        let dir = tempfile::tempdir().unwrap();
        let dst = dir.path().join("out.mp3");
        let dst = dst.to_str().unwrap();
        encode(&src, dst, Target::Mp3320).expect("encode mp3");
        assert!(is_conformant(dst, Target::Mp3320));
        assert!(std::fs::metadata(dst).unwrap().len() > 0);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --lib encode`
Expected: FAIL — `cannot find function encode`.

- [ ] **Step 3: Implement `encode`**

Add above the test module:

```rust
/// Transcode `src` into `dst` for `target`, overwriting `dst`. Surfaces any terminal
/// ffmpeg error and refuses to report success on an empty/missing output. Does NOT apply
/// the no-upscale guard — callers (M4-3) guard before choosing a lossless target.
pub fn encode(src: &str, dst: &str, target: Target) -> Result<(), EncodeError> {
    let codec_args: &[&str] = match target {
        Target::Mp3320 => &["-vn", "-c:a", "libmp3lame", "-b:a", "320k", "-ar", "44100"],
        Target::Aiff1644 => &["-vn", "-c:a", "pcm_s16be", "-ar", "44100"],
    };

    let mut child = FfmpegCommand::new()
        .input(src)
        .args(codec_args)
        .arg("-y")
        .output(dst)
        .spawn()
        .map_err(|e| EncodeError::Ffmpeg(format!("spawn failed: {e}")))?;

    let iter = child
        .iter()
        .map_err(|e| EncodeError::Ffmpeg(format!("iter failed: {e}")))?;

    let mut err: Option<String> = None;
    for ev in iter {
        match ev {
            FfmpegEvent::Log(LogLevel::Error, msg) => err = Some(msg),
            // ffmpeg-sidecar emits this synthetic event whenever no output stream is routed
            // to stdout — always the case for file output. Not a real failure; the
            // output-file check below is the source of truth.
            FfmpegEvent::Error(msg) if msg != "No streams found" => err = Some(msg),
            _ => {}
        }
    }
    let _ = child.wait();

    if let Some(e) = err {
        return Err(EncodeError::Ffmpeg(e));
    }
    match std::fs::metadata(dst) {
        Ok(m) if m.len() > 0 => Ok(()),
        _ => Err(EncodeError::Ffmpeg("no output produced".into())),
    }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --lib encode`
Expected: PASS (7 tests). The two new tests spawn ffmpeg (~1-2 s) and skip if the FLAC
fixture is missing.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/encode.rs
git commit -m "feat(m4-2): ffmpeg transcode (encode) + equivalence tests"
```

---

## Task 4: `tagging` module — `write_tags`

**Files:**
- Create: `src-tauri/src/tagging.rs`
- Modify: `src-tauri/src/lib.rs`

Write `{artist, title}` onto a file in place via lofty. If the file has no primary tag
yet, create one of the file's native type first. Only fields we own are set; everything
else in the tag is preserved (`WriteOptions::default` keeps other tags).

- [ ] **Step 1: Register the module**

In `src-tauri/src/lib.rs`, add `mod tagging;` after `mod sources;`:

```rust
mod sources;
mod tagging;
mod watcher;
```

- [ ] **Step 2: Create `tagging.rs` with `write_tags` + test**

```rust
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
        .expect("primary tag present after insert");

    tag.set_artist(artist.to_string());
    tag.set_title(title.to_string());

    tag.save_to_path(path, WriteOptions::default())
        .map_err(|e| format!("save tags: {e}"))
}

#[cfg(test)]
mod tests {
    use super::write_tags;
    use lofty::file::TaggedFileExt;
    use lofty::probe::Probe;
    use lofty::tag::ItemKey;

    fn fixture(name: &str) -> Option<String> {
        let p = format!("fixtures/{name}");
        if std::path::Path::new(&p).exists() { Some(p) } else { None }
    }

    #[test]
    fn writes_and_reads_back_artist_title() {
        let Some(src) = fixture("real_320.mp3") else { eprintln!("skip: no fixture"); return; };
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
}
```

- [ ] **Step 3: Run to verify it compiles and passes**

Run: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --lib tagging`
Expected: PASSES (1 test; skips only if `real_320.mp3` is absent). `primary_tag()` comes
from `TaggedFileExt`; `get_string(&ItemKey)` is inherent on `Tag`.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/tagging.rs src-tauri/src/lib.rs
git commit -m "feat(m4-2): write canonical tags via lofty (write_tags)"
```

---

## Task 5: Full green + clippy

**Files:**
- None (verification only).

- [ ] **Step 1: Run both modules' tests**

Run: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --lib encode; & "$env:USERPROFILE\.cargo\bin\cargo.exe" test --lib tagging`
Expected: all green.

- [ ] **Step 2: Clippy the new modules**

Run: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" clippy --lib`
Expected: no new warnings referencing `encode.rs` or `tagging.rs`. (Pre-existing warnings
in `analysis/` and `worker.rs` are out of scope — confirm none of the reported paths are
`encode.rs`/`tagging.rs`; fix any that are.)

- [ ] **Step 3: Commit any lint fixes**

```bash
git add src-tauri/src/encode.rs src-tauri/src/tagging.rs
git commit -m "chore(m4-2): clippy clean for encode + tagging"
```

(Skip if clippy was already clean.)

---

## Done criteria

- `encode.rs` and `tagging.rs` exist, declared in `lib.rs`, all inline tests green.
- `Target`, `EncodeError`, `target_for`, `is_conformant`, `guard_no_upscale`, `encode`,
  and `write_tags` are public for M4-3.
- Encoding a FLAC yields a conformant AIFF (16-bit/44.1) and a valid MP3 320; MP3→MP3 is
  never re-encoded; lossy→lossless is refused.

**Next plan:** M4-3 — `library` + `filing` + `actions` (bins, convert→tag→move
orchestration, undo engine, IPC commands + migration v4).
