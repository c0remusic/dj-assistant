//! Acoustic fingerprint (Chromaprint) — the "sound" confirmation behind the name pre-filter.
//! `compute_for_path` decodes the file (reusing the analysis decoder) and produces a compact
//! fingerprint; `similarity` compares two fingerprints by bit-agreement with a small offset
//! search (no dependency on the crate's segment matcher). Reused later by the library scan.

use rusty_chromaprint::{match_fingerprints, Configuration, Fingerprinter};

/// A segment counts as "matching" when its score (0..32 bit-diffs, smaller = closer) is below
/// this. Conservative so unrelated tracks don't accumulate coverage.
const SEGMENT_SCORE_MAX: f64 = 8.0;
/// Match when matching segments cover at least this fraction of the shorter fingerprint.
pub const MATCH_THRESHOLD: f32 = 0.6;

/// Version du PRODUCTEUR d'empreinte, stampée dans `tracks.fingerprint_ver` à chaque écriture du
/// cache (migration v22). Rien ne l'expose au frontend : elle ne sert qu'à rendre bruyant le
/// désaccord entre l'algorithme courant et une empreinte déjà en base (issue #39).
///
/// **À incrémenter dès que deux empreintes produites de part et d'autre du changement cessent
/// d'être comparables entre elles**, c'est-à-dire :
///
/// - bump de `rusty-chromaprint` (épinglé `"0.3"`, `Cargo.toml:39`) — une 0.4 peut changer la
///   représentation d'une empreinte OU la sémantique de `match_fingerprints` ;
/// - changement de `config()` (`preset_test1`), du taux ou du nombre de canaux passés à `start`,
///   ou de la conversion f32 → i16 de `compute_for_path`.
///
/// Ce qu'elle ne couvre PAS : `MATCH_THRESHOLD` et `SEGMENT_SCORE_MAX` ne changent pas la valeur
/// stockée, ils changent la DÉCISION prise dessus. Ce qu'un tel changement périme, ce sont les
/// arêtes de `dup_edges` (v19), pas cette colonne.
///
/// Sans elle rien ne tombe : le dédoublonnage compare simplement des empreintes anciennes à des
/// neuves — des choses incomparables — et le taux de doublons change sans raison affichable, sur
/// une fonction dont l'utilisateur ne peut pas vérifier le résultat à la main.
pub const FINGERPRINT_CACHE_VERSION: i64 = 1;

/// Lit le cache `(tracks.fingerprint, tracks.fingerprint_ver)`. Une version absente (NULL — une
/// ligne écrite avant la v22 que son backfill n'a pas stampée) ou différente rend `None` : un
/// DÉFAUT DE CACHE, jamais une erreur. L'appelant recalcule, exactement comme `ipc::analyze_path`
/// le fait pour `report_cache_ver`.
///
/// Passer par cette fonction plutôt que de comparer la version en SQL garde la constante en un
/// seul endroit — un littéral écrit en dur dans une requête serait précisément la seconde
/// déclaration dont le désaccord ne fait tomber personne.
pub fn cached(raw: Option<String>, ver: Option<i64>) -> Option<String> {
    match ver {
        Some(v) if v == FINGERPRINT_CACHE_VERSION => raw,
        _ => None,
    }
}

fn config() -> Configuration {
    Configuration::preset_test1()
}

/// Decode `path` to mono 44.1 kHz and compute its Chromaprint fingerprint. Streams the PCM
/// through the fingerprinter (no full buffer). Errors on decode/codec failure.
pub fn compute_for_path(path: &str) -> Result<Vec<u32>, String> {
    let cfg = config();
    let mut printer = Fingerprinter::new(&cfg);
    printer
        .start(44100, 1)
        .map_err(|e| format!("fingerprint start: {e}"))?;
    let mut tmp: Vec<i16> = Vec::with_capacity(8192);
    let info = crate::analysis::decode::decode_pcm(path, 1, |block| {
        tmp.clear();
        tmp.extend(
            block
                .iter()
                .map(|&s| (s * 32767.0).clamp(-32768.0, 32767.0) as i16),
        );
        printer.consume(&tmp);
    })?;
    if let Some(err) = info.codec_error {
        return Err(err);
    }
    printer.finish();
    let fp = printer.fingerprint().to_vec();
    if fp.is_empty() {
        return Err("empty fingerprint (audio too short?)".into());
    }
    Ok(fp)
}

