# M6a — Identification Discogs — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** From the review pane, an on-demand "Identifier" button fetches a track's metadata from Discogs (artist/title/label/year/sub-genres/cover/release_id), proposes it without overwriting anything, and on accept enriches what the filing step writes to the file.

**Architecture:** A `MetadataProvider` trait decouples the source; `metadata::discogs` implements it over the Discogs HTTP API (parsing isolated from I/O so it's unit-tested via fixtures). `apply_identity` writes the chosen candidate into the existing `metadata` table plus a new `track_genres` table; the extras are loaded into the `FilePlan` at plan time and written by an extended `write_tags` at filing time (no DB access off the lock). The filename is unchanged — it still derives only from `Canonical{artist,title,version}`.

**Tech Stack:** Rust (rusqlite, lofty, serde), `ureq` (new — tiny blocking HTTP + rustls), TypeScript/Vite frontend, Tauri IPC.

**Spec:** `docs/superpowers/specs/2026-06-14-m6a-discogs-identification-design.md`

---

## Conventions for this codebase (read before starting)

- **Run Rust tests:** the Tauri dev server (`cargo tauri dev`) and `cargo test` race on the same `target/`. If a dev server is running, test in an isolated dir:
  `$env:CARGO_TARGET_DIR="C:\Users\LEETJ\Desktop\dj-assistant\target-audit"; cargo test --lib` (from `src-tauri/`), then delete `target-audit` when done. Otherwise plain `cargo test --lib` from `src-tauri/`.
  Invoke cargo as `& "$env:USERPROFILE\.cargo\bin\cargo.exe"`.
- **Frontend checks:** from repo root, `npx tsc --noEmit` then `npm run build`.
- **Commits:** no `Co-Authored-By` trailer (user rule). Commit with `git commit -m "..."`.
- **ES/Rust edition:** Rust 2021; backend is synchronous (no tokio) — `ureq` is blocking by design.
- **IPC error convention:** commands return `Result<T, String>`; sentinel error codes (e.g. `"NoLibraryRoot"`) let the front route. We add `NO_TOKEN`, `RATE_LIMITED:<s>`, `NETWORK`, `PARSE`.
- **Migrations** are append-only in `src-tauri/src/db.rs` `MIGRATIONS`; never edit an existing entry. Adding an entry bumps `user_version` automatically.
- New modules must be declared in `src-tauri/src/lib.rs` (`mod ...;`) and IPC commands registered in the `generate_handler![...]` list.

---

## File Structure

**Create:**
- `src-tauri/src/genres.rs` — CRUD for the `track_genres` table (ordered set/get/replace). One responsibility: per-track genre list persistence.
- `src-tauri/src/metadata/mod.rs` — source abstraction: `Query`, `Candidate`, `ProviderError`, `MetadataProvider` trait, and the pure `apply_identity` domain fn + `AppliedIdentity` return type.
- `src-tauri/src/metadata/discogs.rs` — Discogs provider: `Discogs{token}` + `parse_search` (pure, tested via fixtures) + HTTP `search` (thin, untested).
- `src-tauri/src/metadata/cover.rs` — cover cache path + best-effort download.
- `src-tauri/src/ipc_identify.rs` — `identify` and `apply_identity_cmd` Tauri commands.

**Modify:**
- `src-tauri/src/db.rs` — append migration v6 (`track_genres`).
- `src-tauri/src/tagging.rs` — add `write_tags_full` (label/year/genres-multi/cover); `write_tags` delegates.
- `src-tauri/src/filing.rs` — `FilePlan` gains `TagExtras`; `plan_file` loads them; `execute_file` uses `write_tags_full`.
- `src-tauri/src/settings.rs` — add `DISCOGS_TOKEN` const.
- `src-tauri/src/lib.rs` — declare new modules + register the two commands.
- `src-tauri/Cargo.toml` — add `ureq`.
- `frontend/ipc.ts` — `identify` / `applyIdentity` wrappers + types.
- `frontend/filing.ts` — Identifier button + candidate UI + apply wiring.
- `frontend/sift-live.ts` (Réglages render) — Discogs token field.

---

### Task 1: Migration v6 + `genres` module

**Files:**
- Modify: `src-tauri/src/db.rs` (append to `MIGRATIONS`, before the closing `];` at line ~93)
- Create: `src-tauri/src/genres.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod genres;`)

- [ ] **Step 1: Append migration v6**

In `src-tauri/src/db.rs`, add this entry as the LAST element of the `MIGRATIONS` array (after the v5 `report_json` block, before `];`):

```rust
    // v6 — M6a Discogs identification: per-track sub-genres (Discogs "style"), multiple per
    // track, ordered. metadata.genre stays for back-compat but track_genres is the source.
    r#"
    CREATE TABLE track_genres (
        track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
        genre    TEXT NOT NULL,
        ord      INTEGER NOT NULL,
        PRIMARY KEY (track_id, genre)
    );
    CREATE INDEX idx_track_genres_track ON track_genres(track_id);
    "#,
```

- [ ] **Step 2: Write the failing test for `genres`**

Create `src-tauri/src/genres.rs` with only the test module first:

