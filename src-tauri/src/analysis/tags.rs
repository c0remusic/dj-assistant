//! Declared audio properties + tag metadata via `lofty` (read-only).

use crate::analysis::Rail;
use lofty::file::{AudioFile, FileType, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::{ItemKey, TagType};

/// What we read from the container without decoding: declared rail, bitrate, duration,
/// channels, tag carrier(s), CDJ-tag sanity, embedded cover presence, and the content-sniffed
/// rail (magic bytes — see `rail_from_content` doc comment below).
#[derive(Debug, Clone, PartialEq)]
pub struct TagInfo {
    pub declared_rail: Rail,
    pub content_rail: Rail,
    pub declared_bitrate: Option<u32>,
    pub duration_sec: f32,
    pub channels: u16,
    /// TYPE(S) de tag réellement présents dans le fichier, noms `lofty` **triés** puis joints par
    /// `+` (« Id3v2 », « RiffInfo », « Id3v1+Id3v2 »…), `None` si le fichier ne porte aucun tag.
    /// L'ordre est alphabétique, pas celui du parcours du conteneur : la valeur est persistée.
    ///
    /// Le nom du champ est historique (colonne SQLite `id3_version`, miroir TS `contracts.ts`) :
    /// il ne porte PLUS le stub `Some("ID3")` posé par extension, mais ne porte pas non plus de
    /// sous-version ID3 — voir `tag_type_name` pour la raison.
    pub id3_version: Option<String>,
    /// Artiste ET Titre vivent dans un tag d'un TYPE que la platine lit en navigation directe
    /// USB — voir `tag_type_readable_on_cdj`. Présence seule ne suffit pas (issue #46).
    pub tags_cdj_ok: bool,
    pub has_cover: bool,
}

/// Nom stable d'un `TagType` de `lofty`, écrit à la main plutôt que via `Debug` : c'est une valeur
/// persistée en base et exposée à l'IPC, elle ne doit pas bouger si `lofty` renomme une variante.
///
/// ⚠️ Aucune sous-version ID3 ici, et ce n'est pas un raccourci : sur un `Tag` GÉNÉRIQUE
/// (`TaggedFile::tags()`), `lofty` 0.24.0 ne l'expose pas — il remonte tout en ID3v2.4 en interne
/// (« This covers all ID3v2 versions since they all get upgraded to ID3v2.4 »,
/// `lofty-0.24.0/src/tag/tag_type.rs:103`). La lire demanderait le tag concret `Id3v2Tag` et son
/// `original_version()`, donc une seconde lecture typée du fichier. Elle ne changerait aucun
/// verdict : les manuels Pioneer déclarent v1, v1.1, v2.2.0, v2.3.0 ET v2.4.0
/// (`docs/cdj-metadata-formats.md`, matrice + § « Piège d'API à connaître »).
fn tag_type_name(tag_type: TagType) -> &'static str {
    match tag_type {
        TagType::Ape => "Ape",
        TagType::Id3v1 => "Id3v1",
        TagType::Id3v2 => "Id3v2",
        TagType::Mp4Ilst => "Mp4Ilst",
        TagType::VorbisComments => "VorbisComments",
        TagType::RiffInfo => "RiffInfo",
        TagType::AiffText => "AiffText",
        // `TagType` est `#[non_exhaustive]` en amont (`lofty-0.24.0/src/tag/tag_type.rs:97`), donc
        // ce bras est obligatoire : une variante ajoutée plus tard doit se lire « inconnu », et
        // jamais se faire passer pour un porteur déjà vérifié contre la matrice.
        _ => "Unknown",
    }
}

