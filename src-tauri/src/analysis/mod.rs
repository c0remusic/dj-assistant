//! M2a audio analysis engine. One FFmpeg decode → online accumulators → AnalysisReport.
//! Pure: no DB writes, no UI. See docs/superpowers/specs/2026-06-12-m2a-analysis-engine-design.md
use serde::{Deserialize, Serialize};

pub mod aac_sfb;
pub mod decode;
pub mod dynamics;
pub mod mdct;
pub mod peaks;
pub mod phase;
pub mod quant_trace;
pub mod spectrum;
pub mod structure;
pub mod tags;
pub mod verdict;

/// Real signal lineage, independent of the declared extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rail {
    Lossless,
    Lossy,
    Unknown,
}

/// Authenticity verdict, derived from cutoff + declared rail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Ok,
    Fake,
    Grey,
}

/// Time×frequency magnitude grid (dB, quantized to u8) for the UI spectrogram (M2c).
///
/// `Default` = the empty grid, which is a real state and not just a derive convenience: it is
/// what `analyze(path, false)` produces, and what the report cache stores (see
/// `ipc::cache_json` — the grid is computed on demand when the Revue collapse opens).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Spectrogram {
    pub frames: usize,
    pub bins: usize,
    pub hz_per_bin: f32,
    pub sec_per_frame: f32,
    /// `frames * bins` values, row-major by frame. 0 = -100 dBFS, 255 = 0 dBFS.
    ///
    /// Travels over the Tauri IPC as a base85 RFC1924 STRING, not as serde's default array of
    /// decimal integers — see `crate::b85_bytes`.
    ///
    /// PLUS au repos : depuis le 2026-08-03, cette grille n'est plus stockée dans
    /// `tracks.report_json` (`ipc::cache_json` la retire, le pool analyse avec
    /// `with_spectrogram: false`). Elle se recalcule à l'ouverture du collapse Diagnostic —
    /// 631 ms mesurées, contre ~450 ko par piste économisés, soit 4,11 Go → 119 Mo sur la base
    /// de production. Le champ vaut donc `Default` sur tout rapport lu depuis le cache.
    /// The layout and the quantization above are unchanged; only the encoding is. Consumers
    /// outside Rust must decode it (`shared/contracts.ts` mirrors the decoded type).
    #[serde(with = "crate::b85_bytes")]
    pub mag_db: Vec<u8>,
}

/// The full analysis result for one file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnalysisReport {
    pub path: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_sec: f32,
    pub declared_format: String,
    pub declared_bitrate: Option<u32>,
    pub declared_rail: Rail,
    pub cutoff_hz: f32,
    pub verdict: Verdict,
    /// True when the declared rail claims Lossless but the real container (magic-byte
    /// sniffed) is Lossy — the specific Fake cause where the spectral cutoff can sit near
    /// Nyquist (unlike a genuine spectral-cliff transcode), so the UI must caption it
    /// differently. Mirrors the same condition `verdict::verdict` short-circuits on.
    pub container_mismatch: bool,
    /// Equivalent lossy bitrate estimated from `cutoff_hz` (FIX-11: single source of truth,
    /// see `verdict::estimate_kbps` — the front no longer computes this itself).
    pub est_kbps: u32,
    pub peaks: Vec<f32>,
    /// Mono samples represented by ONE entry of `peaks`. Equals `PEAKS_WINDOW` unless the envelope
    /// was capped (`peaks::cap`), in which case it is `PEAKS_WINDOW * pooling factor`. The front
    /// needs it to convert a point count back into seconds — deriving that from PEAKS_WINDOW alone
    /// silently under-reports coverage on every capped track.
    #[serde(default = "default_peaks_step")]
    pub peaks_step: usize,
    pub spectrogram: Spectrogram,
    pub clip_runs: u32,
    pub clip_pct: f32,
    pub true_peak_dbtp: f32,
    pub dc_offset: f32,
    pub phase_correlation: f32,
    pub dual_mono: bool,
    pub container_ok: bool,
    pub codec_error: Option<String>,
    pub truncated: bool,
    /// Platitude spectrale de la bande 16-20 kHz, médiane sur les trames, en dB. `None` quand la
    /// bande n'existe pas à ce taux d'échantillonnage.
    ///
    /// Un FAIT sur le fichier tel qu'il est, pas une affirmation sur son histoire : « l'aigu est
    /// clairsemé » se mesure, « ça a été un MP3 » ne se déduit pas. Un master volontairement
    /// sombre et un transcodage donnent la même valeur.
    ///
    /// Depuis le 2026-08-18, `verdict()` LA LIT — mais uniquement pour rendre DOUTEUX, jamais
    /// Faux, et seulement sur un fichier a bande pleine ou la coupure n'a plus rien a dire. La
    /// nommer Faux reviendrait à accuser un master d'une histoire qu'on n'a pas établie ; le dire
    /// douteux ne dit que ce qui est mesure.
    ///
    /// Repère mesuré (2026-08-18) : 44 fichiers authentiques de trois familles musicales et trois
    /// provenances d'achat tiennent dans [-5,4 ; -2,5] dB, les transcodages descendent a -43,8.
    /// Voir `spectrum::HF_FLATNESS_LO_HZ` pour le détail.
    #[serde(default)]
    pub hf_flatness_db: Option<f32>,
    /// Même mesure sur une bande RELATIVE au Nyquist (0,80-0,98) et non en Hz fixes. Les deux sont
    /// nécessaires : chacune est aveugle là où l'autre voit. Un MP3 128 coupe à 16,8 kHz, donc la
    /// bande relative tombe entièrement au-dessus de sa coupure, sur un plancher uniforme donc
    /// parfaitement plat ; un Opus est a 48 kHz et garde du contenu jusqu'a 20 kHz, donc la bande
    /// FIXE y est en pleine bande passante et ne voit rien.
    ///
    /// Mesure sur les 150 transcodages, seuils poses au plancher de 32 authentiques : fixe 63 %,
    /// relative 17 %, UNION 68 %. Opus passe de 0/10 a 6/10. Un premier chiffre de 77 % reposait
    /// sur un seuil relatif tire de 10 authentiques seulement, qui produisait des faux positifs
    /// sur de l'ambient. Voir `spectrum::HF_FLATNESS_REL_LO`.
    #[serde(default)]
    pub hf_flatness_top_db: Option<f32>,
    /// Durée RÉELLEMENT décodée, en secondes — à comparer à `duration_sec`, qui vient de l'en-tête.
    ///
    /// Les deux étaient jusqu'ici une seule valeur, celle DÉCLARÉE, et personne ne vérifiait
    /// qu'elle correspondait au son présent. Un en-tête peut annoncer 6 minutes sur un fichier
    /// tronque a 40 secondes : rien dans le rapport ne le disait, parce que `truncated` teste une
    /// coupure ABRUPTE du signal, pas un désaccord de comptage. Les deux échouent sur des cas
    /// différents — un fichier qui fond proprement vers le silence avant sa fin annoncée passe
    /// l'un et pas l'autre.
    ///
    /// Rendu brut plutôt qu'en booléen : c'est l'appelant qui décide de la tolérance, et un écart
    /// se lit mieux en secondes qu'en « vrai ». Fakin' The Funk fait la même comparaison et en
    /// tire sa classe CORROMPU (« Actual duration does not match stated duration ») ; nous ne la
    /// faisions pas du tout, alors que le fichier est déjà décodé entièrement.
    #[serde(default)]
    pub decoded_duration_sec: f32,
    /// Vraisemblance que le signal soit DÉJÀ passé par la grille de quantification d'un codec AAC
    /// — le troisième signal du verdict (issue #52, méthode d'Olivier Derrien). `None` = **pas
    /// mesurée**, et c'est le cas de l'immense majorité des fichiers.
    ///
    /// Elle n'est calculée que là où elle peut trancher — lossless déclaré, conteneur non démenti,
    /// **bande pleine** (`verdict::needs_quant_probe`), soit ~tout lossless sain, à +0,3 s par
    /// fichier. Sous la falaise ou sur une fraude de conteneur elle vaut `None` parce qu'elle n'a
    /// pas été demandée — pas parce qu'elle serait basse. Elle vaut aussi `None` quand la mesure
    /// n'existe pas : taux d'échantillonnage hors
    /// des tables de bandes AAC, fichier trop court pour un groupe de trames, ou signal trop long
    /// pour être retenu en mémoire (`QUANT_MAX_PCM_SAMPLES`).
    ///
    /// Un FAIT, pas un verdict : c'est `verdict::QUANT_LAMBDA` qui le seuille, et lui seul. Le
    /// champ voyage pour que la Revue puisse un jour l'afficher dans le collapse Détails.
    ///
    /// ⚠️ Ancien rapport en cache : `null`. À lire comme « pas mesuré », jamais comme 0 — 0 est une
    /// vraie valeur, celle d'un signal dont aucune cellule ne porte la grille.
    #[serde(default)]
    pub quant_likelihood: Option<f32>,
    pub silence_head_ms: u32,
    pub silence_tail_ms: u32,
    pub id3_version: Option<String>,
    pub tags_cdj_ok: bool,
    pub has_cover: bool,
}

