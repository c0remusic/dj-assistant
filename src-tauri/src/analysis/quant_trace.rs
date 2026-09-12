//! Vraisemblance de transcodage — l'étage de quantification AAC rejoué sur le signal.
//!
//! POURQUOI (mémo de recherche de l'issue #52, 2026-09-01 ; méthode d'Olivier Derrien, JAES 67(3),
//! 2019, DOI 10.17743/jaes.2019.0002). #51 s'est arrêté sur une limite structurelle : 156 des 345
//! faux du corpus passent `Ok` parce que leur coupure est au-dessus de 20 kHz et leur platitude
//! dans la plage des vrais masters. Le spectre ne peut pas les voir, et la littérature dit que
//! c'est définitif. Derrien ne regarde pas le spectre : il teste l'IDEMPOTENCE de l'arrondi.
//!
//! **Le principe, en une phrase.** Un encodeur AAC range chaque coefficient MDCT sur une grille
//! `v = q · Δ` où `v = |X|^{3/4}` et `Δ` est fixé par le scale factor de la bande. Repasser un
//! signal DÉJÀ quantifié dans cette même grille ne change rien — l'erreur d'arrondi est quasi
//! nulle. Un master, lui, rend une erreur d'arrondi uniforme `U[-½, ½]`, dont la loi est
//! ANALYTIQUE. C'est ce dernier point qui sort la méthode de la famille auto-référentielle réfutée
//! trois fois dans #51 : la loi nulle ne vient pas d'une population de référence, donc un master
//! sombre et un master brillant produisent la même loi nulle.
//!
//! **Ce qui est de Derrien, et ce qui est de nous.** Le papier n'a pas pu être relu ici (HAL derrière
//! une protection anti-robot le 2026-09-01) : le mémo du ticket est la spec. En viennent la
//! statistique `E(s) = Σ ε²`, la loi nulle gaussienne tronquée `μ = K/12`, `σ² = K/180`, le seuil
//! `τ(s)` au centile `p`, le compteur par trame et le balayage `décalages × trames × canaux ×
//! résolutions` au réglage `N_f × N_sf = 8 × 8`. Ne viennent PAS du mémo, et sont donc des choix
//! d'ici, marqués `NON CALIBRÉ` un par un plus bas :
//!
//! - **L'estimation de `Δ`** (le scale factor est inconnu, et notre MDCT n'a pas l'échelle interne
//!   du codec). Choix : `Δ̂ = le plus petit v ACTIF de la bande`, ce qui suppose qu'un coefficient
//!   au moins y vaut `q = 1`. Sans recherche sur une grille de candidats — une minimisation sur
//!   candidats casserait la loi nulle, qui est justement tout l'intérêt de la méthode.
//! - **La porte de bruit** [`GATE_REL`], sans laquelle les coefficients que le codec a mis à zéro
//!   (remontés au plancher 16 bits par le décodage puis le ré-encodage FLAC) fixeraient `Δ̂` sur du
//!   bruit.
//! - **La fenêtre de bandes** [`BANDE_DEBUT_LONG`] / [`N_SF`].
//! - **Le seuil de décision** — parti dans `verdict::QUANT_LAMBDA` le 2026-09-02, avec sa
//!   calibration, parce qu'un seuil se lit dans le code qui l'APPLIQUE (voir plus bas).
//!
//! ## Ce qui est MESURÉ (2026-09-02) — sanity de 8 fichiers, réglage 8×8
//!
//! Corpus : `C:\sift-corpus` (10 sources, celui du 2026-08-17 ; celui de #52, en dossier temporaire
//! de session, avait disparu du disque). Cinq `aac256`, trois `genuine` de la même construction.
//! Préfixe de 17 échantillons sauté (`SIFT_QUANT_SKIP=17`) — le piège nommé par le review du
//! 2026-08-18 : nos faux sont encodés depuis l'échantillon 0, et sans ce saut on mesurerait la
//! fabrication du corpus.
//!
//! | fichier | L | décalage | canal | résolution |
//! |---|---|---|---|---|
//! | src01_aac256 | **0,250** | 47 | S | court |
//! | src02_aac256 | **0,203** | 47 | S | court |
//! | src03_aac256 | **0,125** | 47 | S | court |
//! | src04_aac256 | **0,453** | 47 | M | court |
//! | src05_aac256 | **0,250** | 47 | S | court |
//! | src01_genuine | 0,094 | 95 | D | long |
//! | src02_genuine | 0,078 | 613 | D | long |
//! | src03_genuine | 0,094 | 502 | G | long |
//!
//! **Séparation nette, et le décalage est la preuve, pas le score** : les cinq faux tombent sur le
//! MÊME décalage 47 — c'est la grille du codec, retrouvée à l'échantillon près, exactement la
//! signature que le review du 2026-08-18 avait établie. Les authentiques pointent n'importe où.
//! Coût mesuré : **0,3 s par fichier**, tout compris hors décodage, sur un morceau de 6-7 minutes.
//!
//! ## D'où vient le 47, et le contrôle qui le tranche (2026-09-02)
//!
//! Le 47 n'était affirmé nulle part comme dérivé — il était seulement CONSTATÉ. Il se dérive, et
//! l'arithmétique tient en une ligne. Un encodeur AAC LC introduit un retard d'amorçage de
//! **2112 échantillons** en tête du flux décodé ; la grille des blocs COURTS a un pas de 128. La
//! trame du codec tombe donc, dans le fichier décodé, à `2112 mod 128 = 64` (car `2112 = 16×128 +
//! 64`). Le harnais retire ensuite `saut` échantillons de tête AVANT l'analyse, ce qui déplace le
//! décalage gagnant de `−saut` :
//!
//! ```text
//! décalage attendu = (2112 − saut) mod 128 = (64 − saut) mod 128
//!   saut = 17 →  47      saut = 29 →  35
//! ```
//!
//! **Le contrôle (H4).** Rejouer les cinq `aac256` à `SIFT_QUANT_SKIP=29` doit déplacer le gagnant
//! de 47 à 35, soit `−(29 − 17) = −12`. Mesuré : **les cinq rendent 35**, avec des `L` inchangés à
//! la décimale près. La prédiction était écrite avant la mesure, et elle est tombée juste : le
//! décalage suit bien la grille du codec, pas un artefact du protocole.
//!
//! ⚠️ **Ce que ce contrôle NE prouve pas, et il faut le dire.** Le déplacement de −12 est trivial :
//! l'optimum d'une recherche déterministe sur un signal rogné de 12 échantillons de plus se déplace
//! de −12 quel que soit le signal — les trois authentiques le font aussi (95 → 83, 613 → 601,
//! 502 → 490). Ce que le contrôle prouve, c'est la CONVERGENCE : cinq fichiers indépendants qui
//! tombent tous sur la valeur unique prédite par l'arithmétique d'amorçage, aux deux sauts, quand
//! les authentiques se dispersent. Résultats complets et commande de rejeu :
//! `docs/superpowers/changes/2026-09-01-quant-trace/sanity.md`.
//!
//! ⚠️ **Deux faits à ne pas perdre.**
//!
//! 1. **Le signal est dans les blocs COURTS, pas les longs.** Le même balayage limité aux blocs
//!    longs ne sépare RIEN (faux 0,078–0,109 contre authentiques 0,078–0,094). Une sonde
//!    complémentaire — recherche LIBRE de `Δ` sur 200 candidats, bande par bande, sur les 1024
//!    décalages — confirme que la grille des blocs longs est essentiellement absente après
//!    décodage : `ε²` moyen 0,02–0,04 contre 0,083 sous la loi nulle, soit un facteur 2 à 4 et non
//!    l'idempotence quasi nulle que la statistique attend. La fenêtre [`BANDE_DEBUT_LONG`] est
//!    donc à revoir, ou les blocs longs à abandonner.
//! 2. **`λ` = 0,031 est trop BAS**, pas trop haut : il classerait les trois authentiques en faux.
//!    La bande qui sépare ces 8 fichiers est `0,094 < λ ≤ 0,125`. Huit fichiers ne calibrent rien —
//!    la valeur se dérive sur le corpus entier, par le code qui l'applique.
//!
//! ## Branché sur le verdict depuis le 2026-09-02 (intégration de #52)
//!
//! Ce module **mesure**, il ne décide toujours rien : le seuil `λ` vit dans `verdict::QUANT_LAMBDA`
//! avec sa calibration, et c'est `verdict()` qui le compare. `analysis::analyze` appelle
//! [`likelihood`] sur le PCM déjà décodé, et **seulement** là où elle peut trancher —
//! `verdict::needs_quant_probe` : rail lossless déclaré, conteneur non démenti, **bande pleine**.
//! Sous la falaise ou sur une fraude de conteneur, le verdict est déjà Faux et les 0,3 s de
//! balayage seraient dépensées pour rien.
//!
//! ⚠️ La platitude ne fait PAS partie de la condition, et l'avoir cru a coûté une intégration
//! muette (2026-09-02, corrigée le jour même) : la cible de #52 — les transcodes AAC haut débit —
//! a une platitude DANS la plage des masters, donc sortait `Ok` et n'était jamais sondée. Détail
//! et chiffres sur `verdict::needs_quant_probe`.
//!
//! ⚠️ **Phase 1 = AAC seul.** MP3 a un banc hybride PQMF+MDCT à 576 coefficients par granule —
//! même étage de quantification, autre transformée : 2/60 de détection sur les familles `lame*`
//! du corpus, et c'est attendu. Le banc MP3 est un ticket à part.

use crate::analysis::aac_sfb::{swb_offsets, BlockKind};
use crate::analysis::mdct::{kbd_window, sine_window, MdctFast};

// ---------------------------------------------------------------------------------------------
// Fonctions spéciales — erf et son inverse, à la main.
// ---------------------------------------------------------------------------------------------

/// Fonction d'erreur, approximation rationnelle d'Abramowitz & Stegun 7.1.26.
///
/// SOURCE : Abramowitz & Stegun, *Handbook of Mathematical Functions* (NBS AMS 55, 1964),
/// formule **7.1.26** — `erf(x) ≈ 1 − (a₁t + a₂t² + a₃t³ + a₄t⁴ + a₅t⁵)·e^{−x²}` avec
/// `t = 1/(1 + px)`, valide pour `x ≥ 0`, erreur annoncée `|ε| ≤ 1,5·10⁻⁷`. L'extension à `x < 0`
/// est l'imparité de `erf`, pas une approximation.
///
/// Elle existe parce que `std` n'a pas `erf` et que le projet n'ajoute pas une dépendance pour
/// cinq constantes. Les valeurs sont figées par vecteurs (voir les tests) : `erf(1)`, `erf(0,5)`,
/// `erf(2)` sont des constantes publiées, pas un recalcul de cette formule-ci.
pub fn erf(x: f64) -> f64 {
    const P: f64 = 0.327_591_1;
    const A: [f64; 5] = [
        0.254_829_592,
        -0.284_496_736,
        1.421_413_741,
        -1.453_152_027,
        1.061_405_429,
    ];
    let signe = if x < 0.0 { -1.0 } else { 1.0 };
    let ax = x.abs();
    let t = 1.0 / (1.0 + P * ax);
    let poly = t * (A[0] + t * (A[1] + t * (A[2] + t * (A[3] + t * A[4]))));
    signe * (1.0 - poly * (-ax * ax).exp())
}

/// Fonction de répartition de la loi normale centrée réduite, `Φ(x) = ½(1 + erf(x/√2))`.
pub fn norm_cdf(x: f64) -> f64 {
    0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2))
}

