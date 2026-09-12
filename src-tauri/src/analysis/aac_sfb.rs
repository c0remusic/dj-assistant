//! Bandes de facteur d'échelle AAC LC — la découpe fréquentielle dans laquelle un encodeur AAC
//! choisit UN pas de quantification.
//!
//! POURQUOI ce module (2026-09-01, mémo de recherche de l'issue #52). La vraisemblance de
//! transcodage de Derrien (JAES 67(3), 2019) ne teste pas des coefficients MDCT isolés : elle teste,
//! bande par bande, si les coefficients d'une bande tombent tous sur UNE grille commune. La bande
//! est l'unité parce que c'est l'unité de l'encodeur — un scale factor par bande, donc un pas par
//! bande. Se tromper de découpe, c'est mélanger deux grilles et effacer la statistique.
//!
//! **PROVENANCE DES VALEURS.** Recopiées, jamais écrites de mémoire, et vérifiées identiques sur
//! DEUX implémentations libres indépendantes du décodeur AAC (lecture du 2026-09-01) :
//!
//! - FFmpeg, `libavcodec/aactab.c` — `swb_offset_1024_48`, `swb_offset_128_48`,
//!   `ff_swb_offset_1024`, `ff_swb_offset_128`, `ff_aac_num_swb_1024`, `ff_aac_num_swb_128`.
//! - FAAD2, `libfaad/specrec.c` — `swb_offset_1024_48`, `swb_offset_128_48`,
//!   `swb_offset_1024_window`, `swb_offset_128_window`, `num_swb_1024_window`,
//!   `num_swb_128_window`.
//!
//! Les deux normalisent ISO/IEC 13818-7 (MPEG-2 AAC) et ISO/IEC 14496-3 (MPEG-4 AAC), tables
//! « scalefactor band offsets ». Les deux sources donnent exactement les mêmes entiers, y compris
//! l'appariement 44 100 ↔ 48 000 : les index de taux d'échantillonnage 3 (48 kHz) et 4 (44,1 kHz)
//! pointent la MÊME table, en blocs longs comme en blocs courts. Ce n'est pas une approximation de
//! notre part, c'est la norme.
//!
//! ⚠️ **Un taux non tabulé rend `None`, jamais une table par défaut.** C'est la règle du mémo, et
//! elle est la même que celle de `verdict()` sur l'absence de mesure : hors domaine ne se répare pas
//! par un repli plausible. Un fichier 88,2 kHz analysé avec la découpe de 44,1 produirait un chiffre
//! crédible et faux.

/// Frontières des bandes en blocs LONGS (1024 coefficients) pour 48 000 et 44 100 Hz.
///
/// 50 frontières → **49 bandes**, ce qui recoupe `ff_aac_num_swb_1024[3] = 49` et `[4] = 49`. La
/// dernière valeur est le terminateur, pas une bande.
const SWB_OFFSET_1024_48: &[u16] = &[
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 48, 56, 64, 72, 80, 88, 96, 108, 120, 132, 144, 160,
    176, 196, 216, 240, 264, 292, 320, 352, 384, 416, 448, 480, 512, 544, 576, 608, 640, 672, 704,
    736, 768, 800, 832, 864, 896, 928, 1024,
];

/// Frontières des bandes en blocs COURTS (128 coefficients) pour 48 000 et 44 100 Hz.
///
/// 15 frontières → **14 bandes**, ce qui recoupe `ff_aac_num_swb_128[3] = 14` et `[4] = 14`.
const SWB_OFFSET_128_48: &[u16] = &[0, 4, 8, 12, 16, 20, 28, 36, 44, 56, 68, 80, 96, 112, 128];

/// Résolution de la MDCT du codec — un bloc long, ou un des huit blocs courts d'une commutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// 2048 échantillons → 1024 coefficients.
    Long,
    /// 256 échantillons → 128 coefficients.
    Short,
}

impl BlockKind {
    /// Nombre de coefficients MDCT produits par un bloc de cette résolution.
    pub fn coeffs(self) -> usize {
        match self {
            BlockKind::Long => 1024,
            BlockKind::Short => 128,
        }
    }

    /// Étiquette pour les CSV du harnais — courte, stable, jamais traduite dans la sortie.
    pub fn label(self) -> &'static str {
        match self {
            BlockKind::Long => "long",
            BlockKind::Short => "court",
        }
    }
}

/// Frontières de bandes pour un taux d'échantillonnage et une résolution, ou `None` hors domaine.
///
/// La tranche rendue contient `nb_bandes + 1` frontières : la bande `s` couvre les coefficients
/// `[t[s], t[s+1])`.
///
/// Seuls 44 100 et 48 000 Hz sont tabulés ici — ce sont les deux taux du corpus de #51/#52, et
/// ajouter 32 kHz ou 88,2 kHz veut dire recopier LEUR table depuis les mêmes sources, pas
/// extrapoler celle-ci.
pub fn swb_offsets(sample_rate: u32, kind: BlockKind) -> Option<&'static [u16]> {
    match (sample_rate, kind) {
        (44100 | 48000, BlockKind::Long) => Some(SWB_OFFSET_1024_48),
        (44100 | 48000, BlockKind::Short) => Some(SWB_OFFSET_128_48),
        _ => None,
    }
}