/// Bump whenever a change alters what `analyze()` produces WITHOUT changing `AnalysisReport`'s
/// JSON shape — e.g. spectrogram resolution constants (spectrum.rs), cutoff-detection tuning,
/// the kbps table — so rows cached under the old behavior (structurally still valid JSON) are
/// treated as stale and recomputed, instead of silently serving outdated data forever. Struct
/// field additions/removals are already caught by `serde_json::from_str` failing outright; this
/// constant is for the content changes that a schema check can't see.
///
/// A change to the ENCODING of an existing field counts too (v6: `mag_db` moved to base85), even
/// when the tolerant deserializer would still read the old rows — bumping makes the invalidation
/// explicit instead of leaving inflated rows served forever.
///
/// ⚠️ **Un ajout de champ ne suffit PAS à se garder tout seul quand il porte `#[serde(default)]`.**
/// Le commentaire ci-dessus dit qu'une addition de champ est « déjà attrapée par
/// `serde_json::from_str` qui échoue » — c'est vrai d'un champ nu, et faux d'un champ defaulte.
/// v7 : `hf_flatness_db` et `decoded_duration_sec` sont arrives avec un `default`, donc les
/// anciens rapports se relisaient sans erreur et rendaient `None`/`0`. Conséquence mesurée dans
/// la vraie fenêtre le 2026-08-18 : la ligne « Densité de l'aigu » n'apparaissait sur AUCUNE piste
/// de la bibliothèque, alors que la mesure venait d'être branchée. Une mesure invisible ne mesure
/// rien.
///
/// Le coût est celui de v16, mesuré à l'époque : la bibliothèque se re-analyse (~30 min sur le
/// pool pour 297 h d'audio). Il se paie en arrière-plan pour les pistes en file, et à l'ouverture
/// pour les pistes rangées (`ipc::analyze_path` se répare tout seul).
/// **v9, 2026-09-01 (issue #46) — changement de SÉMANTIQUE, pas de forme.** Deux champs du rapport
/// veulent dire autre chose qu'en v8, à JSON strictement identique :
/// - `tags_cdj_ok` ne teste plus la seule PRÉSENCE d'Artiste+Titre mais le couple conteneur × type
///   de tag contre la matrice de `docs/cdj-metadata-formats.md` (`tags.rs::tag_type_readable_on_cdj`)
///   — un WAV taggé RIFF INFO valait `true`, il vaut `false` ;
/// - `id3_version` portait le stub `Some("ID3")` posé à l'aveugle sur l'extension `.mp3` ; il porte
///   maintenant le ou les TYPES réels du porteur, triés et joints par `+`.
///
/// Le bump n'est pas cosmétique, et c'est là qu'il se gagne : jusqu'au 2026-09-09 le pool
/// (`worker::select_needing_analysis`, alors `select_pending`) ne reprenait JAMAIS une piste RANGÉE
/// (`status='pending'` dans sa clause). Pour toute la bibliothèque rangée, le SEUL chemin qui
/// rafraîchissait un rapport était la réparation à l'ouverture (`ipc::analyze_path`), et elle ne se
/// déclenche que sur désaccord de version. (Depuis l'issue #59 le pool reprend aussi les rangées
/// sans verdict courant — mais un rapport périmé à verdict courant, lui, n'est toujours réparé qu'à
/// l'ouverture.) Sans ce bump, une piste rangée resservirait indéfiniment un `tags_cdj_ok` calculé par
/// l'ancien critère.
///
/// **v10, 2026-09-02 (issue #52) — champ neuf, et il porte `#[serde(default)]`.**
/// `quant_likelihood` arrive avec un `default`, donc les rapports v9 se relisent SANS erreur et
/// rendent `None` : le piège exact nommé par le ⚠️ ci-dessus, et payé en v7 (la ligne « Densité de
/// l'aigu » invisible sur toute la bibliothèque). Sans ce bump, une piste rangée resservirait pour
/// toujours un rapport dépourvu du troisième signal, et le verdict Douteux qu'il aurait pu
/// trancher.
pub const REPORT_CACHE_VERSION: i64 = 10;