/// Quantile de la loi normale centrée réduite, `Φ⁻¹(u)`.
///
/// SOURCE de l'amorce : Abramowitz & Stegun **26.2.23** — pour `0 < u < ½`,
/// `x = t − (c₀ + c₁t + c₂t²)/(1 + d₁t + d₂t² + d₃t³)` avec `t = √(−2 ln u)`, erreur annoncée
/// `|ε| < 4,5·10⁻⁴`, puis `Φ⁻¹(u) = −x`.
///
/// **Deux pas de Newton** ensuite, sur `f(x) = Φ(x) − u` avec `f'(x) = φ(x)`. Ce n'est pas du zèle :
/// 4,5·10⁻⁴ sur un quantile se propage directement dans le seuil `τ`, donc dans le taux de faux
/// positifs par cellule, donc dans `λ` — la seule chose que ce module produise. Après raffinement,
/// l'erreur est bornée par celle d'[`erf`] (~1,5·10⁻⁷ sur `Φ`, divisée par `φ(x)`), soit ~3·10⁻⁶
/// au voisinage du centile 1. Les tests figent `Φ⁻¹(0,01)` et `Φ⁻¹(0,975)`, deux valeurs publiées.
pub fn norm_ppf(u: f64) -> f64 {
    if !(0.0..=1.0).contains(&u) || u == 0.0 || u == 1.0 {
        // Hors domaine : ni panique ni valeur plausible — un infini, qui ne se confond avec aucune
        // mesure et fait tomber la comparaison qui le lit.
        return if u <= 0.0 {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }
    const C: [f64; 3] = [2.515_517, 0.802_853, 0.010_328];
    const D: [f64; 3] = [1.432_788, 0.189_269, 0.001_308];
    let (queue, signe) = if u < 0.5 { (u, -1.0) } else { (1.0 - u, 1.0) };
    let t = (-2.0 * queue.ln()).sqrt();
    let approx = t - (C[0] + t * (C[1] + t * C[2])) / (1.0 + t * (D[0] + t * (D[1] + t * D[2])));
    let mut x = signe * approx;
    for _ in 0..2 {
        let densite = (-0.5 * x * x).exp() / (2.0 * std::f64::consts::PI).sqrt();
        if densite <= f64::MIN_POSITIVE {
            break;
        }
        x -= (norm_cdf(x) - u) / densite;
    }
    x
}

/// Inverse de la fonction d'erreur, `erfinv(y)` tel que `erf(erfinv(y)) = y`.
///
/// Passe par [`norm_ppf`] : `erfinv(y) = Φ⁻¹((y+1)/2)/√2`. C'est une identité exacte, donc la
/// provenance des constantes reste celle d'A&S 26.2.23 citée ci-dessus.
pub fn erfinv(y: f64) -> f64 {
    norm_ppf(0.5 * (y + 1.0)) / std::f64::consts::SQRT_2
}

// ---------------------------------------------------------------------------------------------
// La loi nulle et ses seuils.
// ---------------------------------------------------------------------------------------------

/// Seuils `τ(K)` indexés par NOMBRE DE COEFFICIENTS `K`, du plus petit au plus grand `K` fourni.
///
/// Sous l'hypothèse nulle (le signal n'est pas passé par cette grille), les erreurs d'arrondi
/// `ε_k` sont uniformes sur `[-½, ½]` et indépendantes, donc `E = Σ ε_k²` a pour moments
/// `E[ε²] = 1/12` et `Var[ε²] = E[ε⁴] − E[ε²]² = 1/80 − 1/144 = 1/180`. D'où, par le théorème
/// central limite, `E ~ 𝒩(K/12, K/180)` — c'est la loi du mémo.
///
/// **Tronquée à `[0, +∞)`, et ce n'est pas un raffinement cosmétique** : `μ/σ = √(180K)/12 = 1,118√K`,
/// soit 2,24 pour `K = 4`. La masse gaussienne sous zéro y vaut 1,25 %, du même ordre que le `p` de
/// 1 % qu'on cherche — ignorer la troncature déplacerait le seuil d'un facteur, pas d'une décimale.
/// Le quantile d'ordre `p` de la gaussienne tronquée est
/// `τ = μ + σ·Φ⁻¹( p + (1−p)·Φ(−μ/σ) )`.
///
/// Rend un vecteur aligné sur `widths` : `τ[i]` est le seuil de la largeur `widths[i]`.
pub fn thresholds(p: f64, widths: &[usize]) -> Vec<f64> {
    widths
        .iter()
        .map(|&k| {
            if k == 0 {
                return f64::NEG_INFINITY;
            }
            let mu = k as f64 / 12.0;
            let sigma = (k as f64 / 180.0).sqrt();
            let masse_negative = norm_cdf(-mu / sigma);
            mu + sigma * norm_ppf(p + (1.0 - p) * masse_negative)
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Réglages — tout ce qui n'est pas dicté par le mémo est marqué.
// ---------------------------------------------------------------------------------------------

/// Centile par sous-bande, celui du mémo (« seuils τ(s) au centile 1 par sous-bande »).
pub const P_CENTILE: f64 = 0.01;

/// Trames par groupe — le `N_f` du réglage 8×8.
pub const N_F: usize = 8;

/// Sous-bandes par groupe — le `N_sf` du réglage 8×8.
pub const N_SF: usize = 8;

/// Groupes de `N_F` trames consécutives, répartis sur toute la durée du fichier.
///
/// NON CALIBRÉ. Le mémo ne dit pas combien de groupes examiner ; il dit que le maximum se prend
/// « sur les trames ». Huit groupes étalés couvrent l'introduction, le corps et la fin sans que le
/// coût double.
pub const GROUPES: usize = 8;

/// Première bande de la fenêtre d'analyse, blocs LONGS — **17**, avec [`N_SF_LONG`] = 28 bandes,
/// soit les bandes 17 à 44 : coefficients 96 à 832, **2,1 → 17,9 kHz à 44,1 kHz**.
///
/// MESURÉ le 2026-09-11, et ça remplace un choix a priori (bandes 40 à 47, 14,5 → 20 kHz) qui
/// était faux pour tout le monde. Le diagnostic par bande (`diagnostic::quant_bandes`, décalage
/// forcé 1007, canal M, KBD) sur `src02` donne, en bandes sous `τ` sur 64 trames :
///
/// | bandes | kHz | aac256 | aac128 | authentique |
/// |---|---|---|---|---|
/// | 17-32 | 2,1-9,6 | 12-24 | 9-21 | 0-2 |
/// | 33-44 | 10-17,9 | 21-26 | 0-4 | 0-2 |
/// | 45-47 | 17,9-20 | 7-15 | 0 | 0-2 |
///
/// La grille d'un aac128 vit entre 2 et 10 kHz, celle d'un aac256 entre 2 et 18 kHz, et l'ancienne
/// fenêtre 40-47 tombait exactement là où elle s'éteint. Vingt-huit bandes plutôt que huit
/// glissantes : la statistique gagne en cellules (`8 × 28 = 224` par groupe contre 64), donc la loi
/// nulle se resserre — authentiques ≤ 0,045, 411 lossless taggés magasin ≤ 0,076, contre
/// aac128 ≥ 0,089 et aac256 ≥ 0,357 (`quant_scan`, long seul). Un maximum sur des fenêtres
/// glissantes de huit bandes aurait au contraire ÉLARGI la queue nulle.
///
/// ⚠️ L'échelle de `L` n'est donc plus celle des blocs courts (64 cellules) : chaque résolution se
/// compare à SON seuil, `verdict::quant_lambda_aac`. C'est pour ça que [`likelihood`] rend un
/// résultat PAR résolution au lieu d'un maximum.
pub const BANDE_DEBUT_LONG: usize = 17;

/// Bandes de la fenêtre des blocs LONGS — voir [`BANDE_DEBUT_LONG`].
pub const N_SF_LONG: usize = 28;

/// Première bande de la fenêtre d'analyse, blocs COURTS. NON CALIBRÉ, même raisonnement : les
/// bandes 6 à 13 d'un bloc court de 128 coefficients sont les huit dernières de la table.
pub const BANDE_DEBUT_COURT: usize = 6;

/// Porte de bruit, en rapport au plus grand `v = |X|^{3/4}` de la bande.
///
/// NON CALIBRÉ — choix d'ici. Un coefficient sous cette porte est ignoré : ni pour estimer `Δ̂`, ni
/// dans la somme `E`. Sans elle, la méthode s'effondre pour une raison précise et non évidente :
/// les coefficients que le codec a mis à `q = 0` ne reviennent PAS à zéro dans notre analyse. Le
/// décodeur sort du PCM 16 bits, et la MDCT de ce PCM les remonte au plancher d'arrondi. Le plus
/// petit `v` de la bande serait alors du bruit, `Δ̂` serait faux d'ordres de grandeur, et un vrai
/// transcode passerait pour un master.
///
/// 1/24 en `v` correspond à un rapport d'amplitude de `24^{4/3} ≈ 70`, soit −37 dB sous le plus
/// fort de la bande : assez large pour garder un `q = 1` quand `q_max ≈ 24`, assez serré pour
/// exclure le plancher 16 bits d'un aac256 dans ces bandes-là.
pub const GATE_REL: f64 = 1.0 / 24.0;

/// Coefficients actifs minimaux pour qu'une bande soit jugeable.
///
/// NON CALIBRÉ. En dessous, deux choses cassent ensemble : le TCL qui fonde la loi nulle (à `K = 4`
/// la somme de quatre `ε²` est très asymétrique — voir le test de Monte-Carlo, qui le MESURE au lieu
/// de l'affirmer), et l'estimation de `Δ̂`, qui coûte un degré de liberté sur les `K` disponibles.
pub const MIN_ACTIFS: usize = 8;

/// Niveaux de grille DISTINCTS minimaux pour qu'une bande soit jugeable.
///
/// NON CALIBRÉ au sens du mémo — dérivé le 2026-09-02 d'une mesure, voir plus bas. C'est le même
/// esprit que [`MIN_ACTIFS`], un cran au-dessus : là où `MIN_ACTIFS` exige assez de coefficients,
/// celui-ci exige qu'ils ne soient pas tous empilés sur les mêmes `q`.
///
/// **Pourquoi il existe.** La loi nulle suppose `ε ~ U[-½, ½]` INDÉPENDANTS. Cette hypothèse
/// demande que les coefficients actifs se répartissent sur plusieurs marches de la grille. Quand
/// ils n'occupent qu'un ou deux niveaux — `round(y)` ne prend qu'une ou deux valeurs — « tout tombe
/// près d'un multiple de `Δ̂` » devient presque vide de contenu : la statistique n'a plus de
/// puissance, et `Σε²` passe sous `τ` pour des raisons qui n'ont rien à voir avec un codec.
///
/// **Ce qui l'a fait apparaître (fixture `real_lossless.flac`, 2026-09-02).** Le sinus balayé de
/// `scripts/make-fixtures.mjs` — `0.3*sin(2π(300+20000t/10)t)`, 10 s — rendait `L = 0,734`, donc
/// **Fake**, et faisait tomber `characterization::real_lossless_flac_is_ok`. Diagnostic par
/// `diagnostic::quant_diag` : ce n'était PAS un manque de bandes jugeables (64 sur 64 l'étaient,
/// pleine densité), mais la forme des coefficients À L'INTÉRIEUR de chaque bande. La MDCT d'un ton
/// pur est un peigne à DEUX niveaux — les `y` alternent, par exemple
/// `[5,43 · 1,22 · 5,36 · 1,16 · 5,36 · 1,20 …]`. Deux niveaux, 32 coefficients : `ε` est
/// concentré, pas uniforme, et `Σε² = 1,03` passe sous `τ = 1,62` sans qu'aucune grille de codec
/// n'existe.
///
/// **D'où vient le 4.** Histogramme des niveaux distincts des bandes SOUS-`τ`, au décalage
/// gagnant (`quant_diag`) : le sinus balayé les tire de `{1 niveau : 103, 2 : 57, 3 : 5}`, un vrai
/// `aac256` de `{2 : 5, 3 : 9, 4 : 13, 5 : 5, …}` étalé jusqu'à 12. La dégénérescence est donc
/// du côté des PETITS comptes, et la détection réelle du côté des grands. `L` mesuré au décalage
/// gagnant, en faisant varier ce seuil :
///
/// | seuil | sinus balayé | aacmf256 | aac256 | genuine src10 | genuine src01 |
/// |---|---|---|---|---|---|
/// | 1 (avant) | **0,734** | 0,938 | 0,703 | 0,141 | 0,094 |
/// | 3 | 0,266 | 0,469 | 0,609 | 0,125 | 0,094 |
/// | **4** | **0,125** | 0,375 | 0,469 | 0,078 | 0,094 |
///
/// 4 est la PLUS PETITE valeur qui ramène le signal synthétique sous `verdict::QUANT_LAMBDA`
/// (0,18), en laissant les transcodages réels à 2-3 fois le seuil. À 3, le balayé reste à 0,266,
/// donc Faux.
///
/// ⚠️ **La marge du côté synthétique est mince** — 0,125 contre 0,18, un facteur 1,4. Un autre
/// signal quasi-tonal (drone, intro tenue, bruit de test) pourrait repasser au-dessus. Ce n'est pas
/// une garantie, c'est le seuil que la mesure disponible soutient.
pub const MIN_NIVEAUX_DISTINCTS: usize = 4;

// Le seuil de décision `λ` NE VIT PLUS ICI (2026-09-02, intégration de #52). Il est passé dans
// `verdict::QUANT_LAMBDA`, avec sa calibration, parce que c'est la règle du dépôt : un seuil se
// mesure et se lit dans le code qui l'APPLIQUE. La valeur provisoire `2/64 = 0,031 25` qui
// occupait cette place n'avait été mesurée sur rien et était trop BASSE d'un facteur ~6 (elle
// classait en faux les trois authentiques de la sanity) — elle n'a pas été « déplacée », elle a
// été remplacée par une valeur dérivée à zéro faux positif sur le corpus.

// ---------------------------------------------------------------------------------------------
// Le cœur : une trame, puis le balayage.
// ---------------------------------------------------------------------------------------------

/// Canal analysé. `M`/`S` existent parce qu'un encodeur AAC quantifie souvent la somme et la
/// différence, pas la gauche et la droite : la grille peut n'exister QUE dans le domaine M/S.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Canal {
    Gauche,
    Droite,
    Milieu,
    Cote,
}

impl Canal {
    /// Étiquette CSV — stable, jamais traduite dans la sortie du harnais.
    pub fn label(self) -> &'static str {
        match self {
            Canal::Gauche => "G",
            Canal::Droite => "D",
            Canal::Milieu => "M",
            Canal::Cote => "S",
        }
    }
}

/// Résultat du balayage : la vraisemblance et l'endroit où elle a été atteinte.
///
/// Le décalage est là parce que c'est LUI la preuve de mécanisme, pas le score : le review du
/// 2026-08-18 a montré que la grille vraie se retrouve à l'échantillon près sur des faux et jamais
/// sur un master. Un `L` élevé à un décalage incohérent d'une piste à l'autre serait un artefact.
#[derive(Debug, Clone, Copy)]
pub struct Trace {
    pub l: f64,
    pub decalage: usize,
    pub canal: Canal,
    pub resolution: BlockKind,
    pub fenetre: Fenetre,
}

/// Forme de la fenêtre MDCT essayée par le balayage.
///
/// Elle existe depuis le 2026-09-11, et c'est une correction de fond : l'analyse ne fenêtrait
/// qu'en sinus, or `ffmpeg` encode ses blocs LONGS en Kaiser-Bessel dérivée (`aacpsy.c`,
/// `psy_lame_window` : `window_shape = 1` pour `ONLY_LONG` et `LONG_STOP`, `0` pour `LONG_START`
/// et les huit courts). Une MDCT sinus d'un flux synthétisé en KBD ne rend pas les coefficients
/// du codec — l'annulation du repliement temporel exige la même fenêtre des deux côtés
/// (`mdct::tests::une_synthese_kbd_ne_se_reanalyse_quen_kbd`). C'est ce qui faisait constater à
/// ce module que « la grille des blocs longs est essentiellement absente après décodage » : elle
/// était regardée à travers la mauvaise fenêtre. Le balayage essaie désormais les DEUX formes par
/// résolution et garde le maximum, comme il le fait déjà pour les canaux — la forme d'un encodeur
/// inconnu (Apple, Media Foundation, Fraunhofer) ne se devine pas, elle se mesure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fenetre {
    Sinus,
    Kbd,
}

impl Fenetre {
    pub fn label(self) -> &'static str {
        match self {
            Fenetre::Sinus => "sinus",
            Fenetre::Kbd => "kbd",
        }
    }

    /// La fenêtre de `2N` échantillons pour cette forme et cette résolution. Les `α` sont ceux
    /// de la norme : 4 pour les blocs longs, 6 pour les courts.
    pub fn echantillons(self, kind: BlockKind) -> Vec<f32> {
        let two_n = 2 * kind.coeffs();
        match self {
            Fenetre::Sinus => sine_window(two_n),
            Fenetre::Kbd => kbd_window(
                two_n,
                match kind {
                    BlockKind::Long => 4.0,
                    BlockKind::Short => 6.0,
                },
            ),
        }
    }
}

