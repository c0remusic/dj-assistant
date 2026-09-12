//! Banc MP3 (couche III) de la sonde de quantification — issue #63.
//!
//! POURQUOI. Le banc AAC (`quant_trace`, #52) rejoue la MDCT 2048/256 d'un encodeur AAC et teste
//! si les coefficients tombent sur une grille `v = q·Δ` (`v = |X|^{3/4}`). Un MP3 quantifie avec la
//! MÊME loi de puissance 3/4 — `|xr| = ix^{4/3} · 2^{(gg−210)/4} · 2^{−(1+sfs)·sf}`, donc
//! `|xr|^{3/4} = ix · Δ_bande` — mais dans une AUTRE transformée : le banc hybride de la couche III,
//! 32 sous-bandes polyphase (fenêtre de 512) puis une MDCT de 36 échantillons par sous-bande, avec
//! des papillons de réduction d'aliasing entre sous-bandes voisines. Une MDCT AAC ne voit rien de
//! cette grille : mesuré sur le corpus le 2026-09-10, LAME 320 et V0 sortent `Ok` à 20/20 avec le
//! banc AAC seul, alors qu'ils sont les faux les plus courants pour un DJ.
//!
//! CE QUE FAIT CE MODULE. Il rejoue l'étage d'analyse d'un encodeur couche III sur le PCM décodé —
//! le dual EXACT de la chaîne de synthèse du décodeur (`symphonia-bundle-mp3`, `synthesis.rs` et
//! `layer3/hybrid_synthesis.rs`, relus le 2026-09-11) :
//!
//! | décodeur (synthèse) | ici (analyse) |
//! |---|---|
//! | papillons `l1 = l0·cs − u0·ca ; u1 = u0·cs + l0·ca` | rotation inverse `l = l1·cs + u1·ca ; u = u1·cs − l1·ca` |
//! | IMDCT 36 fenêtrée, recouvrement 18 | MDCT 36 fenêtrée sur 18 anciens + 18 nouveaux |
//! | inversion de fréquence (sous-bandes impaires, échantillons impairs négés) | la même (elle est son propre inverse) |
//! | synthèse polyphase, fenêtre `D` (table B.3) | analyse polyphase, fenêtre `C = D/32` (table C.1), matrice `cos((2k+1)(i−16)π/64)` |
//!
//! puis `quant_trace::frame_likelihood` sur les 576 coefficients d'une granule, avec les bandes de
//! facteur d'échelle de la couche III (`SFB_LONG_44100`, table B.8) au lieu des bandes AAC. La loi
//! nulle, la porte de bruit, `MIN_ACTIFS`, `MIN_NIVEAUX_DISTINCTS`, `τ(K)` : rien n'est refait,
//! c'est le même test d'idempotence de l'arrondi (Derrien, JAES 67(3), 2019).
//!
//! L'ALIGNEMENT. Une granule fait 576 échantillons PCM ; le décalage entre la grille de l'encodeur
//! et le PCM décodé est inconnu (retard d'encodeur 576, de décodeur 529, découpe éventuelle). Le
//! balayage couvre les 576 phases, décomposées en `32 phases de sous-bande × 18 phases de granule` :
//! l'analyse polyphase ne se recalcule que 32 fois par groupe, les 18 phases de granule ne
//! coûtent que des MDCT.
//!
//! BLOCS COURTS. Non traités en phase 1 : LAME ne commute en blocs courts que sur les transitoires,
//! et une granule courte ne fait que baisser `L` pour ce groupe. C'est le même arbitrage que le banc
//! AAC en phase 1 (résolution longue d'abord), à mesurer avant d'aller plus loin.
//!
//! NON CALIBRÉ (marqué un par un) : la fenêtre de bandes [`BANDE_DEBUT`] ; le seuil de décision
//! reste `verdict::QUANT_LAMBDA_MP3` (0,18, la valeur historique du banc AAC en blocs courts — depuis le 2026-09-11 chaque banc a le sien, et `quant_likelihood` voyage en rapport au seuil), tant qu'une mesure ne dit pas qu'il en
//! faut deux.
//!
//! SOURCE des tables : ISO/IEC 11172-3, tables B.3 (fenêtre de synthèse `D`), B.8 (bandes de
//! facteurs d'échelle). Les nombres sont ceux de la norme, un fait normatif : la table B.3 est
//! écrite en numérateurs sur 2⁻¹⁶ (voir [`SYNTHESIS_D_NUM`]) et vérifiée contre PDMP3, du domaine
//! public. Aucune table n'est reprise d'une implémentation sous copyleft.

use crate::analysis::quant_trace::{
    frame_likelihood, thresholds, Canal, GROUPES, N_F, N_SF, P_CENTILE,
};