/// Lit le cache `(tracks.report_json, tracks.report_cache_ver)`. Version absente, version distancée
/// ou JSON vide (sentinelle d'échec de `persist_failure`) = **pas de rapport courant**, rendu comme
/// `None` : l'appelant re-décode et se répare. Jamais une erreur.
///
/// Même raison qu'`verdict::cached` et `fingerprint::cached` de passer par une fonction plutôt que
/// par un littéral au site d'appel : la constante reste déclarée une seule fois, et la règle
/// devient testable sans passer par une commande Tauri.
pub fn cached_report(json: Option<String>, ver: Option<i64>) -> Option<String> {
    match ver {
        Some(v) if v == REPORT_CACHE_VERSION => json.filter(|j| !j.is_empty()),
        _ => None,
    }
}

use dynamics::{ClipAccumulator, DcAccumulator, TruePeakAccumulator};
use peaks::PeaksAccumulator;
use phase::PhaseAccumulator;
use spectrum::SpectrumAccumulator;
use structure::{SilenceAccumulator, TruncationAccumulator};

const FFT_SIZE: usize = 4096;
const PEAKS_WINDOW: usize = 512; // ~11.6 ms @ 44.1k
/// Ceiling on the number of envelope points kept in the report. At PEAKS_WINDOW a 6.5-minute track
/// produced ~33 500 of them — 21% of report_json — to draw a waveform a few hundred pixels wide.
/// 4 000 still gives several points per pixel at any realistic canvas width.
const MAX_PEAKS: usize = 4_000;

/// Serde fallback for reports written before `peaks_step` existed: they were never capped, so their
/// step is exactly PEAKS_WINDOW. Keeps such a report readable instead of failing the whole parse.
fn default_peaks_step() -> usize {
    PEAKS_WINDOW
}
/// Plafond du PCM retenu en mémoire pour la sonde de quantification (issue #52), en échantillons
/// ENTRELACÉS `f32`.
///
/// **Pourquoi une rétention existe.** `quant_trace::likelihood` a besoin du signal décodé, et
/// `analyze()` ne garde rien : il pousse chaque bloc dans des accumulateurs en ligne. Or la
/// décision de sonder ne se prend qu'APRÈS le décodage — elle dépend de la coupure et de la
/// platitude, qui sortent du même passage. Re-décoder serait un second passage complet sur le
/// fichier ; on retient donc le PCM, mais seulement quand la seule condition connue AVANT le
/// décodage est réunie (rail déclaré lossless, conteneur non démenti).
///
/// **Pourquoi un plafond, et l'arithmétique du pic en entier.** Un `f32` pèse 4 octets, le PCM est
/// entrelacé sur au plus 2 canaux :
///
/// ```text
/// par piste  = durée × taux × canaux × 4 o
///   6 min stéréo 44,1 kHz = 360 × 44 100 × 2 × 4 =  127 Mo   (la piste DJ typique)
///   plafond 48 000 000 f32                       =  192 Mo   = 9,07 min stéréo 44,1 kHz
///                                                            = 8,33 min stéréo 48 kHz
/// pic pool   = plafond × worker::analysis_pool_size (≤ 8)
///            = 192 Mo × 8                          = 1,5 Go   (pire cas ABSOLU)
///   cas réaliste, huit pistes de 6 min             = 1,0 Go
/// ```
///
/// Le pire cas suppose les huit threads simultanément sur des fichiers AU plafond, chacun au
/// milieu de son décodage. Sans plafond, un seul mix d'une heure coûterait 1,2 Go à lui seul et
/// rien ne bornerait le pic.
///
/// **Le tampon ne survit pas à son usage** : `analyze()` le libère (`drop`) juste après la
/// décision de sonde, avant `verdict()` et avant la construction du rapport — sur TOUS les
/// chemins, y compris quand la sonde n'a pas été demandée. Le pic ci-dessus est donc borné à la
/// fenêtre décodage → sonde, pas à la durée de l'analyse.
///
/// **Ce que le dépassement coûte, et il est borné** : la rétention est abandonnée, le tampon
/// libéré, `quant_likelihood` vaut `None` — et `None` ne dégrade jamais un verdict (`verdict()`).
/// Un lossless de plus de neuf minutes tombant dans le bras ambigu reste donc **Douteux**, ce
/// qu'il était avant #52. Aucune régression, seulement une portée.
const QUANT_MAX_PCM_SAMPLES: usize = 48_000_000;

