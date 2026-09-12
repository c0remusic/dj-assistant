//! MDCT — la transformée dans laquelle les codecs AAC/MP3 quantifient réellement.
//!
//! POURQUOI ce module existe, et ce qu'il ne prétend pas. Le chantier
//! `docs/superpowers/changes/2026-08-17-detecteur-corpus/` a établi par la mesure que l'AAC à
//! débit élevé reste invisible à tout ce qu'on sait faire : `aac256` et `aacmf256` sont ratés 9/10
//! après l'entrée de la platitude dans le verdict. Trois statistiques de « trous » calculées sur
//! notre FFT Hann 4096 ont été essayées et réfutées — 1/10 sur dix sources.
//!
//! La cause probable, écrite dans le review : **notre FFT n'est pas la transformée du codec**. Un
//! encodeur AAC quantifie dans une MDCT à fenêtres 2048/256 avec commutation, à un décalage de
//! trame inconnu ; une FFT d'une autre base, d'une autre fenêtre et d'un autre alignement étale
//! cette structure jusqu'à l'effacer.
//!
//! ⚠️ **Ce module ne détecte rien pour l'instant.** Il porte la transformée, et rien de plus. Que
//! la structure de quantification survive au décodage puis se retrouve depuis un FLAC, c'est
//! l'hypothèse qu'il sert à tester — pas un acquis. Rien ici n'est branché sur `verdict()`.
//!
//! Coût : [`mdct`] et [`MdctPlan`] restent en O(N²), et c'était un choix de sonde assumé — écrire
//! une MDCT rapide d'abord reviendrait à optimiser un chemin dont on ignorait s'il mesure quelque
//! chose. **Levé le 2026-09-01** (mémo #52) : la vraisemblance de quantification de Derrien a
//! besoin de 1024 décalages × trames × canaux × résolutions par fichier, donc [`MdctFast`] existe
//! désormais à côté. `MdctPlan` reste l'ORACLE — c'est lui que les tests de reconstruction tiennent,
//! et c'est contre lui que le chemin rapide est vérifié par équivalence.

/// Fenêtre sinus, celle des blocs longs AAC et MP3.
///
/// `w[n] = sin(pi/(2N) * (n + 0.5))` sur `2N` échantillons. Elle satisfait la condition de
/// Princen-Bradley (`w[n]² + w[n+N]² = 1`), qui est ce qui rend l'annulation du repliement
/// temporel possible — sans elle, la MDCT n'est pas inversible par recouvrement.
pub fn sine_window(two_n: usize) -> Vec<f32> {
    (0..two_n)
        .map(|n| (std::f32::consts::PI / two_n as f32 * (n as f32 + 0.5)).sin())
        .collect()
}

/// Fenêtre de Kaiser-Bessel dérivée (KBD), celle des blocs longs AAC dans la plupart des
/// encodeurs — ISO/IEC 14496-3 § 4.6.11.3.2, avec `α = 4` pour les blocs longs (`2N = 2048`) et
/// `α = 6` pour les courts (`2N = 256`).
///
/// ```text
/// W'[p] = I₀(π α · √(1 − ((2p − N)/N)²))     pour p = 0..=N
/// w[n]  = √( Σ_{p≤n} W'[p] / Σ_{p≤N} W'[p] )   pour n = 0..N,   w[2N − 1 − n] = w[n]
/// ```
///
/// Elle satisfait Princen-Bradley comme le sinus (`w[n]² + w[n+N]² = 1`, tenu par test), donc
/// un flux synthétisé en KBD se ré-analyse EXACTEMENT en KBD — et pas en sinus. C'est toute la
/// raison de son existence ici (2026-09-11) : `ffmpeg` encode ses blocs longs en KBD
/// (`aacpsy.c`, `psy_lame_window` : `window_shape = 1` sauf `LONG_START`, `0` pour les courts),
/// et la sonde de quantification de [`super::quant_trace`], qui n'analysait qu'en sinus, ne
/// pouvait pas retrouver la grille des blocs longs — ce que sa doc constatait sans l'expliquer
/// (« la grille des blocs longs est essentiellement absente après décodage »). Elle n'était pas
/// absente : elle était regardée à travers la mauvaise fenêtre.
///
/// `I₀` par sa série entière `Σ ((x/2)^{2k} / k!²)`, arrêtée quand le terme passe sous `1e-12`
/// du cumul — la même forme que `ff_kbd_window_init`.
pub fn kbd_window(two_n: usize, alpha: f64) -> Vec<f32> {
    let n = two_n / 2;
    let nf = n as f64;
    let i0 = |x: f64| -> f64 {
        let q = x * x / 4.0;
        let mut terme = 1.0f64;
        let mut somme = 1.0f64;
        let mut k = 1.0f64;
        while terme > somme * 1e-12 {
            terme *= q / (k * k);
            somme += terme;
            k += 1.0;
        }
        somme
    };
    let kaiser: Vec<f64> = (0..=n)
        .map(|p| {
            let u = (2.0 * p as f64 - nf) / nf;
            i0(std::f64::consts::PI * alpha * (1.0 - u * u).max(0.0).sqrt())
        })
        .collect();
    let total: f64 = kaiser.iter().sum();
    let mut w = vec![0.0f32; two_n];
    let mut cumul = 0.0f64;
    for (i, k) in kaiser.iter().enumerate().take(n) {
        cumul += k;
        let v = (cumul / total).sqrt() as f32;
        w[i] = v;
        w[two_n - 1 - i] = v;
    }
    w
}