/// Bandes de facteur d'échelle, blocs longs, 44,1 kHz — ISO/IEC 11172-3 table B.8. Vingt-deux
/// bandes sur 576 coefficients ; la dernière (418 → 576, « sfb21 ») n'a pas de facteur d'échelle
/// propre dans un encodeur et fait 158 coefficients, au-delà du tampon de 128 de
/// `frame_likelihood`, qui la saute d'elle-même.
/// **Provenance des trois tables ci-dessous** : ISO/IEC 11172-3, table B.8. Les valeurs sont
/// celles de la norme, et elles ont été vérifiées entier par entier contre **PDMP3**
/// (`technosaurus/PDMP3`, `g_sf_band_indices`), publié dans le **domaine public** : les trois
/// tables coïncident exactement. Aucune n'est reprise d'une implémentation sous copyleft
/// (contrôle du 2026-09-12, en même temps que celui de [`SYNTHESIS_D_NUM`]).
pub const SFB_LONG_44100: [u16; 23] = [
    0, 4, 8, 12, 16, 20, 24, 30, 36, 44, 52, 62, 74, 90, 110, 134, 162, 196, 238, 288, 342, 418,
    576,
];
/// Idem, 48 kHz.
pub const SFB_LONG_48000: [u16; 23] = [
    0, 4, 8, 12, 16, 20, 24, 30, 36, 42, 50, 60, 72, 88, 106, 128, 156, 190, 230, 276, 330, 384,
    576,
];
/// Idem, 32 kHz.
pub const SFB_LONG_32000: [u16; 23] = [
    0, 4, 8, 12, 16, 20, 24, 30, 36, 44, 54, 66, 82, 102, 126, 156, 194, 240, 296, 364, 448, 550,
    576,
];

/// Table de bandes pour un taux MPEG-1 ; `None` pour tout autre taux (MPEG-2 à 22,05/24/16 kHz a
/// d'autres bandes, non tabulées ici : le banc saute, il n'invente pas).
pub fn sfb_long(sample_rate: u32) -> Option<&'static [u16]> {
    match sample_rate {
        44100 => Some(&SFB_LONG_44100),
        48000 => Some(&SFB_LONG_48000),
        32000 => Some(&SFB_LONG_32000),
        _ => None,
    }
}

/// Première bande de la fenêtre d'analyse.
///
/// NON CALIBRÉ — choix d'ici, argumenté a priori. Les bandes 13 à 20 couvrent les coefficients 90 à
/// 418, soit **3,4 → 16,0 kHz à 44,1 kHz** (38,3 Hz par coefficient). C'est la zone où un LAME 320
/// travaille avec de petits `q` (donc où `Δ̂ = min v` a une vraie chance de tomber sur `q = 1`),
/// tout en restant sous son passe-bas à 20,5 kHz et sous « sfb21 » (16 → 22 kHz), qui n'a pas de
/// facteur d'échelle et dépasse le tampon de bandes.
pub const BANDE_DEBUT: usize = 13;

/// Coefficients par granule (32 sous-bandes × 18).
pub const COEFFS: usize = 576;
const SB: usize = 32;
const GR: usize = 18;