/// Les deux résolutions balayées par la sonde, dans l'ordre du prototype.
///
/// Les DEUX, et ce n'est pas de la prudence : la calibration du 2026-09-02 (λ = 0,18) a été
/// mesurée avec ce réglage exact, et le tableau de détection qu'elle produit dépend des blocs
/// longs autant que des courts — les familles `aac_mf` gagnent souvent en blocs LONGS, avec les
/// plus fortes vraisemblances du corpus. N'en retirer un reviendrait à appliquer un seuil calibré
/// sur une autre mesure.
const QUANT_RESOLUTIONS: [aac_sfb::BlockKind; 2] =
    [aac_sfb::BlockKind::Long, aac_sfb::BlockKind::Short];

/// Fils de balayage accordés à la sonde, **par analyse**.
///
/// `analyze()` tourne DANS un thread du pool d'analyse, dimensionné à
/// `available_parallelism().clamp(1, 8)` (`worker::analysis_pool_size`). La parallélisation interne
/// de `quant_trace::balaye_decalages` s'y multiplie : à son défaut historique (16 fils), une
/// machine à 16 cœurs ouvrirait **8 × 16 = 128 fils** pour 16 cœurs — une sur-souscription d'un
/// facteur 8, qui se paie en changements de contexte.
///
/// 2 plutôt que 1 : le pool ne sature que quand la file est pleine, et une analyse isolée
/// (ouverture d'une piste en Revue) profite du second fil. Dans le pire cas, 8 × 2 = 16, soit
/// exactement le parallélisme d'une machine à 16 cœurs.
///
/// Le résultat ne dépend PAS de ce nombre — `balaye_decalages` partitionne les décalages et prend
/// un `max`, opération associative et commutative. Figé par
/// `quant_trace::tests::le_nombre_de_fils_ne_change_ni_le_l_ni_le_decalage`.
const QUANT_PROBE_THREADS: usize = 2;

const CLIP_THRESHOLD: f32 = 0.99;
const CLIP_MIN_RUN: usize = 3;
const SILENCE_THRESHOLD: f32 = 0.001; // ~ -60 dBFS