/// MDCT d'une trame de `2N` échantillons vers `N` coefficients.
///
/// `X[k] = Σ x[n] · cos(π/N · (n + 1/2 + N/2) · (k + 1/2))`
///
/// Le décalage `N/2` dans la phase n'est pas cosmétique : c'est lui qui place le repliement
/// temporel aux bons endroits, donc c'est lui qui permet à deux trames voisines de se reconstruire
/// mutuellement.
///
/// Panique si `frame.len()` est impair — une trame MDCT a toujours une longueur paire par
/// définition, et un appelant qui n'en fournit pas une s'est trompé de découpe.
pub fn mdct(frame: &[f32]) -> Vec<f32> {
    assert!(
        frame.len() % 2 == 0,
        "trame MDCT de longueur impaire : {}",
        frame.len()
    );
    let n = frame.len() / 2;
    let nf = n as f32;
    (0..n)
        .map(|k| {
            let mut acc = 0.0f32;
            for (idx, &x) in frame.iter().enumerate() {
                let phase =
                    std::f32::consts::PI / nf * (idx as f32 + 0.5 + nf / 2.0) * (k as f32 + 0.5);
                acc += x * phase.cos();
            }
            acc
        })
        .collect()
}

/// MDCT à base précalculée — même transformée que [`mdct`], sans les appels à `cos()`.
///
/// La sonde d'alignement demande la MDCT de la MÊME taille des milliers de fois : pour chaque
/// trame et chaque décalage candidat. [`mdct`] recalcule alors `2N × N` cosinus à chaque appel —
/// 2 millions pour N = 1024 — et c'est là que passe tout le temps, pas dans les multiplications.
///
/// La base coûte `2N × N` flottants en mémoire, soit 8 Mo pour N = 1024. Acceptable pour une
/// sonde ; ce n'est toujours PAS une MDCT rapide (une vraie se factorise par une FFT complexe de
/// N/2 points), et ça reste O(N²) par trame.
pub struct MdctPlan {
    n: usize,
    /// `n × 2n`, rangée `k` = les coefficients de la ligne `k` de la base.
    basis: Vec<f32>,
}

impl MdctPlan {
    pub fn new(n: usize) -> Self {
        let nf = n as f32;
        let mut basis = Vec::with_capacity(n * 2 * n);
        for k in 0..n {
            for idx in 0..2 * n {
                let phase =
                    std::f32::consts::PI / nf * (idx as f32 + 0.5 + nf / 2.0) * (k as f32 + 0.5);
                basis.push(phase.cos());
            }
        }
        Self { n, basis }
    }

    /// Panique si la trame ne fait pas `2N` — une trame d'une autre taille n'est pas un cas
    /// dégradé à rattraper, c'est un appelant qui s'est trompé de plan.
    pub fn transform(&self, frame: &[f32]) -> Vec<f32> {
        assert_eq!(
            frame.len(),
            2 * self.n,
            "trame de {} pour un plan a {}",
            frame.len(),
            2 * self.n
        );
        (0..self.n)
            .map(|k| {
                let row = &self.basis[k * 2 * self.n..(k + 1) * 2 * self.n];
                row.iter().zip(frame).map(|(b, x)| b * x).sum()
            })
            .collect()
    }
}

/// MDCT rapide — même transformée que [`MdctPlan`], en `O(N log N)` au lieu de `O(N²)`.
///
/// **Pourquoi maintenant (2026-09-01, mémo #52).** La sonde d'alignement se contentait du plan
/// `O(N²)` parce qu'elle ne mesurait qu'une poignée de fichiers. La vraisemblance de quantification
/// de Derrien, elle, balaie 1024 décalages × N_f trames × 4 canaux × 2 résolutions par fichier : au
/// budget `MdctPlan`, c'est des heures par piste. C'est le bloqueur d'implémentation nommé par le
/// mémo, et rien d'autre.
///
/// **`MdctPlan` reste l'ORACLE.** Il est tenu par le test de reconstruction (repliement temporel
/// annulé sur la moitié commune), qui est la seule propriété qui distingue une vraie MDCT d'une
/// somme de cosinus au terme de phase faux. Ce chemin-ci est vérifié PAR ÉQUIVALENCE contre lui, et
/// le test de reconstruction est rejoué dessus.
///
/// **Le chemin, en trois étages** (dérivation refaite à la main, pas recopiée) :
///
/// 1. *Repliement* `2N → N`. Le noyau `cos(π/N·(m+½)(k+½))`, où `m = n + N/2`, est antisymétrique
///    autour de `m = N − ½` et antipériodique de période `2N`. Replier les indices hors `[0, N)`
///    avec ces deux signes ramène la MDCT à une DCT-IV de taille `N` sur le signal replié `u`.
/// 2. *DCT-IV par FFT complexe de `N/2` points*. Avec `M = N/2` et
///    `w[p] = (u[2p] + i·u[N−1−2p])·exp(−iπ(8p+1)/(8N))`, `Z = FFT_M(w)`,
///    `r[q] = Z[q]·exp(−iπ(8q+1)/(8N))`, on a `C[2q] = Re r[q]` et `C[N−1−2q] = −Im r[q]`.
///    Les deux demi-tournants `+½` ne sont pas décoratifs : ils compensent le terme croisé
///    `(4p+1)(4q+1)` que la FFT seule ne produit pas. Une répartition naïve
///    `exp(−iπ(4p+1)/(4N))` des deux côtés laisse une phase constante `e^{−iπ/(4N)}` — mesurable,
///    et c'est exactement ce que le test d'équivalence attrape.
/// 3. *Rangement* des sorties paires/impaires depuis parties réelle et imaginaire.
///
/// **Accumulation en `f64`.** L'oracle somme `2N` termes en `f32` ; ce chemin-ci est donc plus
/// PRÉCIS que sa propre référence, et la tolérance du test d'équivalence est dominée par l'erreur
/// de l'oracle, pas par la sienne. Ce n'est pas de la coquetterie : la vraisemblance de
/// quantification teste si `|X|^{3/4}/Δ` tombe près d'un entier, avec des quotients qui montent à
/// plusieurs milliers dans les bandes graves — `f32` y perdrait le chiffre qui décide.
pub struct MdctFast {
    n: usize,
    fft: std::sync::Arc<dyn rustfft::Fft<f64>>,
    /// `exp(−iπ(8j+1)/(8N))` pour `j = 0..N/2` — le même facteur sert avant et après la FFT.
    twiddle: Vec<rustfft::num_complex::Complex<f64>>,
    /// Signal replié `u`, réutilisé d'un appel à l'autre pour ne pas réallouer par trame.
    scratch: std::cell::RefCell<(Vec<f64>, Vec<rustfft::num_complex::Complex<f64>>)>,
}

