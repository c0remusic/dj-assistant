//! Pure verdict logic, keyed on the detected cutoff + DECLARED rail/bitrate.
//!
//! Two frauds are flagged as `Fake`:
//! 1. **Fake lossless** — declared FLAC/WAV/AIFF but the spectrum shows a lossy lowpass cliff.
//! 2. **Over-encoded lossy** — declared e.g. 320 kbps MP3 but the cutoff is far below what
//!    that bitrate produces → it was re-encoded UP from a lower-quality source.
//!
//! An honestly-labelled low-bitrate MP3 (cutoff matches its bitrate) stays `Ok` here — its
//! "below the user's quality threshold" handling is a separate axis (M4 rules).

use crate::analysis::{Rail, Verdict};

/// Version du PRODUCTEUR de verdict, stampée dans `tracks.verdict_ver` par
/// `worker::persist_report` (migration v22). Rien ne l'expose au frontend.
///
/// **À incrémenter dès que `verdict()` ne rend plus le MÊME verdict pour les mêmes entrées** :
/// les bandes de décision (`LOSSLESS_OK_HZ`, `LOSSY_CLIFF_HZ`, `NO_MEASUREMENT_HZ`), le barème de
/// `min_cutoff_hz_for_bitrate`, les planchers de platitude (`HF_FIXED_FLOOR_DB`,
/// `HF_TOP_FLOOR_DB`, tous deux annoncés provisoires ci-dessous), ou l'arrivée d'une entrée
/// nouvelle dans l'arbitrage.
///
/// ⚠️ **Elle n'est PAS redondante avec `analysis::REPORT_CACHE_VERSION`** — établi en lisant qui
/// écrit la colonne `verdict`, pas supposé (issue #39, « ce qui n'est pas tranché ici ») :
///
/// - `worker::persist_report` écrit `verdict` et `report_cache_ver` dans le MÊME `UPDATE`, donc
///   les deux sont d'accord à l'écriture ;
/// - `worker::select_needing_analysis` re-sélectionne sur une version de verdict périmée — en
///   `status='pending'` seulement jusqu'au 2026-09-09, puis sur la bibliothèque rangée aussi
///   (issue #59, voir `worker::STATUSES_TO_ANALYSE`), la file d'abord.
///
/// **Le troisième trou est fermé depuis le 2026-09-01.** `ipc::analyze_path` réparait le cache sur
/// désaccord de version en réécrivant `report_json` + `report_cache_ver` **sans toucher
/// `verdict`** : une piste rangée retrouvait un rapport courant en gardant dans sa colonne le
/// verdict calculé par l'ANCIEN moteur — et c'est cette colonne, pas le rapport, que lisent la
/// Bibliothèque, les Écartés et le compte « à re-sourcer ». La réparation passe désormais par
/// `ipc::heal_cache`, qui écrit les quatre colonnes ensemble depuis le rapport frais qu'elle a déjà
/// en main (`heal_cache_repare_aussi_le_verdict_pas_seulement_le_rapport`). Reste vrai, et c'est
/// pourquoi les deux constantes restent distinctes : le commentaire de la migration v21 (`db.rs`)
/// note que « `verdict` et `cutoff_hz` ne sont couverts par AUCUNE version de cache » — les deux
/// versions bougent pour des raisons différentes, un bump de forme de rapport n'est pas un
/// changement d'arbitrage.
///
/// La bibliothèque rangée que personne n'ouvre est rattrapée, elle, par `reverdict::run` au
/// démarrage : re-verdict depuis les mesures déjà stockées, sans ré-analyse.
///
/// Historique : la lane « durcissement de domaine » du 2026-09-01 (matin) n'avait PAS incrémenté —
/// la sortie des cas « pas mesurable » ne changeait aucun verdict réel (cas 1/4/5 comptés 0/0/0
/// sur les 3 386 pistes de la base). La condition de bump qu'elle posait — « le jour où un SEUIL
/// bouge » — est arrivée le jour même : falaise 19 500 → 20 000 et plancher fixe -5,8 → -12
/// (référence assainie, voir leurs commentaires). D'où **2**. Coût : `cached()` efface les 3 386
/// verdicts au prochain lancement — mais le chemin « re-verdict depuis les mesures stockées, sans
/// ré-analyse » existe désormais (`reverdict::run`, écrit le jour même) et les rejuge au démarrage,
/// au lieu des 15-30 min de CPU en fond qu'aurait coûté une ré-analyse complète par le pool.
///
/// **3, le 2026-09-02 (issue #52) : la vraisemblance de quantification entre dans l'arbitrage.**
/// `verdict()` prend un sixième paramètre, `quant_likelihood`, et **tout le bras de la bande
/// pleine** — ses deux issues, `Ok` comme `Grey` — peut désormais rendre `Fake` (voir
/// [`QUANT_LAMBDA`]). C'est exactement la condition de bump écrite ci-dessus, « l'arrivée d'une
/// entrée nouvelle dans l'arbitrage ».
///
/// ⚠️ **Ce bump ne fait PAS basculer la bibliothèque rangée.** `reverdict::run` rejoue le verdict
/// depuis les mesures STOCKÉES et n'a aucun PCM sous la main : il passe `None`, et `None` ne
/// dégrade jamais un verdict. Une piste rangée est donc re-stampée avec le verdict qu'elle
/// portait, et le gardera jusqu'à une ré-analyse RÉELLE (ouverture en Revue via
/// `ipc::analyze_path`, ou reprise par le pool). C'est voulu : la seule alternative serait de
/// stocker le signal, ou de re-décoder toute la bibliothèque au démarrage.
///
/// ⚠️ **Sauf que « reprise par le pool » n'existait pas pour une piste rangée (issue #59,
/// 2026-09-09).** Le bump de rapport v10, le même jour, a rendu toute rangée analysée avant lui
/// invisible à `reverdict::run` (filtre `report_cache_ver = courant`) — verdict effacé à la lecture,
/// et le pool borné à la file. Depuis #59, le pool reprend aussi `filed` et `resourcing`, après la
/// file. Conséquence à connaître le jour d'un prochain bump : **un bump de RAPPORT re-décode la
/// bibliothèque rangée en fond**, comme il re-décode déjà la file ; un bump de VERDICT seul reste
/// rejoué ici sans décodage.
///
/// **4 (2026-09-11)** : `LOSSLESS_OK_HZ` 20 000 → 20 750, fenêtre LAME 320 rendue `Grey`. Bump de
/// VERDICT seul : rejoué au démarrage depuis le `cutoff_hz` stocké, sans décodage.
pub const VERDICT_CACHE_VERSION: i64 = 4;

/// Lit le cache `(tracks.verdict, tracks.verdict_ver)`. Version absente (NULL — ligne d'avant la
/// v22 que son backfill n'a pas stampée) ou différente = **pas de verdict courant**, rendu comme
/// `None`. C'est exactement l'état qu'un `verdict` NULL décrit déjà et que toute l'app sait
/// afficher : file d'attente qui propose « Réanalyser », badge absent en Bibliothèque, ligne hors
/// du compte « à re-sourcer ». Jamais une erreur.
///
/// Même raison qu'`fingerprint::cached` de passer par une fonction plutôt que par un littéral en
/// SQL : la constante reste déclarée une seule fois.
pub fn cached(raw: Option<String>, ver: Option<i64>) -> Option<String> {
    match ver {
        Some(v) if v == VERDICT_CACHE_VERSION => raw,
        _ => None,
    }
}