/// Fenêtre de synthèse `D[0..512]` d'ISO/IEC 11172-3 (table B.3), en NUMÉRATEURS sur 2⁻¹⁶.
///
/// La norme publie la table en décimal à neuf chiffres. Ces 512 valeurs sont toutes des multiples
/// exacts de 1/65536 — mesuré : l'écart maximal entre la valeur publiée et `n/65536` vaut
/// 5,0·10⁻¹⁰, c'est-à-dire exactement l'arrondi de la neuvième décimale, sur les 512 entrées et
/// sans exception. La table normative est donc une table d'ENTIERS, et c'est elle qui est écrite
/// ici : [`synthesis_d`] rend `n/65536`, ce qui est plus exact que les décimales tronquées qu'on
/// trouve dans les implémentations (deux d'entre elles, aux indices 196 et 316, valent
/// `3776/65536 = 0,0576171875` et se tronquent souvent en `0,057617187`).
///
/// **Provenance, et pourquoi elle est écrite ici.** Les nombres sont ceux de la norme, un fait
/// normatif qu'aucune implémentation ne possède. Ils ont été vérifiés entrée par entrée contre
/// **PDMP3** (`technosaurus/PDMP3`, `g_synth_dtbl`), publié dans le **domaine public**
/// (Unlicense) : 510 valeurs identiques au chiffre près, et les 2 restantes sont l'arrondi
/// ci-dessus. Cette table ne descend donc d'aucun code sous copyleft. Le portage de 2026-09-11
/// citait `symphonia-bundle-mp3` (MPL-2.0) comme source ; la citation est remplacée par la norme
/// et le contrôle croisé, et les valeurs par les entiers exacts (2026-09-12).
#[rustfmt::skip]
static SYNTHESIS_D_NUM: [i32; 512] = [
    0, -1, -1, -1, -1, -1, -1, -2,
    -2, -2, -2, -3, -3, -4, -4, -5,
    -5, -6, -7, -7, -8, -9, -10, -11,
    -13, -14, -16, -17, -19, -21, -24, -26,
    -29, -31, -35, -38, -41, -45, -49, -53,
    -58, -63, -68, -73, -79, -85, -91, -97,
    -104, -111, -117, -125, -132, -139, -147, -154,
    -161, -169, -176, -183, -190, -196, -202, -208,
    213, 218, 222, 225, 227, 228, 228, 227,
    224, 221, 215, 208, 200, 189, 177, 163,
    146, 127, 106, 83, 57, 29, -2, -36,
    -72, -111, -153, -197, -244, -294, -347, -401,
    -459, -519, -581, -645, -711, -779, -848, -919,
    -991, -1064, -1137, -1210, -1283, -1356, -1428, -1498,
    -1567, -1634, -1698, -1759, -1817, -1870, -1919, -1962,
    -2001, -2032, -2057, -2075, -2085, -2087, -2080, -2063,
    2037, 2000, 1952, 1893, 1822, 1739, 1644, 1535,
    1414, 1280, 1131, 970, 794, 605, 402, 185,
    -45, -288, -545, -814, -1095, -1388, -1692, -2006,
    -2330, -2663, -3004, -3351, -3705, -4063, -4425, -4788,
    -5153, -5517, -5879, -6237, -6589, -6935, -7271, -7597,
    -7910, -8209, -8491, -8755, -8998, -9219, -9416, -9585,
    -9727, -9838, -9916, -9959, -9966, -9935, -9863, -9750,
    -9592, -9389, -9139, -8840, -8492, -8092, -7640, -7134,
    6574, 5959, 5288, 4561, 3776, 2935, 2037, 1082,
    70, -998, -2122, -3300, -4533, -5818, -7154, -8540,
    -9975, -11455, -12980, -14548, -16155, -17799, -19478, -21189,
    -22929, -24694, -26482, -28289, -30112, -31947, -33791, -35640,
    -37489, -39336, -41176, -43006, -44821, -46617, -48390, -50137,
    -51853, -53534, -55178, -56778, -58333, -59838, -61289, -62684,
    -64019, -65290, -66494, -67629, -68692, -69679, -70590, -71420,
    -72169, -72835, -73415, -73908, -74313, -74630, -74856, -74992,
    75038, 74992, 74856, 74630, 74313, 73908, 73415, 72835,
    72169, 71420, 70590, 69679, 68692, 67629, 66494, 65290,
    64019, 62684, 61289, 59838, 58333, 56778, 55178, 53534,
    51853, 50137, 48390, 46617, 44821, 43006, 41176, 39336,
    37489, 35640, 33791, 31947, 30112, 28289, 26482, 24694,
    22929, 21189, 19478, 17799, 16155, 14548, 12980, 11455,
    9975, 8540, 7154, 5818, 4533, 3300, 2122, 998,
    -70, -1082, -2037, -2935, -3776, -4561, -5288, -5959,
    6574, 7134, 7640, 8092, 8492, 8840, 9139, 9389,
    9592, 9750, 9863, 9935, 9966, 9959, 9916, 9838,
    9727, 9585, 9416, 9219, 8998, 8755, 8491, 8209,
    7910, 7597, 7271, 6935, 6589, 6237, 5879, 5517,
    5153, 4788, 4425, 4063, 3705, 3351, 3004, 2663,
    2330, 2006, 1692, 1388, 1095, 814, 545, 288,
    45, -185, -402, -605, -794, -970, -1131, -1280,
    -1414, -1535, -1644, -1739, -1822, -1893, -1952, -2000,
    2037, 2063, 2080, 2087, 2085, 2075, 2057, 2032,
    2001, 1962, 1919, 1870, 1817, 1759, 1698, 1634,
    1567, 1498, 1428, 1356, 1283, 1210, 1137, 1064,
    991, 919, 848, 779, 711, 645, 581, 519,
    459, 401, 347, 294, 244, 197, 153, 111,
    72, 36, 2, -29, -57, -83, -106, -127,
    -146, -163, -177, -189, -200, -208, -215, -221,
    -224, -227, -228, -228, -227, -225, -222, -218,
    213, 208, 202, 196, 190, 183, 176, 169,
    161, 154, 147, 139, 132, 125, 117, 111,
    104, 97, 91, 85, 79, 73, 68, 63,
    58, 53, 49, 45, 41, 38, 35, 31,
    29, 26, 24, 21, 19, 17, 16, 14,
    13, 11, 10, 9, 8, 7, 7, 6,
    5, 5, 4, 4, 3, 3, 2, 2,
    2, 2, 1, 1, 1, 1, 1, 1,
];

/// La fenêtre de synthèse en `f64`, `D[i] = n[i]/65536`, construite une fois.
///
/// `OnceLock` et pas un `static [f64; 512]` littéral : la division flottante ne s'évalue pas en
/// contexte constant sous le canal épinglé, et la boucle de synthèse lit la table des millions de
/// fois — elle ne peut pas la reconstruire à chaque appel. (`LazyLock` conviendrait mieux mais
/// demande 1.80 ; `src-tauri/Cargo.toml` déclare `rust-version = "1.77.2"`, et clippy le vérifie
/// par `incompatible_msrv`.)
fn synthesis_d() -> &'static [f64; 512] {
    static D: std::sync::OnceLock<[f64; 512]> = std::sync::OnceLock::new();
    D.get_or_init(|| {
        let mut d = [0.0f64; 512];
        for (i, &n) in SYNTHESIS_D_NUM.iter().enumerate() {
            d[i] = f64::from(n) / 65536.0;
        }
        d
    })
}