```rust
//! Per-track sub-genre list (Discogs "style"), stored ordered in `track_genres`. Replacing a
//! track's genres is a full delete+insert so re-identifying never accumulates stale rows.
#![allow(dead_code)]

use rusqlite::{params, Connection};

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::run_migrations(&conn).unwrap();
        // a track row to satisfy the FK
        conn.execute("INSERT INTO tracks(id, path, status) VALUES(1,'/x.flac','pending')", []).unwrap();
        conn
    }

    #[test]
    fn set_then_get_round_trips_in_order() {
        let conn = db();
        set_genres(&conn, 1, &["Deep House".into(), "House".into()]).unwrap();
        assert_eq!(get_genres(&conn, 1).unwrap(), vec!["Deep House".to_string(), "House".to_string()]);
    }

    #[test]
    fn re_set_replaces_without_accumulating() {
        let conn = db();
        set_genres(&conn, 1, &["Techno".into(), "Acid".into()]).unwrap();
        set_genres(&conn, 1, &["Ambient".into()]).unwrap();
        assert_eq!(get_genres(&conn, 1).unwrap(), vec!["Ambient".to_string()]);
    }

    #[test]
    fn get_missing_is_empty() {
        let conn = db();
        assert_eq!(get_genres(&conn, 1).unwrap(), Vec::<String>::new());
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run (from `src-tauri/`): `cargo test --lib genres::`
Expected: FAIL — `cannot find function set_genres` / `get_genres`.

- [ ] **Step 4: Implement `set_genres` / `get_genres`**

Add above the `#[cfg(test)]` module in `src-tauri/src/genres.rs`:

```rust
/// Replace a track's genre list with `genres` (ordered). Empty `genres` clears them.
pub fn set_genres(conn: &Connection, track_id: i64, genres: &[String]) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM track_genres WHERE track_id=?1", params![track_id])?;
    for (ord, g) in genres.iter().enumerate() {
        let g = g.trim();
        if g.is_empty() {
            continue;
        }
        conn.execute(
            "INSERT OR IGNORE INTO track_genres(track_id, genre, ord) VALUES(?1,?2,?3)",
            params![track_id, g, ord as i64],
        )?;
    }
    Ok(())
}

/// A track's genres, ordered by `ord`.
pub fn get_genres(conn: &Connection, track_id: i64) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT genre FROM track_genres WHERE track_id=?1 ORDER BY ord")?;
    let rows = stmt.query_map(params![track_id], |r| r.get::<_, String>(0))?;
    rows.collect()
}
```

Add `mod genres;` to `src-tauri/src/lib.rs` (with the other `mod` lines, alphabetical: after `mod ffmpeg;`/`mod filing;`/`mod fingerprint;` — put `mod genres;` after `mod fingerprint;`).

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test --lib genres::`
Expected: PASS (3 tests). Also run `cargo test --lib db::` — migration count test should still pass (it asserts latest version dynamically).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/db.rs src-tauri/src/genres.rs src-tauri/src/lib.rs
git commit -m "feat(m6a): track_genres table (migration v6) + genres CRUD module"
```

---

### Task 2: `metadata` module — types + trait

**Files:**
- Create: `src-tauri/src/metadata/mod.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod metadata;`)

- [ ] **Step 1: Write the failing test (Candidate serde round-trip)**

Create `src-tauri/src/metadata/mod.rs`:

```rust
//! Metadata source abstraction. A `MetadataProvider` turns a `Query` into ranked `Candidate`s;
//! `apply_identity` persists a chosen candidate into the DB. Discogs is the first provider
//! (see discogs.rs); the trait keeps a future AcoustID/MusicBrainz provider a drop-in.
#![allow(dead_code)]

pub mod cover;
pub mod discogs;

use serde::{Deserialize, Serialize};

/// What we search for: the track's current best-guess artist/title.
pub struct Query {
    pub artist: String,
    pub title: String,
}

/// A normalized identification result, ranked best-first by the provider.
// Serialize → sent to the UI; Deserialize → the UI returns the chosen one to apply_identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub artist: String,
    pub title: String,
    pub label: Option<String>,
    pub year: Option<i64>,
    pub styles: Vec<String>, // Discogs "style" (sub-genres), ordered
    pub country: Option<String>,
    pub format: Option<String>,
    pub cover_url: Option<String>,
    pub release_id: String,
    pub source: String, // "discogs"
}

/// Why a provider call failed — mapped to stable IPC error codes by the command layer.
#[derive(Debug)]
pub enum ProviderError {
    NoToken,
    RateLimited { retry_after_s: u64 },
    Network(String),
    Parse(String),
}

pub trait MetadataProvider {
    /// Ranked candidates (best first). Empty vec = no results (not an error).
    fn search(&self, q: &Query) -> Result<Vec<Candidate>, ProviderError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_serde_round_trips() {
        let c = Candidate {
            artist: "Larry Heard".into(),
            title: "Mystery of Love".into(),
            label: Some("Alleviated".into()),
            year: Some(1986),
            styles: vec!["Deep House".into(), "House".into()],
            country: Some("US".into()),
            format: Some("Vinyl, 12\"".into()),
            cover_url: Some("https://img/x.jpg".into()),
            release_id: "12345".into(),
            source: "discogs".into(),
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: Candidate = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }
}
```

> Note: `pub mod cover;` and `pub mod discogs;` are declared now but created in Tasks 3–4. To compile Task 2 in isolation, create empty stub files first (Step 3).

- [ ] **Step 2: Add module declaration**

In `src-tauri/src/lib.rs`, add `mod metadata;` after `mod library;`.

- [ ] **Step 3: Create stubs so it compiles**

Create `src-tauri/src/metadata/cover.rs` and `src-tauri/src/metadata/discogs.rs` each containing only:

```rust
#![allow(dead_code)]
```

- [ ] **Step 4: Run the test**

Run: `cargo test --lib metadata::tests::candidate_serde_round_trips`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/metadata/ src-tauri/src/lib.rs
git commit -m "feat(m6a): metadata source abstraction (Candidate/Query/ProviderError/trait)"
```

---

### Task 3: Discogs provider — parsing (fixture-tested) + HTTP

**Files:**
- Modify: `src-tauri/Cargo.toml` (add `ureq`)
- Modify: `src-tauri/src/metadata/discogs.rs`

- [ ] **Step 1: Add the `ureq` dependency**

In `src-tauri/Cargo.toml` under `[dependencies]`, add:

```toml
ureq = { version = "2.10", features = ["json", "tls"] }
```

Run `cargo build --lib` once to fetch it (Expected: compiles; no code uses it yet).

- [ ] **Step 2: Write the failing parse test**

Replace `src-tauri/src/metadata/discogs.rs` contents with the test module (impl comes next):

```rust
//! Discogs implementation of MetadataProvider. The HTTP call (`search`) is a thin wrapper over
//! `ureq`; the response→Candidate mapping (`parse_search`) is pure and unit-tested via a
//! captured fixture, so the matching logic is covered without any network access.
#![allow(dead_code)]