/// Decision bands (Hz) for a file DECLARED lossless. `cutoff_hz` is stored raw upstream so
/// these thresholds stay reconfigurable without re-analysis (Réglages, M2b+).
/// ≥ → bande pleine : la coupure n'a plus rien à dire, la platitude et la sonde tranchent.
///
/// **20 750 depuis le 2026-09-11 (fenêtre LAME 320).** Un MP3 encodé par LAME à 320 kbps CBR
/// pose un passe-bas à 20,5 kHz : mesuré sur les 20 `lame320` de `C:\sift-corpus` (deux scans),
/// coupure entre 20 177 et 20 704 Hz — donc AU-DESSUS de l'ancienne borne de 20 000, et 20/20
/// jugés Vrai par le détecteur livré en v0.1.1. Les authentiques vérifiés au conteneur mesurent
/// 22 050 (8/8) ; les roll-offs naturels de la référence ACID sont recensés dans la review du corpus
/// (§ Audit de provenance). Entre la falaise et cette borne, une déclaration lossless est
/// `Grey` — « à vérifier visuellement » — jamais `Fake` : un mur à 20,3 kHz se voit au
/// spectrogramme, mais un roll-off doux d'un master peut y tomber, et la contrainte reste zéro
/// faux positif. Réglable sans ré-analyse : `cutoff_hz` est stocké brut.
pub const LOSSLESS_OK_HZ: f32 = 20750.0;
/// ≤ → passe-bas d'encodeur lossy → `Fake`. 20 000 depuis le 2026-09-01 (issue #51) : l'intervalle
/// (19 500, 20 000) ne contenait que des faux — 23 lame256, coupures 19 488-19 907 — et zéro
/// authentique (mur écarté à 19 692, premier vrai roll-off bien au-dessus).
///
/// La fenêtre (20 000, 20 750) rouverte le 2026-09-11 n'est PAS l'ancienne fenêtre douteuse
/// (19 500, 20 000) qui était morte parce que peuplée de faux seulement : celle-ci est peuplée
/// de faux (LAME 320) ET potentiellement d'authentiques à roll-off, donc elle rend `Grey`, pas
/// `Fake`. Voir `LOSSLESS_OK_HZ`.
pub const LOSSY_CLIFF_HZ: f32 = 20000.0;

/// Ce que rend `spectrum::detect_cutoff` quand il n'avait **rien à mesurer** — aucune trame
/// décodée. Ce n'est pas une coupure à 0 Hz : c'est l'absence de mesure.
///
/// La valeur est un sentinel sans ambiguïté, et pas par convention : les deux seules autres
/// sorties de `detect_cutoff` sont `k * hz_per_bin` avec `k` au-dessus de son plancher de
/// balayage (donc > 2 kHz) et `nyq_hz`. Zéro n'est atteignable que par le retour anticipé.
///
/// Mesuré le 2026-08-17 sur la bibliothèque réelle : **deux MP3** de plus de six minutes,
/// déclarés 320 kbps, `codec_error` NULL — donc aucune erreur remontée nulle part — rendaient
/// `cutoff_hz = 0`, que `verdict()` lisait comme « coupe à 0 Hz, très en dessous du plancher de
/// 19 000 pour du 320 » et marquait FAKE. Un échec de décodage se présentait comme une preuve
/// de fraude.
pub const NO_MEASUREMENT_HZ: f32 = 0.0;

/// Minimum cutoff a *genuine* MP3 of the given bitrate should reach (≈ encoder lowpass minus
/// a margin for genre/encoder spread). A declared bitrate whose real cutoff is below this is
/// over-encoded (transcoded up from a worse source).
pub fn min_cutoff_hz_for_bitrate(kbps: u32) -> f32 {
    match kbps {
        b if b >= 320 => 19000.0,
        b if b >= 256 => 18000.0,
        b if b >= 192 => 16500.0,
        b if b >= 160 => 15500.0,
        b if b >= 128 => 14500.0,
        _ => 12000.0,
    }
}

/// Equivalent lossy bitrate for a measured cutoff, read off the SAME tiers `verdict()` uses to
/// call a bitrate over-encoded (FIX-11: this used to be duplicated in report-view.ts with a
/// shifted table — e.g. a cutoff the verdict logic scored against the 192kbps band showed as
/// "≈256 kbps" in the UI). Rust is the single source of truth; the front just displays this.
pub fn estimate_kbps(cutoff_hz: f32) -> u32 {
    const TIERS: [u32; 5] = [320, 256, 192, 160, 128];
    for b in TIERS {
        if cutoff_hz >= min_cutoff_hz_for_bitrate(b) {
            return b;
        }
    }
    128
}

/// Les deux bandes de platitude de l'aigu, passées ENSEMBLE et NOMMÉES.
///
/// Deux `Option<f32>` positionnels côte à côte s'inverseraient sans que rien ne le dise — et les
/// inverser change le sens : la bande fixe (16-20 kHz) et la bande relative (0,80-0,98 × Nyquist)
/// n'ont ni la même plage de valeurs ni les mêmes angles morts. Voir `spectrum.rs`.
///
/// `Default` = aucune mesure, et c'est un état réel : la bande n'existe pas à tous les taux
/// d'échantillonnage, et un fichier trop court ne rend aucune trame.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HfFlatness {
    /// Bande FIXE 16-20 kHz, médiane sur les trames, en dB.
    pub fixed_db: Option<f32>,
    /// Bande RELATIVE au Nyquist, médiane sur les trames, en dB.
    pub top_db: Option<f32>,
}

/// Planchers de la plage des masters, une par bande. Un lossless à bande pleine qui passe SOUS
/// l'un des deux n'est plus annoncé Vrai lossless — il devient Douteux.
///
/// ⚠️ **Ce ne sont PAS les bornes d'affichage de `frontend/report-figures.ts`, et c'est délibéré.**
/// Celles-là (`HF_REF_LO = -5.4`) viennent de `scripts/hf-flatness-probe.mjs` : décodage forcé en
/// mono 44,1 kHz, DFT naïve, 200 trames échantillonnées, 150 s au plus. Le juge, lui, mesure au
/// taux natif, sur tout le fichier, par la FFT de `spectrum.rs`. **Deux chemins de mesure, deux
/// valeurs** — et un seuil n'est comparable qu'aux mesures qui l'ont produit.
///
/// Ce que le mélange coûtait, mesuré le 2026-08-18 par un scan complet du corpus : à -5,4, le
/// fichier `src09` du corpus — un ACHAT — mesure -5,79 par le chemin Rust et bascule en Douteux.
/// Un faux positif sur du matériel acheté, produit par un seuil que le code qui juge n'avait
/// jamais mesuré. La sonde le situait à -5,4 ; le review en a même tiré une « distribution
/// bimodale » avec un vide entre -6,5 et -5,4, où ce fichier tombe pourtant.
///
/// D'où vient -5,8 : minimum des 10 authentiques du corpus MESURÉS PAR CE CHEMIN (-5,79), arrondi
/// vers le bas — la valeur exacte serait une borne posée sur un fichier réel, à la merci de son
/// troisième chiffre. Coût mesuré : 98/150 au lieu de 105/150, contre 0/10 faux positifs au lieu
/// de 1/10.
///
/// ⚠️ L'avertissement « provisoire, sur DIX fichiers » de -5,8 a été LEVÉ le 2026-09-01 par la
/// re-dérivation qu'il demandait : 137 lossless d'un dossier de provenance déclarée sûre
/// (D:\MUSIQUE\ACID), mesurés par CE chemin (`corpus_scan`), et — leçon nouvelle — **assainis au
/// spectrogramme** : le dossier contenait des transcodes avérés (coupures 16-18 kHz, murs nets à
/// 19,4-20,4 kHz dont un EP entier), donc le min brut du dossier (-11,75… sur fichier au MUR)
/// n'était pas prenable tel quel. Sont restés dans la référence les fichiers au roll-off naturel,
/// vérifiés un par un (18 spectrogrammes lus) ; leur minimum : **-11,75 dB** (Ege Bam Yasi 1993,
/// roll-off doux confirmé). Plancher posé à -12, arrondi sous ce minimum.
///
/// Coût assumé, mesuré sur la matrice de 345 faux : détection par l'union 69,3 % → 47 % —
/// les transcodes haut débit réemballés passent, ce que TOUTE méthode spectrale rate (littérature,
/// issue #51) ; le rattrapage propre est la détection par traces MDCT, ticket dédié. En échange :
/// zéro master sombre ambré, là où -5,8 ambrait ~20 % du dossier réel (23/114) — dont des achats.
/// Un mur près de Nyquist (≥ ~20,5 kHz) reste ambigu : un anti-aliasing DAT d'époque en fait un
/// vrai, seul un mur FRANC sous ~20 kHz est damnant.
const HF_FIXED_FLOOR_DB: f32 = -12.0;