/// Fenêtre d'ANALYSE `C[i] = D[i] / 32` (table C.1 de la norme : même forme, même signe, échelle
/// 1/32). L'échelle n'a aucune incidence sur le test — `Δ̂` est estimé par bande — mais la FORME et
/// les SIGNES sont ceux que l'encodeur a appliqués, et c'est ce qui aligne la grille.
fn analysis_window() -> Vec<f64> {
    synthesis_d().iter().map(|&d| d / 32.0).collect()
}

/// Matrice d'analyse `M[k][i] = cos((2k+1)(i−16)π/64)`, 32 × 64 (norme, annexe C).
fn analysis_matrix() -> Vec<f64> {
    let mut m = vec![0.0f64; SB * 64];
    for k in 0..SB {
        for i in 0..64 {
            m[k * 64 + i] =
                (((2 * k + 1) as f64) * ((i as f64) - 16.0) * std::f64::consts::PI / 64.0).cos();
        }
    }
    m
}

/// Analyse polyphase : `signal` → `steps` pas de 32 échantillons, chaque pas rend 32 sous-bandes.
/// Sortie `[t][sb]` aplatie. Le tampon d'entrée démarre à zéro (les 16 premiers pas sont du
/// transitoire : l'appelant les laisse en tête, avant la première granule utile).
fn polyphase_analysis(signal: &[f32], steps: usize, c: &[f64], m: &[f64]) -> Vec<f64> {
    let mut x = [0.0f64; 512];
    let mut out = vec![0.0f64; steps * SB];
    let mut y = [0.0f64; 64];
    for t in 0..steps {
        // Décalage du tampon : X[i] = X[i−32], puis les 32 nouveaux échantillons entrent en tête,
        // le plus récent en X[0] (annexe C).
        x.copy_within(0..480, 32);
        for i in 0..32 {
            let idx = t * 32 + i;
            x[31 - i] = if idx < signal.len() {
                signal[idx] as f64
            } else {
                0.0
            };
        }
        for i in 0..64 {
            let mut s = 0.0;
            for j in 0..8 {
                s += c[i + 64 * j] * x[i + 64 * j];
            }
            y[i] = s;
        }
        for k in 0..SB {
            let row = &m[k * 64..k * 64 + 64];
            let mut s = 0.0;
            for i in 0..64 {
                s += row[i] * y[i];
            }
            out[t * SB + k] = s;
        }
    }
    out
}

/// Cosinus de la MDCT 36 → 18 : `cos(π/72 · (2n+1+18) · (2k+1))`, avec la fenêtre sinus des blocs
/// longs `w[n] = sin(π/36 · (n+½))` déjà repliée dedans.
fn mdct36_table() -> Vec<f64> {
    let mut t = vec![0.0f64; GR * 36];
    for k in 0..GR {
        for n in 0..36 {
            let w = (std::f64::consts::PI / 36.0 * (n as f64 + 0.5)).sin();
            t[k * 36 + n] = w
                * (std::f64::consts::PI / 72.0
                    * (2.0 * n as f64 + 1.0 + 18.0)
                    * (2.0 * k as f64 + 1.0))
                    .cos();
        }
    }
    t
}

/// `cs[i]`, `ca[i]` des papillons d'aliasing — table B.9 : `c = [−0,6 −0,535 −0,33 −0,185 −0,095
/// −0,041 −0,0142 −0,0037]`, `cs = 1/√(1+c²)`, `ca = c/√(1+c²)`.
fn alias_coeffs() -> ([f64; 8], [f64; 8]) {
    // Les huit `c_i` d'ISO/IEC 11172-3 table B.9. `cs` et `ca` en sont CALCULÉS, jamais
    // recopiés : les tables à six décimales qu'on trouve dans les décodeurs sont une valeur
    // arrondie de ce calcul (contrôle contre PDMP3, domaine public : écart max 4,5·10⁻⁷,
    // soit leur précision d'affichage).
    const C: [f64; 8] = [
        -0.6, -0.535, -0.33, -0.185, -0.095, -0.041, -0.0142, -0.0037,
    ];
    let mut cs = [0.0; 8];
    let mut ca = [0.0; 8];
    for i in 0..8 {
        let sq = (1.0 + C[i] * C[i]).sqrt();
        cs[i] = 1.0 / sq;
        ca[i] = C[i] / sq;
    }
    (cs, ca)
}