/// Les deux formes, dans l'ordre du balayage. `likelihood` les essaie toutes ; le harnais
/// `quant_scan` peut en isoler une (`SIFT_QUANT_WIN`).
pub const FENETRES: [Fenetre; 2] = [Fenetre::Sinus, Fenetre::Kbd];

/// Ce qu'une trame rapporte au balayage : combien de ses bandes portent la grille, et sur combien
/// de bandes JUGEABLES.
///
/// Les deux voyagent ensemble parce que le second est ce qui donne un sens au premier. `sous_tau`
/// seul ne distingue pas « aucune bande ne porte la grille » de « aucune bande n'était jugeable » —
/// or le premier est une mesure, le second une absence de mesure, et tout ce module est construit
/// sur cette distinction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FrameCount {
    /// Bandes dont l'erreur d'arrondi passe sous le seuil `τ` : la grille y est retrouvée.
    pub sous_tau: usize,
    /// Bandes JUGEABLES — au moins [`MIN_ACTIFS`] coefficients au-dessus de la porte de bruit, et
    /// un `Δ̂` fini. Les autres sont sautées (voir les cinq sorties silencieuses ci-dessous).
    pub eligibles: usize,
}

/// Vraisemblance d'UNE trame : combien des `N_SF` bandes de la fenêtre portent la grille.
///
/// Rend un compte sur `n_sf`, jamais une fraction : une bande injugeable (moins de [`MIN_ACTIFS`]
/// coefficients au-dessus de la porte) compte comme NON quantifiée, elle ne sort pas du
/// dénominateur. Sinon un groupe où une seule bande est jugeable rendrait `L = 1`.
///
/// `taus` est indexé par nombre de coefficients : `taus[k]` est le seuil pour `k` degrés de liberté.
///
/// ## Les cinq sorties silencieuses, et pourquoi aucune n'est une erreur
///
/// Une bande sautée par un `continue` compte comme NON quantifiée (elle reste au dénominateur,
/// voir plus haut). Ce n'est jamais un échec à signaler : c'est le sens même de la mesure. Les
/// cinq cas, dans l'ordre du corps :
///
/// 1. `hi > coeffs.len()` — la table de bandes déborde la trame fournie. Arrive quand l'appelant
///    passe une table jouet plus large que ses coefficients ; en production `coeffs.len() == n`
///    et la table finit exactement à `n`.
/// 2. `bande.len() > 128` — le PLAFOND DUR de `v`, tampon de pile `[f64; 128]`. Il tient toutes
///    les bandes courtes (la plus large de `SWB_OFFSET_128_48` fait 16) mais PAS les bandes
///    longues de l'aigu, larges de 32 : celles-là passent, les hypothétiques bandes de plus de
///    128 coefficients seraient muettement ignorées. Aucune table AAC tabulée ici n'en a — la
///    plus large de `SWB_OFFSET_1024_48` fait 96 (928 → 1024, le terminateur). Une table future
///    plus grossière casserait cette borne EN SILENCE : le jour où une entrée dépasse 128, ce
///    tampon devient un `Vec` ou la constante monte, elle ne se contourne pas.
/// 3. `vmax <= 0` — bande entièrement nulle (silence numérique). Il n'y a pas de grille à tester.
/// 4. `actifs < MIN_ACTIFS` ou `Δ̂` non fini — bande injugeable, voir [`MIN_ACTIFS`].
/// 5. `k_eff == 0 || k_eff >= taus.len()` — pas de seuil tabulé pour ce nombre de degrés de
///    liberté. L'appelant construit `taus` sur `0..=n`, donc ce cas ne se produit qu'avec une
///    table de seuils trop courte.
/// 6. `n_niveaux < MIN_NIVEAUX_DISTINCTS` — les coefficients actifs sont empilés sur trop peu de
///    marches de la grille pour que le test ait la moindre puissance. Voir
///    [`MIN_NIVEAUX_DISTINCTS`] : c'est le cas du ton pur, et il faisait rendre `Fake` à un sinus
///    balayé.
pub fn frame_likelihood(
    coeffs: &[f64],
    offsets: &[u16],
    bande_debut: usize,
    n_sf: usize,
    taus: &[f64],
) -> FrameCount {
    let mut compte = 0usize;
    let mut eligibles = 0usize;
    for s in bande_debut..(bande_debut + n_sf).min(offsets.len() - 1) {
        let (lo, hi) = (offsets[s] as usize, offsets[s + 1] as usize);
        if let Jugement::Jugee { sous_tau, .. } = juge_bande(coeffs, lo, hi, taus) {
            eligibles += 1;
            if sous_tau {
                compte += 1;
            }
        }
    }
    FrameCount {
        sous_tau: compte,
        eligibles,
    }
}

/// Ce qu'une bande rend au jugement — les sorties silencieuses de [`frame_likelihood`], NOMMÉES,
/// pour que le diagnostic puisse les compter au lieu de les deviner. Seule `Jugee` entre au
/// dénominateur de `L`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Jugement {
    /// Table de bandes hors de la trame, ou bande plus large que le tampon de 128.
    HorsTrame,
    /// Bande entièrement nulle.
    Nulle,
    /// Moins de [`MIN_ACTIFS`] coefficients au-dessus de la porte.
    PeuActifs { actifs: usize },
    /// Moins de [`MIN_NIVEAUX_DISTINCTS`] marches occupées.
    PeuNiveaux { actifs: usize, niveaux: usize },
    /// Pas de seuil tabulé pour ce nombre de degrés de liberté.
    SansSeuil,
    Jugee {
        sous_tau: bool,
        actifs: usize,
        niveaux: usize,
    },
}

/// Le jugement d'UNE bande `[lo, hi)` d'une trame — le corps historique de [`frame_likelihood`],
/// sorti tel quel le 2026-09-11 pour que `diagnostic::quant_bandes` le rejoue bande par bande.
pub fn juge_bande(coeffs: &[f64], lo: usize, hi: usize, taus: &[f64]) -> Jugement {
    if hi > coeffs.len() {
        return Jugement::HorsTrame;
    }
    // v = |X|^{3/4}, calculé par deux racines et un cube : `powf` coûte ici plus cher que tout
    // le reste du balayage réuni, et `r = √√|X|` puis `r·r·r` donne exactement |X|^{3/4}.
    let mut v = [0.0f64; 128];
    let bande = &coeffs[lo..hi];
    if bande.len() > v.len() {
        return Jugement::HorsTrame;
    }
    let mut vmax = 0.0f64;
    for (i, &x) in bande.iter().enumerate() {
        let r = x.abs().sqrt().sqrt();
        let vi = r * r * r;
        v[i] = vi;
        if vi > vmax {
            vmax = vi;
        }
    }
    if vmax <= 0.0 {
        return Jugement::Nulle;
    }
    let porte = vmax * GATE_REL;
    let mut delta = f64::INFINITY;
    let mut actifs = 0usize;
    for &vi in &v[..bande.len()] {
        if vi > porte {
            actifs += 1;
            if vi < delta {
                delta = vi;
            }
        }
    }
    if actifs < MIN_ACTIFS || !delta.is_finite() || delta <= 0.0 {
        return Jugement::PeuActifs { actifs };
    }
    // Le degré de liberté consommé par l'estimation de Δ̂ : le coefficient qui l'a fixée rend
    // ε = 0 par construction, il ne compte ni dans la somme ni dans les degrés de liberté.
    let mut somme = 0.0f64;
    // Marques des niveaux `round(y)` déjà vus. 64 entrées suffisent STRUCTURELLEMENT : la
    // porte de bruit borne `vmax/Δ̂` à 24 (voir `GATE_REL`), donc `y < 24` pour tout
    // coefficient actif. La garde d'indice reste, parce qu'une borne raisonnée n'est pas une
    // borne vérifiée par le compilateur.
    let mut niveaux = [false; 64];
    let mut n_niveaux = 0usize;
    for &vi in &v[..bande.len()] {
        if vi <= porte {
            continue;
        }
        let y = vi / delta;
        let eps = y - y.round();
        somme += eps * eps;
        let niv = y.round() as usize;
        if niv < niveaux.len() && !niveaux[niv] {
            niveaux[niv] = true;
            n_niveaux += 1;
        }
    }
    if n_niveaux < MIN_NIVEAUX_DISTINCTS {
        return Jugement::PeuNiveaux {
            actifs,
            niveaux: n_niveaux,
        };
    }
    let k_eff = actifs - 1;
    if k_eff == 0 || k_eff >= taus.len() {
        return Jugement::SansSeuil;
    }
    Jugement::Jugee {
        sous_tau: somme < taus[k_eff],
        actifs,
        niveaux: n_niveaux,
    }
}