impl MdctFast {
    /// Panique si `n` n'est pas pair : le repliement coupe la trame en quarts de `N/2`, et un `N`
    /// impair n'est pas un cas dégradé à rattraper — c'est un appelant qui s'est trompé de taille.
    pub fn new(n: usize) -> Self {
        assert!(n % 2 == 0, "MDCT rapide : N doit être pair, reçu {n}");
        let m = n / 2;
        let fft = rustfft::FftPlanner::<f64>::new().plan_fft_forward(m);
        let nf = n as f64;
        let twiddle = (0..m)
            .map(|j| {
                let a = -std::f64::consts::PI * (8.0 * j as f64 + 1.0) / (8.0 * nf);
                rustfft::num_complex::Complex::new(a.cos(), a.sin())
            })
            .collect();
        Self {
            n,
            fft,
            twiddle,
            scratch: std::cell::RefCell::new((
                vec![0.0; n],
                vec![rustfft::num_complex::Complex::new(0.0, 0.0); m],
            )),
        }
    }

    /// Coefficients en `f64` — la précision dont [`super::quant_trace`] a besoin pour décider si
    /// `|X|^{3/4}/Δ` tombe près d'un entier.
    ///
    /// **Alloue son `Vec` de sortie à chaque appel**, et c'est pourquoi le balayage de
    /// [`super::quant_trace`] appelle [`MdctFast::transform_f64_into`] à la place : dans une boucle
    /// de 1024 décalages × 8 trames × 8 groupes, cette allocation-là est la seule qui reste par
    /// trame, ce que la doc du champ `scratch` (« pour ne pas réallouer par trame ») promettait
    /// déjà de ne pas faire. Cette forme-ci existe pour les appelants ponctuels — tests, oracle.
    ///
    /// Panique si la trame ne fait pas `2N`, exactement comme [`MdctPlan::transform`].
    pub fn transform_f64(&self, frame: &[f32]) -> Vec<f64> {
        let mut out = vec![0.0f64; self.n];
        self.transform_f64_into(frame, &mut out);
        out
    }

    /// Même transformée que [`MdctFast::transform_f64`], écrite dans un tampon fourni.
    ///
    /// `out` doit faire exactement `N` : le chemin écrit chacune de ses cases, il n'a donc rien à
    /// remettre à zéro, mais une taille différente est un appelant qui s'est trompé de plan — même
    /// contrat que la trame.
    pub fn transform_f64_into(&self, frame: &[f32], out: &mut [f64]) {
        let n = self.n;
        assert_eq!(
            frame.len(),
            2 * n,
            "trame de {} pour un plan a {}",
            frame.len(),
            2 * n
        );
        assert_eq!(out.len(), n, "sortie de {} pour un plan a {n}", out.len());
        let m = n / 2;
        let mut borrow = self.scratch.borrow_mut();
        let (u, buf) = &mut *borrow;

        // ÉTAGE 1 — repliement 2N → N. Les trois plages viennent des trois cas du noyau :
        // dans la fenêtre (signe +), replié par antisymétrie autour de N − ½ (signe −), replié par
        // antipériodicité 2N (signe −).
        u[..n].fill(0.0);
        let h = n / 2;
        for (i, &x) in frame.iter().enumerate() {
            let x = x as f64;
            if i < h {
                u[i + h] += x;
            } else if i < n + h {
                u[n + h - 1 - i] -= x;
            } else {
                u[i - n - h] -= x;
            }
        }

        // ÉTAGE 2 — DCT-IV par FFT complexe de M points.
        for p in 0..m {
            let z = rustfft::num_complex::Complex::new(u[2 * p], u[n - 1 - 2 * p]);
            buf[p] = z * self.twiddle[p];
        }
        self.fft.process(buf);

        // ÉTAGE 3 — rangement. Les indices `2q` et `n−1−2q` parcourent tout `0..n` sans trou ni
        // doublon (pairs croissants, impairs décroissants), donc `out` est entièrement réécrit et
        // un tampon réutilisé ne peut pas porter de reste de la trame précédente.
        for q in 0..m {
            let r = buf[q] * self.twiddle[q];
            out[2 * q] = r.re;
            out[n - 1 - 2 * q] = -r.im;
        }
    }