/// Runs the full analysis: one decode, all analyzers in a single streaming pass.
/// `with_spectrogram`: build the (heavy) display spectrogram grid. The verdict and all
/// scalar signals are identical either way — only the display grid is gated. Batch (M2b)
/// passes false; the Revue UI / debug overlay pass true on demand.
pub fn analyze(path: &str, with_spectrogram: bool) -> Result<AnalysisReport, String> {
    let started = std::time::Instant::now();
    // declared properties / tags (no decode)
    let tag = tags::read(path);
    let target_ch = if tag.channels >= 2 { 2 } else { 1 };

    // Native sample rate from the header — drives every frequency-domain accumulator so the
    // cutoff/spectrogram map bins → Hz correctly (no resample, no hardcoded-rate skew).
    let sr = decode::probe(path)?.sample_rate;
    let mut dc = DcAccumulator::new();
    let mut clip = ClipAccumulator::new(CLIP_THRESHOLD, CLIP_MIN_RUN);
    let mut tp = TruePeakAccumulator::new();
    let mut sil = SilenceAccumulator::new(sr, SILENCE_THRESHOLD);
    let mut trunc = TruncationAccumulator::new(sr);
    let mut pk = PeaksAccumulator::new(PEAKS_WINDOW);
    let mut spec = SpectrumAccumulator::new(sr, FFT_SIZE, with_spectrogram);
    let mut ph = PhaseAccumulator::new();

    // Nombre d'échantillons MONO réellement décodés — la seule façon de savoir combien de son le
    // fichier contient vraiment, par opposition a ce que son en-tete annonce. Compte ici et pas
    // dans un accumulateur existant pour que la mesure ne dépende d'aucun de leurs seuils.
    let mut decoded_mono_samples: u64 = 0;

    // Rétention du PCM pour la sonde de quantification (#52). Le pré-filtre est la seule partie de
    // `verdict::needs_quant_probe` connaissable AVANT le décodage : rail déclaré lossless et
    // conteneur qui ne le dément pas. Les deux autres clauses (bande pleine, aigu creux) sortent du
    // décodage lui-même, donc on ne peut pas les anticiper — c'est ce qui force la rétention plutôt
    // qu'un décodage à la demande.
    let quant_pregate = tag.declared_rail == Rail::Lossless && tag.content_rail != Rail::Lossy;
    let mut quant_pcm: Vec<f32> = Vec::new();
    let mut quant_pcm_over_cap = false;

    let info = decode::decode_pcm(path, target_ch, |block| {
        decoded_mono_samples += (block.len() / target_ch as usize) as u64;
        if quant_pregate && !quant_pcm_over_cap {
            if quant_pcm.len() + block.len() > QUANT_MAX_PCM_SAMPLES {
                // Au-delà du plafond on ABANDONNE, on ne tronque pas : un signal tronqué ferait
                // porter les huit groupes de trames sur le seul début du fichier, donc une mesure
                // qui n'est plus celle sur laquelle λ a été calibré. Mieux vaut pas de mesure
                // qu'une mesure d'autre chose.
                quant_pcm_over_cap = true;
                quant_pcm = Vec::new();
            } else {
                quant_pcm.extend_from_slice(block);
            }
        }
        if target_ch == 2 {
            ph.push(block); // interleaved L,R
            let mono: Vec<f32> = block
                .chunks_exact(2)
                .map(|lr| 0.5 * (lr[0] + lr[1]))
                .collect();
            dc.push(&mono);
            clip.push(&mono);
            tp.push(&mono);
            sil.push(&mono);
            trunc.push(&mono);
            pk.push(&mono);
            spec.push(&mono);
        } else {
            dc.push(block);
            clip.push(block);
            tp.push(block);
            sil.push(block);
            trunc.push(block);
            pk.push(block);
            spec.push(block);
        }
    })?;

    let (clip_runs, clip_pct) = clip.finish();
    let (silence_head_ms, silence_tail_ms) = sil.finish();
    let truncated = trunc.finish(info.codec_error.is_some());
    let spec_res = spec.finish();
    let phase_correlation = if target_ch == 2 {
        ph.correlation()
    } else {
        0.0
    };
    let dual_mono = target_ch == 2 && ph.dual_mono();

    let declared_format = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let cutoff_hz = spec_res.cutoff_hz;
    // Content-rail sniffing (magic bytes, independent of the declared extension) comes from the
    // same lofty probe `tags::read` already did — no second file open/read pass. Only matters
    // when the declared rail is Lossless — that's the only case verdict() short-circuits on a
    // mismatch.
    let content_rail = tag.content_rail;
    // content_rail is now sniffed unconditionally in tags::read() (no longer gated on
    // declared_rail here) — both sides of this comparison are load-bearing.
    let container_mismatch = tag.declared_rail == Rail::Lossless && content_rail == Rail::Lossy;
    // `verdict()` peut refuser de trancher : coupure jamais mesurée, ou rail déclaré indéterminé
    // (2026-09-01, issue #51). Ce n'est pas un verdict prudent, c'est une analyse qui n'a pas
    // abouti — donc elle emprunte le chemin d'échec DÉJÀ en place plutôt qu'un état nouveau :
    // `analyze()` rend `Err`, `worker::persist_result` appelle `persist_failure`, qui laisse
    // `verdict` NULL, pose la sentinelle `report_json=''` et incrémente `analysis_attempts` vers
    // `MAX_ANALYSIS_ATTEMPTS`. Aucun champ sérialisé ne change : le rapport n'est pas produit.
    let flatness = verdict::HfFlatness {
        fixed_db: spec_res.hf_flatness_db,
        top_db: spec_res.hf_flatness_top_db,
    };

    // TROISIÈME SIGNAL, à la demande (#52). La condition de déclenchement n'est pas réécrite ici :
    // elle est demandée à `verdict::needs_quant_probe`, seul endroit qui la teste, pour que la
    // dépense et le bras qu'elle sert ne dérivent pas l'un de l'autre.
    let quant_likelihood = if verdict::needs_quant_probe(cutoff_hz, tag.declared_rail, content_rail)
    {
        if quant_pcm.is_empty() {
            // Le pré-filtre du décodage a laissé passer, mais rien n'a été retenu : fichier
            // au-dessus du plafond, ou décodage vide. Pas de mesure, et on le DIT.
            log::info!(
                "quant_trace {} : non mesuré (PCM non retenu, au-delà du plafond = {})",
                path,
                quant_pcm_over_cap
            );
            None
        } else {
            let t0 = std::time::Instant::now();
            let trace = quant_trace::likelihood(
                &quant_pcm,
                info.channels,
                info.sample_rate,
                &QUANT_RESOLUTIONS,
                Some(QUANT_PROBE_THREADS),
            );
            match trace {
                Some(t) => {
                    log::info!(
                        "quant_trace {} : L={:.5} décalage={} canal={} résolution={} en {} ms",
                        path,
                        t.l,
                        t.decalage,
                        t.canal.label(),
                        t.resolution.label(),
                        t0.elapsed().as_millis()
                    );
                    Some(t.l as f32)
                }
                None => {
                    log::info!("quant_trace {path} : non mesuré (taux non tabulé ou signal court)");
                    None
                }
            }
        }
    } else {
        None
    };
    // Le tampon peut peser jusqu'à `QUANT_MAX_PCM_SAMPLES` : il ne survit pas à son seul usage.
    drop(quant_pcm);

    let verdict = verdict::verdict(
        cutoff_hz,
        tag.declared_rail,
        tag.declared_bitrate,
        content_rail,
        flatness,
        quant_likelihood,
    )
    .map_err(|e| e.to_string())?;
    let est_kbps = verdict::estimate_kbps(cutoff_hz);
    let (capped_peaks, peaks_factor) = peaks::cap(pk.finish(), MAX_PEAKS);

    log::info!(
        "analyze {} : {} ms (decode+dsp, {} ch, {:.1}s, spectro={})",
        path,
        started.elapsed().as_millis(),
        info.channels,
        tag.duration_sec,
        with_spectrogram
    );

    Ok(AnalysisReport {
        path: path.to_string(),
        sample_rate: info.sample_rate,
        channels: info.channels,
        duration_sec: tag.duration_sec,
        declared_format,
        declared_bitrate: tag.declared_bitrate,
        declared_rail: tag.declared_rail,
        cutoff_hz,
        verdict,
        container_mismatch,
        est_kbps,
        peaks: capped_peaks,
        peaks_step: PEAKS_WINDOW * peaks_factor,
        spectrogram: spec_res.spectrogram,
        clip_runs,
        clip_pct,
        true_peak_dbtp: tp.finish(),
        dc_offset: dc.finish(),
        phase_correlation,
        dual_mono,
        container_ok: info.codec_error.is_none(),
        codec_error: info.codec_error,
        truncated,
        hf_flatness_db: spec_res.hf_flatness_db,
        hf_flatness_top_db: spec_res.hf_flatness_top_db,
        decoded_duration_sec: decoded_mono_samples as f32 / sr as f32,
        quant_likelihood,
        silence_head_ms,
        silence_tail_ms,
        id3_version: tag.id3_version,
        tags_cdj_ok: tag.tags_cdj_ok,
        has_cover: tag.has_cover,
    })
}

