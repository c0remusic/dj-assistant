//! Occupation d'un volume, ventilée par format de fichier — la donnée derrière le graphique de
//! l'écran Clé USB.
//!
//! **Métadonnées seulement.** Le parcours lit `read_dir` et la taille de chaque entrée, jamais son
//! contenu : c'est ce qui rend l'opération supportable sur un disque de 500 Go. Aucun décodage,
//! aucune analyse, aucun verdict — un fichier n'est classé que par son extension et son
//! emplacement, deux choses connues sans ouvrir quoi que ce soit.
//!
//! Le résultat est mis en cache (voir `ipc_usage`), la clé d'invalidation étant l'espace libre du
//! volume : s'il a bougé, le contenu a bougé.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Component, Path};

/// Dossier de base Rekordbox sur une clé. Son contenu (base `.pdb`, analyses `.DAT`/`.EXT`,
/// vignettes) n'a aucun sens fichier par fichier : c'est un bloc, et c'est ainsi qu'un DJ le lit.
pub const REKORDBOX_DIR: &str = "PIONEER";

/// Le seau affiché pour ce bloc. Le `/` final le distingue à l'œil d'une extension dans la
/// légende, sans avoir à porter un drapeau séparé jusqu'au frontend.
pub const REKORDBOX_BUCKET: &str = "PIONEER/";

/// Un fichier sans extension du tout. Nommé plutôt que vide : une entrée sans libellé dans une
/// légende ressemble à un bug.
pub const NO_EXT_BUCKET: &str = "(sans extension)";

/// Une ligne du graphique : un format, ce qu'il pèse, combien de fichiers.
///
/// `Deserialize` autant que `Serialize` : ces lignes repartent vers le frontend, mais elles
/// dorment aussi en JSON dans le cache `volume_usage` et doivent en revenir.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExtUsage {
    pub ext: String,
    pub bytes: u64,
    pub file_count: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UsageError {
    /// La racine du volume n'est pas lisible (disque débranché entre la liste et le parcours).
    Unreadable(String),
}

impl std::fmt::Display for UsageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UsageError::Unreadable(m) => write!(f, "volume illisible: {m}"),
        }
    }
}

impl std::error::Error for UsageError {}

/// Dans quel seau tombe ce chemin, exprimé RELATIVEMENT à la racine du volume.
///
/// Deux règles, dans cet ordre : tout ce qui vit sous `PIONEER/` est du Rekordbox quelle que soit
/// son extension (sinon la base et les analyses se dispersent en `.pdb`, `.dat`, `.ext`, `.jpg`,
/// et personne ne les reconnaît) ; sinon l'extension, en minuscules et préfixée d'un point pour
/// que la légende affiche `.wav` et non `wav`.
///
/// Fonction pure : c'est elle qui porte toute la logique de classement, et elle se teste sans
/// toucher un disque.
/// Version du schéma de classement. **À incrémenter dès que `bucket_for` change de résultat.**
///
/// Le cache d'occupation s'invalide sur l'espace libre, ce qui détecte un contenu qui bouge — mais
/// pas un classement qui change : un disque intact garderait indéfiniment une ventilation calculée
/// par une ancienne règle. Le dépôt connaît déjà ce piège, c'est toute la raison d'être de la
/// migration v16 (`analysis::REPORT_CACHE_VERSION` bumpée sans purge, 3907 rapports devenus
/// inservables et jamais relus).
pub const BUCKET_SCHEME_VERSION: i64 = 2;

/// Deux orthographes, un seul format. Mesuré sur le vrai disque DJ le 2026-08-01 : `.aif` et
/// `.aiff` cohabitaient, coupant 20,1 Go d'AIFF en deux segments de couleur identique — la barre
/// donnait deux formats là où il n'y en a qu'un. Un DJ ne pense pas « aif contre aiff ».
///
/// Table volontairement courte : uniquement des paires dont l'équivalence ne se discute pas. Une
/// extension inconnue n'est jamais renommée — inventer une équivalence serait pire que la scission.
fn canonical_ext(ext: &str) -> &str {
    match ext {
        ".aif" => ".aiff",
        ".jpeg" => ".jpg",
        ".tif" => ".tiff",
        ".mpeg" => ".mpg",
        _ => ext,
    }
}