/// Balayage complet d'un fichier décodé : pour CHAQUE résolution demandée, `L = max` sur
/// décalages × groupes de trames × canaux × formes de fenêtre.
///
/// `pcm` est ENTRELACÉ (`channels` canaux), tel que le rend `decode::decode_pcm`.
///
/// Un résultat par résolution mesurable, dans l'ordre de `resolutions` — et PAS un maximum entre
/// elles : depuis le 2026-09-11 les blocs longs se jugent sur 28 bandes et les courts sur 8, donc
/// leurs `L` ne sont pas sur la même échelle et se comparent chacun à leur seuil
/// (`verdict::quant_lambda_aac`). Vide quand la mesure n'existe pas — taux d'échantillonnage hors
/// des tables AAC, ou fichier trop court pour un seul groupe. Jamais une valeur par défaut : c'est
/// la règle du mémo, et c'est déjà celle de `verdict()` sur l'absence de mesure.
///
/// `fils_max` plafonne la parallélisation interne du balayage. **`None` = le comportement
/// historique** (`available_parallelism()`, borné à 16) : c'est ce que veut le harnais
/// `quant_scan`, qui tourne seul et doit prendre toute la machine.
///
/// ⚠️ **`analysis::analyze` passe `Some(2)`, et ce n'est pas de la frilosité.** Cette fonction est
/// appelée DEPUIS un thread du pool d'analyse, lui-même dimensionné à
/// `available_parallelism().clamp(1, 8)` (`worker::analysis_pool_size`). Sans plafond, une machine
/// à 16 cœurs faisait 8 threads de pool × 16 fils de balayage = **128 fils** pour 16 cœurs — une
/// sur-souscription d'un facteur 8, qui coûte en changements de contexte ce qu'elle prétend gagner
/// en parallélisme. Le résultat est IDENTIQUE quel que soit le nombre de fils (le balayage est un
/// `max` sur une partition des décalages, pas une réduction dépendante de l'ordre) — figé par
/// `le_nombre_de_fils_ne_change_ni_le_L_ni_le_decalage`.
pub fn likelihood(
    pcm: &[f32],
    channels: u16,
    sample_rate: u32,
    resolutions: &[BlockKind],
    fils_max: Option<usize>,
) -> Vec<Trace> {
    likelihood_fenetres(pcm, channels, sample_rate, resolutions, &FENETRES, fils_max)
}

/// [`likelihood`] avec le choix des formes de fenêtre laissé à l'appelant — pour le harnais de
/// mesure, qui doit pouvoir isoler une forme et comparer. La production passe par [`likelihood`],
/// qui essaie toujours [`FENETRES`] en entier.
pub fn likelihood_fenetres(
    pcm: &[f32],
    channels: u16,
    sample_rate: u32,
    resolutions: &[BlockKind],
    fenetres: &[Fenetre],
    fils_max: Option<usize>,
) -> Vec<Trace> {
    likelihood_reglee(
        pcm,
        channels,
        sample_rate,
        resolutions,
        fenetres,
        (BANDE_DEBUT_LONG, N_SF_LONG),
        fils_max,
    )
}