#[cfg(test)]
mod corpus {
    /// Mesure le détecteur sur un dossier RÉEL et imprime du CSV — le harnais de l'étape 2 du
    /// chantier « le détecteur de faux marche-t-il » (2026-08-17).
    ///
    /// Existe parce que le corpus de fixtures du dépôt descend entièrement d'UN sinus balayé passé
    /// par UN encodeur à deux débits : vert dessus ne dit rien de la vraie musique. Ce test ne
    /// vérifie rien lui-même — il produit la mesure, et c'est le corpus étiqueté qui porte le
    /// jugement.
    ///
    /// `SIFT_CORPUS_DIR=<dossier> cargo test --manifest-path src-tauri/Cargo.toml --release
    ///   corpus_scan -- --ignored --nocapture`
    ///
    /// `--release` obligatoire en pratique : le décodage d'un morceau de 6 minutes en debug se
    /// compte en minutes.
    #[test]
    #[ignore]
    fn corpus_scan() {
        let Ok(dir) = std::env::var("SIFT_CORPUS_DIR") else {
            eprintln!("SIFT_CORPUS_DIR non défini — rien à mesurer");
            return;
        };
        // Compte positif obligatoire : une liste vide et un dossier illisible se ressemblent, et
        // c'est exactement le défaut que ce dépôt passe son temps à corriger.
        let mut seen = 0usize;
        let mut failed = 0usize;
        // Le nom de fichier passe EN DERNIER, et ce n'est pas cosmétique : mesuré le 2026-08-18
        // sur 967 fichiers d'une vraie clé USB, 4 lignes étaient tordues parce que le nom
        // contenait le séparateur — « Jacob Todd - Nevermore (Original ;… ».wav » décalait toutes
        // les colonnes suivantes, et le verdict lu était un bout de titre. En dernière position,
        // un `;` dans le nom ne peut plus déplacer quoi que ce soit : les sept champs qui
        // précèdent se lisent par position, et le nom est « tout ce qui reste ».
        //
        // Les deux colonnes de platitude sont là depuis le 2026-08-18 parce que les taux publiés
        // sur ces bandes (77 % puis 68 %) venaient de scripts ad-hoc perdus avec leur session :
        // impossible de les rejouer, donc impossible de les corriger. Elles sortent du MÊME
        // `analyze()` que le verdict, qui, lui, ne les lit pas encore.
        // `quant_l` depuis le 2026-09-02 (#52) : la vraisemblance de quantification, telle que le
        // verdict l'a vue — « - » quand elle n'a pas été demandée (le cas de l'immense majorité).
        // Elle s'insère AVANT le nom, donc `score-corpus.mjs` la prend sans rien changer : ce
        // script déduit le nombre de colonnes de l'en-tête et lit le nom comme « tout ce qui reste ».
        println!(
            "rail;debit_declare;cutoff_hz;verdict;est_kbps;hf_flat_db;hf_flat_top_db;quant_l;fichier"
        );
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
                "aif" | "aiff" | "flac" | "wav" | "mp3" | "m4a" | "ogg" | "opus" | "wma"
            ) {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|x| x.to_str())
                .unwrap_or("(illisible)");
            seen += 1;
            match super::analyze(&path.to_string_lossy(), false) {
                Ok(r) => println!(
                    "{:?};{};{:.0};{:?};{};{};{};{};{name}",
                    r.declared_rail,
                    r.declared_bitrate
                        .map(|b| b.to_string())
                        .unwrap_or_else(|| "-".into()),
                    r.cutoff_hz,
                    r.verdict,
                    r.est_kbps,
                    // « - » et pas 0 : la bande n'existe pas à tous les taux d'échantillonnage, et
                    // un zéro se lirait comme une mesure plate — exactement l'inverse du fait.
                    r.hf_flatness_db
                        .map(|v| format!("{v:.2}"))
                        .unwrap_or_else(|| "-".into()),
                    r.hf_flatness_top_db
                        .map(|v| format!("{v:.2}"))
                        .unwrap_or_else(|| "-".into()),
                    // « - » et pas 0 : `None` ici veut dire « la sonde n'a pas été demandée »
                    // (le bras ambigu n'a pas été atteint) ou « pas mesurable ». 0 est une vraie
                    // valeur, celle d'un signal dont aucune cellule ne porte la grille.
                    r.quant_likelihood
                        .map(|v| format!("{v:.5}"))
                        .unwrap_or_else(|| "-".into()),
                ),
                Err(err) => {
                    failed += 1;
                    // Un échec d'analyse est une LIGNE du résultat, pas un silence : c'est
                    // précisément le cas qui, non dit, ferait passer un corpus incomplet pour
                    // un corpus propre.
                    //
                    // Le MÊME nombre de colonnes que la ligne normale, et ce n'est pas cosmétique :
                    // le lecteur prend le nom en position fixe. Une ligne d'erreur plus courte
                    // ferait tomber le nom ailleurs, donc hors jointure — un fichier en échec
                    // compterait alors comme « non mesuré » au lieu de « en erreur ».
                    println!("ERREUR;-;-;-;{err};-;-;-;{name}");
                }
            }
        }
        println!("-- {seen} fichiers audio parcourus, {failed} en échec");
        assert!(seen > 0, "aucun fichier audio dans {dir} — mesure vide");
    }
}

#[cfg(test)]
mod tests {

    /// Les quatre états que `cached_report` doit distinguer. Ce que ce test garde est le bump lui-
    /// même : `REPORT_CACHE_VERSION` ne sert à rien si un rapport stampé à l'ANCIENNE version se
    /// fait quand même servir comme courant — la bibliothèque rangée reservirait alors pour
    /// toujours un `tags_cdj_ok` calculé par le critère d'avant (voir la doc de la constante).
    ///
    /// La mutation se porte sur la ligne, pas sur la `const` : `REPORT_CACHE_VERSION - 1` est
    /// l'état d'une ligne écrite avant le bump, quelle que soit la valeur courante de la
    /// constante — écrire un littéral ici périmerait le test au bump suivant.
    #[test]
    fn cached_report_ne_sert_que_la_version_courante() {
        use super::{cached_report, REPORT_CACHE_VERSION};
        let j = || Some("{\"tags_cdj_ok\":true}".to_string());
        assert_eq!(
            cached_report(j(), Some(REPORT_CACHE_VERSION)),
            j(),
            "version courante : le rapport en cache doit être servi tel quel"
        );
        assert_eq!(
            cached_report(j(), Some(REPORT_CACHE_VERSION - 1)),
            None,
            "version distancée (ligne d'avant le bump) : pas de rapport courant"
        );
        assert_eq!(
            cached_report(j(), None),
            None,
            "version absente (colonne jamais stampée) : pas de rapport courant"
        );
        assert_eq!(
            cached_report(Some(String::new()), Some(REPORT_CACHE_VERSION)),
            None,
            "sentinelle d'échec (`report_json=''`) : pas un rapport, même à la bonne version"
        );
    }