/// Une granule : 36 pas de sous-bandes (`sub[(t0−18)..(t0+18)]`, aplati `[t][sb]`) → 576
/// coefficients `xr[sb*18 + k]`, tels que l'encodeur les quantifie. Trois étages, dans l'ordre de
/// l'encodeur : inversion de fréquence, MDCT 36 fenêtrée, papillons d'aliasing (rotation inverse
/// de celle du décodeur).
fn granule_coeffs(
    sub: &[f64],
    t0: usize,
    mdct: &[f64],
    cs: &[f64; 8],
    ca: &[f64; 8],
    out: &mut [f64; COEFFS],
) {
    let mut x = [0.0f64; 36];
    for sb in 0..SB {
        for (n, xn) in x.iter_mut().enumerate() {
            let t = t0 - GR + n;
            let v = sub[t * SB + sb];
            // Inversion de fréquence : sous-bande impaire, échantillon impair DANS la granule
            // (le décodeur nègue `s[18·sb + (1, 3, 5, …)]` de chaque granule ; ici `n` court sur deux
            // granules, la parité de `n` est celle de `n − 18`).
            *xn = if sb & 1 == 1 && n & 1 == 1 { -v } else { v };
        }
        for k in 0..GR {
            let row = &mdct[k * 36..k * 36 + 36];
            let mut s = 0.0;
            for n in 0..36 {
                s += row[n] * x[n];
            }
            out[sb * GR + k] = s;
        }
    }
    // Papillons : l'encodeur applique la rotation inverse de celle du décodeur
    // (`antialias` de symphonia : l1 = l0·cs − u0·ca, u1 = u0·cs + l0·ca).
    for sb in 1..SB {
        let b = sb * GR;
        for i in 0..8 {
            let li = b - 1 - i;
            let ui = b + i;
            let l1 = out[li];
            let u1 = out[ui];
            out[li] = l1 * cs[i] + u1 * ca[i];
            out[ui] = u1 * cs[i] - l1 * ca[i];
        }
    }
}

/// Ce que rend le banc : la vraisemblance maximale, et où elle a été trouvée.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TraceMp3 {
    pub l: f64,
    /// Décalage PCM gagnant, `0 ≤ d < 576` (= phase de sous-bande + 32 × phase de granule).
    pub decalage: usize,
    pub canal: Canal,
}

