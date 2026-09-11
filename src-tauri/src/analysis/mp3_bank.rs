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
//! reste `verdict::QUANT_LAMBDA`, partagé avec le banc AAC tant qu'une mesure ne dit pas qu'il en
//! faut deux.
//!
//! SOURCE des tables : ISO/IEC 11172-3, tables B.3 (fenêtre de synthèse `D`), B.8 (bandes de
//! facteur d'échelle) et B.9 (coefficients d'aliasing), telles que recopiées dans
//! `symphonia-bundle-mp3` 0.6.1 (MPL-2.0 ; les nombres sont ceux de la norme).

use crate::analysis::quant_trace::{
    frame_likelihood, thresholds, Canal, GROUPES, N_F, N_SF, P_CENTILE,
};

/// Bandes de facteur d'échelle, blocs longs, 44,1 kHz — ISO/IEC 11172-3 table B.8. Vingt-deux
/// bandes sur 576 coefficients ; la dernière (418 → 576, « sfb21 ») n'a pas de facteur d'échelle
/// propre dans un encodeur et fait 158 coefficients, au-delà du tampon de 128 de
/// `frame_likelihood`, qui la saute d'elle-même.
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

/// Fenêtre de synthèse `D[0..512]`, ISO/IEC 11172-3 table B.3 (via symphonia 0.6.1).
#[rustfmt::skip]
static SYNTHESIS_D: [f64; 512] = [
    0.000000000, -0.000015259, -0.000015259, -0.000015259, -0.000015259, -0.000015259, -0.000015259, -0.000030518,
    -0.000030518, -0.000030518, -0.000030518, -0.000045776, -0.000045776, -0.000061035, -0.000061035, -0.000076294,
    -0.000076294, -0.000091553, -0.000106812, -0.000106812, -0.000122070, -0.000137329, -0.000152588, -0.000167847,
    -0.000198364, -0.000213623, -0.000244141, -0.000259399, -0.000289917, -0.000320435, -0.000366211, -0.000396729,
    -0.000442505, -0.000473022, -0.000534058, -0.000579834, -0.000625610, -0.000686646, -0.000747681, -0.000808716,
    -0.000885010, -0.000961304, -0.001037598, -0.001113892, -0.001205444, -0.001296997, -0.001388550, -0.001480103,
    -0.001586914, -0.001693726, -0.001785278, -0.001907349, -0.002014160, -0.002120972, -0.002243042, -0.002349854,
    -0.002456665, -0.002578735, -0.002685547, -0.002792358, -0.002899170, -0.002990723, -0.003082275, -0.003173828,
    0.003250122, 0.003326416, 0.003387451, 0.003433228, 0.003463745, 0.003479004, 0.003479004, 0.003463745,
    0.003417969, 0.003372192, 0.003280640, 0.003173828, 0.003051758, 0.002883911, 0.002700806, 0.002487183,
    0.002227783, 0.001937866, 0.001617432, 0.001266479, 0.000869751, 0.000442505, -0.000030518, -0.000549316,
    -0.001098633, -0.001693726, -0.002334595, -0.003005981, -0.003723145, -0.004486084, -0.005294800, -0.006118774,
    -0.007003784, -0.007919312, -0.008865356, -0.009841919, -0.010848999, -0.011886597, -0.012939453, -0.014022827,
    -0.015121460, -0.016235352, -0.017349243, -0.018463135, -0.019577026, -0.020690918, -0.021789551, -0.022857666,
    -0.023910522, -0.024932861, -0.025909424, -0.026840210, -0.027725220, -0.028533936, -0.029281616, -0.029937744,
    -0.030532837, -0.031005859, -0.031387329, -0.031661987, -0.031814575, -0.031845093, -0.031738281, -0.031478882,
    0.031082153, 0.030517578, 0.029785156, 0.028884888, 0.027801514, 0.026535034, 0.025085449, 0.023422241,
    0.021575928, 0.019531250, 0.017257690, 0.014801025, 0.012115479, 0.009231567, 0.006134033, 0.002822876,
    -0.000686646, -0.004394531, -0.008316040, -0.012420654, -0.016708374, -0.021179199, -0.025817871, -0.030609131,
    -0.035552979, -0.040634155, -0.045837402, -0.051132202, -0.056533813, -0.061996460, -0.067520142, -0.073059082,
    -0.078628540, -0.084182739, -0.089706421, -0.095169067, -0.100540161, -0.105819702, -0.110946655, -0.115921021,
    -0.120697021, -0.125259399, -0.129562378, -0.133590698, -0.137298584, -0.140670776, -0.143676758, -0.146255493,
    -0.148422241, -0.150115967, -0.151306152, -0.151962280, -0.152069092, -0.151596069, -0.150497437, -0.148773193,
    -0.146362305, -0.143264771, -0.139450073, -0.134887695, -0.129577637, -0.123474121, -0.116577148, -0.108856201,
    0.100311279, 0.090927124, 0.080688477, 0.069595337, 0.057617187, 0.044784546, 0.031082153, 0.016510010,
    0.001068115, -0.015228271, -0.032379150, -0.050354004, -0.069168091, -0.088775635, -0.109161377, -0.130310059,
    -0.152206421, -0.174789429, -0.198059082, -0.221984863, -0.246505737, -0.271591187, -0.297210693, -0.323318481,
    -0.349868774, -0.376800537, -0.404083252, -0.431655884, -0.459472656, -0.487472534, -0.515609741, -0.543823242,
    -0.572036743, -0.600219727, -0.628295898, -0.656219482, -0.683914185, -0.711318970, -0.738372803, -0.765029907,
    -0.791213989, -0.816864014, -0.841949463, -0.866363525, -0.890090942, -0.913055420, -0.935195923, -0.956481934,
    -0.976852417, -0.996246338, -1.014617920, -1.031936646, -1.048156738, -1.063217163, -1.077117920, -1.089782715,
    -1.101211548, -1.111373901, -1.120223999, -1.127746582, -1.133926392, -1.138763428, -1.142211914, -1.144287109,
    1.144989014, 1.144287109, 1.142211914, 1.138763428, 1.133926392, 1.127746582, 1.120223999, 1.111373901,
    1.101211548, 1.089782715, 1.077117920, 1.063217163, 1.048156738, 1.031936646, 1.014617920, 0.996246338,
    0.976852417, 0.956481934, 0.935195923, 0.913055420, 0.890090942, 0.866363525, 0.841949463, 0.816864014,
    0.791213989, 0.765029907, 0.738372803, 0.711318970, 0.683914185, 0.656219482, 0.628295898, 0.600219727,
    0.572036743, 0.543823242, 0.515609741, 0.487472534, 0.459472656, 0.431655884, 0.404083252, 0.376800537,
    0.349868774, 0.323318481, 0.297210693, 0.271591187, 0.246505737, 0.221984863, 0.198059082, 0.174789429,
    0.152206421, 0.130310059, 0.109161377, 0.088775635, 0.069168091, 0.050354004, 0.032379150, 0.015228271,
    -0.001068115, -0.016510010, -0.031082153, -0.044784546, -0.057617187, -0.069595337, -0.080688477, -0.090927124,
    0.100311279, 0.108856201, 0.116577148, 0.123474121, 0.129577637, 0.134887695, 0.139450073, 0.143264771,
    0.146362305, 0.148773193, 0.150497437, 0.151596069, 0.152069092, 0.151962280, 0.151306152, 0.150115967,
    0.148422241, 0.146255493, 0.143676758, 0.140670776, 0.137298584, 0.133590698, 0.129562378, 0.125259399,
    0.120697021, 0.115921021, 0.110946655, 0.105819702, 0.100540161, 0.095169067, 0.089706421, 0.084182739,
    0.078628540, 0.073059082, 0.067520142, 0.061996460, 0.056533813, 0.051132202, 0.045837402, 0.040634155,
    0.035552979, 0.030609131, 0.025817871, 0.021179199, 0.016708374, 0.012420654, 0.008316040, 0.004394531,
    0.000686646, -0.002822876, -0.006134033, -0.009231567, -0.012115479, -0.014801025, -0.017257690, -0.019531250,
    -0.021575928, -0.023422241, -0.025085449, -0.026535034, -0.027801514, -0.028884888, -0.029785156, -0.030517578,
    0.031082153, 0.031478882, 0.031738281, 0.031845093, 0.031814575, 0.031661987, 0.031387329, 0.031005859,
    0.030532837, 0.029937744, 0.029281616, 0.028533936, 0.027725220, 0.026840210, 0.025909424, 0.024932861,
    0.023910522, 0.022857666, 0.021789551, 0.020690918, 0.019577026, 0.018463135, 0.017349243, 0.016235352,
    0.015121460, 0.014022827, 0.012939453, 0.011886597, 0.010848999, 0.009841919, 0.008865356, 0.007919312,
    0.007003784, 0.006118774, 0.005294800, 0.004486084, 0.003723145, 0.003005981, 0.002334595, 0.001693726,
    0.001098633, 0.000549316, 0.000030518, -0.000442505, -0.000869751, -0.001266479, -0.001617432, -0.001937866,
    -0.002227783, -0.002487183, -0.002700806, -0.002883911, -0.003051758, -0.003173828, -0.003280640, -0.003372192,
    -0.003417969, -0.003463745, -0.003479004, -0.003479004, -0.003463745, -0.003433228, -0.003387451, -0.003326416,
    0.003250122, 0.003173828, 0.003082275, 0.002990723, 0.002899170, 0.002792358, 0.002685547, 0.002578735,
    0.002456665, 0.002349854, 0.002243042, 0.002120972, 0.002014160, 0.001907349, 0.001785278, 0.001693726,
    0.001586914, 0.001480103, 0.001388550, 0.001296997, 0.001205444, 0.001113892, 0.001037598, 0.000961304,
    0.000885010, 0.000808716, 0.000747681, 0.000686646, 0.000625610, 0.000579834, 0.000534058, 0.000473022,
    0.000442505, 0.000396729, 0.000366211, 0.000320435, 0.000289917, 0.000259399, 0.000244141, 0.000213623,
    0.000198364, 0.000167847, 0.000152588, 0.000137329, 0.000122070, 0.000106812, 0.000106812, 0.000091553,
    0.000076294, 0.000076294, 0.000061035, 0.000061035, 0.000045776, 0.000045776, 0.000030518, 0.000030518,
    0.000030518, 0.000030518, 0.000015259, 0.000015259, 0.000015259, 0.000015259, 0.000015259, 0.000015259,
];

/// Fenêtre d'ANALYSE `C[i] = D[i] / 32` (table C.1 de la norme : même forme, même signe, échelle
/// 1/32). L'échelle n'a aucune incidence sur le test — `Δ̂` est estimé par bande — mais la FORME et
/// les SIGNES sont ceux que l'encodeur a appliqués, et c'est ce qui aligne la grille.
fn analysis_window() -> Vec<f64> {
    SYNTHESIS_D.iter().map(|&d| d / 32.0).collect()
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
            for j in 0..32 {
                let mut acc = 0.0;
                for i in 0..16 {
                    acc += u[j + 32 * i] * SYNTHESIS_D[j + 32 * i];
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

    /// Un lossless ne porte pas de grille MP3 : `real_lossless.flac` (sinus balayé, le cas
    /// dégénéré que `MIN_NIVEAUX_DISTINCTS` garde) reste sous `verdict::QUANT_LAMBDA`. Le MP3 de la
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
            t.l < crate::analysis::verdict::QUANT_LAMBDA as f64,
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