/// Un tag de ce TYPE, dans ce CONTENEUR, est-il lu par une platine Pioneer / AlphaTheta en
/// navigation directe sur clé USB (sans base rekordbox) ?
///
/// Table de vérité = la matrice de `docs/cdj-metadata-formats.md` (§ Matrice de synthèse et
/// § « Ce qui est implémentable côté Sift »), établie le 2026-08-21 par recherche sourcée
/// (issue #46). Le couple conteneur × type est nécessaire : `Id3v2` est lisible dans un MP3 et
/// dans un AIFF, mais PAS dans un WAV (chunk `id3 ` non standard).
///
/// Défaut = `false`, par prudence : tout couple que le doc n'établit pas — AIFF natif
/// (`AiffText`, « non établi »), WAV sous n'importe quel porteur, `Ape` — ne compte pas.
///
/// ⚠️ Périmètre : ce prédicat ne juge QUE le porteur des tags. Il ne dit rien du codec (un FLAC
/// est injouable avant 2016, donc rien ne s'affiche), ni de l'encodage du texte, ni du système de
/// fichiers — traités ailleurs dans l'app.
fn tag_type_readable_on_cdj(file_type: FileType, tag_type: TagType) -> bool {
    match (file_type, tag_type) {
        // MP3 : ID3v2 (toutes sous-versions) et ID3v1/v1.1 — « Haute » et « Moyenne-haute ».
        (FileType::Mpeg, TagType::Id3v2 | TagType::Id3v1) => true,
        // AIFF : uniquement l'ID3 en chunk. Les chunks natifs NAME/AUTH (`AiffText`) sont
        // « Non établi » dans le doc, donc ils ne comptent pas.
        (FileType::Aiff, TagType::Id3v2) => true,
        // FLAC : Vorbis comments.
        (FileType::Flac, TagType::VorbisComments) => true,
        // MP4 (.m4a, AAC/ALAC) : atomes iTunes — « OUI » dans la matrice (confiance Moyenne /
        // Moyenne-haute, rang Primaire).
        (FileType::Mp4, TagType::Mp4Ilst) => true,
        // AAC brut en ADTS (`.aac`, extension scannée par `scanner.rs`) : `lofty` en fait un
        // conteneur distinct du MP4, sans atome iTunes — le porteur y est un ID3v2 collé au flux.
        // La matrice range `.aac` dans la même ligne « AAC (.m4a/.aac) → OUI » que `.m4a`, mais sa
        // colonne Porteur n'y nomme QUE `Mp4Ilst`, qu'un ADTS ne peut pas porter : sans ce bras,
        // une ligne que le doc déclare lisible n'a aucun porteur qui compte, donc rend toujours
        // `false`. Le bras suit la ligne du doc, pas sa colonne Porteur — le seul couple de cette
        // table dont la source est une déduction et non une lecture directe.
        (FileType::Aac, TagType::Id3v2) => true,
        // WAV : ni `RiffInfo` (« NON fiable — souvent nom de fichier seul ») ni l'ID3 en chunk
        // `id3 ` (« présumé non fiable »). C'est le faux positif central de l'issue #46.
        _ => false,
    }
}

/// Lossless vs lossy from the file extension (container/codec lineage).
pub fn rail_from_ext(ext: &str) -> Rail {
    match ext.to_ascii_lowercase().as_str() {
        "flac" | "wav" | "aif" | "aiff" | "alac" => Rail::Lossless,
        "mp3" | "aac" | "m4a" | "ogg" | "opus" => Rail::Lossy,
        _ => Rail::Unknown,
    }
}

/// Maps a lofty-detected `FileType` to our lossless/lossy rail. `Rail::Unknown` on anything not
/// confidently identified — callers must never manufacture a mismatch they can't back with a
/// confident read.
fn rail_from_file_type(file_type: FileType) -> Rail {
    match file_type {
        FileType::Flac | FileType::Wav | FileType::Aiff | FileType::Ape | FileType::WavPack => {
            Rail::Lossless
        }
        FileType::Mpeg | FileType::Vorbis | FileType::Opus | FileType::Speex => Rail::Lossy,
        _ => Rail::Unknown,
    }
}

/// Lossless vs lossy from the file's ACTUAL content (lofty's content-sniffing probe, magic
/// bytes — NOT the extension). Used where extension trust actually matters: the filing
/// no-upscale guard (`filing.rs::plan_file`), to catch a lossy file mislabeled with a lossless
/// extension (e.g. an MP3 renamed `.flac`) before it gets "converted" into a fabricated lossless
/// file. The analysis pipeline gets the same signal for free from `read()`'s `content_rail` (one
/// probe instead of two) — this standalone entry point stays for callers with no `TagInfo` in
/// hand.
pub fn rail_from_content(path: &str) -> Rail {
    fn try_read(path: &str) -> lofty::error::Result<lofty::file::TaggedFile> {
        Probe::open(path)?.guess_file_type()?.read()
    }
    match try_read(path) {
        Ok(tagged) => rail_from_file_type(tagged.file_type()),
        Err(_) => Rail::Unknown,
    }
}