/// Vraisemblance qu'un signal soit déjà passé par la grille de quantification d'un encodeur
/// couche III. `pcm` entrelacé, `channels` canaux. `None` si le taux n'est pas tabulé ou si le
/// signal est trop court pour `N_F` granules par groupe.
///
/// Même contrat que `quant_trace::likelihood` : `L = max` sur canaux (G, D, M, S) et décalages, du
/// compte de bandes sous `τ` sur `N_F × N_SF` par groupe, maximum sur les groupes. `fils_max`
/// plafonne les fils du balayage (les 32 phases de sous-bande se répartissent sur les cœurs).
pub fn likelihood(
    pcm: &[f32],
    channels: u16,
    sample_rate: u32,
    fils_max: Option<usize>,
) -> Option<TraceMp3> {
    let offsets = sfb_long(sample_rate)?;
    let ch = channels.max(1) as usize;
    let n = pcm.len() / ch;
    if n == 0 {
        return None;
    }
    let canaux: Vec<(Canal, Vec<f32>)> = if ch >= 2 {
        let g: Vec<f32> = (0..n).map(|i| pcm[i * ch]).collect();
        let d: Vec<f32> = (0..n).map(|i| pcm[i * ch + 1]).collect();
        let m: Vec<f32> = g.iter().zip(&d).map(|(a, b)| 0.5 * (a + b)).collect();
        let s: Vec<f32> = g.iter().zip(&d).map(|(a, b)| 0.5 * (a - b)).collect();
        vec![
            (Canal::Gauche, g),
            (Canal::Droite, d),
            (Canal::Milieu, m),
            (Canal::Cote, s),
        ]
    } else {
        vec![(Canal::Gauche, pcm.to_vec())]
    };

    // Pas de sous-bande par groupe : 16 de transitoire du filtre, 18 de granule précédente, 18 de
    // phase de granule, puis N_F granules.
    let pas_par_groupe = 16 + GR + GR + N_F * GR;
    let dispo = n.saturating_sub(pas_par_groupe * 32 + 576) / 576;
    if dispo < N_F {
        return None;
    }
    let departs: Vec<usize> = (0..GROUPES)
        .map(|g| {
            if GROUPES <= 1 {
                0
            } else {
                g * (dispo - N_F) / (GROUPES - 1)
            }
        })
        .collect();

    let largeurs: Vec<usize> = (0..=COEFFS).collect();
    let taus = thresholds(P_CENTILE, &largeurs);
    let c = analysis_window();
    let m = analysis_matrix();
    let mdct = mdct36_table();
    let (cs, ca) = alias_coeffs();

    let dispo_fils = std::thread::available_parallelism()
        .map(|v| v.get())
        .unwrap_or(1);
    let fils = match fils_max {
        Some(x) => dispo_fils.min(x).max(1),
        None => dispo_fils.clamp(1, 16),
    };
    let par_fil = SB.div_ceil(fils);
    let denominateur = (N_F * N_SF) as f64;

    let mut best: Option<TraceMp3> = None;
    for (canal, signal) in &canaux {
        let resultats: Vec<(f64, usize)> = std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(fils);
            for f in 0..fils {
                let debut = f * par_fil;
                let fin = ((f + 1) * par_fil).min(SB);
                if debut >= fin {
                    continue;
                }
                let (c, m, mdct, taus, departs) = (&c, &m, &mdct, &taus, &departs);
                handles.push(scope.spawn(move || {
                    let mut coeffs = [0.0f64; COEFFS];
                    let mut meilleur = (0.0f64, debut);
                    for p in debut..fin {
                        for &depart in departs {
                            // PCM du groupe : à partir de `p + 576·depart`, `pas_par_groupe` pas.
                            let base = p + 576 * depart;
                            let fin_pcm = (base + pas_par_groupe * 32).min(signal.len());
                            let sub =
                                polyphase_analysis(&signal[base..fin_pcm], pas_par_groupe, c, m);
                            for gq in 0..GR {
                                let mut compte = 0usize;
                                for fr in 0..N_F {
                                    let t0 = 16 + GR + gq + fr * GR;
                                    if t0 + GR > pas_par_groupe {
                                        break;
                                    }
                                    granule_coeffs(&sub, t0, mdct, &cs, &ca, &mut coeffs);
                                    let fc =
                                        frame_likelihood(&coeffs, offsets, BANDE_DEBUT, N_SF, taus);
                                    compte += fc.sous_tau;
                                }
                                let l = compte as f64 / denominateur;
                                if l > meilleur.0 {
                                    meilleur = (l, p + 32 * gq);
                                }
                            }
                        }
                    }
                    meilleur
                }));
            }
            handles
                .into_iter()
                .map(|h| h.join().unwrap_or_else(|e| std::panic::resume_unwind(e)))
                .collect()
        });
        let (l, d) =
            resultats
                .into_iter()
                .fold((0.0f64, 0usize), |acc, r| if r.0 > acc.0 { r } else { acc });
        if best.map(|b| l > b.l).unwrap_or(true) {
            best = Some(TraceMp3 {
                l,
                decalage: d,
                canal: *canal,
            });
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
    }

    fn decode(name: &str) -> Option<(Vec<f32>, u16, u32)> {
        let path = fixture(name);
        if !std::path::Path::new(&path).exists() {
            return None;
        }
        let mut pcm = Vec::new();
        let info =
            crate::analysis::decode::decode_pcm(&path, 2, |b| pcm.extend_from_slice(b)).ok()?;
        Some((pcm, info.channels, info.sample_rate))
    }

    /// Un sinus à 3 kHz ressort dans la sous-bande 4 (chaque sous-bande fait 689 Hz à 44,1 kHz :
    /// 3000/689 = 4,35) et nulle part ailleurs à plus de −40 dB. C'est le test de forme de la
    /// fenêtre et de la matrice : une erreur de signe ou d'indice éparpille l'énergie.
    #[test]
    fn le_banc_polyphase_range_un_sinus_dans_la_bonne_sous_bande() {
        let sr = 44100.0;
        let signal: Vec<f32> = (0..32 * 200)
            .map(|i| (2.0 * std::f64::consts::PI * 3000.0 * i as f64 / sr).sin() as f32)
            .collect();
        let sub = polyphase_analysis(&signal, 200, &analysis_window(), &analysis_matrix());
        let mut energie = [0.0f64; SB];
        for t in 40..200 {
            for sb in 0..SB {
                energie[sb] += sub[t * SB + sb].powi(2);
            }
        }
        let (argmax, emax) =
            energie
                .iter()
                .enumerate()
                .fold((0, 0.0), |a, (i, &e)| if e > a.1 { (i, e) } else { a });
        assert_eq!(
            argmax, 4,
            "3 kHz doit tomber dans la sous-bande 4, énergies {energie:?}"
        );
        for (sb, &e) in energie.iter().enumerate() {
            if sb != 4 && sb != 3 && sb != 5 {
                assert!(
                    e < emax * 1e-4,
                    "fuite en sous-bande {sb} : {e} contre {emax}"
                );
            }
        }
    }

    /// La MDCT 36 est bien l'adjointe de l'IMDCT du décodeur : `Σ_k X[k]·cos(π/72·(2n+1+18)(2k+1))`
    /// remis dans la MDCT rend `18·X` (à la fenêtre près, ici sans fenêtre). Vérifié sur une entrée
    /// aléatoire déterministe.
    #[test]
    fn la_mdct36_est_l_adjointe_de_l_imdct_du_decodeur() {
        let mut xk = [0.0f64; GR];
        let mut seed = 12345u64;
        for v in xk.iter_mut() {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            *v = ((seed >> 33) as f64 / (1u64 << 31) as f64) - 0.5;
        }
        let mut y = [0.0f64; 36];
        for (n, yn) in y.iter_mut().enumerate() {
            for (k, &xkk) in xk.iter().enumerate() {
                *yn += xkk
                    * (std::f64::consts::PI / 72.0
                        * (2.0 * n as f64 + 1.0 + 18.0)
                        * (2.0 * k as f64 + 1.0))
                        .cos();
            }
        }
        let t = mdct36_table();
        for k in 0..GR {
            let mut s = 0.0;
            for n in 0..36 {
                let w = (std::f64::consts::PI / 36.0 * (n as f64 + 0.5)).sin();
                s += t[k * 36 + n] / w * y[n];
            }
            assert!(
                (s - 18.0 * xk[k]).abs() < 1e-9,
                "k={k} : {s} contre {}",
                18.0 * xk[k]
            );
        }
    }

    /// Reconstruction parfaite : analyse (fenêtre `C = D/32`, matrice `cos((2k+1)(i−16)π/64)`)
    /// puis synthèse de la norme (`V = N·S`, `N[i][k] = cos((16+i)(2k+1)π/64)`, FIFO de 1024, `U`,
    /// `W = U·D`, somme de 16) rendent l'entrée à 481 échantillons de retard, gain 1, résidu sous
    /// −80 dB. C'est ce qui prouve que la FORME et les SIGNES de la fenêtre d'analyse sont ceux du
    /// codec : `|D|/32` sans les signes rend une corrélation de 0,17 (mesuré en numpy le
    /// 2026-09-11 avant d'écrire ce test).
    #[test]
    fn analyse_puis_synthese_reconstruisent_le_signal() {
        let c = analysis_window();
        let m = analysis_matrix();
        let mut seed = 7u64;
        let steps = 300usize;
        let x: Vec<f32> = (0..32 * steps)
            .map(|_| {
                seed = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((seed >> 33) as f64 / (1u64 << 31) as f64 - 0.5) as f32
            })
            .collect();
        let sub = polyphase_analysis(&x, steps, &c, &m);
        // Synthèse de la norme (annexe B), telle que le décodeur la fait.
        let mut nmat = vec![0.0f64; 64 * SB];
        for i in 0..64 {
            for k in 0..SB {
                nmat[i * SB + k] =
                    ((16 + i) as f64 * (2 * k + 1) as f64 * std::f64::consts::PI / 64.0).cos();
            }
        }
        let mut v = vec![0.0f64; 1024];
        let mut out = vec![0.0f64; 32 * steps];
        for t in 0..steps {
            v.copy_within(0..960, 64);
            for i in 0..64 {
                let mut acc = 0.0;
                for k in 0..SB {
                    acc += nmat[i * SB + k] * sub[t * SB + k];
                }
                v[i] = acc;
            }
            let mut u = [0.0f64; 512];
            for i in 0..8 {
                u[i * 64..i * 64 + 32].copy_from_slice(&v[i * 128..i * 128 + 32]);
                u[i * 64 + 32..i * 64 + 64].copy_from_slice(&v[i * 128 + 96..i * 128 + 128]);
            }
            let d = synthesis_d();
            for j in 0..32 {
                let mut acc = 0.0;
                for i in 0..16 {
                    acc += u[j + 32 * i] * d[j + 32 * i];
                }
                out[t * 32 + j] = acc;
            }
        }
        let retard = 481usize;
        let (mut num, mut den, mut err) = (0.0f64, 0.0f64, 0.0f64);
        for i in 1000..(32 * steps - retard - 1000) {
            let a = x[i] as f64;
            let b = out[i + retard];
            num += a * b;
            den += a * a;
            err += (b - a) * (b - a);
        }
        let gain = num / den;
        let residu_db = 10.0 * (err / den).log10();
        assert!((gain - 1.0).abs() < 1e-3, "gain {gain}");
        assert!(residu_db < -80.0, "résidu {residu_db:.1} dB");
    }

    /// La table B.3 reste des ENTIERS normatifs sur 2⁻¹⁶, et pas des décimales recopiées.
    ///
    /// Ce que ce test garde n'est pas la justesse du banc — c'est
    /// `analyse_puis_synthese_reconstruisent_le_signal` qui la tient, une table fausse y ferait
    /// remonter le résidu. C'est la PROVENANCE : le portage du 2026-09-11 citait
    /// `symphonia-bundle-mp3` (MPL-2.0) comme source des nombres, ce qui aurait contaminé ce
    /// fichier au moment de fermer la licence (2026-09-12). La table est donc écrite en
    /// numérateurs, un fait de la norme vérifié contre PDMP3 (domaine public), et ce test tombe
    /// si quelqu'un la remplace par des flottants copiés d'une implémentation.
    ///
    /// La signature (somme, extrêmes, valeurs aux indices qui distinguent les arrondis) vient de
    /// la mesure du 2026-09-12 sur les 512 entrées.
    #[test]
    fn la_table_b3_est_en_entiers_normatifs_sur_deux_puissance_seize() {
        assert_eq!(SYNTHESIS_D_NUM.len(), 512);
        let somme: i64 = SYNTHESIS_D_NUM.iter().map(|&n| i64::from(n)).sum();
        assert_eq!(somme, 92_686, "somme des numérateurs");
        assert_eq!(
            SYNTHESIS_D_NUM
                .iter()
                .map(|&n| i64::from(n).abs())
                .sum::<i64>(),
            5_574_752,
            "somme des valeurs absolues"
        );
        assert_eq!(SYNTHESIS_D_NUM.iter().copied().min(), Some(-74_992));
        assert_eq!(SYNTHESIS_D_NUM.iter().copied().max(), Some(75_038));
        // Les indices 196 et 316 valent 3776/65536 : c'est là que les implémentations divergent,
        // certaines tronquant 0,0576171875 en 0,057617187.
        for (i, attendu) in [(0, 0), (16, -5), (196, 3776), (316, -3776), (511, 1)] {
            assert_eq!(SYNTHESIS_D_NUM[i], attendu, "D_NUM[{i}]");
        }
        // Et la fenêtre flottante est exactement le quotient, pas une valeur arrondie.
        assert_eq!(synthesis_d()[196], 3776.0 / 65536.0);
        assert_eq!(synthesis_d()[196], 0.057_617_187_5);
    }

    /// Un lossless ne porte pas de grille MP3 : `real_lossless.flac` (sinus balayé, le cas
    /// dégénéré que `MIN_NIVEAUX_DISTINCTS` garde) reste sous `verdict::QUANT_LAMBDA_MP3`. Le MP3 de la
    /// même fixture (`real_320.mp3`) n'est PAS un test de détection : un sinus balayé encodé est
    /// tonal, la garde le désarme, `L ≈ 0,06` mesuré — c'est le comportement attendu, pas une
    /// faiblesse du banc. La détection se mesure sur de la musique, ci-dessous.
    #[test]
    fn un_lossless_synthetique_ne_porte_pas_de_grille_mp3() {
        let Some((pcm, ch, sr)) = decode("real_lossless.flac") else {
            eprintln!(
                "fixture real_lossless.flac absente — test sauté (voir CLAUDE.md § fixtures)"
            );
            return;
        };
        let t = likelihood(&pcm, ch, sr, Some(4)).expect("mesure");
        eprintln!("real_lossless.flac : L = {:.3}", t.l);
        assert!(
            t.l < crate::analysis::verdict::QUANT_LAMBDA_MP3 as f64,
            "un lossless ne doit pas porter de grille MP3 : L = {:.3}",
            t.l
        );
    }

    /// L'ancre qui compte : un vrai morceau transcodé par LAME 320 (`fixtures/anchor_lame320.flac`,
    /// copie de `C:\sift-corpusake\src01_lame320.flac`, gitignorée comme les autres ancres —
    /// `fixtures/README.md`). Mesuré le 2026-09-11 : `L = 0,984` au décalage 16, canal G ; le V0 de
    /// la même source 0,953 ; l'authentique 0,078. Absente, le test se saute en le disant.
    #[test]
    fn un_vrai_lame320_porte_la_grille() {
        let Some((pcm, ch, sr)) = decode("anchor_lame320.flac") else {
            eprintln!("ancre anchor_lame320.flac absente — test sauté (copie locale du corpus)");
            return;
        };
        let t = likelihood(&pcm, ch, sr, Some(4)).expect("mesure");
        eprintln!(
            "anchor_lame320.flac : L = {:.3} au décalage {} canal {}",
            t.l,
            t.decalage,
            t.canal.label()
        );
        assert!(t.l > 0.5, "la grille LAME doit ressortir : L = {:.3}", t.l);
    }
}

