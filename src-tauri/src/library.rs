//! The destination bins: every subdirectory (recursive) under the configured library
//! root. Walks the tree with `walkdir`, skipping hidden dirs (e.g. the `.sift-trash`
//! corbeille). Also creates new bins and resolves collision-free destination paths. Pure
//! filesystem work; the root path comes from `settings::LIBRARY_ROOT`.
#![allow(dead_code)]

use serde::Serialize;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// One destination folder under the library root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Bin {
    /// Path relative to the root, forward-slash separated (e.g. "House/Deep").
    pub rel: String,
    /// Display name = last path component (e.g. "Deep").
    pub name: String,
    /// Nesting depth under root (1 = direct child).
    pub depth: usize,
}

/// Whether a directory name is hidden (leading dot) — excluded from bins.
fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

/// List all bins (recursive subdirectories) under `root`, sorted by relative path. Returns
/// an empty list if root doesn't exist. Hidden directories and their subtrees are skipped.
pub fn list_bins(root: &Path) -> Vec<Bin> {
    let mut bins = Vec::new();
    let walker = WalkDir::new(root)
        .min_depth(1)
        .into_iter()
        .filter_entry(|e| {
            // skip hidden dirs entirely (prunes their subtree too)
            !e.file_name().to_str().map(is_hidden).unwrap_or(false)
        });
    for entry in walker.flatten() {
        if !entry.file_type().is_dir() {
            continue;
        }
        let rel_path = match entry.path().strip_prefix(root) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let rel = rel_path
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect::<Vec<_>>()
            .join("/");
        if rel.is_empty() {
            continue;
        }
        let name = entry.file_name().to_str().unwrap_or_default().to_string();
        let depth = entry.depth();
        bins.push(Bin { rel, name, depth });
    }
    bins.sort_by(|a, b| a.rel.cmp(&b.rel));
    bins
}

/// Join `rel` under `root`, sanitizing every path segment and refusing anything that would
/// escape the root: `..`, absolute paths, or drive prefixes (both `/` and `\` separators
/// are accepted from the UI). Returns the contained absolute path. This is the single
/// containment guard every filesystem-mutating command must funnel destinations through —
/// `bin_rel` / `parent_rel` arrive from the (untrusted) webview and are otherwise free to
/// point anywhere (`..\..\Startup`, `C:\Windows\…`), which `Path::join` would honour.
pub fn safe_join(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let mut out = root.to_path_buf();
    for raw in rel.split(['/', '\\']) {
        if raw.is_empty() || raw == "." {
            continue;
        }
        if raw == ".." {
            return Err("path escapes the library root".into());
        }
        let safe = crate::naming::sanitize(raw);
        if safe.is_empty() {
            return Err("invalid path component".into());
        }
        out.push(safe);
    }
    Ok(out)
}

/// Create a new bin folder named `name` (sanitized) under `root/parent_rel`. `parent_rel`
/// "" means directly under root. Both `name` and every component of `parent_rel` are
/// sanitized and contained under `root` (see `safe_join`). Returns the created Bin.
pub fn create_bin(root: &Path, parent_rel: &str, name: &str) -> Result<Bin, String> {
    let safe = crate::naming::sanitize(name);
    if safe.is_empty() {
        return Err("empty bin name".into());
    }
    let abs = safe_join(root, parent_rel)?.join(&safe);
    std::fs::create_dir_all(&abs).map_err(|e| format!("create bin: {e}"))?;
    let rel = abs
        .strip_prefix(root)
        .map_err(|_| "bin outside root".to_string())?
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("/");
    let depth = rel.split('/').count();
    Ok(Bin { rel, name: safe, depth })
}

/// Return a path that does not already exist, appending " (N)" before the extension when
/// the given path is taken. Used so filing never overwrites an existing file.
pub fn ensure_unique(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|e| e.to_str());
    for n in 2..10_000 {
        let candidate = match ext {
            Some(e) => parent.join(format!("{stem} ({n}).{e}")),
            None => parent.join(format!("{stem} ({n})")),
        };
        if !candidate.exists() {
            return candidate;
        }
    }
    // pathological fallback: timestamped name
    parent.join(format!("{stem} ({}).bak", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_recursive_bins_sorted_skipping_hidden() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("House/Deep")).unwrap();
        std::fs::create_dir_all(root.join("House/Acid")).unwrap();
        std::fs::create_dir_all(root.join("Techno")).unwrap();
        std::fs::create_dir_all(root.join(".sift-trash/42")).unwrap();

        let bins = list_bins(root);
        let rels: Vec<&str> = bins.iter().map(|b| b.rel.as_str()).collect();
        assert_eq!(rels, vec!["House", "House/Acid", "House/Deep", "Techno"]);
        // hidden subtree excluded
        assert!(!rels.iter().any(|r| r.contains("sift-trash")));
        // depth + name sane
        let deep = bins.iter().find(|b| b.rel == "House/Deep").unwrap();
        assert_eq!(deep.name, "Deep");
        assert_eq!(deep.depth, 2);
    }

    #[test]
    fn missing_root_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("nope");
        assert!(list_bins(&root).is_empty());
    }

    #[test]
    fn create_bin_makes_sanitized_subfolder() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("House")).unwrap();

        let bin = create_bin(root, "House", "Deep/Soulful?").unwrap();
        assert_eq!(bin.rel, "House/Deep Soulful"); // "/" and "?" sanitized to spaces→collapsed
        assert!(root.join("House/Deep Soulful").is_dir());
    }

    #[test]
    fn create_bin_at_root_level() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let bin = create_bin(root, "", "Disco").unwrap();
        assert_eq!(bin.rel, "Disco");
        assert_eq!(bin.depth, 1);
        assert!(root.join("Disco").is_dir());
    }

    #[test]
    fn safe_join_contains_under_root() {
        let root = Path::new("C:/lib");
        // traversal (either separator) is refused
        assert!(safe_join(root, "../evil").is_err());
        assert!(safe_join(root, "House/../../x").is_err());
        assert!(safe_join(root, "..\\evil").is_err());
        // normal nested path is contained
        let j = safe_join(root, "House/Deep").unwrap();
        assert!(j.ends_with("Deep") && j.starts_with("C:/lib"));
        // an absolute/drive-prefixed rel is sanitized into components under root, not honoured
        let a = safe_join(root, "C:/Windows/System32").unwrap();
        assert!(a.starts_with("C:/lib"));
        // "" and "." resolve to the root itself
        assert_eq!(safe_join(root, "").unwrap(), root.to_path_buf());
    }

    #[test]
    fn create_bin_rejects_parent_traversal() {
        let dir = tempfile::tempdir().unwrap();
        assert!(create_bin(dir.path(), "../../etc", "evil").is_err());
    }

    #[test]
    fn ensure_unique_appends_suffix_on_collision() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("Track.mp3");
        // free → unchanged
        assert_eq!(ensure_unique(&base), base);
        // occupied → " (2)"
        std::fs::write(&base, b"x").unwrap();
        assert_eq!(ensure_unique(&base), dir.path().join("Track (2).mp3"));
    }
}