/// Serialize/deserialize the fingerprint for the `tracks.fingerprint` cache column.
pub fn encode(fp: &[u32]) -> String {
    fp.iter()
        .map(|x| x.to_string())
        .collect::<Vec<_>>()
        .join(",")
}
pub fn decode(s: &str) -> Vec<u32> {
    s.split(',').filter_map(|t| t.parse::<u32>().ok()).collect()
}

/// Similarity 0..1 = fraction of the shorter fingerprint covered by well-aligned, low-score
/// segments (the crate's matcher handles offset alignment). Unrelated tracks yield little
/// covered duration; two encodes of the same recording align over most of their length.
pub fn similarity(a: &[u32], b: &[u32]) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let cfg = config();
    let segments = match match_fingerprints(a, b, &cfg) {
        Ok(s) => s,
        Err(_) => return 0.0,
    };
    let n = a.len().min(b.len()).max(1);
    let matched: usize = segments
        .iter()
        .filter(|s| s.score < SEGMENT_SCORE_MAX)
        .map(|s| s.items_count)
        .sum();
    (matched as f32 / n as f32).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Option<String> {
        let p = format!("fixtures/{name}");
        if std::path::Path::new(&p).exists() {
            Some(p)
        } else {
            None
        }
    }

    #[test]
    fn fingerprint_is_deterministic_and_self_identical() {
        let Some(p) = fixture("real_320.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        crate::ffmpeg::init_ffmpeg_path();
        let a = compute_for_path(&p).expect("fingerprint");
        let b = compute_for_path(&p).expect("fingerprint");
        assert_eq!(a, b, "same file → same fingerprint");
        assert!(similarity(&a, &b) > 0.99, "self-similarity ≈ 1");
    }

    #[test]
    fn same_source_different_encode_matches() {
        // real_320.mp3 is the 320k MP3 of real_lossless.flac — same recording, two encodings.
        // This is the core M5 promise: detect the dupe across format/name.
        let (Some(p1), Some(p2)) = (fixture("real_320.mp3"), fixture("real_lossless.flac")) else {
            eprintln!("skip: no fixtures");
            return;
        };
        crate::ffmpeg::init_ffmpeg_path();
        let a = compute_for_path(&p1).expect("fp1");
        let b = compute_for_path(&p2).expect("fp2");
        let sim = similarity(&a, &b);
        assert!(
            sim >= MATCH_THRESHOLD,
            "same recording, different encode must match (got {sim})"
        );
    }

    #[test]
    fn different_audio_below_threshold() {
        // sweep (real) vs a steady dual-mono tone — clearly different audio.
        let (Some(p1), Some(p2)) = (fixture("real_320.mp3"), fixture("dual_mono.wav")) else {
            eprintln!("skip: no fixtures");
            return;
        };
        crate::ffmpeg::init_ffmpeg_path();
        let a = compute_for_path(&p1).expect("fp1");
        let b = compute_for_path(&p2).expect("fp2");
        let sim = similarity(&a, &b);
        assert!(
            sim < MATCH_THRESHOLD,
            "different audio must not match (got {sim})"
        );
    }

    #[test]
    fn encode_decode_round_trip() {
        let fp = vec![1u32, 42, 4_000_000_000];
        assert_eq!(decode(&encode(&fp)), fp);
    }

    /// Les trois états que `cached` doit distinguer, et le seul qui sert l'empreinte en base.
    /// Le cas NULL n'est pas théorique : c'est celui de toute ligne écrite avant la v22 que son
    /// backfill n'a pas stampée (empreinte vide, ou rapport lui-même périmé).
    #[test]
    fn cached_ne_sert_que_la_version_courante() {
        let fp = || Some("1,2,3".to_string());
        assert_eq!(
            cached(fp(), Some(FINGERPRINT_CACHE_VERSION)),
            fp(),
            "version courante : l'empreinte en cache doit être servie telle quelle"
        );
        assert_eq!(
            cached(fp(), None),
            None,
            "version absente (base d'avant la v22) : défaut de cache, pas une erreur"
        );
        assert_eq!(
            cached(fp(), Some(FINGERPRINT_CACHE_VERSION + 1)),
            None,
            "version différente : défaut de cache"
        );
        assert_eq!(
            cached(fp(), Some(FINGERPRINT_CACHE_VERSION - 1)),
            None,
            "une version ANCIENNE est aussi périmée — la comparaison est d'égalité, pas d'ordre"
        );
    }
}