/// Bande relative. Valeur issue de la sonde, CONFIRMÉE le 2026-09-01 par le chemin qui juge :
/// sur les 137 lossless de la référence élargie, minimum mesuré -19,36 — aucun authentique n'en
/// dépend, et elle seule attrape 85/345 faux (Opus et lame192-256 notamment) à zéro coût.
/// Inchangée, désormais mesurée.
const HF_TOP_FLOOR_DB: f32 = -23.8;

/// Seuil de décision de la vraisemblance de quantification — le `λ` du mémo de l'issue #52
/// (méthode d'Olivier Derrien, JAES 67(3), 2019). Au-dessus, un lossless à bande pleine dont
/// l'aigu est creux n'est plus Douteux mais **Faux** : la grille du codec a été retrouvée dans son
/// signal, et un transcodage établi par la grille n'est pas un doute.
///
/// ⚠️ **PROVISOIRE, et la valeur le dit.** Elle vient de la calibration du 2026-09-02 sur
/// `C:\sift-corpus` (le corpus du 2026-08-17 : 10 sources × 15 encodeurs = 150 faux, **10
/// authentiques**), harnais `quant_trace::corpus::quant_scan`, réglage 8×8, saut 17. Le critère est
/// celui du mémo — la plus petite valeur à **zéro faux positif** sur NOS authentiques, jamais reprise
/// du papier, dont les encodeurs (iTunes) ne sont pas les nôtres (ffmpeg `aac` / `aac_mf`) :
///
/// - maximum authentique mesuré **0,172** (`src10`, blocs longs, décalage 1009) — spectrogramme
///   relu à l'œil le jour même : roll-off naturel, vrai master, le 0,172 est du bruit de blocs
///   longs et pas un transcodage caché ;
/// - les neuf autres authentiques ≤ 0,094 ;
/// - d'où **0,18**, juste au-dessus du maximum authentique.
///
/// **Dix authentiques ne calibrent pas sérieusement un seuil à zéro faux positif.** À re-dériver
/// sur le corpus régénéré de #52 (≥ 23 authentiques vettés au spectrogramme, `make-corpus.mjs`),
/// par le code qui l'applique. Autres fragilités consignées le même jour : une seule famille
/// musicale, encodeurs ffmpeg seulement, et un taux de faux positifs PAR CELLULE mesuré à
/// 0,42-0,61 % (`k_eff` 7..15) et non les 1 % nominaux — voir le Monte-Carlo de `quant_trace`.
///
/// Détection observée à cette valeur, ventilée : aacmf128+aacmf256 **20/20**, aac256 **8/10**,
/// aac128 **7/10**, MP3 2/60 (attendu — banc hybride PQMF+MDCT absent, phase 2), divers 12/50.
///
/// `f32` et non `f64` : la comparaison se fait contre la valeur telle qu'elle voyage dans
/// `AnalysisReport::quant_likelihood`. Aucun `L` atteignable n'est ambigu à cette précision — les
/// `L` sont des multiples de 1/64, et les deux plus proches de 0,18 sont 0,171875 et 0,1875.
pub const QUANT_LAMBDA: f32 = 0.18;

/// Vrai quand au moins une des deux bandes sort par le bas de la plage des masters.
///
/// L'UNION et pas l'intersection : mesuré sur le corpus étiqueté, les deux bandes sont aveugles à
/// des endroits différents — la fixe ne voit aucun Opus, la relative ne voit aucun MP3 128, dont
/// la coupure passe sous elle. Exiger les deux ramènerait la détection à ce que chacune rate.
///
/// Une bande non mesurée n'est pas un grief : `None` ne déclenche rien.
fn below_master_range(f: HfFlatness) -> bool {
    f.fixed_db.is_some_and(|v| v < HF_FIXED_FLOOR_DB)
        || f.top_db.is_some_and(|v| v < HF_TOP_FLOOR_DB)
}

/// Pourquoi la décision n'a **pas pu être rendue**. Ce n'est pas un verdict, et c'est tout
/// l'intérêt du type : une absence de mesure n'est ni un doute, ni un blanchiment, ni une
/// accusation — c'est un échec d'analyse, que l'appelant route vers le chemin d'échec existant
/// (`persist_failure`, `analysis_attempts`, facette « Non analysés »).
///
/// **Décision du 2026-09-01 (issue #51, lane « durcissement de domaine »).** Trois bras de
/// `verdict()` rendaient `Verdict::Grey` pour « rien mesuré » : coupure sentinelle sous Lossless,
/// la même sous Lossy, et `Rail::Unknown` déclaré. `Grey` agrégeait donc du vrai doute spectral et
/// de l'absence de mesure, et la pastille ambre ne signalait plus rien.
///
/// Forme retenue — un `Result` rendu par `verdict()` plutôt qu'une garde chez l'appelant : la
/// condition « pas mesurable » n'est pas indépendante de la décision, elle s'articule avec elle
/// (le court-circuit fraude passe AVANT, le bras de coupure passe avant celui du débit). Une garde
/// chez l'appelant aurait dupliqué cet ordre hors du seul endroit qui le teste, et le seam de test
/// convenu par la spec est `verdict()`. Enum à la main + `Display` + `impl Error`, comme
/// `MasterDbError` — le dépôt n'utilise ni `thiserror` ni `anyhow`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotMeasured {
    /// `spectrum::detect_cutoff` n'avait rien à mesurer — aucune trame décodée. Voir
    /// `NO_MEASUREMENT_HZ`.
    Cutoff,
    /// Le rail DÉCLARÉ est `Rail::Unknown` : ni l'extension ni le conteneur n'ont conclu, donc
    /// aucune des deux grilles de décision (lossless / lossy) ne s'applique.
    Rail,
}

impl std::fmt::Display for NotMeasured {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotMeasured::Cutoff => {
                write!(f, "aucune coupure spectrale mesurée (aucune trame décodée)")
            }
            NotMeasured::Rail => write!(f, "rail indéterminé : conteneur non reconnu"),
        }
    }
}

impl std::error::Error for NotMeasured {}