    /// Même sortie que [`MdctPlan::transform`], au format de l'oracle.
    pub fn transform(&self, frame: &[f32]) -> Vec<f32> {
        self.transform_f64(frame)
            .into_iter()
            .map(|v| v as f32)
            .collect()
    }
}

/// IMDCT — `N` coefficients vers `2N` échantillons, à recouvrir-additionner avec la trame voisine.
///
/// Seule ne rend PAS le signal : chaque moitié porte un repliement temporel que seule la trame
/// suivante annule. C'est le principe même de la transformée, et c'est ce que le test de
/// reconstruction vérifie.
///
/// Existe pour ce test, justement : sans inverse, rien ne distingue une MDCT correcte d'une
/// somme de cosinus au terme de phase faux — les deux « localisent » un sinus dans le bon bin.
pub fn imdct(coeffs: &[f32]) -> Vec<f32> {
    let n = coeffs.len();
    let nf = n as f32;
    (0..2 * n)
        .map(|idx| {
            let mut acc = 0.0f32;
            for (k, &c) in coeffs.iter().enumerate() {
                let phase =
                    std::f32::consts::PI / nf * (idx as f32 + 0.5 + nf / 2.0) * (k as f32 + 0.5);
                acc += c * phase.cos();
            }
            acc * 2.0 / nf
        })
        .collect()
}

/// Sonde d'alignement — le harnais de recherche, pas un détecteur.
///
/// L'idée testée, et pourquoi elle diffère des trois formulations déjà réfutées : celles-là
/// mesuraient une sparsité ABSOLUE, et le review a conclu que « les valeurs absolues sont pilotées
/// par le matériau, pas par le codec » — un morceau clairsemé l'est sur toutes les mesures, faux
/// ou pas. Ici on mesure le CONTRASTE entre le meilleur décalage de trame et le décalage médian.
///
/// Ce que ça change : un master n'a aucune raison d'avoir un alignement privilégié, donc son
/// contraste doit rester proche de 1 quel que soit son matériau. Un fichier passé par un encodeur
/// AAC a été quantifié sur UNE grille, et si quelque chose en survit, ça doit apparaître à un
/// décalage et pas aux autres. Le rapport élimine le niveau absolu, donc le matériau.
///
/// ⚠️ Hypothèse, pas résultat. Rien ne dit que la structure survive au décodage puis au ré-encodage
/// FLAC.
///
/// **Ce qui est mesuré (2026-08-18) : le pic est étroit.** Sur une grille de 32, 10 sources ×
/// {aac256, aacmf256} donnent 17/20 au-dessus du plus haut authentique — mais en décalant l'entrée
/// de 17 échantillons, ce taux tombe à 5/20 et le décalage retenu cesse d'être 0. Le pic fait donc
/// moins de ±16 échantillons de large : une grille grossière ne le trouve QUE sur des fichiers
/// fabriqués depuis l'échantillon 0, c'est-à-dire les nôtres. D'où le repérage au pas de 1.
#[cfg(test)]
mod sonde {
    use super::*;

    const N: usize = 1024; // bloc long AAC : fenêtre 2048, 1024 coefficients
    /// Pas de la grille de RÉFÉRENCE, celle qui donne la médiane. Elle n'a pas besoin d'être fine :
    /// elle sert à établir le niveau ordinaire, pas à trouver un pic.
    const PAS_REFERENCE: usize = 32;
    /// Trames du repérage — peu, parce qu'il tourne sur les 1024 décalages.
    const TRAMES_REPERAGE: usize = 8;
    const TRAMES: usize = 60;
    const BANDE_LO_HZ: f32 = 8000.0;
    const BANDE_HI_HZ: f32 = 16000.0;
    /// Un coefficient compte comme « creux » sous ce rapport à la RMS de sa bande dans sa trame.
    const SEUIL_CREUX: f32 = 0.1;

    fn decode_mono(path: &str) -> Result<(Vec<f32>, u32), String> {
        let mut pcm = Vec::new();
        let info = crate::analysis::decode::decode_pcm(path, 1, |b| pcm.extend_from_slice(b))?;
        // `SIFT_MDCT_SKIP` retire des échantillons de tête AVANT toute analyse, et ce n'est pas
        // une commodité : les faux du corpus sont encodés depuis l'échantillon 0, donc leur grille
        // de trames tombe pile sur un point de la grille de recherche. Un fichier réel rogné, lui,
        // a un alignement quelconque. Sonder avec un décalage premier avec `PAS_DECALAGE` est le
        // seul moyen de savoir si le contraste mesuré est une propriété du signal ou un cadeau de
        // la façon dont on a fabriqué le corpus.
        let skip: usize = std::env::var("SIFT_MDCT_SKIP")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        if skip > 0 && skip < pcm.len() {
            pcm.drain(..skip);
        }
        Ok((pcm, info.sample_rate))
    }