pub fn bucket_for(relative: &Path) -> String {
    let mut components = relative.components();
    if let Some(Component::Normal(first)) = components.next() {
        if first.eq_ignore_ascii_case(REKORDBOX_DIR) {
            // `PIONEER` seul, sans rien dessous, n'est pas un fichier : on n'arrive ici que pour
            // un fichier, donc il y a forcément au moins un composant de plus.
            return REKORDBOX_BUCKET.to_string();
        }
    }
    match relative.extension().and_then(|e| e.to_str()) {
        Some(ext) if !ext.is_empty() => {
            canonical_ext(&format!(".{}", ext.to_ascii_lowercase())).to_string()
        }
        _ => NO_EXT_BUCKET.to_string(),
    }
}

/// Agrège des `(seau, taille)` en lignes triées du plus gros au plus petit — l'ordre du graphique.
/// À taille égale, l'ordre alphabétique, sinon deux exécutions sur les mêmes données peuvent
/// rendre des barres différentes (HashMap n'a aucun ordre).
pub fn aggregate(entries: impl IntoIterator<Item = (String, u64)>) -> Vec<ExtUsage> {
    let mut totals: HashMap<String, (u64, u64)> = HashMap::new();
    for (bucket, bytes) in entries {
        let slot = totals.entry(bucket).or_insert((0, 0));
        slot.0 += bytes;
        slot.1 += 1;
    }
    let mut out: Vec<ExtUsage> = totals
        .into_iter()
        .map(|(ext, (bytes, file_count))| ExtUsage {
            ext,
            bytes,
            file_count,
        })
        .collect();
    out.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.ext.cmp(&b.ext)));
    out
}

/// Fusionne les ventilations de plusieurs volumes d'un même disque physique.
///
/// Un disque partitionné en deux monte deux lettres ; ses tailles totale et libre sont déjà la
/// somme des deux (voir `VolumeFacts`), donc la ventilation doit l'être aussi, sinon la barre ne
/// totalise pas la carte qui la surplombe.
pub fn merge(all: impl IntoIterator<Item = Vec<ExtUsage>>) -> Vec<ExtUsage> {
    let mut totals: HashMap<String, (u64, u64)> = HashMap::new();
    for list in all {
        for u in list {
            let slot = totals.entry(u.ext).or_insert((0, 0));
            slot.0 += u.bytes;
            slot.1 += u.file_count;
        }
    }
    let mut out: Vec<ExtUsage> = totals
        .into_iter()
        .map(|(ext, (bytes, file_count))| ExtUsage {
            ext,
            bytes,
            file_count,
        })
        .collect();
    out.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.ext.cmp(&b.ext)));
    out
}