/// [`likelihood_fenetres`] avec la fenêtre de bandes des blocs LONGS `(première bande, nombre de
/// bandes)` laissée à l'appelant — harnais de mesure uniquement (`SIFT_QUANT_BANDES`). Le
/// dénominateur de `L` suit le nombre de bandes.
pub fn likelihood_reglee(
    pcm: &[f32],
    channels: u16,
    sample_rate: u32,
    resolutions: &[BlockKind],
    fenetres: &[Fenetre],
    bandes_long: (usize, usize),
    fils_max: Option<usize>,
) -> Vec<Trace> {
    let ch = channels.max(1) as usize;
    let n_trames = pcm.len() / ch;
    if n_trames == 0 {
        return Vec::new();
    }

    // Canaux dérivés. En mono, G seul : M et S seraient G et zéro, donc trois mesures pour une.
    let canaux: Vec<(Canal, Vec<f32>)> = if ch >= 2 {
        let g: Vec<f32> = (0..n_trames).map(|i| pcm[i * ch]).collect();
        let d: Vec<f32> = (0..n_trames).map(|i| pcm[i * ch + 1]).collect();
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

    let mut traces: Vec<Trace> = Vec::with_capacity(resolutions.len());
    for &kind in resolutions {
        let mut best: Option<Trace> = None;
        // Indisponibilité de résolution = cette résolution SAUTE, pas le fichier entier. Le `?`
        // d'origine sortait de toute la fonction : un taux tabulé en long mais pas en court (ou
        // l'inverse, si les tables divergent un jour) aurait rendu `None` alors qu'une des deux
        // résolutions était mesurable. Même fail-open que les deux `continue` qui suivent.
        let Some(offsets) = swb_offsets(sample_rate, kind) else {
            continue;
        };
        let n = kind.coeffs();
        let (bande_debut, n_sf) = match kind {
            BlockKind::Long => bandes_long,
            BlockKind::Short => (BANDE_DEBUT_COURT, N_SF),
        };
        if bande_debut + n_sf > offsets.len() - 1 {
            continue;
        }
        // Table de seuils indexée par degrés de liberté, calculée UNE fois : le balayage la lit des
        // millions de fois.
        let largeurs: Vec<usize> = (0..=n).collect();
        let taus = thresholds(P_CENTILE, &largeurs);

        // Départs de groupes, communs à tous les décalages : sinon les groupes ne comparent pas les
        // mêmes instants d'un décalage à l'autre.
        let dispo = n_trames.saturating_sub(n - 1 + 2 * n) / n;
        if dispo < N_F {
            continue;
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

        for &fenetre in fenetres {
            // Les blocs courts sont en sinus chez ffmpeg comme chez Media Foundation (mesuré le
            // 2026-09-11 : court KBD ≤ 0,078 sur 56 fichiers, rien nulle part). Pas de balayage
            // KBD en court.
            if kind == BlockKind::Short && fenetre == Fenetre::Kbd {
                continue;
            }
            let w = fenetre.echantillons(kind);
            let reglage = Balayage {
                w: &w,
                n,
                offsets,
                bande_debut,
                n_sf,
                taus: &taus,
                departs: &departs,
                fils_max,
            };
            for (canal, signal) in &canaux {
                let (l, decalage) = balaye_decalages(signal, &reglage);
                if best.map(|b| l > b.l).unwrap_or(true) {
                    best = Some(Trace {
                        l,
                        decalage,
                        canal: *canal,
                        resolution: kind,
                        fenetre,
                    });
                }
            }
        }
        traces.extend(best);
    }
    traces
}

/// Le balayage des `N` décalages pour un canal et une résolution, réparti sur les cœurs.
///
/// Parallélisation par `std::thread::scope` — le motif du dépôt (`worker.rs` : pool
/// `std::thread::spawn` dimensionné sur `available_parallelism()`), pas d'async, pas de rayon.
/// Chaque thread construit SON `MdctFast` : le plan porte un tampon de travail mutable, donc il ne
/// se partage pas, et le reconstruire coûte une FFT planifiée par thread — rien à l'échelle du
/// balayage.
///
/// Les réglages voyagent dans [`Balayage`] plutôt qu'en huit paramètres positionnels : au huitième
/// (le plafond de fils, 2026-09-02) `clippy::too_many_arguments` tombe, et il a raison — `n`,
/// `bande_debut` et `fils_max` sont trois `usize` d'affilée, que rien n'empêcherait d'interchanger
/// silencieusement. Ce sont des réglages CONSTANTS sur toute une résolution ; seul `signal` change
/// d'un canal à l'autre, et il reste un paramètre à part pour que ça se voie.
struct Balayage<'a> {
    w: &'a [f32],
    n: usize,
    offsets: &'static [u16],
    bande_debut: usize,
    n_sf: usize,
    taus: &'a [f64],
    departs: &'a [usize],
    fils_max: Option<usize>,
}

fn balaye_decalages(signal: &[f32], r: &Balayage<'_>) -> (f64, usize) {
    let Balayage {
        w,
        n,
        offsets,
        bande_debut,
        n_sf,
        taus,
        departs,
        fils_max,
    } = *r;
    // `None` = le comportement historique. Un plafond fourni ne peut que RESSERRER : passer
    // `Some(64)` sur une machine à 4 cœurs ne crée pas 64 fils, il en crée 4. Un `Some(0)` est
    // ramené à 1 plutôt que de produire une division par zéro sur `par_fil`.
    let dispo = std::thread::available_parallelism()
        .map(|v| v.get())
        .unwrap_or(1);
    let fils = match fils_max {
        Some(m) => dispo.min(m).max(1),
        None => dispo.clamp(1, 16),
    };
    let par_fil = n.div_ceil(fils);
    let denominateur = (N_F * n_sf) as f64;

    let resultats: Vec<(f64, usize)> = std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(fils);
        for f in 0..fils {
            let debut = f * par_fil;
            let fin = ((f + 1) * par_fil).min(n);
            if debut >= fin {
                continue;
            }
            handles.push(scope.spawn(move || {
                let plan = MdctFast::new(n);
                let mut trame = vec![0.0f32; 2 * n];
                // Sortie réutilisée d'une trame à l'autre : `transform_f64` allouerait un `Vec` de
                // N par trame, soit des millions d'allocations sur le balayage.
                let mut coeffs = vec![0.0f64; n];
                let mut meilleur = (0.0f64, debut);
                for d in debut..fin {
                    for &depart in departs {
                        let mut compte = 0usize;
                        let mut eligibles = 0usize;
                        for t in 0..N_F {
                            let base = d + (depart + t) * n;
                            if base + 2 * n > signal.len() {
                                break;
                            }
                            for i in 0..2 * n {
                                trame[i] = signal[base + i] * w[i];
                            }
                            plan.transform_f64_into(&trame, &mut coeffs);
                            let fc = frame_likelihood(&coeffs, offsets, bande_debut, n_sf, taus);
                            compte += fc.sous_tau;
                            eligibles += fc.eligibles;
                        }
                        let l = compte as f64 / denominateur;
                        let _ = eligibles;
                        if l > meilleur.0 {
                            meilleur = (l, d);
                        }
                    }
                }
                meilleur
            }));
        }
        // Un fil qui panique ne se tait pas. `filter_map(.ok())` rendrait un `L` calculé sur une
        // FRACTION des décalages — un chiffre plus bas, crédible, et faux : exactement le fallback
        // silencieux que la méthode du dépôt interdit. Ici, contrairement au `worker_loop` de
        // `worker.rs`, il n'y a ni file à reprendre ni ligne de base à mettre en échec : le seul
        // état correct est de reporter la panique à l'appelant, qui est déjà en train de paniquer.
        // `resume_unwind` n'est donc pas un `unwrap` sur un `Result` d'erreur métier.
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|e| std::panic::resume_unwind(e)))
            .collect()
    });

    resultats
        .into_iter()
        .fold((0.0f64, 0usize), |acc, r| if r.0 > acc.0 { r } else { acc })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vecteurs gelés d'[`erf`] — des constantes PUBLIÉES, pas un recalcul de la formule qu'elles
    /// gardent. `erf(0,5) = 0,520499877813…`, `erf(1) = 0,842700792950…`,
    /// `erf(2) = 0,995322265019…`.
    ///
    /// Tolérance `2e-7`, soit la borne annoncée par A&S pour 7.1.26 (`1,5e-7`) avec un peu de
    /// marge : la serrer davantage ferait tomber le test sur l'approximation elle-même, pas sur
    /// une régression.
    #[test]
    fn erf_colle_aux_valeurs_publiees() {
        for (x, attendu) in [
            (0.0, 0.0),
            (0.5, 0.520_499_877_813),
            (1.0, 0.842_700_792_950),
            (2.0, 0.995_322_265_019),
        ] {
            assert!(
                (erf(x) - attendu).abs() < 2e-7,
                "erf({x}) = {} contre {attendu}",
                erf(x)
            );
            assert!(
                (erf(-x) + attendu).abs() < 2e-7,
                "erf n'est pas impaire en {x}"
            );
        }
    }

    /// Vecteurs gelés de [`norm_ppf`] / [`erfinv`] — quantiles normaux publiés :
    /// `Φ⁻¹(0,01) = −2,326347874…`, `Φ⁻¹(0,975) = 1,959963985…`, `Φ⁻¹(0,5) = 0`.
    ///
    /// Tolérance `1e-5` : après les deux pas de Newton, l'erreur résiduelle est celle d'[`erf`]
    /// divisée par la densité au point (~3e-6 au centile 1). C'est ce qui distingue ce test de la
    /// borne brute d'A&S 26.2.23, qui est de 4,5e-4 — s'il passait à 1e-5 SANS le raffinement, le
    /// raffinement ne servirait à rien.
    #[test]
    fn le_quantile_normal_colle_aux_valeurs_publiees() {
        for (u, attendu) in [
            (0.5, 0.0),
            (0.01, -2.326_347_874),
            (0.975, 1.959_963_985),
            (0.99, 2.326_347_874),
        ] {
            let obtenu = norm_ppf(u);
            assert!(
                (obtenu - attendu).abs() < 1e-5,
                "Φ⁻¹({u}) = {obtenu} contre {attendu}"
            );
        }
        // L'identité qui relie les deux fonctions, dans le sens qui compte : erf(erfinv(y)) = y.
        for y in [-0.98f64, -0.5, 0.0, 0.5, 0.9, 0.99] {
            assert!(
                (erf(erfinv(y)) - y).abs() < 1e-6,
                "erf(erfinv({y})) = {}",
                erf(erfinv(y))
            );
        }
    }

    /// Générateur uniforme déterministe — LCG 64 bits (constantes de Knuth/MMIX), 53 bits de
    /// mantisse retenus. Déterministe parce qu'un test de Monte-Carlo qui tomberait une fois sur
    /// dix selon la graine ne serait pas un test.
    struct Lcg(u64);

    impl Lcg {
        fn suivant(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    /// **LE test qui tient la méthode** (mémo #52).
    ///
    /// Toute la valeur de Derrien vient de ce que la loi nulle est ANALYTIQUE : on n'apprend pas un
    /// seuil sur une population de référence, on le calcule. Si `thresholds` ne rendait pas
    /// réellement le quantile d'ordre `p` de `Σ ε²` sous `ε ~ U[-½, ½]`, le taux de faux positifs
    /// par cellule ne serait pas `p`, `λ` ne voudrait plus rien dire, et la méthode retomberait dans
    /// la famille auto-référentielle que #51 a réfutée trois fois. Rien d'autre dans ce module ne
    /// vérifie ça : ni le type, ni la compilation, ni la mesure sur corpus.
    ///
    /// Protocole : simuler `ε` uniformes, sommer `K` carrés, compter la fraction sous `τ(K)`.
    ///
    /// **Mesuré le 2026-09-01, K=7 ajouté le 2026-09-02** (200 000 tirages par `K`, graine fixe) :
    ///
    /// | K | τ | fraction sous τ |
    /// |---|---|---|
    /// | 7 | 0,13522 | 0,00423 |
    /// | 8 | 0,18216 | 0,00464 |
    /// | 15 | 0,57852 | 0,00607 |
    /// | 16 | 0,63979 | 0,00615 |
    /// | 32 | 1,68579 | 0,00758 |
    /// | 64 | 3,94617 | 0,00840 |
    ///
    /// Le biais est CONSERVATEUR (moins de faux positifs que le 1 % nominal) et il se referme
    /// quand `K` grandit — signature du TCL, exactement ce qu'on attend d'une somme de variables
    /// asymétriques.
    ///
    /// ⚠️ **`K = 7` est le `k_eff` MINIMAL RÉEL, et c'est lui le cas dimensionnant.** Le tableau
    /// ne s'arrêtait pas là par hasard : il avait été lu comme si le chemin de production
    /// travaillait à `K = 32`, ce qui est faux. Le chemin qui sépare le corpus est celui des blocs
    /// COURTS, bandes 6 à 13 de `SWB_OFFSET_128_48`, larges de 8, 8, 12, 12, 12, 16, 16, 16 — et
    /// `k_eff = actifs − 1` retire encore un degré de liberté (celui consommé par `Δ̂`). Le `k_eff`
    /// réellement soumis à `taus` va donc de **7** ([`MIN_ACTIFS`] = 8, actifs − 1 = 7) à **15**,
    /// jamais 32. C'est exactement le régime où le biais du TCL est le plus fort : le taux de faux
    /// positifs par cellule à connaître avant de calibrer `λ` est **0,42 % à 0,61 %** (lignes 7 et
    /// 15), pas les 0,76 % de la ligne 32 qu'un rapport précédent citait, ni le 1 % nominal.
    ///
    /// **La tolérance dépend de `K`, et c'est le résultat, pas une commodité.** La loi nulle est une
    /// approximation gaussienne (TCL) d'une somme de `K` variables très asymétriques (la densité de
    /// `ε²` diverge en 0). À `K = 7` l'approximation est encore franchement biaisée ; elle se serre
    /// quand `K` grandit. D'où [`MIN_ACTIFS`] à 8, et d'où le fait que ce test MESURE le biais au
    /// lieu de le supposer. Les `K` de 32 et 64 restent tabulés comme témoins de convergence :
    /// c'est leur écart à `K = 7` qui montre que le biais est bien celui du TCL.
    #[test]
    fn monte_carlo_le_seuil_rend_bien_la_fraction_visee() {
        const TIRAGES: usize = 200_000;
        let p = P_CENTILE;
        // 7 = le k_eff minimal du chemin de production (MIN_ACTIFS = 8, moins le degré de liberté
        // de Δ̂). 15 = son maximum (bande courte de 16 coefficients tous actifs).
        let ks = [7usize, 8, 15, 16, 32, 64];
        let taus = thresholds(p, &ks);

        for (i, &k) in ks.iter().enumerate() {
            let mut rng = Lcg(0x5eed_1234_abcd_0001 ^ (k as u64));
            let mut sous = 0usize;
            for _ in 0..TIRAGES {
                let mut e = 0.0f64;
                for _ in 0..k {
                    let eps = rng.suivant() - 0.5;
                    e += eps * eps;
                }
                if e < taus[i] {
                    sous += 1;
                }
            }
            let fraction = sous as f64 / TIRAGES as f64;
            println!(
                "K={k} τ={:.5} fraction mesurée={fraction:.5} (visée {p})",
                taus[i]
            );
            // Bande large ASSUMÉE : elle couvre le biais du TCL à petit K, mesuré ci-dessus. Ce
            // qu'elle attrape et qui compte : un seuil faux d'un facteur (troncature oubliée,
            // variance en 1/12 au lieu de 1/180, quantile du mauvais côté) déplace la fraction
            // d'un ordre de grandeur, pas de 40 %.
            assert!(
                fraction > 0.3 * p && fraction < 3.0 * p,
                "K={k} : fraction {fraction} hors bande autour de {p} (τ={})",
                taus[i]
            );
        }
    }

    /// La troncature n'est pas décorative : à petit `K` elle déplace `τ` de façon mesurable.
    ///
    /// Vérité de référence : le quantile NON tronqué `μ + σΦ⁻¹(p)`, qui est la formule qu'on
    /// écrirait sans y penser. Le test dit qu'elles diffèrent, donc qu'un futur « nettoyage »
    /// supprimant la correction tombe ici.
    ///
    /// **L'assertion fige un SIGNE, pas un écart.** Un `> sans_troncature + 0,01` serait une marge
    /// arbitraire, à re-régler dès que `p` ou `K` bouge. Ce qui est structurel à `K = 4`, c'est que
    /// `μ/σ = 2,24` met 1,25 % de masse gaussienne SOUS ZÉRO, plus que le `p` de 1 % cherché : le
    /// quantile naïf tombe donc de l'autre côté de zéro, ce qu'aucune somme de carrés ne peut
    /// valoir. `sans_troncature < 0 < avec` est la formulation qui dit exactement ça, et elle tombe
    /// pour toute suppression de la correction, pas seulement pour celles qui déplacent τ de 0,01.
    #[test]
    fn la_troncature_deplace_le_seuil_aux_petites_bandes() {
        let k = 4usize;
        let mu = k as f64 / 12.0;
        let sigma = (k as f64 / 180.0).sqrt();
        let sans_troncature = mu + sigma * norm_ppf(P_CENTILE);
        let avec = thresholds(P_CENTILE, &[k])[0];
        assert!(
            sans_troncature < 0.0,
            "K={k} : le quantile naïf {sans_troncature} devrait être NÉGATIF — \
             sans ça le test ne prouve plus rien"
        );
        assert!(
            avec > 0.0,
            "K={k} : τ tronqué {avec} n'est pas strictement positif — \
             une somme de carrés ne peut pas être négative"
        );
    }

    /// Une bande DÉJÀ posée sur la grille doit être reconnue ; la même bande dé-quantifiée non.
    ///
    /// C'est le test d'idempotence lui-même, en miniature et sans codec : on fabrique des
    /// coefficients `X = (q·Δ)^{4/3}` avec des `q` entiers plausibles, on vérifie que la bande
    /// compte comme quantifiée, puis on perturbe les `q` d'une demi-marche et on vérifie qu'elle ne
    /// compte plus. La vérité de référence est la CONSTRUCTION, pas un recalcul du code testé.
    #[test]
    fn une_bande_sur_la_grille_est_reconnue_et_une_bande_hors_grille_non() {
        // Table jouet : une seule bande de 32 coefficients, la fenêtre commence à 0.
        let offsets: &'static [u16] = &[0, 32];
        let taus = thresholds(P_CENTILE, &(0..=64).collect::<Vec<_>>());
        let delta = 0.017f64;

        let mut rng = Lcg(0xabcd_0000_0000_0007);
        // q entiers dans 1..=20 : q_min = 1 est ce que l'estimateur de Δ̂ suppose, et 20 tient sous
        // la porte de bruit (24).
        let qs: Vec<f64> = (0..32)
            .map(|i| {
                if i == 0 {
                    1.0
                } else {
                    1.0 + (rng.suivant() * 19.0).floor()
                }
            })
            .collect();
        let sur_grille: Vec<f64> = qs.iter().map(|q| (q * delta).powf(4.0 / 3.0)).collect();
        assert_eq!(
            frame_likelihood(&sur_grille, offsets, 0, 1, &taus),
            FrameCount {
                sous_tau: 1,
                eligibles: 1
            },
            "une bande posée exactement sur la grille doit compter comme quantifiée"
        );

        // Hors grille : chaque coefficient est déplacé d'un décalage uniforme dans la maille —
        // c'est LITTÉRALEMENT l'hypothèse nulle, pas une perturbation choisie pour faire passer le
        // test. Une demi-marche constante serait piégeuse : (q + ½)/(q_min + ½) retombe sur des
        // entiers pour un tiers des q, et le test mesurerait alors sa propre arithmétique.
        let hors_grille: Vec<f64> = qs
            .iter()
            .map(|q| ((q + 1.0 + rng.suivant() - 0.5) * delta).powf(4.0 / 3.0))
            .collect();
        assert_eq!(
            frame_likelihood(&hors_grille, offsets, 0, 1, &taus),
            FrameCount {
                sous_tau: 0,
                eligibles: 1
            },
            "une bande décalée d'une demi-marche ne doit PAS compter comme quantifiée"
        );
    }

    /// **LE test de la propriété vendue** : un signal PCM réellement posé sur une grille AAC à un
    /// décalage connu doit être retrouvé à ce décalage-là, avec un `L` élevé — et du bruit blanc,
    /// qui n'est passé par aucune grille, doit rester bas.
    ///
    /// Tout le reste du module teste des morceaux : les seuils par Monte-Carlo, une bande isolée
    /// par [`frame_likelihood`], la transformée par équivalence. Rien ne vérifiait la CHAÎNE —
    /// fenêtrage, alignement de trames, repliement/recouvrement, estimation de `Δ̂`, maximum sur
    /// décalages. C'est pourtant elle que la sanity du corpus prétend mesurer, et un décalage
    /// retrouvé sur cinq vrais faux ne dit pas si le mécanisme est celui qu'on croit : il pourrait
    /// être n'importe quelle corrélation avec la façon dont le corpus a été fabriqué. Ici la vérité
    /// de référence est la CONSTRUCTION — on sait où on a mis la grille.
    ///
    /// **Protocole.** Pour chacune des 240 trames courtes, les huit bandes de la fenêtre d'analyse
    /// (6 à 13 de `SWB_OFFSET_128_48`) reçoivent des coefficients `X = ±(q·Δ)^{4/3}` avec `q`
    /// entiers, `q = 1` présent dans chaque bande — c'est exactement ce que `Δ̂ = min v actif`
    /// suppose. Ces coefficients passent par [`imdct`], sont fenêtrés et recouverts-additionnés au
    /// pas de 128 à partir de l'échantillon `D = 47`. Le repliement temporel s'annule d'une trame à
    /// l'autre (propriété tenue par les tests de `mdct`), donc le PCM obtenu re-analysé au décalage
    /// `D` rend EXACTEMENT les coefficients de départ : le signal est authentiquement quantifié,
    /// pas décoré pour ressembler à du quantifié.
    ///
    /// `D = 47` n'est pas décoratif : c'est le décalage que les cinq `aac256` du corpus rendent à
    /// `SIFT_QUANT_SKIP=17`. Le retrouver ici sur un signal dont on connaît la grille est ce qui
    /// autorise à lire le 47 du corpus comme une grille de codec.
    ///
    /// **Mesuré le 2026-09-02 : `L = 1,00000` au décalage 47, contre `L = 0,04688` pour le bruit
    /// témoin** — un facteur 21, et les deux traces sont imprimées à chaque exécution. Les
    /// assertions sont posées à 0,9 et 0,3, donc à un facteur 9 et 6 des valeurs mesurées : assez
    /// larges pour ne pas tomber sur une décimale, assez serrées pour qu'un mécanisme cassé ne
    /// puisse pas passer. À titre de repère, les vrais `aac256` du corpus plafonnent à 0,45 — un
    /// signal synthétique n'a ni psychoacoustique ni bandes mises à zéro, donc `L = 1` y est
    /// attendu et ne dit rien du corpus.
    #[test]
    fn un_signal_pose_sur_une_grille_connue_est_retrouve_a_son_decalage() {
        use crate::analysis::mdct::{imdct, sine_window};

        const N: usize = 128; // bloc court AAC
        const T: usize = 240; // trames synthétisées
        const D: usize = 47; // décalage de la grille dans le PCM
        const SR: u32 = 44_100;
        const DELTA: f64 = 0.05;

        let offsets = swb_offsets(SR, BlockKind::Short).expect("44,1 kHz / court est tabulé");
        let w = sine_window(2 * N);
        let mut rng = Lcg(0x9e37_79b9_7f4a_7c15);

        let mut signal = vec![0.0f32; D + (T + 1) * N];
        // Préfixe : du bruit, pas du silence. Un préfixe muet ferait du décalage D une frontière
        // détectable autrement que par la grille.
        for x in signal[..D].iter_mut() {
            *x = 0.01 * (rng.suivant() as f32 - 0.5);
        }
        for t in 0..T {
            let mut coeffs = vec![0.0f32; N];
            for s in BANDE_DEBUT_COURT..(BANDE_DEBUT_COURT + N_SF) {
                let (lo, hi) = (offsets[s] as usize, offsets[s + 1] as usize);
                for (j, k) in (lo..hi).enumerate() {
                    // q = 1 en tête de chaque bande : c'est l'hypothèse de l'estimateur de Δ̂. Les
                    // autres restent sous 20, donc au-dessus de la porte de bruit (rapport 1/24).
                    let q = if j == 0 {
                        1.0
                    } else {
                        1.0 + (rng.suivant() * 19.0).floor()
                    };
                    let signe = if rng.suivant() < 0.5 { -1.0 } else { 1.0 };
                    coeffs[k] = (signe * (q * DELTA).powf(4.0 / 3.0)) as f32;
                }
            }
            let y = imdct(&coeffs);
            for i in 0..2 * N {
                signal[D + t * N + i] += y[i] * w[i];
            }
        }

        let trace = likelihood(&signal, 1, SR, &[BlockKind::Short], None)
            .into_iter()
            .next()
            .expect("le signal synthétique est assez long pour être mesuré");
        println!(
            "synthétique quantifié : L={:.5} décalage={} (attendu {D})",
            trace.l, trace.decalage
        );
        assert_eq!(
            trace.decalage, D,
            "la grille a été posée au décalage {D}, elle doit y être retrouvée"
        );
        assert!(
            trace.l > 0.9,
            "un signal réellement quantifié doit rendre un L élevé, obtenu {}",
            trace.l
        );

        // CONTRÔLE — du bruit blanc de même longueur, jamais passé par une grille. Sans lui, un
        // `L` élevé ne prouverait rien : il pourrait être ce que ce balayage rend sur n'importe
        // quoi d'assez long.
        let mut rng2 = Lcg(0x0123_4567_89ab_cdef);
        let bruit: Vec<f32> = (0..signal.len())
            .map(|_| (rng2.suivant() as f32 - 0.5) * 0.5)
            .collect();
        let temoin = likelihood(&bruit, 1, SR, &[BlockKind::Short], None)
            .into_iter()
            .next()
            .expect("le bruit témoin a la même longueur, il est mesurable");
        println!("bruit blanc témoin : L={:.5}", temoin.l);
        assert!(
            temoin.l < 0.3,
            "du bruit non quantifié doit rester bas, obtenu {}",
            temoin.l
        );
        assert!(
            trace.l > 3.0 * temoin.l,
            "la séparation quantifié / non quantifié s'est effondrée : {} contre {}",
            trace.l,
            temoin.l
        );
    }

    /// **Une bande dégénérée — trop peu de niveaux de grille distincts — ne compte PAS**, même
    /// quand son `Σε²` passe largement sous `τ`.
    ///
    /// C'est le défaut qui faisait rendre `Fake` au sinus balayé de `fixtures/real_lossless.flac`
    /// (`characterization::real_lossless_flac_is_ok`, 2026-09-02). La MDCT d'un ton pur est un
    /// peigne à deux niveaux : `ε` y est concentré, pas uniforme, donc la loi nulle ne s'applique
    /// pas et `Σε²` passe sous `τ` sans qu'aucun codec ne soit passé par là.
    ///
    /// Le test construit les deux cas à la main, à `Σε²` VOLONTAIREMENT NUL dans les deux — donc
    /// la seule différence est le nombre de niveaux. Sans ça, on ne saurait pas si c'est la
    /// dégénérescence qui est rejetée ou simplement une somme plus grande.
    ///
    /// ⚠️ Il ne remplace pas la caractérisation sur la vraie fixture : celui-ci tient la RÈGLE,
    /// elle tient le SIGNAL. Les deux tombent si la constante repasse à 1 (mesuré).
    #[test]
    fn une_bande_a_trop_peu_de_niveaux_distincts_ne_compte_pas() {
        let offsets: &'static [u16] = &[0, 32];
        let taus = thresholds(P_CENTILE, &(0..=64).collect::<Vec<_>>());
        let delta = 0.017f64;
        // `q` PILE sur la grille dans les deux cas : Σε² = 0, très en dessous de tout τ.
        let bande =
            |qs: &[f64]| -> Vec<f64> { qs.iter().map(|q| (q * delta).powf(4.0 / 3.0)).collect() };

        // Dégénéré : deux niveaux seulement (1 et 5), le peigne d'un ton pur. 32 coefficients,
        // tous actifs (rapport 5 < 24, donc aucun sous la porte).
        let deux_niveaux: Vec<f64> = bande(
            &(0..32)
                .map(|i| if i % 2 == 0 { 5.0 } else { 1.0 })
                .collect::<Vec<f64>>(),
        );
        assert_eq!(
            frame_likelihood(&deux_niveaux, offsets, 0, 1, &taus),
            FrameCount {
                sous_tau: 0,
                eligibles: 0
            },
            "deux niveaux distincts : la bande n'est pas jugeable, donc ni comptee ni eligible"
        );

        // Le MÊME Σε² nul, mais quatre niveaux distincts : jugeable, et reconnue.
        let quatre_niveaux: Vec<f64> = bande(
            &(0..32)
                .map(|i| [1.0, 2.0, 4.0, 5.0][i % 4])
                .collect::<Vec<f64>>(),
        );
        assert_eq!(
            frame_likelihood(&quatre_niveaux, offsets, 0, 1, &taus),
            FrameCount {
                sous_tau: 1,
                eligibles: 1
            },
            "quatre niveaux distincts, meme somme nulle : la bande compte"
        );

        // La borne, au niveau près : trois niveaux ne suffisent pas, quatre suffisent.
        let trois_niveaux: Vec<f64> = bande(
            &(0..32)
                .map(|i| [1.0, 3.0, 5.0][i % 3])
                .collect::<Vec<f64>>(),
        );
        assert_eq!(
            frame_likelihood(&trois_niveaux, offsets, 0, 1, &taus).sous_tau,
            0,
            "trois niveaux : encore sous MIN_NIVEAUX_DISTINCTS = {MIN_NIVEAUX_DISTINCTS}"
        );
    }

    /// **Le plafond de fils ne change RIEN au résultat** — même `L`, même décalage, même canal,
    /// même résolution, de 1 fil à la valeur historique.
    ///
    /// C'est ce qui autorise `analysis::analyze` à passer `Some(2)` pour ne pas sur-souscrire la
    /// machine (8 threads de pool × 16 fils de balayage = 128 fils pour 16 cœurs) sans rien
    /// changer au verdict — et ce qui autorise le harnais `quant_scan` à garder `None`. La
    /// propriété est structurelle : `balaye_decalages` partitionne les décalages entre les fils et
    /// prend un `max` sur les résultats, opération associative et commutative, donc indépendante
    /// du découpage. Ce test le MESURE plutôt que de le supposer — un futur découpage qui
    /// dépendrait de l'ordre (un `fold` avec un état porté, un `first` au lieu d'un `max`)
    /// tomberait ici.
    ///
    /// Le signal est celui, quantifié à une grille connue, du test de bout en bout : il rend un
    /// `L` élevé à un décalage précis, donc un désaccord entre deux découpages se voit. Sur du
    /// bruit plat, plusieurs décalages ex aequo pourraient masquer un vrai bug de partition.
    #[test]
    fn le_nombre_de_fils_ne_change_ni_le_l_ni_le_decalage() {
        use crate::analysis::mdct::{imdct, sine_window};

        const N: usize = 128;
        const T: usize = 120;
        const D: usize = 47;
        const SR: u32 = 44_100;
        const DELTA: f64 = 0.05;

        let offsets = swb_offsets(SR, BlockKind::Short).expect("44,1 kHz / court est tabulé");
        let w = sine_window(2 * N);
        let mut rng = Lcg(0x9e37_79b9_7f4a_7c15);
        let mut signal = vec![0.0f32; D + (T + 1) * N];
        for x in signal[..D].iter_mut() {
            *x = 0.01 * (rng.suivant() as f32 - 0.5);
        }
        for t in 0..T {
            let mut coeffs = vec![0.0f32; N];
            for s in BANDE_DEBUT_COURT..(BANDE_DEBUT_COURT + N_SF) {
                let (lo, hi) = (offsets[s] as usize, offsets[s + 1] as usize);
                for (j, k) in (lo..hi).enumerate() {
                    let q = if j == 0 {
                        1.0
                    } else {
                        1.0 + (rng.suivant() * 19.0).floor()
                    };
                    let signe = if rng.suivant() < 0.5 { -1.0 } else { 1.0 };
                    coeffs[k] = (signe * (q * DELTA).powf(4.0 / 3.0)) as f32;
                }
            }
            let y = imdct(&coeffs);
            for i in 0..2 * N {
                signal[D + t * N + i] += y[i] * w[i];
            }
        }

        let mesure = |fils: Option<usize>| {
            likelihood(&signal, 1, SR, &[BlockKind::Short], fils)
                .into_iter()
                .next()
                .expect("le signal synthétique est mesurable")
        };
        let reference = mesure(Some(1));
        println!(
            "1 fil : L={:.5} décalage={} canal={} résolution={}",
            reference.l,
            reference.decalage,
            reference.canal.label(),
            reference.resolution.label()
        );
        // Contrôle positif : une mesure dégénérée (L nul, ou décalage 0 par défaut) rendrait ce
        // test vert quel que soit le découpage.
        assert!(
            reference.l > 0.9 && reference.decalage == D,
            "la reference elle-meme est fausse : L={} decalage={}",
            reference.l,
            reference.decalage
        );

        for fils in [Some(2), Some(3), Some(8), None] {
            let t = mesure(fils);
            assert_eq!(
                (t.l, t.decalage, t.canal, t.resolution),
                (
                    reference.l,
                    reference.decalage,
                    reference.canal,
                    reference.resolution
                ),
                "resultat different a fils_max={fils:?}"
            );
        }
    }

    /// Les DEUX chemins par lesquels [`likelihood`] ne rend RIEN, tenus séparément.
    ///
    /// « Absence de mesure ≠ valeur par défaut » est la règle du mémo et celle de `verdict()`. Elle
    /// ne vaut que si `None` sort vraiment : un futur repli sur la table 44,1 kHz, ou un `L = 0`
    /// rendu pour un fichier trop court, produirait un chiffre crédible et faux. Rien d'autre ne
    /// couvrait ces deux sorties.
    #[test]
    fn les_deux_absences_de_mesure_rendent_none() {
        let toutes = [BlockKind::Long, BlockKind::Short];

        // 1. Taux non tabulé. Le signal est assez long pour la résolution demandée — donc seul le
        //    taux peut faire échouer la mesure. 88 200 Hz est un taux AAC réel : c'est le cas où un
        //    repli sur la table voisine serait crédible, pas une entrée absurde. Résolution COURTE
        //    seule, pour que le test coûte 8192 MDCT de 256 et non 65 536 de 2048 en debug.
        let assez_long = vec![0.01f32; 4000];
        let courte = [BlockKind::Short];
        assert!(
            likelihood(&assez_long, 1, 88_200, &courte, None).is_empty(),
            "88 200 Hz n'est pas tabulé : la mesure ne doit pas exister"
        );
        // Le même signal, à la même résolution, à un taux tabulé DOIT rendre une mesure — sans quoi
        // le test ci-dessus passerait pour une raison sans rapport avec le taux.
        assert!(
            !likelihood(&assez_long, 1, 44_100, &courte, None).is_empty(),
            "44 100 Hz est tabulé : la mesure doit exister sur un signal assez long"
        );

        // 2. Signal trop court pour un seul groupe. Le seuil est `dispo >= N_F`, avec
        //    `dispo = (n_trames − 3n + 1) / n` : en blocs courts (n = 128) il faut donc au moins
        //    ~1400 échantillons. 1000 les rate pour les deux résolutions.
        let court = vec![0.01f32; 1000];
        assert!(
            likelihood(&court, 1, 44_100, &toutes, None).is_empty(),
            "1000 échantillons ne portent aucun groupe de {N_F} trames"
        );
        // Et le cas dégénéré du bord : aucun échantillon du tout.
        assert!(likelihood(&[], 1, 44_100, &toutes, None).is_empty());
        assert!(likelihood(&[], 2, 44_100, &toutes, None).is_empty());
    }
}

#[cfg(test)]
mod diagnostic {
    use super::*;

    /// Diagnostic PAR BANDE, à décalage forcé — pour voir OÙ la grille est visible et pourquoi
    /// les autres bandes sont écartées, sur toute la table et pas seulement la fenêtre jugée.
    ///
    /// ```text
    /// SIFT_QUANT_DIAG=<fichier> SIFT_QUANT_OFFSET=<d> SIFT_QUANT_RES=long|court
    /// SIFT_QUANT_WIN=sinus|kbd SIFT_QUANT_CANAL=G|D|M|S [SIFT_QUANT_SKIP=17]
    ///   cargo test --release quant_bandes -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore]
    fn quant_bandes() {
        let Ok(path) = std::env::var("SIFT_QUANT_DIAG") else {
            eprintln!("SIFT_QUANT_DIAG non défini — rien à diagnostiquer");
            return;
        };
        let decalage: usize = std::env::var("SIFT_QUANT_OFFSET")
            .ok()
            .and_then(|v| v.parse().ok())
            .expect("SIFT_QUANT_OFFSET requis");
        let kind = match std::env::var("SIFT_QUANT_RES").as_deref() {
            Ok("court") => BlockKind::Short,
            _ => BlockKind::Long,
        };
        let fenetre = match std::env::var("SIFT_QUANT_WIN").as_deref() {
            Ok("kbd") => Fenetre::Kbd,
            _ => Fenetre::Sinus,
        };
        let canal = match std::env::var("SIFT_QUANT_CANAL").as_deref() {
            Ok("D") => Canal::Droite,
            Ok("M") => Canal::Milieu,
            Ok("S") => Canal::Cote,
            _ => Canal::Gauche,
        };
        let saut: usize = std::env::var("SIFT_QUANT_SKIP")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let mut pcm: Vec<f32> = Vec::new();
        let info = crate::analysis::decode::decode_pcm(&path, 2, |b| pcm.extend_from_slice(b))
            .expect("décodage");
        let ch = info.channels.max(1) as usize;
        if saut > 0 && saut * ch < pcm.len() {
            pcm.drain(..saut * ch);
        }
        let n_trames = pcm.len() / ch;
        let signal: Vec<f32> = match canal {
            Canal::Gauche => (0..n_trames).map(|i| pcm[i * ch]).collect(),
            Canal::Droite => (0..n_trames).map(|i| pcm[i * ch + 1]).collect(),
            Canal::Milieu => (0..n_trames)
                .map(|i| 0.5 * (pcm[i * ch] + pcm[i * ch + 1]))
                .collect(),
            Canal::Cote => (0..n_trames)
                .map(|i| 0.5 * (pcm[i * ch] - pcm[i * ch + 1]))
                .collect(),
        };
        let offsets = swb_offsets(info.sample_rate, kind).expect("taux tabulé");
        let n = kind.coeffs();
        let largeurs: Vec<usize> = (0..=n).collect();
        let taus = thresholds(P_CENTILE, &largeurs);
        let w = fenetre.echantillons(kind);
        let dispo = n_trames.saturating_sub(n - 1 + 2 * n) / n;
        let departs: Vec<usize> = (0..GROUPES)
            .map(|g| g * (dispo - N_F) / (GROUPES - 1))
            .collect();
        let plan = MdctFast::new(n);
        let mut trame = vec![0.0f32; 2 * n];
        let mut coeffs = vec![0.0f64; n];
        let nb = offsets.len() - 1;
        // Par bande : [nulle, peu_actifs, peu_niveaux, sans_seuil, jugée, sous_tau, Σactifs, Σniveaux]
        let mut stats = vec![[0usize; 8]; nb];
        let mut trames = 0usize;
        for &depart in &departs {
            for tr in 0..N_F {
                let base = decalage + (depart + tr) * n;
                if base + 2 * n > signal.len() {
                    break;
                }
                trames += 1;
                for i in 0..2 * n {
                    trame[i] = signal[base + i] * w[i];
                }
                plan.transform_f64_into(&trame, &mut coeffs);
                for s in 0..nb {
                    let (lo, hi) = (offsets[s] as usize, offsets[s + 1] as usize);
                    let st = &mut stats[s];
                    match juge_bande(&coeffs, lo, hi, &taus) {
                        Jugement::HorsTrame | Jugement::Nulle => st[0] += 1,
                        Jugement::PeuActifs { actifs } => {
                            st[1] += 1;
                            st[6] += actifs;
                        }
                        Jugement::PeuNiveaux { actifs, niveaux } => {
                            st[2] += 1;
                            st[6] += actifs;
                            st[7] += niveaux;
                        }
                        Jugement::SansSeuil => st[3] += 1,
                        Jugement::Jugee {
                            sous_tau,
                            actifs,
                            niveaux,
                        } => {
                            st[4] += 1;
                            if sous_tau {
                                st[5] += 1;
                            }
                            st[6] += actifs;
                            st[7] += niveaux;
                        }
                    }
                }
            }
        }
        let hz = info.sample_rate as f64 / (2.0 * n as f64);
        println!(
            "fichier={path} canal={} res={} fenetre={} decalage={decalage} trames={trames}",
            canal.label(),
            kind.label(),
            fenetre.label()
        );
        println!("bande  lo..hi   kHz        larg  nulle  peu_act  peu_niv  jugee  sous_tau  act/tr  niv/tr");
        for s in 0..nb {
            let (lo, hi) = (offsets[s] as usize, offsets[s + 1] as usize);
            let st = stats[s];
            let vus = (trames - st[0]).max(1) as f64;
            println!(
                "{s:5}  {lo:4}..{hi:4}  {:5.1}-{:5.1}  {:4}  {:5}  {:7}  {:7}  {:5}  {:8}  {:6.1}  {:6.1}",
                lo as f64 * hz / 1000.0,
                hi as f64 * hz / 1000.0,
                hi - lo,
                st[0],
                st[1],
                st[2],
                st[4],
                st[5],
                st[6] as f64 / vus,
                st[7] as f64 / vus
            );
        }
    }

    /// Sonde de DIAGNOSTIC — imprime, pour un fichier donné, la ventilation par trame du décalage
    /// gagnant : bandes jugeables et bandes sous `τ`. Ne juge rien.
    ///
    /// `SIFT_QUANT_DIAG=<fichier> cargo test --release quant_diag -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn quant_diag() {
        let Ok(path) = std::env::var("SIFT_QUANT_DIAG") else {
            eprintln!("SIFT_QUANT_DIAG non défini — rien à diagnostiquer");
            return;
        };
        let mut pcm: Vec<f32> = Vec::new();
        let info = crate::analysis::decode::decode_pcm(&path, 2, |b| pcm.extend_from_slice(b))
            .expect("décodage");
        let ch = info.channels.max(1) as usize;
        let n_trames = pcm.len() / ch;
        println!(
            "fichier={path}\ncanaux={} taux={} trames={n_trames}",
            info.channels, info.sample_rate
        );

        let traces = likelihood(
            &pcm,
            info.channels,
            info.sample_rate,
            &QUANT_RESOLUTIONS_DIAG,
            None,
        );
        for t in &traces {
            println!(
                "  {} : L={:.5} rapport={:.2} décalage={} canal={} fenêtre={}",
                t.resolution.label(),
                t.l,
                t.l / crate::analysis::verdict::quant_lambda_aac(t.resolution) as f64,
                t.decalage,
                t.canal.label(),
                t.fenetre.label()
            );
        }
        let Some(t) = traces.into_iter().max_by(|a, b| {
            let ra = a.l / crate::analysis::verdict::quant_lambda_aac(a.resolution) as f64;
            let rb = b.l / crate::analysis::verdict::quant_lambda_aac(b.resolution) as f64;
            ra.total_cmp(&rb)
        }) else {
            println!("aucune mesure");
            return;
        };
        println!(
            "GAGNANT L={:.5} décalage={} canal={} résolution={} fenêtre={}",
            t.l,
            t.decalage,
            t.canal.label(),
            t.resolution.label(),
            t.fenetre.label()
        );

        // Rejoue EXACTEMENT le groupe gagnant, trame par trame, pour ventiler le compte.
        let signal: Vec<f32> = match t.canal {
            Canal::Gauche => (0..n_trames).map(|i| pcm[i * ch]).collect(),
            Canal::Droite => (0..n_trames).map(|i| pcm[i * ch + 1]).collect(),
            Canal::Milieu => (0..n_trames)
                .map(|i| 0.5 * (pcm[i * ch] + pcm[i * ch + 1]))
                .collect(),
            Canal::Cote => (0..n_trames)
                .map(|i| 0.5 * (pcm[i * ch] - pcm[i * ch + 1]))
                .collect(),
        };
        let offsets = swb_offsets(info.sample_rate, t.resolution).expect("taux tabulé");
        let n = t.resolution.coeffs();
        let (bande_debut, n_sf) = match t.resolution {
            BlockKind::Long => (BANDE_DEBUT_LONG, N_SF_LONG),
            BlockKind::Short => (BANDE_DEBUT_COURT, N_SF),
        };
        let largeurs: Vec<usize> = (0..=n).collect();
        let taus = thresholds(P_CENTILE, &largeurs);
        let w = t.fenetre.echantillons(t.resolution);
        let dispo = n_trames.saturating_sub(n - 1 + 2 * n) / n;
        let departs: Vec<usize> = (0..GROUPES)
            .map(|g| {
                if GROUPES <= 1 {
                    0
                } else {
                    g * (dispo - N_F) / (GROUPES - 1)
                }
            })
            .collect();

        let plan = MdctFast::new(n);
        let mut trame = vec![0.0f32; 2 * n];
        let mut coeffs = vec![0.0f64; n];
        let mut meilleur_groupe = (0usize, 0usize, 0usize); // (sous_tau, eligibles, depart)
        for &depart in &departs {
            let mut st = 0usize;
            let mut el = 0usize;
            let mut detail = Vec::new();
            for tr in 0..N_F {
                let base = t.decalage + (depart + tr) * n;
                if base + 2 * n > signal.len() {
                    break;
                }
                for i in 0..2 * n {
                    trame[i] = signal[base + i] * w[i];
                }
                plan.transform_f64_into(&trame, &mut coeffs);
                let fc = frame_likelihood(&coeffs, offsets, bande_debut, n_sf, &taus);
                st += fc.sous_tau;
                el += fc.eligibles;
                detail.push((fc.eligibles, fc.sous_tau));
            }
            println!(
                "depart={depart:6}  sous_tau={st:3}  eligibles={el:3}  L={:.5}  par trame (elig/sous_tau) {:?}",
                st as f64 / (N_F * n_sf) as f64,
                detail
            );
            if st > meilleur_groupe.0 {
                meilleur_groupe = (st, el, depart);
            }
        }
        println!(
            "-- groupe gagnant : sous_tau={} eligibles={} depart={}",
            meilleur_groupe.0, meilleur_groupe.1, meilleur_groupe.2
        );

        // HISTOGRAMME des niveaux distincts, sur TOUTES les bandes SOUS-TAU du décalage gagnant
        // (tous groupes, toutes trames). C'est la mesure qui décide de la constante.
        {
            let mut hist_sous_tau = std::collections::BTreeMap::<usize, usize>::new();
            let mut hist_eligibles = std::collections::BTreeMap::<usize, usize>::new();
            for &depart in &departs {
                for tr in 0..N_F {
                    let base = t.decalage + (depart + tr) * n;
                    if base + 2 * n > signal.len() {
                        break;
                    }
                    for i in 0..2 * n {
                        trame[i] = signal[base + i] * w[i];
                    }
                    plan.transform_f64_into(&trame, &mut coeffs);
                    for s in bande_debut..(bande_debut + n_sf).min(offsets.len() - 1) {
                        let (lo, hi) = (offsets[s] as usize, offsets[s + 1] as usize);
                        if hi > coeffs.len() {
                            continue;
                        }
                        let v: Vec<f64> = coeffs[lo..hi]
                            .iter()
                            .map(|x| {
                                let r = x.abs().sqrt().sqrt();
                                r * r * r
                            })
                            .collect();
                        let vmax = v.iter().cloned().fold(0.0f64, f64::max);
                        if vmax <= 0.0 {
                            continue;
                        }
                        let porte = vmax * GATE_REL;
                        let actifs: Vec<f64> = v.iter().cloned().filter(|&x| x > porte).collect();
                        let delta = actifs.iter().cloned().fold(f64::INFINITY, f64::min);
                        if actifs.len() < MIN_ACTIFS || !delta.is_finite() || delta <= 0.0 {
                            continue;
                        }
                        let mut somme = 0.0;
                        let mut niveaux = std::collections::BTreeSet::<i64>::new();
                        for &vi in &actifs {
                            let y = vi / delta;
                            let e = y - y.round();
                            somme += e * e;
                            niveaux.insert(y.round() as i64);
                        }
                        let k_eff = actifs.len() - 1;
                        if k_eff == 0 || k_eff >= taus.len() {
                            continue;
                        }
                        *hist_eligibles.entry(niveaux.len()).or_default() += 1;
                        if somme < taus[k_eff] {
                            *hist_sous_tau.entry(niveaux.len()).or_default() += 1;
                        }
                    }
                }
            }
            println!("HIST niveaux distincts — bandes ELIGIBLES : {hist_eligibles:?}");
            println!("HIST niveaux distincts — bandes SOUS-TAU  : {hist_sous_tau:?}");
        }

        // Détail BANDE PAR BANDE de la première trame du groupe gagnant : c'est le seul niveau où
        // l'on voit POURQUOI `Σε²` passe sous `τ`.
        let depart = meilleur_groupe.2;
        for tr in 0..2usize {
            let base = t.decalage + (depart + tr) * n;
            if base + 2 * n > signal.len() {
                break;
            }
            for i in 0..2 * n {
                trame[i] = signal[base + i] * w[i];
            }
            plan.transform_f64_into(&trame, &mut coeffs);
            println!("--- trame {tr} (depart={depart}) ---");
            for s in bande_debut..(bande_debut + N_SF).min(offsets.len() - 1) {
                let (lo, hi) = (offsets[s] as usize, offsets[s + 1] as usize);
                if hi > coeffs.len() {
                    continue;
                }
                let bande = &coeffs[lo..hi];
                let v: Vec<f64> = bande
                    .iter()
                    .map(|x| {
                        let r = x.abs().sqrt().sqrt();
                        r * r * r
                    })
                    .collect();
                let vmax = v.iter().cloned().fold(0.0f64, f64::max);
                if vmax <= 0.0 {
                    println!("  bande {s}: entierement nulle");
                    continue;
                }
                let porte = vmax * GATE_REL;
                let actifs: Vec<f64> = v.iter().cloned().filter(|&x| x > porte).collect();
                let delta = actifs.iter().cloned().fold(f64::INFINITY, f64::min);
                if actifs.len() < MIN_ACTIFS || !delta.is_finite() {
                    println!("  bande {s}: {} actifs — injugeable", actifs.len());
                    continue;
                }
                let mut somme = 0.0;
                let ys: Vec<f64> = actifs
                    .iter()
                    .map(|&vi| {
                        let y = vi / delta;
                        let e = y - y.round();
                        somme += e * e;
                        y
                    })
                    .collect();
                let k_eff = actifs.len() - 1;
                let tau = taus[k_eff];
                println!(
                    "  bande {s}: actifs={} k_eff={k_eff} vmax={vmax:.4e} delta={delta:.4e} \
                     ratio_dyn={:.1} somme={somme:.5} tau={tau:.5} {}",
                    actifs.len(),
                    vmax / delta,
                    if somme < tau { "SOUS-TAU" } else { "" }
                );
                let apercu: Vec<String> = ys.iter().take(12).map(|y| format!("{y:.3}")).collect();
                println!("      y = [{}]", apercu.join(", "));
            }
        }
    }

    const QUANT_RESOLUTIONS_DIAG: [BlockKind; 2] = [BlockKind::Long, BlockKind::Short];
}