/// Reads tag/property info. On unreadable container, returns a conservative Unknown info
/// (the caller still has decode results + codec_error).
pub fn read(path: &str) -> TagInfo {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let rail = rail_from_ext(ext);

    fn try_read(path: &str) -> lofty::error::Result<lofty::file::TaggedFile> {
        Probe::open(path)?.guess_file_type()?.read()
    }

    match try_read(path) {
        Ok(tagged) => {
            // Le conteneur vient de ce que `lofty` a RECONNU dans les octets, pas de l'extension :
            // un WAV renommé `.mp3` doit rester jugé comme un WAV.
            let file_type = tagged.file_type();
            let content_rail = rail_from_file_type(file_type);
            let props = tagged.properties();
            let has_cover = tagged.tags().iter().any(|t| !t.pictures().is_empty());
            // Capture honnête du porteur, en remplacement du stub `Some("ID3")` que l'extension
            // `.mp3` posait à l'aveugle (issue #46, point « id3_version est aussi un stub »).
            let mut carriers: Vec<&'static str> = Vec::new();
            for t in tagged.tags() {
                let name = tag_type_name(t.tag_type());
                if !carriers.contains(&name) {
                    carriers.push(name);
                }
            }
            // Tri avant jointure : `tagged.tags()` rend les porteurs dans l'ordre de lecture du
            // conteneur, qui n'est garanti par rien. Deux fichiers portant les mêmes tags doivent
            // rendre la MÊME chaîne — c'est une valeur persistée en base et un contrat de
            // `shared/contracts.ts` (« Id3v2+Id3v1 »), pas un ordre de parcours.
            carriers.sort_unstable();
            let id3_version = (!carriers.is_empty()).then(|| carriers.join("+"));
            // #46 : le badge affirmait « CDJ compatible » sur la seule PRÉSENCE d'Artiste+Titre,
            // quel que soit le porteur — un WAV taggé en RIFF INFO passait vrai alors que la
            // platine affiche le nom de fichier. Le porteur est maintenant jugé contre la matrice
            // de docs/cdj-metadata-formats.md.
            //
            // `REPORT_CACHE_VERSION` passe à 9 dans le même geste (2026-09-01) : le bump verdict v2
            // du 2026-08-29 ne couvre PAS la bibliothèque rangée, que le pool ne reprenait alors
            // jamais (borne `status='pending'`, levée le 2026-09-09 par l'issue #59) — voir la doc
            // de la constante.
            let tags_cdj_ok = tagged.tags().iter().any(|t| {
                tag_type_readable_on_cdj(file_type, t.tag_type())
                    && t.get_string(ItemKey::TrackArtist).is_some()
                    && t.get_string(ItemKey::TrackTitle).is_some()
            });
            TagInfo {
                declared_rail: rail,
                content_rail,
                declared_bitrate: props.audio_bitrate(),
                duration_sec: props.duration().as_secs_f32(),
                channels: props.channels().unwrap_or(0) as u16,
                id3_version,
                tags_cdj_ok,
                has_cover,
            }
        }
        Err(_) => TagInfo {
            declared_rail: rail,
            content_rail: Rail::Unknown,
            declared_bitrate: None,
            duration_sec: 0.0,
            channels: 0,
            id3_version: None,
            tags_cdj_ok: false,
            has_cover: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rail_from_ext_classifies_known_formats() {
        assert_eq!(rail_from_ext("flac"), Rail::Lossless);
        assert_eq!(rail_from_ext("FLAC"), Rail::Lossless);
        assert_eq!(rail_from_ext("mp3"), Rail::Lossy);
        assert_eq!(rail_from_ext("xyz"), Rail::Unknown);
    }

    #[test]
    fn read_missing_file_is_conservative() {
        let info = read("does-not-exist.flac");
        assert_eq!(info.declared_rail, Rail::Lossless);
        assert_eq!(info.channels, 0);
        assert!(!info.has_cover);
    }

    fn fixture(name: &str) -> Option<String> {
        let p = format!("fixtures/{name}");
        std::path::Path::new(&p).exists().then_some(p)
    }

    /// The exact BUG-1 scenario: an MP3 renamed with a `.flac` extension. `rail_from_ext`
    /// (extension only) is fooled and says Lossless; `rail_from_content` (magic bytes) must see
    /// through the renamed extension and correctly report Lossy.
    #[test]
    fn rail_from_content_sees_through_a_renamed_mp3() {
        let Some(mp3) = fixture("real_320.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let disguised = dir.path().join("disguised.flac");
        std::fs::copy(&mp3, &disguised).unwrap();
        let path = disguised.to_str().unwrap();

        assert_eq!(
            rail_from_ext("flac"),
            Rail::Lossless,
            "extension alone is fooled"
        );
        assert_eq!(
            rail_from_content(path),
            Rail::Lossy,
            "content sniffing is not fooled"
        );
    }

    /// A genuine FLAC must not be misclassified by content sniffing (no false positive).
    #[test]
    fn rail_from_content_confirms_a_real_flac() {
        let Some(flac) = fixture("real_lossless.flac") else {
            eprintln!("skip: no fixture");
            return;
        };
        assert_eq!(rail_from_content(&flac), Rail::Lossless);
    }

    // ---- issue #46 : `tags_cdj_ok` juge le PORTEUR, pas la seule présence ----

    /// Copie un fixture dans un dossier temporaire et y grave Artiste+Titre dans un tag du TYPE
    /// demandé. Fabrique la fixture taggée à la volée plutôt que de committer un binaire.
    /// Rend `None` (test sauté) quand le fixture gitignoré est absent du checkout.
    fn tagged_copy(
        dir: &tempfile::TempDir,
        fixture_name: &str,
        out_name: &str,
        tag_type: TagType,
    ) -> Option<String> {
        use lofty::config::WriteOptions;
        use lofty::prelude::{Accessor, TagExt};
        use lofty::tag::Tag;

        let src = fixture(fixture_name)?;
        let dest = dir.path().join(out_name);
        std::fs::copy(&src, &dest).unwrap();

        let mut tag = Tag::new(tag_type);
        tag.set_artist("Fixture Artist".to_string());
        tag.set_title("Fixture Title".to_string());
        tag.save_to_path(&dest, WriteOptions::default()).unwrap();

        Some(dest.to_str().unwrap().to_string())
    }

    /// Le cas menteur central du ticket : un WAV dont Artiste+Titre vivent dans un RIFF INFO.
    /// L'ancien critère (présence seule) rendait `true` ; la platine, elle, affiche le nom de
    /// fichier (`docs/cdj-metadata-formats.md`, § Le cas WAV).
    #[test]
    fn wav_tagged_riff_info_is_not_cdj_ok() {
        let dir = tempfile::tempdir().unwrap();
        let Some(path) = tagged_copy(&dir, "silence_pad.wav", "tagged.wav", TagType::RiffInfo)
        else {
            eprintln!("skip: no fixture");
            return;
        };

        let info = read(&path);
        assert_eq!(
            info.id3_version.as_deref(),
            Some("RiffInfo"),
            "le porteur doit être capturé tel quel"
        );
        assert!(
            !info.tags_cdj_ok,
            "un WAV taggé en RIFF INFO n'est PAS lisible en navigation directe"
        );
    }

    /// Le même WAV taggé RIFF INFO, mais RENOMMÉ `.mp3` : le prédicat juge par
    /// `tagged.file_type()` (ce que `lofty` a reconnu dans les octets), jamais par l'extension.
    /// Sans ça, renommer un fichier suffirait à lui faire gagner le critère — exactement la classe
    /// de mensonge que l'issue #46 corrige, et le pendant tags du BUG-1 de `rail_from_content`.
    #[test]
    fn wav_renomme_mp3_nest_pas_cdj_ok() {
        let dir = tempfile::tempdir().unwrap();
        // Le porteur est un **ID3v2**, pas un RIFF INFO, et ce choix porte tout le pouvoir
        // discriminant du test : mesuré en mutation, re-dériver le conteneur depuis l'extension ne
        // fait RIEN sur un WAV taggé RIFF INFO — le couple deviendrait (Mpeg, RiffInfo), absent de
        // la table, donc `false` par le mauvais chemin et le test resterait vert. Avec un ID3v2 les
        // deux lectures divergent vraiment : (Wav, Id3v2) = false, (Mpeg, Id3v2) = true. Le cas
        // RIFF INFO non déguisé reste couvert par `wav_tagged_riff_info_is_not_cdj_ok`.
        let Some(wav) = tagged_copy(&dir, "silence_pad.wav", "source.wav", TagType::Id3v2) else {
            eprintln!("skip: no fixture");
            return;
        };
        // Renommage APRÈS écriture du tag : c'est bien un WAV taggé, déguisé, pas un fichier
        // fabriqué autrement.
        let disguised = dir.path().join("disguised.mp3");
        std::fs::rename(&wav, &disguised).unwrap();
        let path = disguised.to_str().unwrap();

        let info = read(path);
        assert_eq!(
            info.declared_rail,
            Rail::Lossy,
            "l'extension .mp3 trompe bien le rail déclaré — c'est le point du test"
        );
        assert_eq!(info.content_rail, Rail::Lossless, "le contenu reste un WAV");
        // DEUX porteurs : l'ID3v2 qu'on vient d'écrire (chunk `id3 `) et le RIFF INFO que ffmpeg
        // avait laissé dans le fixture. La chaîne est donc aussi le témoin du tri de `read` —
        // alphabétique, pas l'ordre de parcours de `lofty`.
        assert_eq!(
            info.id3_version.as_deref(),
            Some("Id3v2+RiffInfo"),
            "les deux porteurs, triés"
        );
        assert!(
            !info.tags_cdj_ok,
            "un WAV déguisé en .mp3 ne gagne pas le critère : le conteneur vient des octets"
        );
    }

    /// Un fichier parfaitement lisible mais SANS aucun tag : pas de porteur à nommer, et rien à
    /// juger contre la matrice. Les deux champs doivent le dire — `None` n'est pas `Some("")`, et
    /// `tags_cdj_ok` faux ici vient de l'absence de tag, pas d'un porteur refusé.
    #[test]
    fn fichier_sans_aucun_tag_na_ni_porteur_ni_critere() {
        let dir = tempfile::tempdir().unwrap();
        let Some(src) = fixture("real_320.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        let dest = dir.path().join("nu.mp3");
        std::fs::copy(&src, &dest).unwrap();
        // Le fixture est généré par ffmpeg et porte des tags d'encodeur : les retirer plutôt que de
        // SUPPOSER qu'il n'y en a pas — sinon le test passerait pour la mauvaise raison. Boucle
        // bornée et RELUE à chaque passe : retirer un porteur réécrit le fichier, donc la liste des
        // porteurs restants se re-mesure, elle ne se devine pas (première version : une seule passe
        // sur une liste fixe, elle laissait un `Id3v1` derrière).
        let path = dest.to_str().unwrap().to_string();
        for _ in 0..4 {
            let restants = read(&path).id3_version;
            let Some(restants) = restants else { break };
            for nom in restants.split('+') {
                for t in [
                    TagType::Id3v2,
                    TagType::Id3v1,
                    TagType::Ape,
                    TagType::Mp4Ilst,
                    TagType::RiffInfo,
                    TagType::AiffText,
                    TagType::VorbisComments,
                ] {
                    if tag_type_name(t) == nom {
                        let _ = t.remove_from_path(&path);
                    }
                }
            }
        }
        let path = path.as_str();

        let info = read(path);
        assert!(
            info.duration_sec > 0.0,
            "le fichier doit rester lisible — sinon le test mesure l'échec de lecture"
        );
        assert_eq!(
            info.id3_version, None,
            "aucun tag : pas de porteur à nommer, obtenu {:?}",
            info.id3_version
        );
        assert!(
            !info.tags_cdj_ok,
            "aucun tag : aucun porteur ne peut satisfaire la matrice"
        );
    }

    /// Le vrai positif : un MP3 dont Artiste+Titre vivent dans un ID3v2.
    #[test]
    fn mp3_tagged_id3v2_is_cdj_ok() {
        let dir = tempfile::tempdir().unwrap();
        let Some(path) = tagged_copy(&dir, "real_320.mp3", "tagged.mp3", TagType::Id3v2) else {
            eprintln!("skip: no fixture");
            return;
        };

        let info = read(&path);
        assert!(
            info.id3_version
                .as_deref()
                .is_some_and(|c| c.contains("Id3v2")),
            "le porteur ID3v2 doit apparaître, obtenu {:?}",
            info.id3_version
        );
        assert!(info.tags_cdj_ok);
    }

    /// Second vrai positif, sur un autre couple de la matrice : FLAC + Vorbis comments.
    #[test]
    fn flac_tagged_vorbis_comments_is_cdj_ok() {
        let dir = tempfile::tempdir().unwrap();
        let Some(path) = tagged_copy(
            &dir,
            "real_lossless.flac",
            "tagged.flac",
            TagType::VorbisComments,
        ) else {
            eprintln!("skip: no fixture");
            return;
        };

        let info = read(&path);
        assert_eq!(info.id3_version.as_deref(), Some("VorbisComments"));
        assert!(info.tags_cdj_ok);
    }

    /// Les lignes de la matrice qu'aucun fixture ne couvre (AIFF, MP4, APE), figées sans I/O.
    /// Cible directe : la table de vérité, pas le chemin de lecture.
    #[test]
    fn cdj_readable_matrix_matches_the_doc() {
        // Lisibles (docs/cdj-metadata-formats.md § Matrice de synthèse)
        assert!(tag_type_readable_on_cdj(FileType::Mpeg, TagType::Id3v2));
        assert!(tag_type_readable_on_cdj(FileType::Mpeg, TagType::Id3v1));
        assert!(tag_type_readable_on_cdj(FileType::Aiff, TagType::Id3v2));
        assert!(tag_type_readable_on_cdj(
            FileType::Flac,
            TagType::VorbisComments
        ));

        // AIFF natif : « Non établi » → ne compte pas.
        assert!(!tag_type_readable_on_cdj(FileType::Aiff, TagType::AiffText));
        // WAV : aucun porteur ne compte, ID3-en-chunk compris.
        assert!(!tag_type_readable_on_cdj(FileType::Wav, TagType::RiffInfo));
        assert!(!tag_type_readable_on_cdj(FileType::Wav, TagType::Id3v2));
        // MP4 (.m4a) : atomes iTunes lisibles — « OUI » au doc.
        assert!(tag_type_readable_on_cdj(FileType::Mp4, TagType::Mp4Ilst));
        // AAC brut (.aac, ADTS) : même ligne du doc, porteur ID3v2 puisqu'il n'a pas d'atome.
        assert!(tag_type_readable_on_cdj(FileType::Aac, TagType::Id3v2));
        // Couples hors matrice retenue : prudence.
        assert!(!tag_type_readable_on_cdj(FileType::Ape, TagType::Ape));
        assert!(!tag_type_readable_on_cdj(FileType::Flac, TagType::Id3v2));
    }

    /// Le porteur se nomme, mais jamais avec une sous-version ID3 : `lofty` ne l'expose pas sur un
    /// `Tag` générique (voir `tag_type_name`). Fige les deux moitiés du nom du test — les noms
    /// eux-mêmes, ET l'absence de sous-version, que la seule table d'égalités ci-dessus ne portait
    /// pas. La seconde moitié est le garde du jour où `original_version()` arriverait : un nom
    /// rendu « Id3v2.4 » ou « Id3v2 (v2.3) » ferait tomber ce test au lieu d'entrer en base.
    #[test]
    fn tag_type_names_are_stable_and_versionless() {
        let noms = [
            (TagType::Id3v2, "Id3v2"),
            (TagType::Id3v1, "Id3v1"),
            (TagType::RiffInfo, "RiffInfo"),
            (TagType::VorbisComments, "VorbisComments"),
            (TagType::AiffText, "AiffText"),
            (TagType::Mp4Ilst, "Mp4Ilst"),
            (TagType::Ape, "Ape"),
        ];
        for (tag_type, attendu) in noms {
            let rendu = tag_type_name(tag_type);
            assert_eq!(rendu, attendu);
            // « Versionless » : la seule marque de version tolérée est la MAJEURE collée au nom de
            // famille (le `v1` / `v2` d'`Id3v1` / `Id3v2`). Trois interdits qui couvrent les formes
            // que produirait un jour `original_version()` — « Id3v2.4 », « Id3v2 (v2.3) », « v2. » :
            assert!(
                !rendu.contains('.'),
                "{rendu} porte un point : sous-version interdite dans un nom persisté"
            );
            assert!(
                rendu.chars().all(|c| c.is_ascii_alphanumeric()),
                "{rendu} sort de l'alphanumérique ASCII : un nom persisté n'a ni espace ni ponctuation"
            );
            let marques_de_version = rendu
                .as_bytes()
                .windows(2)
                .filter(|w| w[0] == b'v' && w[1].is_ascii_digit())
                .count();
            assert!(
                marques_de_version <= 1,
                "{rendu} porte plus d'une marque « v<chiffre> » : sous-version interdite"
            );
        }
    }
}