#[cfg(test)]
mod corpus {
    use super::*;

    /// Harnais de mesure sur un dossier : `SIFT_MP3_DIR=<dossier> cargo test --release --lib
    /// mp3_scan -- --ignored --nocapture`. CSV `L;decalage;canal;secondes;fichier`.
    #[test]
    #[ignore]
    fn mp3_scan() {
        let Ok(dir) = std::env::var("SIFT_MP3_DIR") else {
            eprintln!("SIFT_MP3_DIR non défini — rien à mesurer");
            return;
        };
        let mut vus = 0usize;
        println!("L;decalage;canal;secondes;fichier");
        for e in walkdir::WalkDir::new(&dir).into_iter().flatten() {
            if !e.file_type().is_file() {
                continue;
            }
            let path = e.path();
            let ext = path
                .extension()
                .and_then(|x| x.to_str())
                .unwrap_or("")
                .to_lowercase();
            if !matches!(
                ext.as_str(),
                "flac" | "wav" | "aif" | "aiff" | "m4a" | "mp3"
            ) {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|x| x.to_str())
                .unwrap_or("(illisible)");
            vus += 1;
            let mut pcm: Vec<f32> = Vec::new();
            let info = match crate::analysis::decode::decode_pcm(&path.to_string_lossy(), 2, |b| {
                pcm.extend_from_slice(b)
            }) {
                Ok(v) => v,
                Err(err) => {
                    println!("ERREUR;-;-;-;{name} ({err})");
                    continue;
                }
            };
            let t0 = std::time::Instant::now();
            match likelihood(&pcm, info.channels, info.sample_rate, None) {
                Some(t) => println!(
                    "{:.5};{};{};{:.1};{name}",
                    t.l,
                    t.decalage,
                    t.canal.label(),
                    t0.elapsed().as_secs_f64()
                ),
                None => println!("NON-MESURE;-;-;{:.1};{name}", t0.elapsed().as_secs_f64()),
            }
        }
        println!("-- {vus} fichiers parcourus");
        assert!(vus > 0, "aucun fichier audio dans {dir} — mesure vide");
    }
}