use crate::metadata::{Candidate, MetadataProvider, ProviderError, Query};
use serde_json::Value;

#[cfg(test)]
mod tests {
    use super::*;

    // Trimmed but representative shape of GET /database/search?type=release
    const FIXTURE: &str = r#"{
      "results": [
        {
          "id": 12345,
          "title": "Larry Heard - Mystery Of Love",
          "year": "1986",
          "country": "US",
          "label": ["Alleviated Records", "Alleviated"],
          "genre": ["Electronic"],
          "style": ["Deep House", "House"],
          "format": ["Vinyl", "12\""],
          "cover_image": "https://img.discogs.com/x.jpg"
        },
        {
          "id": 999,
          "title": "Larry Heard - Mystery Of Love (Remix)",
          "label": ["Alleviated"],
          "style": ["House"],
          "cover_image": "https://img.discogs.com/y.jpg"
        },
        { "id": 7, "title": "" }
      ]
    }"#;

    #[test]
    fn parse_maps_style_to_styles_and_ignores_broad_genre() {
        let v: Value = serde_json::from_str(FIXTURE).unwrap();
        let cands = parse_search(&v);
        assert_eq!(cands.len(), 2, "title-less result is filtered out");
        let first = &cands[0];
        assert_eq!(first.artist, "Larry Heard");
        assert_eq!(first.title, "Mystery Of Love");
        assert_eq!(first.styles, vec!["Deep House".to_string(), "House".to_string()]);
        assert_eq!(first.year, Some(1986));
        assert_eq!(first.label.as_deref(), Some("Alleviated Records"));
        assert_eq!(first.country.as_deref(), Some("US"));
        assert_eq!(first.format.as_deref(), Some("Vinyl, 12\""));
        assert_eq!(first.release_id, "12345");
        assert_eq!(first.source, "discogs");
    }

    #[test]
    fn parse_keeps_provider_order_and_handles_missing_optionals() {
        let v: Value = serde_json::from_str(FIXTURE).unwrap();
        let cands = parse_search(&v);
        assert_eq!(cands[1].release_id, "999"); // order preserved
        assert_eq!(cands[1].year, None);
        assert_eq!(cands[1].country, None);
    }
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test --lib metadata::discogs`
Expected: FAIL — `cannot find function parse_search`.

- [ ] **Step 4: Implement `parse_search`, splitting, and the HTTP `Discogs` provider**

Add above the test module in `src-tauri/src/metadata/discogs.rs`:

```rust
const USER_AGENT: &str = concat!("Sift/", env!("CARGO_PKG_VERSION"));

pub struct Discogs {
    pub token: String,
}

/// Discogs "title" is `"Artist - Title"`. Split on the first " - "; if absent, the whole
/// string is the title and the artist is empty.
fn split_title(s: &str) -> (String, String) {
    match s.find(" - ") {
        Some(i) => (s[..i].trim().to_string(), s[i + 3..].trim().to_string()),
        None => (String::new(), s.trim().to_string()),
    }
}

fn first_string(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(|x| x.as_array())
        .and_then(|a| a.first())
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

fn string_array(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(|x| x.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str()).map(|s| s.to_string()).collect())
        .unwrap_or_default()
}