/// Vrai quand — et seulement quand — la sonde de quantification a quelque chose à trancher.
///
/// **BANDE PLEINE, un point c'est tout** : rail lossless déclaré, conteneur non démenti, coupure
/// réellement mesurée et à `LOSSLESS_OK_HZ` ou au-dessus. Sous la falaise, le verdict est déjà
/// Faux ; sur un désaccord de conteneur, il l'est aussi et le court-circuit passe avant tout.
/// Ailleurs, la sonde ne changerait rien.
///
/// ⚠️ **Elle ne teste PAS la platitude, et cette clause a été retirée le 2026-09-02 après
/// mesure.** La première intégration ne sondait que le bras `Grey` — bande pleine ET aigu SOUS la
/// plage des masters. Or la cible même de l'issue #52 — les transcodes AAC haut débit,
/// invisibles au spectre — a une platitude DANS la plage depuis la re-dérivation du plancher à
/// -12 (#51) : elle sort en `Ok`, pas en `Grey`. Mesuré sur les 160 fichiers de `C:\sift-corpus` :
/// avec la clause, les 40 fichiers `aac256`/`aacmf128`/`aacmf256`/`aac128` haut débit n'étaient
/// **jamais** sondés (`quant_l = "-"`), et l'intégration entière valait +2 détections sur des
/// familles hors cible (un `opus128`, un `wma192`). La condition qui vise la cible est donc la
/// bande pleine seule.
///
/// **Coût, dit et non minimisé.** La sonde tourne désormais sur ~tout lossless SAIN à l'analyse,
/// pas sur une poignée d'ambigus : +0,3 s de balayage sur une analyse de 2-4 s, soit ~10 %.
/// Mesuré le 2026-09-02 sur les 160 fichiers étiquetés de `C:\sift-corpus` (`corpus_scan`,
/// `--release`) :
///
/// | | fichiers | part |
/// |---|---|---|
/// | sonde DEMANDÉE (lossless + bande pleine) | 111 | **69,4 %** |
/// | mesure effectivement RENDUE | 98 | 61,3 % |
/// | demandée mais sans mesure (plafond PCM) | 13 | 8,1 % |
///
/// ⚠️ Ces 69 % sont la part d'un corpus **saturé de faux à bande pleine** (150 transcodes pour 10
/// authentiques), pas celle d'une bibliothèque réelle — sur laquelle le chiffre n'a pas été
/// mesuré. Les 13 sans mesure sont les 13 variantes d'un seul morceau de 10 min 53 s, au-delà de
/// `analysis::QUANT_MAX_PCM_SAMPLES` : `None`, donc verdict inchangé.
///
/// Ce n'est plus « à la demande » au sens de « rare » — c'est « à la demande » au sens de
/// « seulement là où elle peut trancher ».
///
/// **Elle vit ici, pas chez l'appelant, pour une raison de couplage** : c'est la seule façon que
/// la condition de déclenchement et le bras qu'elle sert ne dérivent pas l'un de l'autre. Un
/// `analyze()` qui déciderait tout seul quand sonder aurait une copie de l'arbitrage hors du seul
/// endroit qui le teste — et une copie qui dérive dépense 0,3 s pour rien, ou pire, ne les dépense
/// pas là où le verdict attendait la mesure.
///
/// `verdict()` reste PUR : cette fonction ne mesure rien non plus, elle ne fait que nommer la
/// condition. Le calcul MDCT vit dans `analysis::analyze`, qui a le PCM.
pub fn needs_quant_probe(cutoff_hz: f32, declared: Rail, content_rail: Rail) -> bool {
    declared == Rail::Lossless
        && content_rail != Rail::Lossy
        && cutoff_hz > NO_MEASUREMENT_HZ
        // Depuis le 2026-09-11 la sonde couvre aussi la fenêtre (20 000, 20 750) : un fichier
        // qui y tombe est `Grey` par la coupure, et la grille d'un codec retrouvée le rend
        // `Fake` — le doute ne doit pas désarmer la mesure qui pourrait le lever.
        && cutoff_hz > LOSSY_CLIFF_HZ
}