/// Largeurs de bandes (nombre de coefficients par bande), dérivées des frontières.
///
/// Forme pratique pour [`super::quant_trace::thresholds`], qui n'a besoin que des `K`.
pub fn swb_widths(sample_rate: u32, kind: BlockKind) -> Option<Vec<usize>> {
    let t = swb_offsets(sample_rate, kind)?;
    Some(t.windows(2).map(|w| (w[1] - w[0]) as usize).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les invariants qu'une table de frontières ne peut pas violer sans être fausse — et
    /// qu'aucune relecture à l'œil n'attrape de façon fiable sur 50 entiers.
    ///
    /// « Somme = 1024 / 128 » attrape un chiffre recopié de travers, une frontière oubliée et une
    /// frontière en double, parce qu'il lie la table à la taille de la transformée qui la consomme.
    /// Il ne suffit PAS : voir le commentaire au milieu du corps, et les deux invariants de norme
    /// ajoutés le 2026-09-02 après l'avoir mesuré.
    #[test]
    fn les_tables_sont_contigues_croissantes_et_completes() {
        for (sr, kind, total, bandes) in [
            (44100u32, BlockKind::Long, 1024usize, 49usize),
            (48000, BlockKind::Long, 1024, 49),
            (44100, BlockKind::Short, 128, 14),
            (48000, BlockKind::Short, 128, 14),
        ] {
            let Some(t) = swb_offsets(sr, kind) else {
                panic!("{sr} Hz / {:?} devrait être tabulé", kind);
            };
            assert_eq!(
                t[0], 0,
                "{sr} Hz / {:?} : la première frontière n'est pas 0",
                kind
            );
            assert_eq!(
                *t.last().unwrap_or(&0),
                total as u16,
                "{sr} Hz / {:?} : la dernière frontière n'est pas {total}",
                kind
            );
            assert_eq!(
                t.len() - 1,
                bandes,
                "{sr} Hz / {:?} : {} bandes au lieu de {bandes}",
                kind,
                t.len() - 1
            );
            for w in t.windows(2) {
                assert!(
                    w[1] > w[0],
                    "{sr} Hz / {:?} : frontières non strictement croissantes ({} → {})",
                    kind,
                    w[0],
                    w[1]
                );
            }
            // Contiguïté ET complétude : les largeurs dérivées doivent recouvrir la transformée
            // entière, sans trou ni recouvrement — ce que la somme exprime exactement, puisque les
            // frontières sont déjà croissantes.
            let Some(widths) = swb_widths(sr, kind) else {
                panic!("{sr} Hz / {:?} : largeurs indisponibles", kind);
            };
            assert_eq!(widths.len(), bandes);
            assert_eq!(
                widths.iter().sum::<usize>(),
                total,
                "{sr} Hz / {:?} : les largeurs ne somment pas à {total}",
                kind
            );
            // ⚠️ Les trois invariants ci-dessus NE SUFFISENT PAS, et c'est mesuré (2026-09-02) :
            // déplacer une frontière INTERNE de 108 à 104 les laisse tous les trois verts — la
            // table reste croissante, contiguë, et somme toujours à 1024, seules deux largeurs
            // voisines s'échangent. Les deux invariants qui suivent l'attrapent, et ils ne sont pas
            // inventés pour l'occasion : ce sont deux propriétés de la NORME.
            //
            // 1. Les largeurs sont non décroissantes avec la fréquence — une bande de facteur
            //    d'échelle suit les bandes critiques de l'oreille, qui s'élargissent vers l'aigu.
            //    ATTRIBUTION EXACTE de la mutation 108 → 104 (recomptée le 2026-09-02, la version
            //    précédente de ce commentaire désignait la mauvaise paire) : les largeurs 12 et 12
            //    des bandes 18 et 19 deviennent 8 et 16, ce qui MONTE et ne déclenche donc rien.
            //    C'est la paire SUIVANTE qui tombe — 16 (bande 19) puis 12 (bande 20, inchangée) —
            //    parce que le 16 volé au voisin dépasse la largeur d'après.
            for w in widths.windows(2) {
                assert!(
                    w[1] >= w[0],
                    "{sr} Hz / {:?} : largeur qui RÉTRÉCIT vers l'aigu ({} → {})",
                    kind,
                    w[0],
                    w[1]
                );
            }
            // 2. Toute largeur est un multiple de 4 — l'AAC code les coefficients par quadruplets,
            //    et aucune bande de facteur d'échelle n'en coupe un.
            for (s, &k) in widths.iter().enumerate() {
                assert_eq!(
                    k % 4,
                    0,
                    "{sr} Hz / {:?} : bande {s} large de {k}, qui n'est pas un multiple de 4",
                    kind
                );
            }
        }
    }

    /// 44,1 et 48 kHz partagent la table — c'est la norme, pas une commodité, et un futur
    /// « nettoyage » qui donnerait sa propre table à 44,1 doit tomber ici.
    #[test]
    fn quarante_quatre_un_et_quarante_huit_partagent_la_meme_table() {
        for kind in [BlockKind::Long, BlockKind::Short] {
            assert_eq!(
                swb_offsets(44100, kind),
                swb_offsets(48000, kind),
                "{:?} : 44,1 et 48 kHz doivent pointer la même table",
                kind
            );
        }
    }

    /// Hors domaine = absence de mesure, jamais un défaut.
    ///
    /// Les taux listés ici sont des taux RÉELS d'AAC (index 0, 1, 2, 5, 6 de la norme) : ce ne sont
    /// pas des entrées absurdes, ce sont exactement les cas où un repli silencieux serait crédible.
    #[test]
    fn un_taux_non_tabule_ne_rend_aucune_table() {
        for sr in [96000u32, 88200, 64000, 32000, 24000, 22050, 16000, 8000, 0] {
            for kind in [BlockKind::Long, BlockKind::Short] {
                assert!(
                    swb_offsets(sr, kind).is_none(),
                    "{sr} Hz / {:?} rend une table alors qu'il n'est pas tabulé",
                    kind
                );
                assert!(swb_widths(sr, kind).is_none());
            }
        }
    }
}