/// Parcourt le volume et rend la ventilation par seau.
///
/// Itératif, pas récursif : une arborescence profonde ou un cycle de jonctions ferait exploser la
/// pile, et sur un disque fourni par l'utilisateur la profondeur n'est pas notre décision.
///
/// Les liens symboliques et points d'analyse sont **sautés** — `file_type()` les rapporte sans les
/// suivre. Les suivre, c'est compter deux fois le même octet au mieux, et boucler à l'infini au
/// pire ; une jonction Windows qui pointe vers la racine du volume suffit.
///
/// Un sous-dossier illisible (droits, verrou) est ignoré silencieusement au niveau du dossier :
/// abandonner tout le parcours parce qu'un dossier système résiste rendrait la fonctionnalité
/// inutilisable, alors que le total reste juste à ce dossier près. La racine, elle, est une vraie
/// erreur : si elle ne s'ouvre pas, le disque a été débranché.
pub fn scan_volume(root: &Path) -> Result<Vec<ExtUsage>, UsageError> {
    let first = std::fs::read_dir(root).map_err(|e| UsageError::Unreadable(e.to_string()))?;

    let mut buckets: Vec<(String, u64)> = Vec::new();
    let mut stack: Vec<std::fs::ReadDir> = vec![first];

    while let Some(mut dir) = stack.pop() {
        for entry in dir.by_ref() {
            let Ok(entry) = entry else { continue };
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            let path = entry.path();
            if kind.is_dir() {
                if let Ok(sub) = std::fs::read_dir(&path) {
                    stack.push(sub);
                }
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            let relative = path.strip_prefix(root).unwrap_or(&path);
            buckets.push((bucket_for(relative), meta.len()));
        }
    }

    Ok(aggregate(buckets))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn extension_is_lowercased_and_dotted() {
        assert_eq!(bucket_for(&PathBuf::from("Musique/Track.WAV")), ".wav");
        assert_eq!(bucket_for(&PathBuf::from("a/b/c.mp3")), ".mp3");
    }

    /// Une clé DJ mélange `.WAV`, `.Wav` et `.wav` selon l'outil qui a écrit le fichier. Trois
    /// segments pour un seul format rendrait le graphique faux, pas seulement laid.
    #[test]
    fn case_variants_land_in_the_same_bucket() {
        let mixed = vec![
            (bucket_for(&PathBuf::from("A.WAV")), 100u64),
            (bucket_for(&PathBuf::from("B.wav")), 200),
            (bucket_for(&PathBuf::from("C.Wav")), 300),
        ];
        let out = aggregate(mixed);
        assert_eq!(out.len(), 1, "un seul seau attendu, obtenu {out:?}");
        assert_eq!(out[0].bytes, 600);
        assert_eq!(out[0].file_count, 3);
    }

    /// Tout le contenu de PIONEER/ est un bloc : la base, les analyses et les vignettes n'ont
    /// aucun sens séparément, et les éclater en .pdb/.dat/.ext/.jpg noie la légende.
    #[test]
    fn everything_under_pioneer_is_one_bucket() {
        for p in [
            "PIONEER/rekordbox/export.pdb",
            "PIONEER/USBANLZ/P016/0000B4E5/ANLZ0000.DAT",
            "PIONEER/artwork/cover.jpg",
        ] {
            assert_eq!(bucket_for(&PathBuf::from(p)), REKORDBOX_BUCKET, "{p}");
        }
    }

    /// Le dossier est écrit `PIONEER` par Rekordbox, mais un volume FAT32 recopié peut le rendre
    /// en `Pioneer`. La comparaison est insensible à la casse, comme le système de fichiers.
    #[test]
    fn pioneer_folder_matches_whatever_the_case() {
        assert_eq!(
            bucket_for(&PathBuf::from("Pioneer/rekordbox/export.pdb")),
            REKORDBOX_BUCKET
        );
    }

    /// Un `.pdb` ailleurs que sous PIONEER/ n'est PAS du Rekordbox — la règle est l'emplacement,
    /// pas l'extension.
    #[test]
    fn same_extension_outside_pioneer_is_not_rekordbox() {
        assert_eq!(bucket_for(&PathBuf::from("sauvegardes/export.pdb")), ".pdb");
    }

    /// Le défaut trouvé en faisant tourner le graphique sur le vrai disque DJ : 14,9 Go de `.aif`
    /// et 5,2 Go de `.aiff` en deux segments, pour un seul et même format.
    #[test]
    fn aif_and_aiff_are_one_format() {
        assert_eq!(bucket_for(&PathBuf::from("a.aif")), ".aiff");
        assert_eq!(bucket_for(&PathBuf::from("b.AIFF")), ".aiff");
        let out = aggregate(vec![
            (bucket_for(&PathBuf::from("a.aif")), 100u64),
            (bucket_for(&PathBuf::from("b.aiff")), 50),
        ]);
        assert_eq!(out.len(), 1, "un seul seau attendu, obtenu {out:?}");
        assert_eq!(out[0].bytes, 150);
    }

    #[test]
    fn other_undisputed_spellings_merge_too() {
        assert_eq!(bucket_for(&PathBuf::from("cover.jpeg")), ".jpg");
        assert_eq!(bucket_for(&PathBuf::from("scan.TIF")), ".tiff");
    }

    /// La table d'equivalences reste courte exprès : renommer une extension inconnue inventerait
    /// une équivalence, ce qui est pire que la scission qu'on corrige.
    #[test]
    fn unknown_extensions_are_never_renamed() {
        for p in ["a.wav", "b.mp3", "c.flac", "d.xyz", "e.opus"] {
            let bucket = bucket_for(&PathBuf::from(p));
            let ext = format!(".{}", p.rsplit('.').next().unwrap_or(""));
            assert_eq!(bucket, ext, "{p} ne doit pas être renommé");
        }
    }

    #[test]
    fn files_without_extension_get_a_named_bucket() {
        assert_eq!(bucket_for(&PathBuf::from("LISEZMOI")), NO_EXT_BUCKET);
        assert_eq!(
            bucket_for(&PathBuf::from("dossier/Makefile")),
            NO_EXT_BUCKET
        );
    }

    /// Un fichier caché de type `.DS_Store` n'a pas d'extension au sens de `Path::extension` —
    /// c'est un nom qui commence par un point. Le compter comme un format `.ds_store` inventerait
    /// une ligne de légende.
    #[test]
    fn dotfiles_are_not_an_extension() {
        assert_eq!(bucket_for(&PathBuf::from(".DS_Store")), NO_EXT_BUCKET);
    }

    #[test]
    fn aggregate_sorts_biggest_first() {
        let out = aggregate(vec![
            (".mp3".to_string(), 10),
            (".wav".to_string(), 100),
            (".flac".to_string(), 50),
        ]);
        assert_eq!(
            out.iter().map(|e| e.ext.as_str()).collect::<Vec<_>>(),
            vec![".wav", ".flac", ".mp3"]
        );
    }

    /// À taille égale, l'ordre doit être stable : un HashMap n'en a aucun, donc sans départage
    /// deux appels sur les mêmes données rendraient des barres dans un ordre différent.
    #[test]
    fn equal_sizes_break_the_tie_alphabetically() {
        let out = aggregate(vec![
            (".wav".to_string(), 100),
            (".aiff".to_string(), 100),
            (".mp3".to_string(), 100),
        ]);
        assert_eq!(
            out.iter().map(|e| e.ext.as_str()).collect::<Vec<_>>(),
            vec![".aiff", ".mp3", ".wav"]
        );
    }

    #[test]
    fn missing_root_is_an_error_not_an_empty_result() {
        let res = scan_volume(&PathBuf::from("Z:/ce-volume-n-existe-pas-42"));
        assert!(matches!(res, Err(UsageError::Unreadable(_))), "{res:?}");
    }

    /// Parcours réel sur une arborescence temporaire : c'est le seul test qui exerce la pile
    /// itérative, le saut des dossiers et le calcul de chemin relatif.
    #[test]
    fn scan_walks_subdirectories_and_buckets_by_location() {
        let root = tempfile::tempdir().expect("tempdir");
        let r = root.path();
        std::fs::create_dir_all(r.join("Musique/Sets")).expect("mkdir");
        std::fs::create_dir_all(r.join("PIONEER/rekordbox")).expect("mkdir");
        std::fs::write(r.join("Musique/a.wav"), vec![0u8; 1000]).expect("write");
        std::fs::write(r.join("Musique/Sets/b.WAV"), vec![0u8; 500]).expect("write");
        std::fs::write(r.join("Musique/c.mp3"), vec![0u8; 200]).expect("write");
        std::fs::write(r.join("PIONEER/rekordbox/export.pdb"), vec![0u8; 50]).expect("write");

        let out = scan_volume(r).expect("scan");
        let find = |e: &str| out.iter().find(|u| u.ext == e).cloned();

        let wav = find(".wav").expect("wav manquant");
        assert_eq!(
            wav.bytes, 1500,
            "les deux .wav de dossiers différents s'additionnent"
        );
        assert_eq!(wav.file_count, 2);
        assert_eq!(find(".mp3").expect("mp3").bytes, 200);
        assert_eq!(find(REKORDBOX_BUCKET).expect("pioneer").bytes, 50);
        assert_eq!(out.len(), 3, "trois seaux attendus, obtenu {out:?}");
    }
}
