//! Dev-only helper for the click-to-source inspector overlay: given a CSS selector/class
//! clicked in the running app, find where it's actually defined/consumed in `frontend/`.
//! Plain substring search + line context, same responsibility as
//! `docs/archive/design_handoff_sift_refonte/token-sync/locate.cjs` (that one runs under Node for the
//! token editor; this one runs inside the compiled app, so it's a separate implementation
//! rather than shared code across the Rust/JS boundary).
//!
//! Gated to debug builds only (same pattern as the log plugin in lib.rs) — never useful, and
//! never reachable from the UI, in a release build.
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
pub struct SourceMatch {
    file: String,
    line: usize,
    excerpt: String,
}

fn frontend_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("frontend")
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("ts") | Some("css")
        ) {
            out.push(path);
        }
    }
}

#[tauri::command]
pub fn locate_source(identifier: String) -> Result<Vec<SourceMatch>, String> {
    if !cfg!(debug_assertions) {
        return Err("locate_source is a dev-only command".into());
    }
    let root = frontend_dir();
    let mut files = Vec::new();
    walk(&root, &mut files);

    let mut matches = Vec::new();
    for file in files {
        let Ok(content) = std::fs::read_to_string(&file) else {
            continue;
        };
        let lines: Vec<&str> = content.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if !line.contains(&identifier) {
                continue;
            }
            let start = i.saturating_sub(1);
            let end = (i + 1).min(lines.len() - 1);
            let excerpt = lines[start..=end].join("\n");
            let rel = file
                .strip_prefix(root.parent().unwrap_or(&root))
                .unwrap_or(&file)
                .to_string_lossy()
                .replace('\\', "/");
            matches.push(SourceMatch {
                file: rel,
                line: i + 1,
                excerpt,
            });
        }
    }
    Ok(matches)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_known_token_consumer() {
        let matches = locate_source("--color-text-danger".to_string()).unwrap();
        assert!(
            !matches.is_empty(),
            "expected at least one real consumer of --color-text-danger in frontend/"
        );
        assert!(matches.iter().any(|m| m.file.ends_with("styles.css")));
    }

    #[test]
    fn unknown_identifier_returns_empty() {
        let matches = locate_source("this-token-does-not-exist-anywhere".to_string()).unwrap();
        assert!(matches.is_empty());
    }
}
