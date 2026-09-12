//! Dev-only: persiste les annotations du pointeur visuel (Alt+Clic) dans
//! docs/annotations.jsonl — une ligne JSON par annotation, append seul.
//! Jamais d'écriture dans les sources. Gated debug comme dev_locate.
use std::io::Write;
use std::path::{Path, PathBuf};

fn annotations_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs")
        .join("annotations.jsonl")
}

fn append_line(path: &Path, mut annotation: serde_json::Value) -> Result<(), String> {
    let Some(obj) = annotation.as_object_mut() else {
        return Err("annotation doit être un objet JSON".into());
    };
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    obj.insert("ts".into(), serde_json::Value::from(ts));
    let line = serde_json::to_string(&annotation).map_err(|e| e.to_string())?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("ouverture {}: {e}", path.display()))?;
    writeln!(file, "{line}").map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_annotation(annotation: serde_json::Value) -> Result<(), String> {
    if !cfg!(debug_assertions) {
        return Err("save_annotation is a dev-only command".into());
    }
    append_line(&annotations_path(), annotation)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_file(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("sift-annot-test-{name}.jsonl"));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn appends_one_json_line_per_call() {
        let path = tmp_file("append");
        append_line(&path, serde_json::json!({"note": "trop tassé"})).unwrap();
        append_line(
            &path,
            serde_json::json!({"note": "couleur \"bizarre\"\nsur 2 lignes"}),
        )
        .unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2, "2 appels -> 2 lignes");
        for line in &lines {
            let v: serde_json::Value =
                serde_json::from_str(line).expect("chaque ligne est du JSON valide");
            assert!(v.get("note").is_some());
            assert!(
                v.get("ts").and_then(|t| t.as_u64()).is_some(),
                "ts epoch ajouté côté Rust"
            );
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn rejects_non_object_annotation() {
        let path = tmp_file("nonobj");
        let err = append_line(&path, serde_json::json!("just a string")).unwrap_err();
        assert!(err.contains("objet"), "message d'erreur explicite: {err}");
        assert!(!path.exists(), "rien écrit en cas de refus");
    }
}