#[cfg(test)]
mod corpus {
    use super::*;

    /// Harnais de mesure — imprime du CSV, ne juge rien. Modèle : `analysis::corpus::corpus_scan`.
    ///
    /// `SIFT_QUANT_DIR=<dossier> cargo test --manifest-path src-tauri/Cargo.toml --release
    ///   quant_scan -- --ignored --nocapture`
    ///
    /// `--release` obligatoire : le balayage fait 1024 décalages × 64 trames × 4 canaux de MDCT par
    /// fichier, et un build debug le compte en dizaines de minutes.
    ///
    /// `SIFT_QUANT_SKIP=<n>` retire `n` échantillons de tête AVANT toute analyse. Ce n'est pas une
    /// commodité : les faux du corpus sont encodés depuis l'échantillon 0, donc leur grille de
    /// trames tombe pile sur le décalage 0 de la recherche. Sonder avec un préfixe IMPAIR est le
    /// seul moyen de savoir si `L` mesure le signal ou la façon dont on a fabriqué le corpus —
    /// c'est le piège nommé par le review du 2026-08-18 et repris par le mémo #52.
    #[test]
    #[ignore]
    fn quant_scan() {
        let Ok(dir) = std::env::var("SIFT_QUANT_DIR") else {
            eprintln!("SIFT_QUANT_DIR non défini — rien à mesurer");
            return;
        };
        let saut: usize = std::env::var("SIFT_QUANT_SKIP")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let resolutions: Vec<BlockKind> = match std::env::var("SIFT_QUANT_RES").as_deref() {
            Ok("long") => vec![BlockKind::Long],
            Ok("court") => vec![BlockKind::Short],
            _ => vec![BlockKind::Long, BlockKind::Short],
        };
        // `SIFT_QUANT_WIN=sinus|kbd` isole une forme de fenêtre ; sans elle, les deux.
        let fenetres: Vec<Fenetre> = match std::env::var("SIFT_QUANT_WIN").as_deref() {
            Ok("sinus") => vec![Fenetre::Sinus],
            Ok("kbd") => vec![Fenetre::Kbd],
            _ => FENETRES.to_vec(),
        };
        // `SIFT_QUANT_BANDES=lo..hi` remplace la fenêtre de bandes des blocs longs (hi exclu).
        let bandes_long: (usize, usize) = std::env::var("SIFT_QUANT_BANDES")
            .ok()
            .and_then(|v| {
                let (lo, hi) = v.split_once("..")?;
                let (lo, hi): (usize, usize) = (lo.parse().ok()?, hi.parse().ok()?);
                (hi > lo).then_some((lo, hi - lo))
            })
            .unwrap_or((BANDE_DEBUT_LONG, N_SF_LONG));

        let mut vus = 0usize;
        let mut rates = 0usize;
        // Le nom de fichier EN DERNIER — piège mesuré le 2026-08-18 sur 967 fichiers d'une vraie
        // clé USB : un « ; » dans un titre décalait toutes les colonnes suivantes. En dernière
        // position il ne peut plus rien déplacer, les champs qui précèdent se lisant par position.
        println!("L;decalage;canal;resolution;fenetre;secondes;fichier");
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
                    rates += 1;
                    // Un échec est une LIGNE, avec le même nombre de colonnes : une ligne plus
                    // courte ferait tomber le nom hors jointure, et un fichier en échec compterait
                    // comme non mesuré.
                    println!("ERREUR;-;-;-;-;-;{name} ({err})");
                    continue;
                }
            };
            let ch = info.channels.max(1) as usize;
            if saut > 0 && saut * ch < pcm.len() {
                pcm.drain(..saut * ch);
            }

            let t0 = std::time::Instant::now();
            let traces = likelihood_reglee(
                &pcm,
                info.channels,
                info.sample_rate,
                &resolutions,
                &fenetres,
                bandes_long,
                None,
            );
            let secondes = t0.elapsed().as_secs_f64();
            if traces.is_empty() {
                rates += 1;
                println!("NON-MESURE;-;-;-;-;{secondes:.1};{name}");
            }
            // Une ligne PAR résolution : leurs `L` ne sont pas sur la même échelle, un maximum
            // entre elles n'aurait pas de sens (voir `BANDE_DEBUT_LONG`).
            for t in traces {
                println!(
                    "{:.5};{};{};{};{};{secondes:.1};{name}",
                    t.l,
                    t.decalage,
                    t.canal.label(),
                    t.resolution.label(),
                    t.fenetre.label()
                );
            }
        }
        println!("-- {vus} fichiers parcourus, {rates} sans mesure (saut={saut} bandes_long={bandes_long:?})");
        assert!(vus > 0, "aucun fichier audio dans {dir} — mesure vide");
    }
}