    /// Fraction moyenne de coefficients creux dans la bande, sur `TRAMES` trames réparties.
    fn creux_au_decalage(
        pcm: &[f32],
        plan: &MdctPlan,
        w: &[f32],
        decalage: usize,
        bins: (usize, usize),
        n_max: usize,
    ) -> f32 {
        let (lo, hi) = bins;
        let dispo = (pcm.len().saturating_sub(decalage + 2 * N)) / N;
        if dispo == 0 {
            return f32::NAN;
        }
        let pas = (dispo / n_max).max(1);
        let mut somme = 0.0f32;
        let mut n_trames = 0usize;
        let mut trame = vec![0.0f32; 2 * N];
        for f in (0..dispo).step_by(pas).take(n_max) {
            let debut = decalage + f * N;
            for i in 0..2 * N {
                trame[i] = pcm[debut + i] * w[i];
            }
            let x = plan.transform(&trame);
            let bande = &x[lo..hi];
            let rms = (bande.iter().map(|v| v * v).sum::<f32>() / bande.len() as f32).sqrt();
            if rms <= f32::EPSILON {
                continue; // trame silencieuse : tout y est « creux », ça ne dit rien du codec
            }
            let creux = bande.iter().filter(|v| v.abs() < SEUIL_CREUX * rms).count();
            somme += creux as f32 / bande.len() as f32;
            n_trames += 1;
        }
        if n_trames == 0 {
            f32::NAN
        } else {
            somme / n_trames as f32
        }
    }

    /// `SIFT_MDCT_PATHS="a.flac;b.flac" cargo test --manifest-path src-tauri/Cargo.toml --release
    ///   mdct_alignment -- --ignored --nocapture`
    ///
    /// `--release` obligatoire : la transformée reste en O(N²), et un fichier coûte 32 décalages ×
    /// 60 trames × 2,1 M multiplications.
    #[test]
    #[ignore]
    fn mdct_alignment() {
        let Ok(liste) = std::env::var("SIFT_MDCT_PATHS") else {
            eprintln!("SIFT_MDCT_PATHS non défini — rien à sonder");
            return;
        };
        let plan = MdctPlan::new(N);
        let w = sine_window(2 * N);
        println!("fichier;sr;meilleur_decalage;creux_max;creux_median;contraste");
        let mut vus = 0usize;
        for path in liste.split(';').filter(|p| !p.trim().is_empty()) {
            let (pcm, sr) = match decode_mono(path.trim()) {
                Ok(v) => v,
                Err(e) => {
                    // Un échec est une ligne, pas un silence.
                    println!("{};ERREUR;-;-;-;{e}", nom(path));
                    continue;
                }
            };
            vus += 1;
            let hz_par_bin = sr as f32 / (2.0 * N as f32);
            let lo = (BANDE_LO_HZ / hz_par_bin).ceil() as usize;
            let hi = ((BANDE_HI_HZ / hz_par_bin).floor() as usize).min(N);
            // ÉTAGE 1 — repérage, tous les décalages, peu de trames.
            //
            // Le pas de 1 n'est pas du zèle : mesuré le 2026-08-18, décaler l'entrée de 17
            // échantillons fait tomber le taux de 17/20 à 5/20 sur une grille de 32. Le pic est
            // donc plus étroit que ±16, et une grille grossière ne le trouve QUE sur des fichiers
            // fabriqués depuis l'échantillon 0 — c'est-à-dire les nôtres, et pas ceux d'un DJ.
            let grossier: Vec<f32> = (0..N)
                .map(|d| creux_au_decalage(&pcm, &plan, &w, d, (lo, hi), TRAMES_REPERAGE))
                .collect();
            let Some((decalage_max, _)) = grossier
                .iter()
                .enumerate()
                .filter(|(_, v)| v.is_finite())
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            else {
                println!("{};{sr};ERREUR;-;-;aucune trame", nom(path));
                continue;
            };

            // ÉTAGE 2 — mesure. Le maximum ET la médiane se recalculent avec le MÊME nombre de
            // trames, sinon le rapport compare une estimation fine à une estimation grossière et
            // le contraste devient un artefact du protocole.
            let max = creux_au_decalage(&pcm, &plan, &w, decalage_max, (lo, hi), TRAMES);
            let mut valeurs: Vec<f32> = (0..N)
                .step_by(PAS_REFERENCE)
                .map(|d| creux_au_decalage(&pcm, &plan, &w, d, (lo, hi), TRAMES))
                .filter(|v| v.is_finite())
                .collect();
            if !max.is_finite() || valeurs.is_empty() {
                println!("{};{sr};ERREUR;-;-;aucune trame", nom(path));
                continue;
            }
            valeurs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let median = valeurs[valeurs.len() / 2];
            println!(
                "{};{sr};{decalage_max};{max:.4};{median:.4};{:.4}",
                nom(path),
                max / median
            );
        }
        assert!(vus > 0, "aucun fichier sondé — mesure vide");
    }

    fn nom(path: &str) -> &str {
        path.rsplit(['/', '\\']).next().unwrap_or(path)
    }
}

#[cfg(test)]
mod tests {
    /// La KBD est une fenêtre MDCT au sens de Princen-Bradley, symétrique, croissante sur sa
    /// première moitié — sinon rien de ce que [`kbd_window`] promet ne tient.
    #[test]
    fn la_kbd_satisfait_princen_bradley() {
        for (two_n, alpha) in [(2048usize, 4.0f64), (256, 6.0)] {
            let w = super::kbd_window(two_n, alpha);
            let n = two_n / 2;
            for i in 0..n {
                let pb = w[i] * w[i] + w[i + n] * w[i + n];
                assert!((pb - 1.0).abs() < 1e-5, "PB en {i} : {pb}");
                assert_eq!(w[i], w[two_n - 1 - i], "symétrie en {i}");
                if i > 0 {
                    assert!(w[i] >= w[i - 1], "non croissante en {i}");
                }
            }
            assert!(
                w[0] < 0.01 && w[n - 1] > 0.99,
                "bords {} {}",
                w[0],
                w[n - 1]
            );
        }
    }