    /// La durée décodée doit être MESURÉE, pas recopiée de l'en-tête.
    ///
    /// La fixture est fabriquée ici et pas prise dans `fixtures/` : `truncated.wav` est un fichier
    /// COMPLET de 1,5 s malgré son nom, donc son en-tête et son contenu s'accordent et il ne
    /// discrimine rien — une première version de ce test l'utilisait et passait AUSSI quand on
    /// remplissait le champ avec `tag.duration_sec`. Ce qu'il faut est un en-tête qui MENT.
    ///
    /// On écrit donc un WAV dont le chunk `data` annonce deux fois les octets réellement présents :
    /// c'est exactement le cas d'un téléchargement interrompu, et le seul où les deux durées
    /// divergent.
    #[test]
    fn decoded_duration_is_measured_not_copied_from_the_header() {
        const SR: u32 = 44100;
        const REAL_SAMPLES: u32 = SR; // 1 s réellement présente
        let declared_bytes = REAL_SAMPLES * 2 * 2; // on ANNONCE le double (2 s)
        let real_bytes = REAL_SAMPLES * 2; // mono 16 bits => 1 s d'octets

        let mut w = Vec::new();
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&(36 + declared_bytes).to_le_bytes());
        w.extend_from_slice(b"WAVEfmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes()); // PCM
        w.extend_from_slice(&1u16.to_le_bytes()); // mono
        w.extend_from_slice(&SR.to_le_bytes());
        w.extend_from_slice(&(SR * 2).to_le_bytes()); // byte rate
        w.extend_from_slice(&2u16.to_le_bytes()); // block align
        w.extend_from_slice(&16u16.to_le_bytes()); // bits
        w.extend_from_slice(b"data");
        w.extend_from_slice(&declared_bytes.to_le_bytes());
        for n in 0..REAL_SAMPLES {
            let v = ((n as f32 * 440.0 * std::f32::consts::TAU / SR as f32).sin() * 12000.0) as i16;
            w.extend_from_slice(&v.to_le_bytes());
        }
        assert_eq!(w.len() as u32, 44 + real_bytes, "fixture mal formee");

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("header_lies.wav");
        std::fs::write(&path, &w).unwrap();
        let p = path.to_string_lossy().to_string();

        let r = analyze(&p, false).expect("analyze");
        // Contrôle positif : sans lui, un zéro partout passerait l'assertion suivante.
        assert!(
            r.decoded_duration_sec > 0.5,
            "rien n'a été décodé, la mesure ne vaut rien: {}",
            r.decoded_duration_sec
        );
        assert!(
            (r.decoded_duration_sec - 1.0).abs() < 0.15,
            "1 s de son est réellement présente, mesure {}",
            r.decoded_duration_sec
        );
        // Ce que la fixture établit, et ce qu'elle n'établit pas — mesuré, pas supposé : sur ce
        // WAV incohérent lofty ne rend PAS la durée annoncée, il rend **0** (il refuse de croire
        // l'en-tête plutôt que de le recopier). Donc `duration_sec` vaut 0 ici, et c'est ce qui
        // fait tenir ce test sous mutation : remplir le champ avec `tag.duration_sec` donne 0 et
        // fait tomber le contrôle positif ci-dessus. Vérifié en mutant réellement le code.
        //
        // En revanche la fixture ne montre PAS le cas « l'en-tête annonce plus que le contenu »
        // vu depuis `duration_sec`, puisque lofty s'y dérobe. Ce cas-là existe (un MP3 dont le
        // Xing ment) et n'est couvert par aucun test.
        assert_eq!(
            r.duration_sec, 0.0,
            "lofty rend 0 sur cet en-tête incohérent — si ça change, ce test doit être relu"
        );
    }

    use super::*;

    /// Mirrors shared/contracts.ts's `Spectrogram`. Exhaustive destructure (no `..`): fails to
    /// compile if a field is added/removed/renamed on the Rust struct — the forcing function to
    /// also update contracts.ts. Phase 2 — docs/superpowers/plans/2026-07-13-phase2-ipc-contract-tests.md.
    #[test]
    fn spectrogram_shape_matches_contracts_ts() {
        let v = Spectrogram {
            frames: 0,
            bins: 0,
            hz_per_bin: 0.0,
            sec_per_frame: 0.0,
            // NOTE: this destructure only checks the field's PRESENCE. It compiles unchanged when
            // the wire type diverges (Rust `Vec<u8>` ↔ base85 string ↔ `Uint8Array` in
            // shared/contracts.ts:84) — it will NOT catch a type mismatch with the TS mirror.
            mag_db: Vec::new(),
        };
        let Spectrogram {
            frames,
            bins,
            hz_per_bin,
            sec_per_frame,
            mag_db,
        } = v;
        let _ = (frames, bins, hz_per_bin, sec_per_frame, mag_db);
    }