/// Maps cutoff + declared rail + declared bitrate to a verdict, ou dit pourquoi il n'y a pas de
/// verdict à rendre (`NotMeasured`).
///
/// `quant_likelihood` est le troisième signal (issue #52) : la vraisemblance que le signal soit
/// déjà passé par la grille de quantification d'un codec AAC, mesurée par
/// `analysis::quant_trace::likelihood` sur le PCM décodé. **`None` = pas mesurée**, et c'est un
/// état réel et fréquent — chemin de re-verdict depuis les mesures stockées (`reverdict.rs`), taux
/// d'échantillonnage hors des tables AAC, signal trop court, ou tout simplement un fichier que
/// [`needs_quant_probe`] n'a pas désigné. **`None` ne dégrade JAMAIS un verdict** : c'est la même
/// règle que pour les bandes de platitude non mesurées, et pour la même raison — accuser un
/// fichier de n'avoir pas pu être mesuré est l'erreur que ce module passe son temps à corriger.
///
/// `content_rail` is the rail sniffed from the actual container/codec (independent of the
/// declared extension/tag) — when it disagrees with `declared` on the lossless side (declared
/// FLAC/WAV/AIFF but the container is actually a lossy codec, e.g. an MP3 renamed to `.flac`),
/// that mismatch alone is fraud, before even looking at the spectral cutoff: a renamed lossy
/// source can coincidentally have a cutoff high enough to pass the cutoff-only check below.
/// `Rail::Unknown` (sniffing failed/inconclusive) never triggers this short-circuit — it falls
/// through to the existing cutoff-based logic unchanged.
pub fn verdict(
    cutoff_hz: f32,
    declared: Rail,
    declared_bitrate: Option<u32>,
    content_rail: Rail,
    flatness: HfFlatness,
    quant_likelihood: Option<f32>,
) -> Result<Verdict, NotMeasured> {
    match declared {
        Rail::Lossless => {
            if content_rail == Rail::Lossy {
                // Le désaccord de conteneur reste EN PREMIER : c'est une fraude établie sans le
                // spectre, donc l'absence de mesure ne doit pas l'effacer.
                Ok(Verdict::Fake)
            } else if cutoff_hz <= NO_MEASUREMENT_HZ {
                // Rien mesuré → hors domaine (2026-09-01). Rendait `Grey` jusque-là.
                Err(NotMeasured::Cutoff)
            } else if cutoff_hz <= LOSSY_CLIFF_HZ {
                Ok(Verdict::Fake)
            } else if quant_likelihood.is_some_and(|l| l > QUANT_LAMBDA) {
                // Le TROISIÈME signal (issue #52), lu AVANT la platitude et AVANT la fenêtre — sur
                // tout ce que la falaise n'a pas tranché. La cible de #52 est précisément le fichier
                // dont la platitude est NORMALE et qui sortait donc `Ok` : le brancher sous le seul
                // `Grey` le rendait muet là où il compte (mesuré sur corpus, voir
                // `needs_quant_probe`).
                //
                // Une grille de codec retrouvée dans le signal n'est pas un doute — c'est un
                // transcodage établi, donc Faux.
                //
                // `is_some_and` et pas un `unwrap_or(0.0)` : la mesure absente doit sortir de la
                // comparaison, pas y entrer avec une valeur basse plausible.
                Ok(Verdict::Fake)
            } else if cutoff_hz < LOSSLESS_OK_HZ {
                // Fenêtre LAME 320 (2026-09-11) : coupure au-dessus de la falaise mais sous la
                // bande pleine. Peuplée de faux (LAME 320, 20 177-20 704 Hz) et possiblement de
                // roll-offs authentiques : un doute, à trancher à l'œil — jamais `Fake` d'office.
                Ok(Verdict::Grey)
            } else if below_master_range(flatness) {
                // Bande pleine : la coupure n'a plus rien à dire. C'est ici, et seulement ici, que
                // la platitude de l'aigu tranche — elle ne DÉGRADE jamais un verdict déjà négatif,
                // elle rattrape ce que la falaise ne peut pas voir. Inchangé depuis #51.
                Ok(Verdict::Grey)
            } else {
                Ok(Verdict::Ok)
            }
        }
        Rail::Lossy => match declared_bitrate {
            // Rien mesuré → hors domaine (2026-09-01 ; rendait `Grey` jusque-là). Ce bras passe
            // AVANT celui du débit : sans lui, un cutoff de 0 est inférieur à tous les planchers de
            // `min_cutoff_hz_for_bitrate` et accuse le fichier de sur-encodage sur la foi d'un
            // décodage qui n'a pas eu lieu.
            _ if cutoff_hz <= NO_MEASUREMENT_HZ => Err(NotMeasured::Cutoff),
            // declared bitrate the real spectrum can't support → over-encoded fraud
            Some(b) if cutoff_hz < min_cutoff_hz_for_bitrate(b) => Ok(Verdict::Fake),
            _ => Ok(Verdict::Ok),
        },
        // Sniffing non conclusif : aucune des deux grilles ne s'applique, donc rien à juger — hors
        // domaine (2026-09-01 ; rendait `Grey` jusque-là).
        Rail::Unknown => Err(NotMeasured::Rail),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les trois états que `cached` doit distinguer. Le cas NULL est celui de toute ligne d'avant
    /// la v22 que son backfill n'a pas stampée — un rapport lui-même périmé, typiquement.
    #[test]
    fn cached_ne_sert_que_la_version_courante() {
        let v = || Some("fake".to_string());
        assert_eq!(
            cached(v(), Some(VERDICT_CACHE_VERSION)),
            v(),
            "version courante : le verdict en cache doit être servi tel quel"
        );
        assert_eq!(
            cached(v(), None),
            None,
            "version absente (base d'avant la v22) : pas de verdict courant"
        );
        assert_eq!(
            cached(v(), Some(VERDICT_CACHE_VERSION + 1)),
            None,
            "version différente : pas de verdict courant"
        );
    }

    #[test]
    fn lossless_with_full_band_is_ok() {
        assert_eq!(
            verdict(
                21000.0,
                Rail::Lossless,
                None,
                Rail::Lossless,
                HfFlatness::default(),
                None
            ),
            Ok(Verdict::Ok)
        );
    }

    #[test]
    fn lossless_with_lossy_cliff_is_fake() {
        assert_eq!(
            verdict(
                16000.0,
                Rail::Lossless,
                None,
                Rail::Lossless,
                HfFlatness::default(),
                None
            ),
            Ok(Verdict::Fake)
        );
        assert_eq!(
            verdict(
                19000.0,
                Rail::Lossless,
                None,
                Rail::Lossless,
                HfFlatness::default(),
                None
            ),
            Ok(Verdict::Fake)
        );
    }

    // `lossless_in_grey_band_is_grey` (19 800 → Douteux) est parti le 2026-09-01 avec la fenêtre
    // qu'il gardait — son sujet n'existe plus. Le comportement de l'ex-fenêtre est désormais figé
    // par `la_fenetre_douteuse_est_morte_la_falaise_rejoint_20000`, remplacé le 2026-09-11 par
    // `la_fenetre_lame320_est_ambre_entre_la_falaise_et_la_bande_pleine` (fenêtre (20 000, 20 750)
    // rouverte en Grey pour LAME 320).

    /// Les VALEURS des planchers, gelées en littéral — les tests d'encadrement sont symboliques
    /// (`FLOOR - 0.1`) et suivraient n'importe quelle dérive sans tomber. Ces deux chiffres sont
    /// des mesures (référence assainie du 2026-09-01, min authentique vérifié -11,75 / -19,36) :
    /// les changer exige de rejouer `score-corpus.mjs`, pas d'éditer ce test.
    #[test]
    fn les_planchers_sont_ceux_de_la_reference_assainie() {
        assert_eq!(HF_FIXED_FLOOR_DB, -12.0);
        assert_eq!(HF_TOP_FLOOR_DB, -23.8);
    }

    /// Un lossless à bande PLEINE dont l'aigu est creux devient **Douteux**, pas Vrai lossless.
    ///
    /// C'est le cas que la coupure ne peut pas voir : le fichier va jusqu'au bout du spectre, donc
    /// il n'y a aucune falaise à seuiller — et pourtant l'aigu est clairsemé au point de sortir de
    /// la plage des masters mesurés. Mesuré sur le corpus étiqueté : c'est la forme de tout l'AAC
    /// haut débit, du LAME 320 et du V0 (voir `docs/superpowers/changes/2026-08-17-detecteur-corpus/`).
    ///
    /// **Douteux et pas Faux, délibérément.** La plage de référence repose sur 32 authentiques,
    /// et deux morceaux ambient s'en approchent légitimement : le haut du spectre d'un master
    /// sombre EST clairsemé. Accuser sur cette seule base produirait des faux positifs sur du
    /// matériel acheté. Douteux dit ce qu'on sait — « cet aigu ne ressemble pas à un master » —
    /// sans dire ce qu'on ne sait pas.
    #[test]
    fn lossless_a_bande_pleine_mais_aigu_creux_est_douteux() {
        assert_eq!(
            verdict(
                21000.0,
                Rail::Lossless,
                None,
                Rail::Lossless,
                HfFlatness {
                    fixed_db: Some(-40.0),
                    top_db: Some(-4.0),
                },
                None
            ),
            Ok(Verdict::Grey)
        );
    }

    /// L'AUTRE moitié de l'union : la bande relative seule suffit, la fixe étant normale.
    ///
    /// C'est la forme d'un Opus — 48 kHz, du contenu jusqu'à 20 kHz, donc la bande fixe 16-20 kHz
    /// tombe en pleine bande passante et ne voit rien. Sans ce cas, retirer la clause relative de
    /// `below_master_range` laisserait toute la suite verte.
    ///
    /// ⚠️ Ce test n'a jamais été rouge : l'union était déjà écrite quand il est arrivé. Il pinne,
    /// il n'a rien piloté — et c'est un écart à la boucle, pas une propriété du code.
    #[test]
    fn lossless_dont_seul_le_tout_haut_est_creux_est_douteux() {
        assert_eq!(
            verdict(
                21000.0,
                Rail::Lossless,
                None,
                Rail::Lossless,
                HfFlatness {
                    fixed_db: Some(-3.0),
                    top_db: Some(-30.0),
                },
                None
            ),
            Ok(Verdict::Grey)
        );
    }

    /// Le plancher est une borne INCLUSE : à la valeur exacte, on est encore dans la plage.
    ///
    /// Ce que ça protège : les deux morceaux ambient qui ont fait descendre la borne relative à
    /// -23,8 sont EXACTEMENT à cette valeur. Un `<=` au lieu d'un `<` les annoncerait douteux —
    /// c'est-à-dire le faux positif que ce seuil existe pour éviter.
    #[test]
    fn un_authentique_pile_sur_le_plancher_reste_vrai_lossless() {
        assert_eq!(
            verdict(
                21000.0,
                Rail::Lossless,
                None,
                Rail::Lossless,
                HfFlatness {
                    fixed_db: Some(HF_FIXED_FLOOR_DB),
                    top_db: Some(HF_TOP_FLOOR_DB),
                },
                None
            ),
            Ok(Verdict::Ok)
        );
    }

    /// Une bande NON MESURÉE n'est pas un grief. `None` ne doit jamais valoir « creux ».
    ///
    /// Le cas réel : la bande fixe 16-20 kHz n'existe pas sous 40 kHz d'échantillonnage, et un
    /// fichier trop court ne rend aucune trame. Traiter cette absence comme une mesure basse
    /// accuserait un fichier pour n'avoir pas pu être mesuré — le défaut exact que
    /// `NO_MEASUREMENT_HZ` corrige sur la coupure.
    #[test]
    fn une_bande_non_mesuree_ne_declenche_rien() {
        assert_eq!(
            verdict(
                21000.0,
                Rail::Lossless,
                None,
                Rail::Lossless,
                HfFlatness {
                    fixed_db: None,
                    top_db: None,
                },
                None
            ),
            Ok(Verdict::Ok)
        );
    }

    #[test]
    fn honest_mp3_matching_its_bitrate_is_ok() {
        // genuine 320 (~20.5k), genuine 128 (~16k)
        assert_eq!(
            verdict(
                20500.0,
                Rail::Lossy,
                Some(320),
                Rail::Lossy,
                HfFlatness::default(),
                None
            ),
            Ok(Verdict::Ok)
        );
        assert_eq!(
            verdict(
                16000.0,
                Rail::Lossy,
                Some(128),
                Rail::Lossy,
                HfFlatness::default(),
                None
            ),
            Ok(Verdict::Ok)
        );
    }

    #[test]
    fn over_encoded_mp3_is_fake() {
        // declared 320 but cuts at 16k (transcoded up from ~128) → fraud
        assert_eq!(
            verdict(
                16000.0,
                Rail::Lossy,
                Some(320),
                Rail::Lossy,
                HfFlatness::default(),
                None
            ),
            Ok(Verdict::Fake)
        );
        // declared 256 but cuts at 15k
        assert_eq!(
            verdict(
                15000.0,
                Rail::Lossy,
                Some(256),
                Rail::Lossy,
                HfFlatness::default(),
                None
            ),
            Ok(Verdict::Fake)
        );
    }

    // FIX-18: 192/160 kbps were the only two of the six min_cutoff_hz_for_bitrate tiers never
    // exercised by a direct test (only 320/256/128 were covered above).
    #[test]
    fn honest_192_and_160_mp3_is_ok() {
        assert_eq!(
            verdict(
                17000.0,
                Rail::Lossy,
                Some(192),
                Rail::Lossy,
                HfFlatness::default(),
                None
            ),
            Ok(Verdict::Ok)
        );
        assert_eq!(
            verdict(
                16000.0,
                Rail::Lossy,
                Some(160),
                Rail::Lossy,
                HfFlatness::default(),
                None
            ),
            Ok(Verdict::Ok)
        );
    }

    #[test]
    fn over_encoded_192_and_160_mp3_is_fake() {
        // declared 192 but cuts at 15k (below the 16500Hz floor for 192) → fraud
        assert_eq!(
            verdict(
                15000.0,
                Rail::Lossy,
                Some(192),
                Rail::Lossy,
                HfFlatness::default(),
                None
            ),
            Ok(Verdict::Fake)
        );
        // declared 160 but cuts at 14k (below the 15500Hz floor for 160) → fraud
        assert_eq!(
            verdict(
                14000.0,
                Rail::Lossy,
                Some(160),
                Rail::Lossy,
                HfFlatness::default(),
                None
            ),
            Ok(Verdict::Fake)
        );
    }

    #[test]
    fn lossy_without_known_bitrate_is_ok() {
        // can't judge over-encoding without a declared bitrate → don't false-flag
        assert_eq!(
            verdict(
                13000.0,
                Rail::Lossy,
                None,
                Rail::Lossy,
                HfFlatness::default(),
                None
            ),
            Ok(Verdict::Ok)
        );
    }

    /// Un décodage qui n'a produit aucune trame ne doit accuser personne — et depuis le
    /// 2026-09-01, ne doit pas non plus **douter** : il sort du domaine de la décision.
    ///
    /// Mesuré sur la bibliothèque réelle le 2026-08-17 : deux MP3 320 kbps de plus de six minutes,
    /// sans `codec_error`, portaient `cutoff_hz = 0` et sortaient FAKE. Sur les deux rails, parce
    /// que le défaut est le même des deux côtés — un zéro qui se fait lire comme une mesure. Le
    /// premier correctif en avait fait un Douteux ; celui-ci en fait un échec d'analyse.
    ///
    /// Le contrôle positif est dans le même corps : la MÊME entrée avec un cutoff réellement bas
    /// doit toujours sortir Fake. Sans lui, on ne saurait pas si le correctif distingue l'absence
    /// de mesure ou s'il a simplement désarmé la détection de sur-encodage.
    #[test]
    fn une_mesure_absente_sort_du_domaine() {
        assert_eq!(
            verdict(
                NO_MEASUREMENT_HZ,
                Rail::Lossy,
                Some(320),
                Rail::Lossy,
                HfFlatness::default(),
                None
            ),
            Err(NotMeasured::Cutoff)
        );
        assert_eq!(
            verdict(
                NO_MEASUREMENT_HZ,
                Rail::Lossless,
                None,
                Rail::Lossless,
                HfFlatness::default(),
                None
            ),
            Err(NotMeasured::Cutoff)
        );
        // Contrôle positif : une vraie mesure basse reste une fraude.
        assert_eq!(
            verdict(
                16000.0,
                Rail::Lossy,
                Some(320),
                Rail::Lossy,
                HfFlatness::default(),
                None
            ),
            Ok(Verdict::Fake)
        );
        assert_eq!(
            verdict(
                16000.0,
                Rail::Lossless,
                None,
                Rail::Lossless,
                HfFlatness::default(),
                None
            ),
            Ok(Verdict::Fake)
        );
    }

    /// L'absence de mesure n'efface pas une fraude déjà établie SANS le spectre : un MP3 renommé
    /// en `.flac` est un faux même si rien n'a pu être décodé.
    #[test]
    fn no_measurement_does_not_erase_a_container_mismatch() {
        assert_eq!(
            verdict(
                NO_MEASUREMENT_HZ,
                Rail::Lossless,
                None,
                Rail::Lossy,
                HfFlatness::default(),
                None
            ),
            Ok(Verdict::Fake)
        );
    }

    /// La borne du sentinel, par le HAUT : la plus petite coupure strictement positive est une
    /// MESURE, donc elle reste dans le domaine et se fait juger normalement.
    ///
    /// Ce que ça garde, sur les deux rails : le bras « pas mesuré » teste `<= NO_MEASUREMENT_HZ`,
    /// pas `<` un plancher quelconque. Élargir ce bras d'un cheveu ferait sortir du domaine des
    /// fichiers réellement mesurés — c'est-à-dire transformerait une fraude prouvée en échec
    /// d'analyse, exactement l'erreur symétrique de celle que cette lane corrige.
    #[test]
    fn la_plus_petite_coupure_positive_reste_dans_le_domaine() {
        let epsilon = f32::MIN_POSITIVE;
        assert_eq!(
            verdict(
                epsilon,
                Rail::Lossless,
                None,
                Rail::Lossless,
                HfFlatness::default(),
                None
            ),
            Ok(Verdict::Fake),
            "lossless mesuré au ras de zéro : falaise lossy, donc Faux"
        );
        assert_eq!(
            verdict(
                epsilon,
                Rail::Lossy,
                Some(320),
                Rail::Lossy,
                HfFlatness::default(),
                None
            ),
            Ok(Verdict::Fake),
            "320 kbps mesuré au ras de zéro : sur-encodage, donc Faux"
        );
    }

    /// **Cas 2 — la fenêtre LAME 320 est ambre (2026-09-11).** Sous 20 000 Hz une déclaration
    /// lossless est Fausse (falaise, #51 : l'intervalle (19 500, 20 000) ne contenait que des
    /// faux). Entre 20 000 exclu et 20 750 exclu, `Grey` : les 20 `lame320` du corpus coupent
    /// entre 20 177 et 20 704 Hz et sortaient tous Vrai en v0.1.1 ; un roll-off authentique peut y
    /// tomber, donc jamais `Fake` d'office. À 20 750 et au-dessus, la platitude tranche. La grille
    /// d'un codec (#52) rend `Fake` sur les deux derniers bras. Mesuré en mutant : ramener
    /// `LOSSLESS_OK_HZ` à 20 000 fait tomber les assertions de la fenêtre.
    #[test]
    fn la_fenetre_lame320_est_ambre_entre_la_falaise_et_la_bande_pleine() {
        let at = |hz: f32, l: Option<f32>| {
            verdict(
                hz,
                Rail::Lossless,
                None,
                Rail::Lossless,
                HfFlatness::default(),
                l,
            )
        };
        assert_eq!(
            at(19750.0, None),
            Ok(Verdict::Fake),
            "cœur de l'ex-fenêtre : Faux"
        );
        assert_eq!(
            at(LOSSY_CLIFF_HZ, None),
            Ok(Verdict::Fake),
            "20 000 inclus : Faux"
        );
        assert_eq!(
            at(LOSSY_CLIFF_HZ + 0.1, None),
            Ok(Verdict::Grey),
            "juste au-dessus de la falaise : la fenêtre s'ouvre"
        );
        for hz in [20177.0, 20300.0, 20704.0] {
            assert_eq!(
                at(hz, None),
                Ok(Verdict::Grey),
                "lame320 mesuré à {hz} Hz : À vérifier"
            );
            assert_eq!(
                at(hz, Some(QUANT_LAMBDA + 0.01)),
                Ok(Verdict::Fake),
                "grille de codec à {hz} Hz : Faux malgré la fenêtre"
            );
        }
        assert_eq!(
            at(LOSSLESS_OK_HZ - 0.1, None),
            Ok(Verdict::Grey),
            "juste sous la bande pleine : encore la fenêtre"
        );
        assert_eq!(
            at(LOSSLESS_OK_HZ, None),
            Ok(Verdict::Ok),
            "20 750 inclus : bande pleine, Vrai"
        );
        assert_eq!(at(22050.0, None), Ok(Verdict::Ok), "Nyquist : Vrai");
        // Deux `assert_eq!` et pas des `assert!` sur constantes (clippy::assertions_on_constants) :
        // la borne est gelée en littéral, 20 750 couvre le maximum mesuré (20 704) avec 46 Hz de
        // marge, et l'écart avec la falaise est ce qui fait exister la fenêtre — sans lui, LAME 320
        // redevient Vrai (v0.1.1, 20/20 ratés).
        assert_eq!(
            LOSSLESS_OK_HZ, 20750.0,
            "borne de la bande pleine gelée en littéral"
        );
        assert_eq!(LOSSY_CLIFF_HZ, 20000.0, "falaise gelée en littéral");
        assert_eq!(
            LOSSLESS_OK_HZ.max(LOSSY_CLIFF_HZ),
            LOSSLESS_OK_HZ,
            "la bande pleine commence au-dessus de la falaise : c'est l'écart qui fait la fenêtre"
        );
    }

    /// **Cas 3 — les planchers encadrés au dixième de dB.** Les deux planchers de platitude,
    /// juste EN DESSOUS de leur valeur, chacun seul (l'autre bande normale). Symbolique, donc
    /// il a suivi sans édition la re-dérivation du 2026-09-01 (fixe -5,8 → -12).
    ///
    /// Complète `un_authentique_pile_sur_le_plancher_reste_vrai_lossless`, qui pinne la borne par
    /// le haut : les deux ensemble encadrent chaque plancher au dixième de dB près, donc tout
    /// déplacement d'une des deux valeurs fait tomber l'un des deux tests.
    #[test]
    fn juste_sous_un_plancher_de_platitude_reste_douteux() {
        assert_eq!(
            verdict(
                21000.0,
                Rail::Lossless,
                None,
                Rail::Lossless,
                HfFlatness {
                    fixed_db: Some(HF_FIXED_FLOOR_DB - 0.1),
                    top_db: Some(HF_TOP_FLOOR_DB),
                },
                None
            ),
            Ok(Verdict::Grey),
            "bande fixe sous son plancher, bande relative pile dessus"
        );
        assert_eq!(
            verdict(
                21000.0,
                Rail::Lossless,
                None,
                Rail::Lossless,
                HfFlatness {
                    fixed_db: Some(HF_FIXED_FLOOR_DB),
                    top_db: Some(HF_TOP_FLOOR_DB - 0.1),
                },
                None
            ),
            Ok(Verdict::Grey),
            "bande relative sous son plancher, bande fixe pile dessus"
        );
    }

    /// Un rail déclaré indéterminé n'est pas un doute sur le fichier : c'est un sniffing qui n'a
    /// pas conclu, donc rien à juger. Il sort du domaine (2026-09-01), là où il rendait Douteux.
    ///
    /// Le contrôle est dans le même corps : une coupure parfaitement mesurée ne rachète PAS un rail
    /// inconnu — sans elle, on ne saurait pas si le bras sort du domaine pour la bonne raison.
    #[test]
    fn un_rail_declare_inconnu_sort_du_domaine() {
        assert_eq!(
            verdict(
                16000.0,
                Rail::Unknown,
                None,
                Rail::Unknown,
                HfFlatness::default(),
                None
            ),
            Err(NotMeasured::Rail)
        );
        assert_eq!(
            verdict(
                21000.0,
                Rail::Unknown,
                Some(320),
                Rail::Lossless,
                HfFlatness::default(),
                None
            ),
            Err(NotMeasured::Rail)
        );
    }

    // Bug case: an MP3 renamed to `.flac` declares Lossless but its container is actually
    // lossy — the mismatch alone is fraud, regardless of how high the cutoff happens to be
    // (a 256-320kbps source can cut near/above the lossless-OK threshold).
    #[test]
    fn declared_lossless_but_content_lossy_is_fake() {
        assert_eq!(
            verdict(
                20500.0,
                Rail::Lossless,
                None,
                Rail::Lossy,
                HfFlatness::default(),
                None
            ),
            Ok(Verdict::Fake)
        );
    }

    // Content-rail sniffing failed/inconclusive → must not false-positive off the mismatch
    // short-circuit; falls back to the existing cutoff-only logic (Ok here).
    #[test]
    fn declared_lossless_content_unknown_falls_back_to_cutoff() {
        assert_eq!(
            verdict(
                21000.0,
                Rail::Lossless,
                None,
                Rail::Unknown,
                HfFlatness::default(),
                None
            ),
            Ok(Verdict::Ok)
        );
    }

    // Declared lossy stays on the existing cutoff/bitrate logic when content_rail agrees —
    // the mismatch short-circuit is Lossless-branch-only.
    #[test]
    fn declared_lossy_content_lossy_unchanged_behavior() {
        assert_eq!(
            verdict(
                20500.0,
                Rail::Lossy,
                Some(320),
                Rail::Lossy,
                HfFlatness::default(),
                None
            ),
            Ok(Verdict::Ok)
        );
        assert_eq!(
            verdict(
                16000.0,
                Rail::Lossy,
                Some(320),
                Rail::Lossy,
                HfFlatness::default(),
                None
            ),
            Ok(Verdict::Fake)
        );
    }

    // -----------------------------------------------------------------------------------------
    // Troisième signal — vraisemblance de quantification (issue #52, 2026-09-02).
    // -----------------------------------------------------------------------------------------

    /// L'entrée AMBIGUË, celle et seulement celle que la sonde vise : bande pleine (la falaise n'a
    /// rien à dire) + aigu sous la plage des masters (la platitude doute). Les trois tests qui
    /// suivent ne changent QUE `quant_likelihood` sur cette entrée-là.
    fn ambigu(quant: Option<f32>) -> Result<Verdict, NotMeasured> {
        verdict(
            21000.0,
            Rail::Lossless,
            None,
            Rail::Lossless,
            HfFlatness {
                fixed_db: Some(-40.0),
                top_db: Some(-4.0),
            },
            quant,
        )
    }

    /// **Le comportement vendu par #52** : sur le seul bras où falaise et platitude ne tranchent
    /// pas, une grille de codec retrouvée dans le signal fait passer le Douteux en **Faux**.
    ///
    /// Les trois états de la mesure sont dans le même corps, et c'est ce qui rend le test
    /// couvrant : au-dessus de λ → Faux, en dessous → Douteux (le comportement d'avant #52),
    /// et NON MESURÉE → Douteux aussi. Sans les deux derniers, un `Ok(Fake)` constant passerait.
    #[test]
    fn la_grille_du_codec_retrouvee_transforme_le_doute_en_faux() {
        assert_eq!(
            ambigu(Some(0.45)),
            Ok(Verdict::Fake),
            "L bien au-dessus de lambda : transcodage etabli par la grille, donc Faux"
        );
        assert_eq!(
            ambigu(Some(0.094)),
            Ok(Verdict::Grey),
            "L au niveau des authentiques : le doute reste un doute"
        );
        assert_eq!(
            ambigu(None),
            Ok(Verdict::Grey),
            "mesure absente : verdict INCHANGE, jamais degrade"
        );
    }

    /// La VALEUR de λ, gelée en littéral, et sa borne encadrée au plus près de la calibration.
    ///
    /// Même raison que `les_planchers_sont_ceux_de_la_reference_assainie` : les tests symboliques
    /// (`QUANT_LAMBDA + ε`) suivraient n'importe quelle dérive sans tomber. 0,18 est une mesure —
    /// le maximum des 10 authentiques du 2026-09-02 vaut 0,172, vetted au spectrogramme — et la
    /// changer exige de rejouer la calibration, pas d'éditer ce test.
    ///
    /// La comparaison est **stricte** (`> λ`), et le test le fige des deux côtés : à la valeur
    /// exacte on est encore dans le doute, un cheveu au-dessus on ne l'est plus.
    #[test]
    fn lambda_est_la_valeur_calibree_et_sa_borne_est_stricte() {
        assert_eq!(QUANT_LAMBDA, 0.18);
        assert_eq!(
            ambigu(Some(0.172)),
            Ok(Verdict::Grey),
            "le MAXIMUM authentique mesure (src10) doit rester Douteux — c'est le critere zero faux positif"
        );
        assert_eq!(
            ambigu(Some(QUANT_LAMBDA)),
            Ok(Verdict::Grey),
            "pile sur lambda : borne INCLUSE du cote du doute"
        );
        assert_eq!(
            ambigu(Some(QUANT_LAMBDA + 0.001)),
            Ok(Verdict::Fake),
            "juste au-dessus : Faux"
        );
    }

    /// Le troisième signal **ne se lit que dans le bras de la bande pleine**. Une vraisemblance
    /// écrasante ne doit rien changer là où la falaise, le conteneur ou le rail ont déjà tranché.
    ///
    /// ⚠️ Le cas « bande pleine + aigu NORMAL » n'est PAS ici : depuis la correction du
    /// 2026-09-02 il appartient au bras, et c'est même la cible de #52 — il est figé par
    /// `la_sonde_se_declenche_sur_la_bande_pleine_et_nulle_part_ailleurs`. Ce test garde les
    /// frontières qui restent fermées.
    #[test]
    fn la_vraisemblance_ne_deborde_pas_de_son_bras() {
        // Sous la falaise : Faux par la coupure, la sonde n'a pas voix au chapitre. Le sens du
        // test est inversé par rapport aux suivants — ici une mesure ÉLEVÉE ne doit rien ajouter,
        // et une mesure BASSE ne doit surtout rien retirer.
        assert_eq!(
            verdict(
                16000.0,
                Rail::Lossless,
                None,
                Rail::Lossless,
                HfFlatness::default(),
                Some(0.0),
            ),
            Ok(Verdict::Fake),
            "falaise lossy : une vraisemblance NULLE ne blanchit pas"
        );
        // Fraude de conteneur : le court-circuit passe avant tout, y compris avant la sonde.
        assert_eq!(
            verdict(
                21000.0,
                Rail::Lossless,
                None,
                Rail::Lossy,
                HfFlatness::default(),
                Some(0.0),
            ),
            Ok(Verdict::Fake),
            "MP3 renomme .flac : Faux sans le spectre, la sonde ne le rachete pas"
        );
        // Rail lossy : l'autre grille de décision, qui ne connaît pas ce signal.
        assert_eq!(
            verdict(
                20500.0,
                Rail::Lossy,
                Some(320),
                Rail::Lossy,
                HfFlatness::default(),
                Some(1.0),
            ),
            Ok(Verdict::Ok)
        );
        // Hors domaine : une vraisemblance ne rachète pas une mesure qui n'existe pas.
        assert_eq!(
            verdict(
                NO_MEASUREMENT_HZ,
                Rail::Lossless,
                None,
                Rail::Lossless,
                HfFlatness::default(),
                Some(1.0),
            ),
            Err(NotMeasured::Cutoff)
        );
        assert_eq!(
            verdict(
                21000.0,
                Rail::Unknown,
                None,
                Rail::Unknown,
                HfFlatness::default(),
                Some(1.0),
            ),
            Err(NotMeasured::Rail)
        );
    }

    /// **La CONDITION de déclenchement, figée cas par cas** — la sonde tourne sur la bande pleine
    /// et nulle part ailleurs.
    ///
    /// `analysis::analyze` décide de dépenser 0,3 s de MDCT sur la foi de [`needs_quant_probe`],
    /// pas de `verdict()`. Si les deux divergent, soit on calcule pour rien, soit — bien pire —
    /// on ne calcule pas là où le verdict attendait la mesure, et le troisième signal devient muet
    /// en silence. C'est exactement ce qui est arrivé à la première intégration (clause de
    /// platitude, 2026-09-02) : cible jamais sondée, +2 détections hors cible.
    ///
    /// Les quatre faits figés, dans l'ordre où ils comptent :
    ///
    /// 1. **bande pleine → sondé**, que la platitude soit dans la plage des masters ou non — les
    ///    DEUX issues, `Ok` et `Grey`. C'est le cas 3 qui manquait ;
    /// 2. **bande coupée → pas sondé** : le verdict est déjà Faux par la falaise ;
    /// 3. **fraude de conteneur → pas sondé** : le court-circuit passe avant, le verdict est Faux
    ///    sans le spectre ;
    /// 4. **hors domaine → pas sondé** : coupure sentinelle, rail non lossless, rail indéterminé.
    ///
    /// Le contrôle est l'implication : partout où la sonde est VRAIE, une vraisemblance au-dessus
    /// de λ doit réellement produire `Fake`. Sans lui, un `needs_quant_probe` qui rendrait `true`
    /// partout passerait les quatre points ci-dessus tout en dépensant du CPU pour rien.
    #[test]
    fn la_sonde_se_declenche_sur_la_bande_pleine_et_nulle_part_ailleurs() {
        let plate = HfFlatness {
            fixed_db: Some(-3.0),
            top_db: Some(-4.0),
        }; // dans la plage des masters → bras Ok
        let creuse = HfFlatness {
            fixed_db: Some(-40.0),
            top_db: Some(-4.0),
        }; // sous la plage → bras Grey

        // 1. Bande pleine, les DEUX platitudes, et la borne inclusive de LOSSLESS_OK_HZ.
        for c in [LOSSLESS_OK_HZ, 21000.0, 22050.0] {
            assert!(
                needs_quant_probe(c, Rail::Lossless, Rail::Lossless),
                "bande pleine a {c} Hz : la sonde doit tourner"
            );
            assert!(
                needs_quant_probe(c, Rail::Lossless, Rail::Unknown),
                "conteneur non conclusif : ne desarme pas la sonde"
            );
            // Et sur les deux bras : la mesure tranche, quelle que soit la platitude.
            for f in [plate, creuse] {
                assert_eq!(
                    verdict(c, Rail::Lossless, None, Rail::Lossless, f, Some(0.45)),
                    Ok(Verdict::Fake),
                    "grille retrouvee a {c} Hz, platitude {f:?} : Faux"
                );
            }
            // Sans mesure, le verdict de platitude s'applique tel quel — inchangé depuis #51.
            assert_eq!(
                verdict(c, Rail::Lossless, None, Rail::Lossless, plate, None),
                Ok(Verdict::Ok)
            );
            assert_eq!(
                verdict(c, Rail::Lossless, None, Rail::Lossless, creuse, None),
                Ok(Verdict::Grey)
            );
        }

        // 1 bis. Fenêtre LAME 320 (2026-09-11) : sondée, et la mesure y tranche aussi.
        for c in [LOSSY_CLIFF_HZ + 0.1, 20300.0, LOSSLESS_OK_HZ - 0.1] {
            assert!(
                needs_quant_probe(c, Rail::Lossless, Rail::Lossless),
                "fenetre a {c} Hz : Grey par la coupure, la sonde doit pouvoir lever le doute"
            );
            assert_eq!(
                verdict(c, Rail::Lossless, None, Rail::Lossless, plate, Some(0.45)),
                Ok(Verdict::Fake),
                "grille retrouvee dans la fenetre a {c} Hz : Faux"
            );
            assert_eq!(
                verdict(c, Rail::Lossless, None, Rail::Lossless, plate, None),
                Ok(Verdict::Grey),
                "fenetre sans mesure a {c} Hz : A verifier"
            );
        }

        // 2. Bande coupée : la falaise a déjà tranché.
        for c in [LOSSY_CLIFF_HZ, LOSSY_CLIFF_HZ - 0.1, 19750.0, 16000.0] {
            assert!(
                !needs_quant_probe(c, Rail::Lossless, Rail::Lossless),
                "coupure a {c} Hz : Faux par la falaise, rien a sonder"
            );
        }

        // 3. Fraude de conteneur : le court-circuit passe avant, y compris à bande pleine.
        assert!(
            !needs_quant_probe(21000.0, Rail::Lossless, Rail::Lossy),
            "MP3 renomme .flac : Faux sans le spectre, rien a sonder"
        );

        // 4. Hors domaine.
        assert!(
            !needs_quant_probe(NO_MEASUREMENT_HZ, Rail::Lossless, Rail::Lossless),
            "aucune trame decodee : pas de mesure, donc pas de sonde"
        );
        for d in [Rail::Lossy, Rail::Unknown] {
            assert!(
                !needs_quant_probe(21000.0, d, Rail::Lossless),
                "rail declare {d:?} : l'autre grille de decision, ou aucune"
            );
        }
    }
}