/// Map a Discogs search response into ranked Candidates. Pure: no I/O. Results with an empty
/// title are dropped; provider order is preserved.
pub fn parse_search(v: &Value) -> Vec<Candidate> {
    let Some(results) = v.get("results").and_then(|x| x.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for r in results {
        let raw_title = r.get("title").and_then(|x| x.as_str()).unwrap_or("").trim();
        if raw_title.is_empty() {
            continue;
        }
        let (artist, title) = split_title(raw_title);
        let format = {
            let parts = string_array(r, "format");
            if parts.is_empty() { None } else { Some(parts.join(", ")) }
        };
        let year = r
            .get("year")
            .and_then(|x| x.as_str())
            .and_then(|s| s.parse::<i64>().ok());
        out.push(Candidate {
            artist,
            title,
            label: first_string(r, "label"),
            year,
            styles: string_array(r, "style"),
            country: r.get("country").and_then(|x| x.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string()),
            format,
            cover_url: r.get("cover_image").and_then(|x| x.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string()),
            release_id: r.get("id").map(|x| x.to_string()).unwrap_or_default(),
            source: "discogs".into(),
        });
    }
    out
}

impl MetadataProvider for Discogs {
    fn search(&self, q: &Query) -> Result<Vec<Candidate>, ProviderError> {
        if self.token.trim().is_empty() {
            return Err(ProviderError::NoToken);
        }
        let resp = ureq::get("https://api.discogs.com/database/search")
            .set("User-Agent", USER_AGENT)
            .set("Authorization", &format!("Discogs token={}", self.token))
            .query("type", "release")
            .query("artist", &q.artist)
            .query("track", &q.title)
            .query("per_page", "8")
            .call();
        match resp {
            Ok(r) => {
                let v: Value = r.into_json().map_err(|e| ProviderError::Parse(e.to_string()))?;
                Ok(parse_search(&v))
            }
            Err(ureq::Error::Status(429, r)) => {
                let retry = r.header("Retry-After").and_then(|s| s.parse::<u64>().ok()).unwrap_or(60);
                Err(ProviderError::RateLimited { retry_after_s: retry })
            }
            Err(ureq::Error::Status(code, _)) => Err(ProviderError::Network(format!("HTTP {code}"))),
            Err(ureq::Error::Transport(t)) => Err(ProviderError::Network(t.to_string())),
        }
    }
}
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test --lib metadata::discogs`
Expected: PASS (2 tests).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/metadata/discogs.rs
git commit -m "feat(m6a): Discogs provider — fixture-tested parse + ureq search"
```

---

### Task 4: Cover cache path + best-effort download

**Files:**
- Modify: `src-tauri/src/metadata/cover.rs`

- [ ] **Step 1: Write the failing test (path mapping is pure & testable)**

Replace `src-tauri/src/metadata/cover.rs` with:

```rust
//! Cover-art cache. Covers are downloaded into a per-app cache dir keyed by Discogs release id
//! so the same release isn't re-fetched. Download is best-effort: failures are non-fatal (the
//! caller applies metadata anyway). Only the path mapping is unit-tested (no network in CI).
#![allow(dead_code)]

use std::path::{Path, PathBuf};

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
        // a stray slash must not escape the cache dir
        let p = cover_path(dir, "a/b");
        assert_eq!(p, std::path::Path::new("/cache/covers/a_b.jpg"));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib metadata::cover`
Expected: FAIL — `cannot find function cover_path`.

- [ ] **Step 3: Implement `cover_path` + `download_cover`**

Add above the test module:

```rust
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
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib metadata::cover`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/metadata/cover.rs
git commit -m "feat(m6a): cover cache path + best-effort download"
```

---

### Task 5: `apply_identity` domain fn + `AppliedIdentity`

**Files:**
- Modify: `src-tauri/src/metadata/mod.rs`

- [ ] **Step 1: Write the failing test**

In `src-tauri/src/metadata/mod.rs`, add to the `#[cfg(test)] mod tests`:

```rust
    fn db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::run_migrations(&conn).unwrap();
        conn.execute("INSERT INTO tracks(id, path, status) VALUES(1,'/x.flac','pending')", []).unwrap();
        conn
    }

    fn sample() -> Candidate {
        Candidate {
            artist: "Larry Heard".into(),
            title: "Mystery of Love".into(),
            label: Some("Alleviated".into()),
            year: Some(1986),
            styles: vec!["Deep House".into(), "House".into()],
            country: Some("US".into()),
            format: None,
            cover_url: Some("https://img/x.jpg".into()),
            release_id: "12345".into(),
            source: "discogs".into(),
        }
    }

    #[test]
    fn apply_writes_metadata_and_genres() {
        let conn = db();
        let applied = apply_identity(&conn, 1, &sample(), Some("/cache/12345.jpg".into())).unwrap();

        // metadata row
        let (artist, label, year, cover, rel, src): (String, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>) =
            conn.query_row(
                "SELECT artist, label, year, cover_path, discogs_release_id, source FROM metadata WHERE track_id=1",
                [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            ).unwrap();
        assert_eq!(artist, "Larry Heard");
        assert_eq!(label.as_deref(), Some("Alleviated"));
        assert_eq!(year, Some(1986));
        assert_eq!(cover.as_deref(), Some("/cache/12345.jpg"));
        assert_eq!(rel.as_deref(), Some("12345"));
        assert_eq!(src.as_deref(), Some("discogs"));

        // genres (ordered) via the genres module
        assert_eq!(crate::genres::get_genres(&conn, 1).unwrap(), vec!["Deep House".to_string(), "House".to_string()]);

        // returned payload
        assert_eq!(applied.canonical.artist, "Larry Heard");
        assert_eq!(applied.styles, vec!["Deep House".to_string(), "House".to_string()]);
        assert_eq!(applied.cover_path.as_deref(), Some("/cache/12345.jpg"));
    }

    #[test]
    fn re_apply_replaces_genres() {
        let conn = db();
        apply_identity(&conn, 1, &sample(), None).unwrap();
        let mut other = sample();
        other.styles = vec!["Techno".into()];
        apply_identity(&conn, 1, &other, None).unwrap();
        assert_eq!(crate::genres::get_genres(&conn, 1).unwrap(), vec!["Techno".to_string()]);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib metadata::tests::apply`
Expected: FAIL — `cannot find function apply_identity` / `AppliedIdentity`.

- [ ] **Step 3: Implement `AppliedIdentity` + `apply_identity`**

In `src-tauri/src/metadata/mod.rs`, add after the `Candidate` struct (and add the `use` for `Canonical`/`Confidence` and `rusqlite`):

```rust
use crate::naming::{Canonical, Confidence};
use rusqlite::{params, Connection};

/// What the UI needs after applying a candidate: the (name-driving) canonical plus the extra
/// tag fields, so it can refresh the preview, cover, and genre chips.
#[derive(Debug, Clone, Serialize)]
pub struct AppliedIdentity {
    pub canonical: Canonical,
    pub label: Option<String>,
    pub year: Option<i64>,
    pub styles: Vec<String>,
    pub cover_path: Option<String>,
}

/// Persist a chosen candidate for `track_id`: upsert the single-value fields into `metadata`,
/// replace the track's sub-genres, and (when provided) record the downloaded cover path. The
/// cover download itself happens in the command layer (network); this fn is pure DB so it is
/// unit-tested. Returns the payload the UI refreshes from.
pub fn apply_identity(
    conn: &Connection,
    track_id: i64,
    c: &Candidate,
    cover_path: Option<String>,
) -> rusqlite::Result<AppliedIdentity> {
    conn.execute(
        "INSERT INTO metadata(track_id, artist, title, label, year, cover_path, discogs_release_id, source)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
         ON CONFLICT(track_id) DO UPDATE SET
            artist=excluded.artist, title=excluded.title, label=excluded.label,
            year=excluded.year, cover_path=COALESCE(excluded.cover_path, metadata.cover_path),
            discogs_release_id=excluded.discogs_release_id, source=excluded.source",
        params![track_id, c.artist, c.title, c.label, c.year, cover_path, c.release_id, c.source],
    )?;
    crate::genres::set_genres(conn, track_id, &c.styles)?;
    if cover_path.is_some() {
        conn.execute("UPDATE tracks SET has_cover=1 WHERE id=?1", params![track_id])?;
    }
    Ok(AppliedIdentity {
        canonical: Canonical {
            artist: c.artist.clone(),
            title: c.title.clone(),
            version: None,
            confidence: Confidence::Green, // a Discogs match is a high-confidence rename
        },
        label: c.label.clone(),
        year: c.year,
        styles: c.styles.clone(),
        cover_path,
    })
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib metadata::tests`
Expected: PASS (round-trip + apply_writes + re_apply = 3 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/metadata/mod.rs
git commit -m "feat(m6a): apply_identity domain fn (metadata upsert + genres replace)"
```

---

### Task 6: Extend `write_tags` with label/year/genres/cover

**Files:**
- Modify: `src-tauri/src/tagging.rs`

- [ ] **Step 1: Write the failing test**

In `src-tauri/src/tagging.rs` test module, add (keep the existing `fixture` helper):

```rust
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

        // a tiny throwaway cover file
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
        // at least the first genre is present
        let genres: Vec<_> = tag.get_strings(&ItemKey::Genre).collect();
        assert!(genres.contains(&"Deep House"), "genres = {genres:?}");
        assert!(!tag.pictures().is_empty(), "cover embedded");
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib tagging::`
Expected: FAIL — `cannot find function write_tags_full`. (If no `real_320.mp3` fixture exists the new test would print "skip" and pass vacuously — to truly drive this, confirm a fixture is present at `src-tauri/fixtures/real_320.mp3`; the existing tagging tests use the same one.)

- [ ] **Step 3: Implement `write_tags_full` and make `write_tags` delegate**

Replace the body of `write_tags` and add `write_tags_full` in `src-tauri/src/tagging.rs`. Update the `use` lines to include picture + item APIs:

```rust
use lofty::config::WriteOptions;
use lofty::file::TaggedFileExt;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::{Accessor, ItemKey, TagExt};
use lofty::probe::Probe;
use lofty::tag::{Tag, TagItem};
use lofty::tag::items::ItemValue;
```

```rust
/// Back-compat: artist + title only (used where no rich metadata is available).
pub fn write_tags(path: &str, artist: &str, title: &str) -> Result<(), String> {
    write_tags_full(path, artist, title, None, None, &[], None)
}

/// Write the full canonical+enrichment set: artist, title, and optionally label, year,
/// sub-genres (one Genre item per value), and an embedded front cover read from `cover_path`.
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
    if !genres.is_empty() {
        // replace any existing Genre items with our ordered list (one item per sub-genre)
        tag.remove_key(&ItemKey::Genre);
        for g in genres.iter().filter(|s| !s.trim().is_empty()) {
            tag.push(TagItem::new(ItemKey::Genre, ItemValue::Text(g.clone())));
        }
    }
    if let Some(cp) = cover_path {
        if let Ok(bytes) = std::fs::read(cp) {
            let mime = if cp.to_lowercase().ends_with(".png") { MimeType::Png } else { MimeType::Jpeg };
            let pic = Picture::new_unchecked(PictureType::CoverFront, Some(mime), None, bytes);
            tag.push_picture(pic);
        }
    }

    tag.save_to_path(path, WriteOptions::default())
        .map_err(|e| format!("save tags: {e}"))
}
```

> If a lofty 0.22 API name differs (e.g. `insert_text` / `push` / `remove_key` / `push_picture`), check the exact signatures with `cargo doc -p lofty --open` or context7 docs for lofty 0.22 and adjust — the intent (set label, set year, multiple Genre items, one front-cover picture) is what must hold. The test pins the observable result.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib tagging::`
Expected: PASS (existing 2 + new 1; the new one runs only if the fixture exists).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/tagging.rs
git commit -m "feat(m6a): write_tags_full — label/year/multi-genre/embedded cover"
```

---

### Task 7: Thread tag extras through filing

**Files:**
- Modify: `src-tauri/src/filing.rs`

- [ ] **Step 1: Write the failing test**

In `src-tauri/src/filing.rs` test module, add a test that an applied identity reaches the filed file. Adapt to the existing test helpers (they already build a root + track + fixture). Add:

```rust
    #[test]
    fn filing_writes_applied_genres_to_the_file() {
        let Some(_) = std::path::Path::new("fixtures/real_320.mp3").exists().then_some(()) else {
            eprintln!("skip: no fixture");
            return;
        };
        let (conn, root, id, _tmp) = setup_track_with_fixture("real_320.mp3"); // existing helper
        // apply a Discogs identity first
        let cand = crate::metadata::Candidate {
            artist: "Larry Heard".into(), title: "Mystery of Love".into(),
            label: Some("Alleviated".into()), year: Some(1986),
            styles: vec!["Deep House".into()], country: None, format: None,
            cover_url: None, release_id: "12345".into(), source: "discogs".into(),
        };
        crate::metadata::apply_identity(&conn, id, &cand, None).unwrap();

        let res = file_track(&conn, &root, "{artist} - {title}", id, "House", None, None).unwrap();

        use lofty::file::TaggedFileExt;
        use lofty::probe::Probe;
        use lofty::tag::ItemKey;
        let tagged = Probe::open(&res.path).unwrap().read().unwrap();
        let tag = tagged.primary_tag().unwrap();
        let genres: Vec<_> = tag.get_strings(&ItemKey::Genre).collect();
        assert!(genres.contains(&"Deep House"), "filed file has applied genre; got {genres:?}");
    }
```

> If a `setup_track_with_fixture` helper doesn't already exist, model this test on the nearest existing filing test (e.g. `files_conformant_mp3_by_moving`) — reuse its setup verbatim rather than inventing a new helper, then apply the identity and assert on the genre after filing.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib filing::tests::filing_writes_applied_genres`
Expected: FAIL — filed file has no genre (write path ignores extras).

- [ ] **Step 3: Add `TagExtras` to `FilePlan`, load it in `plan_file`, use it in `execute_file`**

In `src-tauri/src/filing.rs`:

Add the struct near `FilePlan`:

```rust
/// Enrichment tag fields loaded once (under the lock) so phase 2 writes them without DB access.
#[derive(Default, Clone)]
pub struct TagExtras {
    pub label: Option<String>,
    pub year: Option<i64>,
    pub genres: Vec<String>,
    pub cover_path: Option<String>,
}
```

Add a field to `FilePlan`:

```rust
    extras: TagExtras,
```

In `plan_file`, before building the `FilePlan`, load the extras:

```rust
    let extras = TagExtras {
        label: conn
            .query_row("SELECT label FROM metadata WHERE track_id=?1", params![track_id], |r| r.get::<_, Option<String>>(0))
            .ok()
            .flatten(),
        year: conn
            .query_row("SELECT year FROM metadata WHERE track_id=?1", params![track_id], |r| r.get::<_, Option<i64>>(0))
            .ok()
            .flatten(),
        genres: crate::genres::get_genres(conn, track_id).unwrap_or_default(),
        cover_path: conn
            .query_row("SELECT cover_path FROM metadata WHERE track_id=?1", params![track_id], |r| r.get::<_, Option<String>>(0))
            .ok()
            .flatten(),
    };
```

and add `extras,` to the returned `FilePlan { ... }`.

In `execute_file`, replace both `tagging::write_tags(&plan.source, ...)` / `(&plan.dest, ...)` calls with:

```rust
        tagging::write_tags_full(
            &plan.source, &plan.canonical.artist, &plan.canonical.title,
            plan.extras.label.as_deref(), plan.extras.year, &plan.extras.genres,
            plan.extras.cover_path.as_deref(),
        ).map_err(FilingError::Tag)?;
```

(and the analogous one for `&plan.dest` in the non-conformant branch, keeping its orphan-cleanup-on-error).

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib filing::`
Expected: PASS (existing filing tests + the new one). Then run the whole suite: `cargo test --lib` → all green.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/filing.rs
git commit -m "feat(m6a): thread Discogs tag extras (label/year/genres/cover) into filing"
```

---

### Task 8: IPC commands + settings key + registration

**Files:**
- Modify: `src-tauri/src/settings.rs` (add `DISCOGS_TOKEN`)
- Create: `src-tauri/src/ipc_identify.rs`
- Modify: `src-tauri/src/lib.rs` (declare module + register commands)

- [ ] **Step 1: Add the settings key**

In `src-tauri/src/settings.rs`, add near the other `pub const` keys:

```rust
/// Discogs personal access token (entered in Réglages). Empty/unset = identification disabled.
pub const DISCOGS_TOKEN: &str = "discogs_token";
```

- [ ] **Step 2: Create the IPC module**

Create `src-tauri/src/ipc_identify.rs`:

```rust
//! IPC surface for M6a identification. `identify` queries Discogs (token from settings) and
//! returns ranked candidates; `apply_identity_cmd` downloads the cover (best-effort) and
//! persists the chosen candidate. Errors are flattened to stable sentinel codes the front maps
//! to messages: NO_TOKEN, RATE_LIMITED:<s>, NETWORK, PARSE.

use crate::metadata::{self, AppliedIdentity, Candidate, MetadataProvider, ProviderError, Query};
use crate::settings;
use rusqlite::Connection;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

fn err_code(e: ProviderError) -> String {
    match e {
        ProviderError::NoToken => "NO_TOKEN".into(),
        ProviderError::RateLimited { retry_after_s } => format!("RATE_LIMITED:{retry_after_s}"),
        ProviderError::Network(m) => format!("NETWORK:{m}"),
        ProviderError::Parse(m) => format!("PARSE:{m}"),
    }
}

/// Query Discogs for `track_id`'s best-guess artist/title; ranked candidates, best first.
#[tauri::command]
pub fn identify(
    conn: State<'_, Mutex<Connection>>,
    track_id: i64,
) -> Result<Vec<Candidate>, String> {
    let (token, query) = {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        let token = settings::get(&conn, settings::DISCOGS_TOKEN)
            .map_err(|e| e.to_string())?
            .unwrap_or_default();
        let canonical = crate::filing::reconcile_track(&conn, track_id).map_err(|e| e.to_string())?;
        (token, Query { artist: canonical.artist, title: canonical.title })
    };
    if token.trim().is_empty() {
        return Err("NO_TOKEN".into());
    }
    let provider = metadata::discogs::Discogs { token };
    provider.search(&query).map_err(err_code)
}

/// Persist a chosen candidate for `track_id`: download its cover (best-effort) then write the
/// metadata + genres. Emits `queue:changed` so the front refreshes.
#[tauri::command]
pub fn apply_identity_cmd(
    app: AppHandle,
    conn: State<'_, Mutex<Connection>>,
    track_id: i64,
    candidate: Candidate,
) -> Result<AppliedIdentity, String> {
    // Cover download (network, off the DB lock). Failure is non-fatal.
    let cover_path = candidate.cover_url.as_ref().and_then(|url| {
        let dir = app.path().app_cache_dir().ok()?.join("covers");
        metadata::cover::download_cover(&dir, &candidate.release_id, url)
            .ok()
            .map(|p| p.to_string_lossy().to_string())
    });
    let applied = {
        let conn = conn.lock().map_err(|e| e.to_string())?;
        metadata::apply_identity(&conn, track_id, &candidate, cover_path).map_err(|e| e.to_string())?
    };
    app.emit("queue:changed", ()).ok();
    Ok(applied)
}
```

- [ ] **Step 3: Register module + commands**

In `src-tauri/src/lib.rs`:
- Add `mod ipc_identify;` after `mod ipc_filing;`.
- In `generate_handler![...]`, add after `ipc_filing::find_duplicate`:

```rust
            ,ipc_identify::identify,
            ipc_identify::apply_identity_cmd
```

(ensure commas are correct — append with leading comma if `find_duplicate` is the last entry without a trailing comma).

- [ ] **Step 4: Build**

Run (from `src-tauri/`): `cargo build --lib` then `cargo test --lib`
Expected: compiles; full suite green. (No new unit test here — the command layer is thin glue; its pieces are covered by Tasks 3–6.)

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/settings.rs src-tauri/src/ipc_identify.rs src-tauri/src/lib.rs
git commit -m "feat(m6a): identify + apply_identity IPC commands; discogs_token setting"
```

---

### Task 9: Frontend — token field, Identifier button, candidate UI

**Files:**
- Modify: `frontend/ipc.ts`
- Modify: `frontend/filing.ts`
- Modify: `frontend/sift-live.ts` (Réglages render — locate the existing settings/Réglages view)

- [ ] **Step 1: Add IPC wrappers + types**

In `frontend/ipc.ts`, add (matching the existing `invoke` wrapper style):

```ts
export interface Candidate {
  artist: string;
  title: string;
  label: string | null;
  year: number | null;
  styles: string[];
  country: string | null;
  format: string | null;
  cover_url: string | null;
  release_id: string;
  source: string;
}

export interface AppliedIdentity {
  canonical: { artist: string; title: string; version: string | null; confidence: string };
  label: string | null;
  year: number | null;
  styles: string[];
  cover_path: string | null;
}

export const identify = (trackId: number) => invoke<Candidate[]>("identify", { trackId });
export const applyIdentity = (trackId: number, candidate: Candidate) =>
  invoke<AppliedIdentity>("apply_identity_cmd", { trackId, candidate });
```

> Confirm the param-name casing Tauri expects (the codebase passes camelCase like `trackId` elsewhere — match the existing calls in `ipc.ts`).

- [ ] **Step 2: Add the Discogs token field to Réglages**

In the Réglages view render (find it in `frontend/sift-live.ts` — search for the settings panel that already reads/writes `library_root` via `get_setting`/`set_setting`), add a labelled text input for the token, wired the same way:

```ts
// in the réglages markup, alongside the library-root row:
`<label class="set-row">
   <span>Token Discogs</span>
   <input id="set-discogs-token" type="password" placeholder="colle ton token perso" />
   <a href="#" id="discogs-token-help">obtenir un token</a>
 </label>`
```

```ts
// after render, wire it (mirror the library_root wiring):
const tok = document.getElementById("set-discogs-token") as HTMLInputElement | null;
if (tok) {
  getSetting("discogs_token").then((v) => { if (v) tok.value = v; });
  tok.addEventListener("change", () => void setSetting("discogs_token", tok.value.trim()));
}
document.getElementById("discogs-token-help")?.addEventListener("click", (e) => {
  e.preventDefault();
  void openUrl("https://www.discogs.com/settings/developers");
});
```

(Use the existing `getSetting`/`setSetting`/`openUrl` wrappers; import them if not already in scope.)

- [ ] **Step 3: Add the Identifier button + candidate panel to the review pane**

In `frontend/filing.ts`, in `renderFoot` (the review actions area), add an "Identifier" button near the metadata fields:

```ts
`<button class="sift-btn sift-identify" title="Chercher sur Discogs">
   <i class="ti ti-vinyl"></i> Identifier
 </button>
 <div class="sift-cands" hidden></div>`
```

Wire it (inside the same function that wires the other foot buttons):

```ts
const idBtn = root.querySelector<HTMLButtonElement>(".sift-identify");
const cands = root.querySelector<HTMLElement>(".sift-cands");
idBtn?.addEventListener("click", async () => {
  if (!cands) return;
  idBtn.disabled = true;
  idBtn.textContent = "Recherche…";
  try {
    const list = await identify(state.trackId);
    renderCandidates(cands, list);
  } catch (e) {
    const code = String(e);
    cands.hidden = false;
    cands.innerHTML =
      code === "NO_TOKEN"
        ? `<div class="sift-cands-msg">Ajoute ton token Discogs dans Réglages.</div>`
        : code.startsWith("RATE_LIMITED")
        ? `<div class="sift-cands-msg">Discogs limite les requêtes — réessaie dans ${esc(code.split(":")[1] || "60")}s.</div>`
        : `<div class="sift-cands-msg">Discogs injoignable.</div>`;
  } finally {
    idBtn.disabled = false;
    idBtn.innerHTML = `<i class="ti ti-vinyl"></i> Identifier`;
  }
});
```

Add the `renderCandidates` helper (top candidate expanded, rest behind "autres"):

```ts
function renderCandidates(host: HTMLElement, list: Candidate[]) {
  host.hidden = false;
  if (!list.length) {
    host.innerHTML = `<div class="sift-cands-msg">Rien sur Discogs.</div>`;
    return;
  }
  const row = (c: Candidate, i: number) => `
    <button class="sift-cand" data-i="${i}">
      ${c.cover_url ? `<img src="${esc(c.cover_url)}" alt="">` : `<span class="sift-cand-noart"><i class="ti ti-vinyl"></i></span>`}
      <span class="sift-cand-meta">
        <b>${esc(c.artist)} — ${esc(c.title)}</b>
        <small>${[c.label, c.year, c.styles.join(" · "), c.country].filter(Boolean).map(esc).join(" · ")}</small>
      </span>
    </button>`;
  const top = list[0];
  const rest = list.slice(1);
  host.innerHTML =
    row(top, 0) +
    (rest.length
      ? `<details class="sift-cand-more"><summary>autres (${rest.length})</summary>${rest.map((c, i) => row(c, i + 1)).join("")}</details>`
      : "");
  host.querySelectorAll<HTMLButtonElement>(".sift-cand").forEach((btn) => {
    btn.addEventListener("click", async () => {
      const c = list[Number(btn.dataset.i)];
      const applied = await applyIdentity(state.trackId, c);
      onIdentityApplied(applied); // update canonical/cover/genre chips + regenerate preview name
      host.hidden = true;
    });
  });
}
```

Implement `onIdentityApplied(applied: AppliedIdentity)` to update `state.canonical` (artist/title), refresh the editable fields, show the cover (via `convertFileSrc(applied.cover_path)` when set), render `applied.styles` as chips, and re-run whatever recomputes the filename preview (the same code path the editable fields already trigger on change).

Import `identify`, `applyIdentity`, `Candidate`, `AppliedIdentity` from `./ipc` at the top of `filing.ts`.

- [ ] **Step 4: Minimal styles**

Add to `frontend/styles.css` (match existing token-based colors; keep it lean):

```css
.sift-cands { margin-top: 8px; display: flex; flex-direction: column; gap: 4px; }
.sift-cands-msg { font-size: 12px; color: var(--color-text-tertiary); padding: 4px 2px; }
.sift-cand { display: flex; gap: 8px; align-items: center; text-align: left; background: none; border: 1px solid rgba(237,233,224,.12); border-radius: 6px; padding: 4px 6px; cursor: pointer; }
.sift-cand:hover { border-color: rgba(237,233,224,.3); }
.sift-cand img, .sift-cand-noart { width: 36px; height: 36px; border-radius: 4px; object-fit: cover; display: grid; place-items: center; background: rgba(237,233,224,.06); }
.sift-cand-meta { display: flex; flex-direction: column; min-width: 0; }
.sift-cand-meta small { color: var(--color-text-tertiary); }
.sift-cand-more summary { font-size: 12px; color: var(--color-text-tertiary); cursor: pointer; padding: 2px; }
```

- [ ] **Step 5: Verify frontend builds**

Run (from repo root): `npx tsc --noEmit` then `npm run build`
Expected: both green. Fix any type mismatch against the `Candidate`/`AppliedIdentity` interfaces.

- [ ] **Step 6: Commit**

```bash
git add frontend/ipc.ts frontend/filing.ts frontend/sift-live.ts frontend/styles.css
git commit -m "feat(m6a): UI — Discogs token in Réglages, Identifier button + candidate picker"
```

---

### Task 10: Manual verification + docs

**Files:**
- Modify: `docs/plan-implementation.md` (mark M6a status)

- [ ] **Step 1: Manual smoke test**

Relaunch the app (`cargo tauri dev`; Rust changes need a rebuild). With a real Discogs token saved in Réglages:
1. Open a track in Revue → click **Identifier** → candidates appear (top + autres).
2. Pick the right one → cover + label/year/genre chips update; proposed name updates.
3. **Ranger** → open the filed file in a tag editor (or re-open in Sift) → confirm label/year/genres + embedded cover are written, and the filename matches.
4. Clear the token → Identifier shows the "ajoute ton token" message (no crash).

- [ ] **Step 2: Update the milestone doc**

In `docs/plan-implementation.md`, mark M6a (Discogs identification) done with a one-line note pointing to the spec, and note M6b (Library tab) + AcoustID remain.

- [ ] **Step 3: Commit**

```bash
git add docs/plan-implementation.md
git commit -m "docs(m6a): mark Discogs identification done; M6b + AcoustID remain"
```

---

## Self-Review

**Spec coverage:**
- On-demand button → Task 9 (Identifier). ✅
- `MetadataProvider` trait, Discogs first → Tasks 2–3. ✅
- Best match + "autres" → Task 9 `renderCandidates`. ✅
- Sub-genres only (Discogs `style`), multiple → Tasks 1, 3 (`styles`), 5. ✅
- Token in Réglages (`discogs_token`) → Tasks 8, 9. ✅
- Cover downloaded + embedded at filing → Tasks 4, 6, 7, 8. ✅
- Nothing written before Ranger → apply only touches DB/cache; file writes happen in filing (Task 7). ✅
- Error handling (NO_TOKEN / RATE_LIMITED / NETWORK / no results / cover fail) → Tasks 3, 8, 9. ✅
- Tests on fixtures, no network in CI → Tasks 3 (parse fixture), 4 (path), 5 (DB), 6 (lofty round-trip), 7 (filing). ✅
- Storage: reuse `metadata`, new `track_genres` (migration v6) → Tasks 1, 5. ✅
- Out of scope (library tab, batch, AcoustID, genre editing) → not in any task. ✅

**Placeholder scan:** No TBD/TODO. Frontend steps that depend on existing UI (Réglages render location, preview-recompute path) are flagged with "find/confirm" notes rather than invented code, since the exact host markup must match what's there — this is intentional, not a placeholder for logic.

**Type consistency:** `Candidate` (fields identical across Rust Tasks 2/3/5 and TS Task 9). `AppliedIdentity` (Rust Task 5 ↔ TS Task 9). `apply_identity` (domain, Task 5) vs `apply_identity_cmd` (IPC, Task 8) — deliberately distinct names. `write_tags_full` signature identical in Tasks 6 and 7. `set_genres`/`get_genres` (Task 1) used in Tasks 5, 7. ✅

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-14-m6a-discogs-identification.md`. Two execution options:

1. **Subagent-Driven (recommended)** — fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** — execute tasks in this session with checkpoints for review.

Which approach?