    /// Ce que la sonde de quantification exige de la fenêtre : un flux synthétisé (IMDCT +
    /// recouvrement) dans une forme se ré-analyse EXACTEMENT dans la même forme, et PAS dans
    /// l'autre. Sinus→sinus et KBD→KBD rendent les coefficients à `1e-4` près ; KBD→sinus s'en
    /// écarte d'un facteur mesurable. C'est l'idempotence que `quant_trace` teste, et la raison
    /// pour laquelle le balayage essaie les deux formes.
    #[test]
    fn une_synthese_kbd_ne_se_reanalyse_quen_kbd() {
        const N: usize = 128;
        let sinus = super::sine_window(2 * N);
        let kbd = super::kbd_window(2 * N, 6.0);
        // Signal pseudo-aléatoire déterministe, 4 trames de recouvrement.
        let mut x = 0x2545_F491u32;
        let signal: Vec<f32> = (0..5 * N)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                (x as f32 / u32::MAX as f32) - 0.5
            })
            .collect();
        let ecart = |synthese: &[f32], analyse: &[f32]| -> f32 {
            // Synthèse : MDCT fenêtrée de chaque trame, IMDCT, refenêtrage, recouvrement-addition.
            let mut out = vec![0.0f32; 5 * N];
            let mut coeffs_ref: Vec<Vec<f32>> = Vec::new();
            for t in 0..4 {
                let trame: Vec<f32> = (0..2 * N)
                    .map(|i| signal[t * N + i] * synthese[i])
                    .collect();
                let c = super::mdct(&trame);
                let y = super::imdct(&c);
                for i in 0..2 * N {
                    out[t * N + i] += y[i] * synthese[i];
                }
                coeffs_ref.push(c);
            }
            // Ré-analyse de la trame centrale (recouvrement complet des deux côtés).
            let t = 1;
            let trame: Vec<f32> = (0..2 * N).map(|i| out[t * N + i] * analyse[i]).collect();
            let c = super::mdct(&trame);
            let num: f32 = c
                .iter()
                .zip(&coeffs_ref[t])
                .map(|(a, b)| (a - b) * (a - b))
                .sum();
            let den: f32 = coeffs_ref[t].iter().map(|b| b * b).sum();
            (num / den).sqrt()
        };
        let ss = ecart(&sinus, &sinus);
        let kk = ecart(&kbd, &kbd);
        let ks = ecart(&kbd, &sinus);
        assert!(ss < 1e-4, "sinus→sinus {ss}");
        assert!(kk < 1e-4, "KBD→KBD {kk}");
        assert!(ks > 1e-2, "KBD→sinus devrait s'écarter, mesuré {ks}");
    }

    use super::*;

    /// **La propriété qui pinne vraiment une MDCT** : deux trames voisines se reconstruisent
    /// mutuellement, exactement.
    ///
    /// Ce que ça attrape et que le test de localisation ne peut pas : le terme de phase. Une somme
    /// de cosinus dont le décalage `N/2` serait faux range TOUJOURS un sinus dans le bon bin — elle
    /// ne reconstruit simplement rien. La vérité de référence ici est le signal d'entrée lui-même,
    /// pas une valeur recalculée comme le code la calcule.
    ///
    /// Le signal est déterministe (un générateur congruentiel écrit sur place) : un test qui
    /// échouerait une fois sur mille selon le tirage ne serait pas un test.
    #[test]
    fn deux_trames_voisines_reconstruisent_le_signal() {
        const N: usize = 64;

        let mut graine = 12345u32;
        let signal: Vec<f32> = (0..3 * N)
            .map(|_| {
                graine = graine.wrapping_mul(1664525).wrapping_add(1013904223);
                (graine >> 16) as f32 / 32768.0 - 1.0
            })
            .collect();

        let w = sine_window(2 * N);
        let analyse = |offset: usize| {
            let trame: Vec<f32> = (0..2 * N).map(|i| signal[offset + i] * w[i]).collect();
            let y = imdct(&mdct(&trame));
            y.iter()
                .zip(w.iter())
                .map(|(v, wi)| v * wi)
                .collect::<Vec<f32>>()
        };

        let a = analyse(0);
        let b = analyse(N);

        // Le segment [N, 2N) est le seul que DEUX trames couvrent — c'est là, et seulement là, que
        // le repliement s'annule.
        for i in 0..N {
            let reconstruit = a[N + i] + b[i];
            assert!(
                (reconstruit - signal[N + i]).abs() < 1e-3,
                "échantillon {i} : {reconstruit} contre {} attendu",
                signal[N + i]
            );
        }
    }

    /// La base précalculée doit rendre EXACTEMENT ce que rend la transformée directe.
    ///
    /// Vérité de référence : [`mdct`], déjà tenue par le test de reconstruction. C'est le seul cas
    /// de ce module où recalculer la même chose est légitime — c'est précisément la question posée,
    /// « ces deux chemins sont-ils le même ».
    ///
    /// Tolérance relative à l'amplitude des coefficients, pas absolue : la somme se fait dans un
    /// ordre différent, donc les arrondis f32 diffèrent, et un seuil absolu serait soit trop lâche
    /// sur les petits coefficients soit trop serré sur les grands.
    #[test]
    fn la_base_precalculee_rend_la_meme_chose_que_la_directe() {
        const N: usize = 32;

        let mut graine = 999u32;
        let frame: Vec<f32> = (0..2 * N)
            .map(|_| {
                graine = graine.wrapping_mul(1664525).wrapping_add(1013904223);
                (graine >> 16) as f32 / 32768.0 - 1.0
            })
            .collect();

        let attendu = mdct(&frame);
        let obtenu = MdctPlan::new(N).transform(&frame);
        let echelle = attendu.iter().fold(0.0f32, |m, v| m.max(v.abs()));

        for (k, (a, b)) in attendu.iter().zip(&obtenu).enumerate() {
            assert!(
                (a - b).abs() < 1e-4 * echelle,
                "coefficient {k} : {b} contre {a}"
            );
        }
    }

    /// Générateur congruentiel — même recette que les autres tests du module : un test qui
    /// échouerait une fois sur mille selon le tirage ne serait pas un test.
    fn bruit(graine: u32, n: usize) -> Vec<f32> {
        let mut g = graine;
        (0..n)
            .map(|_| {
                g = g.wrapping_mul(1664525).wrapping_add(1013904223);
                (g >> 16) as f32 / 32768.0 - 1.0
            })
            .collect()
    }

    /// **Équivalence MDCT rapide ↔ oracle** (2026-09-01, mémo #52 — livrable 1).
    ///
    /// La vérité de référence est [`MdctPlan`], lui-même tenu par le test de reconstruction : la
    /// question posée ici est exactement « ces deux chemins sont-ils la même transformée », donc
    /// recalculer la même chose est légitime, comme pour la base précalculée.
    ///
    /// **Tolérance justifiée.** L'oracle somme `2N` produits en `f32` ; l'erreur d'arrondi d'une
    /// telle somme croît en `√(2N)·ε_f32`, soit `√512 · 1,19e-7 ≈ 2,7e-6` d'écart TYPE relatif à
    /// l'amplitude des coefficients, et le pire des `N = 256` coefficients tire la queue de cette
    /// loi. **Mesuré le 2026-09-02 : 5,50e-5** — la trace imprimée plus bas donne le chiffre à
    /// chaque exécution, et c'est LUI qu'un rapport cite, pas la borne. Le chemin rapide accumule
    /// en `f64` : sa propre erreur est quatre ordres de grandeur plus bas, donc ce qu'on mesure ici
    /// est l'erreur de l'ORACLE et rien d'autre. La borne `2e-4 · max|X|` lui laisse un facteur
    /// 3,6 — assez pour qu'un autre jeu d'arrondis ne la fasse pas tomber, et **quinze fois** sous
    /// l'erreur qu'un terme de phase faux produirait (~3e-3, mesuré en mutant le tournant : 3e-3 /
    /// 2e-4 = 15, soit un ordre de grandeur, pas deux — le chiffre affiché ici disait « deux ordres »
    /// et ne suivait pas de sa propre division). Elle est relative et non
    /// absolue pour la même raison que le test de la base précalculée : un seuil absolu serait trop
    /// lâche sur les petits coefficients et trop serré sur les grands.
    ///
    /// N = 256 et non 1024 : `MdctPlan::new(1024)` alloue 8 Mo et coûte 2 M cosinus, ce qui n'a pas
    /// sa place dans la suite normale. Le repliement et les tournants ne dépendent pas de N.
    #[test]
    fn la_mdct_rapide_rend_la_meme_chose_que_loracle() {
        const N: usize = 256;

        let frame = bruit(4242, 2 * N);
        let attendu = MdctPlan::new(N).transform(&frame);
        let obtenu = MdctFast::new(N).transform(&frame);
        let echelle = attendu.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(echelle > 0.0, "trame de test dégénérée");

        let mut pire = 0.0f32;
        for (k, (a, b)) in attendu.iter().zip(&obtenu).enumerate() {
            let ecart = (a - b).abs() / echelle;
            pire = pire.max(ecart);
            assert!(
                ecart < 2e-4,
                "coefficient {k} : {b} contre {a} (écart relatif {ecart:e})"
            );
        }
        // Trace lisible : c'est le chiffre qu'un rapport doit citer, pas la borne.
        println!("écart relatif maximal MdctFast ↔ MdctPlan (N={N}) : {pire:e}");
    }

    /// Équivalence rapide ↔ oracle à un SECOND `N`, choisi pour que `M = N/2` soit IMPAIR.
    ///
    /// Le test ci-dessus prend `N = 256`, donc `M = 128`, donc une FFT en radix-2 pur. C'est le cas
    /// le plus favorable et le seul couvert : ni le repliement (qui coupe en quarts de `N/2`), ni le
    /// rangement pair/impair, ni le planificateur de `rustfft` ne sont exercés hors puissance de
    /// deux. `N = 510` donne `M = 255 = 3 × 5 × 17` — trois facteurs impairs, aucune puissance de
    /// deux, et `N ≡ 2 (mod 4)`, donc `N/2` impair : c'est le régime où une erreur d'indice dans
    /// l'étage 3 (`out[n − 1 − 2q]` parcourt les impairs à rebours) ou un algorithme de FFT mixte
    /// se verrait, et où le test précédent est aveugle.
    ///
    /// Seconde graine aussi (7 au lieu de 4242) : deux entrées différentes, sinon l'équivalence
    /// n'est établie que sur un vecteur.
    ///
    /// **Tolérance `4e-4`, et le doublement est dérivé, pas concédé.** L'erreur mesurée ici est
    /// celle de l'ORACLE, dont l'accumulation `f32` croît en `√(2N)` : 1020 termes contre 512 font
    /// un facteur 1,4, et la queue de la loi est tirée par `N = 510` coefficients contre 256.
    /// **Mesuré le 2026-09-02 : 1,05e-4**, contre 5,50e-5 à `N = 256` — soit exactement le facteur
    /// 1,9 attendu. Garder `2e-4` ici ne laisserait qu'une marge de 1,9 quand le test à `N = 256`
    /// s'en donne 3,6 ; `4e-4` rétablit la même marge (3,8) sur la même justification. Reste très
    /// en dessous de ce qu'un terme de phase faux produit (~3e-3). La trace imprimée donne le
    /// chiffre réellement mesuré, et c'est LUI qu'un rapport cite.
    #[test]
    fn la_mdct_rapide_rend_la_meme_chose_que_loracle_a_m_impair() {
        const N: usize = 510;
        assert_eq!(
            N % 4,
            2,
            "N doit valoir 2 mod 4 pour que M = N/2 soit impair"
        );
        assert_eq!((N / 2) % 2, 1, "M = N/2 doit être impair");

        let frame = bruit(7, 2 * N);
        let attendu = MdctPlan::new(N).transform(&frame);
        let obtenu = MdctFast::new(N).transform(&frame);
        let echelle = attendu.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(echelle > 0.0, "trame de test dégénérée");

        let mut pire = 0.0f32;
        for (k, (a, b)) in attendu.iter().zip(&obtenu).enumerate() {
            let ecart = (a - b).abs() / echelle;
            pire = pire.max(ecart);
            assert!(
                ecart < 4e-4,
                "coefficient {k} : {b} contre {a} (écart relatif {ecart:e})"
            );
        }
        println!("écart relatif maximal MdctFast ↔ MdctPlan (N={N}, M impair) : {pire:e}");
    }

    /// Le test de reconstruction, rejoué sur le chemin rapide.
    ///
    /// L'équivalence ci-dessus le rendrait redondant SI l'oracle était infaillible ; il ne l'est
    /// pas, et surtout un futur changement de `MdctFast` qui casserait le terme de phase pourrait
    /// être « équivalent » à un oracle cassé au même endroit. La reconstruction, elle, a pour
    /// vérité de référence le SIGNAL D'ENTRÉE — rien de recalculé.
    #[test]
    fn deux_trames_voisines_reconstruisent_le_signal_chemin_rapide() {
        const N: usize = 64;

        let signal = bruit(12345, 3 * N);
        let w = sine_window(2 * N);
        let plan = MdctFast::new(N);
        let analyse = |offset: usize| {
            let trame: Vec<f32> = (0..2 * N).map(|i| signal[offset + i] * w[i]).collect();
            let y = imdct(&plan.transform(&trame));
            y.iter()
                .zip(w.iter())
                .map(|(v, wi)| v * wi)
                .collect::<Vec<f32>>()
        };

        let a = analyse(0);
        let b = analyse(N);
        for i in 0..N {
            let reconstruit = a[N + i] + b[i];
            assert!(
                (reconstruit - signal[N + i]).abs() < 1e-3,
                "échantillon {i} : {reconstruit} contre {} attendu",
                signal[N + i]
            );
        }
    }

    /// Un sinus pile au centre d'un bin doit concentrer son énergie DANS ce bin.
    ///
    /// La valeur attendue ne vient pas d'un recalcul de la formule — elle vient de ce qu'une
    /// transformée est censée FAIRE : la fréquence centrale du bin `k` d'une MDCT de `N` points à
    /// `sr` est `(k + 1/2) · sr / 2N`, et une transformée qui ne range pas cette fréquence-là dans
    /// ce bin-là n'est pas une MDCT, quelle que soit l'élégance de son code.
    #[test]
    fn un_sinus_au_centre_dun_bin_y_concentre_son_energie() {
        const N: usize = 64;
        const SR: f32 = 44100.0;
        const K: usize = 17;

        let f = (K as f32 + 0.5) * SR / (2.0 * N as f32);
        let w = sine_window(2 * N);
        let frame: Vec<f32> = (0..2 * N)
            .map(|i| (2.0 * std::f32::consts::PI * f * i as f32 / SR).sin() * w[i])
            .collect();

        let x = mdct(&frame);
        let total: f32 = x.iter().map(|v| v * v).sum();
        let argmax = x
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
            .map(|(i, _)| i)
            .unwrap();

        assert_eq!(argmax, K, "le pic n'est pas dans le bin attendu");

        // Le bin seul ne porte que 38 % : une MDCT n'est pas invariante par translation, et la
        // part exacte dépend de la phase du sinus dans la trame. Ce qui est une propriété de la
        // transformée, et pas un accident de cette entrée-ci, c'est que tout se joue dans le bin
        // et ses deux voisins. Mesuré ici : 99,12 % — la borne est posée en dessous avec marge,
        // pour ne pas transformer un test de localisation en test de troisième décimale.
        let voisinage: f32 = x[K - 1..=K + 1].iter().map(|v| v * v).sum();
        assert!(
            voisinage / total > 0.95,
            "énergie hors du voisinage du bin {K} : {:.2} % dedans seulement",
            100.0 * voisinage / total
        );
    }
}