    /// Mirrors shared/contracts.ts's `AnalysisReport`. Exhaustive destructure (no `..`): fails to
    /// compile if a field is added/removed/renamed on the Rust struct — the forcing function to
    /// also update contracts.ts. Phase 2 — docs/superpowers/plans/2026-07-13-phase2-ipc-contract-tests.md.
    #[test]
    fn analysis_report_shape_matches_contracts_ts() {
        let v = AnalysisReport {
            path: String::new(),
            sample_rate: 0,
            channels: 0,
            duration_sec: 0.0,
            declared_format: String::new(),
            declared_bitrate: None,
            declared_rail: Rail::Unknown,
            cutoff_hz: 0.0,
            verdict: Verdict::Ok,
            container_mismatch: false,
            est_kbps: 0,
            peaks: Vec::new(),
            peaks_step: PEAKS_WINDOW,
            spectrogram: Spectrogram {
                frames: 0,
                bins: 0,
                hz_per_bin: 0.0,
                sec_per_frame: 0.0,
                mag_db: Vec::new(),
            },
            clip_runs: 0,
            clip_pct: 0.0,
            true_peak_dbtp: 0.0,
            dc_offset: 0.0,
            phase_correlation: 0.0,
            dual_mono: false,
            container_ok: false,
            codec_error: None,
            truncated: false,
            hf_flatness_db: None,
            hf_flatness_top_db: None,
            decoded_duration_sec: 0.0,
            quant_likelihood: None,
            silence_head_ms: 0,
            silence_tail_ms: 0,
            id3_version: None,
            tags_cdj_ok: false,
            has_cover: false,
        };
        let AnalysisReport {
            path,
            sample_rate,
            channels,
            duration_sec,
            declared_format,
            declared_bitrate,
            declared_rail,
            cutoff_hz,
            verdict,
            container_mismatch,
            est_kbps,
            peaks,
            peaks_step,
            spectrogram,
            clip_runs,
            clip_pct,
            true_peak_dbtp,
            dc_offset,
            phase_correlation,
            dual_mono,
            container_ok,
            codec_error,
            truncated,
            hf_flatness_db,
            hf_flatness_top_db,
            decoded_duration_sec,
            quant_likelihood,
            silence_head_ms,
            silence_tail_ms,
            id3_version,
            tags_cdj_ok,
            has_cover,
        } = v;
        let _ = (
            path,
            sample_rate,
            channels,
            duration_sec,
            declared_format,
            declared_bitrate,
            declared_rail,
            cutoff_hz,
            verdict,
            container_mismatch,
            est_kbps,
            peaks,
            peaks_step,
            spectrogram,
            clip_runs,
            clip_pct,
            true_peak_dbtp,
            dc_offset,
            phase_correlation,
            dual_mono,
            container_ok,
            codec_error,
            truncated,
            hf_flatness_db,
            hf_flatness_top_db,
            decoded_duration_sec,
            quant_likelihood,
            silence_head_ms,
            silence_tail_ms,
            id3_version,
            tags_cdj_ok,
            has_cover,
        );
    }

    #[test]
    fn report_serializes_to_json() {
        let r = AnalysisReport {
            path: "x.flac".into(),
            sample_rate: 44100,
            channels: 2,
            duration_sec: 1.0,
            declared_format: "flac".into(),
            declared_bitrate: None,
            declared_rail: Rail::Lossless,
            cutoff_hz: 21000.0,
            verdict: Verdict::Ok,
            container_mismatch: false,
            est_kbps: 320,
            peaks: vec![0.0, 1.0],
            peaks_step: PEAKS_WINDOW,
            spectrogram: Spectrogram {
                frames: 1,
                bins: 3,
                hz_per_bin: 10.0,
                sec_per_frame: 0.1,
                mag_db: vec![0, 127, 255],
            },
            clip_runs: 0,
            clip_pct: 0.0,
            true_peak_dbtp: -1.0,
            dc_offset: 0.0,
            phase_correlation: 1.0,
            dual_mono: false,
            container_ok: true,
            codec_error: None,
            truncated: false,
            hf_flatness_db: None,
            hf_flatness_top_db: None,
            decoded_duration_sec: 0.0,
            quant_likelihood: None,
            silence_head_ms: 0,
            silence_tail_ms: 0,
            id3_version: None,
            tags_cdj_ok: true,
            has_cover: false,
        };
        let j = serde_json::to_string(&r).unwrap();
        assert!(j.contains("\"verdict\":\"ok\""));
        assert!(j.contains("\"declared_rail\":\"lossless\""));
        assert!(j.contains("\"container_mismatch\":false"));
        // mag_db must be a base85 STRING, never the historic array of decimal integers — that
        // encoding is the whole point of REPORT_CACHE_VERSION 6 (see crate::b85_bytes).
        assert!(
            j.contains("\"mag_db\":\""),
            "mag_db must serialize as a string: {j}"
        );
        assert!(!j.contains("\"mag_db\":["), "mag_db regressed to an array");
        let back: AnalysisReport = serde_json::from_str(&j).unwrap();
        assert_eq!(back.spectrogram.mag_db, vec![0u8, 127, 255]);
        assert_eq!(back, r);
    }

    /// BUG-1 end-to-end: an MP3 renamed with a `.flac` extension must be caught by
    /// `analyze()` as Fake, via `tags::rail_from_content` sniffing the real magic bytes
    /// (not just the declared/extension rail, which is fooled).
    #[test]
    fn analyze_catches_a_renamed_mp3_as_fake() {
        let p = "fixtures/real_320.mp3";
        if !std::path::Path::new(p).exists() {
            eprintln!("skip: no fixture");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let disguised = dir.path().join("disguised.flac");
        std::fs::copy(p, &disguised).unwrap();
        let path = disguised.to_str().unwrap();

        let report = analyze(path, false).unwrap();
        assert_eq!(report.verdict, Verdict::Fake);
    }

    /// A genuine, honestly-extensioned MP3 must never report `container_mismatch`. Content
    /// sniffing runs unconditionally in `tags::read()` regardless of declared rail, so
    /// `content_rail == Rail::Lossy` alone is true for every ordinary MP3 too — the
    /// `declared_rail == Rail::Lossless` half of the check is load-bearing, not redundant.
    #[test]
    fn analyze_does_not_flag_a_genuine_mp3_as_container_mismatch() {
        let p = "fixtures/real_320.mp3";
        if !std::path::Path::new(p).exists() {
            eprintln!("skip: no fixture");
            return;
        }
        let report = analyze(p, false).unwrap();
        assert_eq!(report.declared_rail, Rail::Lossy);
        assert!(!report.container_mismatch);
    }
}
