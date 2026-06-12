# M2a — Moteur d'analyse audio (Rust pur) — Design

> Jalon M2a de Sift (`dj-assistant`). Source de vérité fonctionnelle :
> `docs/plan-implementation.md` (section « M2 — Analyseur »). Ce document découpe la
> **première tranche** de M2 : le moteur d'analyse Rust, testé isolément, **sans UI ni
> worker auto**. Validé au brainstorm du 2026-06-12.

## Décisions tranchées (brainstorm 2026-06-12)

1. **Découpage M2 = option A** :
   - **M2a** (ce doc) : moteur Rust pur `analyze(path) -> AnalysisReport`, testé, aucune UI,
     aucun déclenchement auto. Appelable via une command IPC de debug.
   - **M2b** : worker background (thread throttlé), cache des résultats en DB (colonnes
     `tracks` déjà au schéma), analyse auto à l'arrivée d'un fichier (reprend le `queue:changed`
     de M1), progression globale.
   - **M2c** : UI Revue réelle — waveform (wavesurfer), spectrogramme (canvas), badges verdict,
     transparence (coupure visible).

2. **Fixtures de test** : **fabriquées par re-encodage FFmpeg** (vrai→faux transcodé,
   génération de tronqué/silence tête-queue/DC offset/écrêté/dual-mono/hors-phase). Les tests
   **unitaires** des analyseurs s'appuient sur des **signaux synthétiques** générés en code
   (sinus, bruit lowpassé, buffers saturés) → aucun fichier requis. La caractérisation
   bout-en-bout utilise les fixtures fabriquées ; l'utilisateur **déposera plus tard** 1–2
   **vrais lossless** + un **vrai MP3 320 du commerce** dans `src-tauri/fixtures/` pour ancrer
   les cas « authentiques » qu'on ne peut pas fabriquer fidèlement.

3. **Seuil de coupure fake** : bandes de décision proposées au §Verdict retenues (lossless
   attendu ≥ ~20 kHz ; zone grise ; cutoff franc ≤ ~19 kHz = transcodé). On **stocke
   `cutoff_hz` brut** ; la bande est appliquée par-dessus (seuil reconfigurable sans réanalyse).

4. **Lecture des tags** : crate **`lofty` 0.22.2** (ID3/Vorbis/MP4 unifié, lecture seule) pour
   version ID3 / champs CDJ / présence pochette.

---

## But du jalon (livrable M2a)

Une fonction Rust **`analysis::analyze(path) -> Result<AnalysisReport>`** qui, à partir d'un
**seul décodage FFmpeg**, produit un rapport complet et **déterministe** :

| # | Champ du rapport | Description |
|---|---|---|
| 1 | `peaks: Vec<f32>` | enveloppe min/max sous-échantillonnée (→ waveform, consommée en M2c) |
| 2 | `spectrogram: Spectrogram` | bins fréq×temps (→ canvas en M2c) |
| 3 | `cutoff_hz: f32` | fréquence de coupure détectée (→ verdict) |
| 4 | `real_quality` + `declared` | rail lossless/lossy **réel** vs déclaré ; `bitrate` déclaré |
| 5 | `clip_runs`, `clip_pct`, `true_peak_dbtp` | écrêtage / intégrité dynamique |
| 6 | `truncated: bool` (+ raison) | fin abrupte OU erreur décodage fin de fichier |
| 7 | `silence_head_ms`, `silence_tail_ms` | lead-in / run-out |
| 8 | `dc_offset` | moyenne du signal ≠ 0 |
| + | `phase_correlation`, `dual_mono` | compat mono/phase |
| + | `container_ok`, `codec_error` | intégrité conteneur/codec (stderr FFmpeg) |
| + | `id3_version`, `tags_cdj_ok`, `has_cover` | compat tags CDJ + pochette |
| = | `verdict: Verdict` | `Ok` / `Fake` / `Grey`, **dérivé** de cutoff + format déclaré |

**Hors M2a** : pas de worker auto, pas de cache DB, pas d'UI, pas de fan-out vers des
consommateurs concurrents. Juste le moteur + ses tests.

## Décision d'architecture clé : passe unique en streaming (pas de `named_pipes`)

Le plan évoque le fan-out `named_pipes` « 1 décodage → 8 sorties ». **Proposition : on ne
l'utilise PAS en M2a.** Le fan-out vers N pipes a du sens quand N **consommateurs concurrents**
lisent en parallèle. Ici, un seul appelant veut un rapport agrégé. Plus simple et borné en
mémoire :

> **Un décodage FFmpeg → PCM `s16le` streamé par blocs → tous les analyseurs tournent en
> ligne (online) sur le même flux, en une passe.**

- Évite de charger un morceau entier en RAM (10 min stéréo f32 ≈ 100 Mo × throughput batch
  de 15 000 fichiers = intenable).
- La plupart des signaux sont des **accumulateurs en ligne** (somme pour DC offset, compteurs
  d'écrêtage, min/max glissant pour les peaks, énergie par bande, FFT par fenêtre glissante).
- Le **spectrogramme** est la seule sortie intrinsèquement volumineuse : on stocke des frames
  FFT **sous-échantillonnées** (ex. une colonne tous les ~50 ms, magnitude en dB quantifiée
  u8) — taille maîtrisée, suffisant pour l'affichage M2c et la détection de coupure.
- `named_pipes` reste une option si un jour un consommateur temps-réel apparaît — noté, non
  retenu maintenant (YAGNI).

## Modules (`src-tauri/src/analysis/`)

Un module = une responsabilité, testable isolément :

- **`mod.rs`** — `AnalysisReport` (struct sérialisable serde), `analyze(path) -> Result<…>` :
  orchestre décodage + analyseurs en une passe, assemble le rapport, calcule le verdict.
- **`decode.rs`** — pilote `ffmpeg-sidecar` : un décodage → PCM `s16le`, expose un itérateur
  de blocs `(&[i16], channels, sample_rate)`. Capture `Log(LogLevel::Error)` → `codec_error`.
  Lit la durée/bitrate déclarés (ffprobe).
- **`spectrum.rs`** — FFT fenêtrée (Hann) via **`rustfft`** ; LTAS (spectre moyen long-terme,
  méthode de Welch sur frames non-silencieuses) ; **détection de la fréquence de coupure** ;
  frames sous-échantillonnées pour le spectrogramme.
- **`dynamics.rs`** — écrêtage (`clip_runs`, `clip_pct`), true-peak (`true_peak_dbtp`,
  inter-sample via sur-échantillonnage 4×), DC offset (moyenne par canal).
- **`structure.rs`** — silence tête/queue (seuil dBFS + durée), troncature (énergie de fin
  qui ne retombe pas / erreur décodage terminale / durée < attendue).
- **`phase.rs`** — corrélation inter-canaux (dual-mono = corrélation ≈ 1,0 ; hors-phase =
  corrélation < 0).
- **`tags.rs`** — `lofty` : version ID3, champs lus par CDJ, présence pochette → `tags_cdj_ok`,
  `has_cover`, `id3_version`.
- **`verdict.rs`** — combine `cutoff_hz` + format déclaré → `Verdict` (logique pure, très testée).

## Algorithme de détection « fake » (cœur)

Un transcodé lossy→lossless garde la **signature du lowpass** de l'encodeur d'origine : une
**falaise spectrale** au-delà de laquelle il n'y a plus d'énergie réelle (juste le bruit de
quantif). Profils MP3 typiques (cutoff indicatif) : 128k ≈ 16 kHz · 192k ≈ 17–18 kHz ·
256k ≈ 19 kHz · 320k ≈ 20 kHz · lossless → jusqu'à Nyquist (≈ 22,05 kHz @ 44,1).

1. **LTAS** : sur les frames non-silencieuses (on saute les blancs), FFT 4096-pt fenêtrée
   Hann, moyenne des magnitudes² → spectre moyen long-terme, converti en dB.
2. **Détection de coupure** : depuis Nyquist vers le bas, trouver la **falaise** — la
   fréquence où l'énergie remonte franchement (chute > ~Δ dB sur une bande étroite) puis
   reste soutenue en dessous. `cutoff_hz` = cette fréquence. (Robustesse : ignorer les
   pics isolés, exiger une bande sous le cutoff durablement au-dessus du plancher.)
3. **Verdict** (couplé au format déclaré — un vrai MP3 320 a AUSSI une coupure ~20 kHz,
   ce n'est PAS un fake, c'est un MP3 honnête) :
   - **déclaré lossless (FLAC/WAV/AIFF)** :
     - `cutoff` ≥ ~20–21 kHz (proche Nyquist) → **`Ok`** (lossless authentique)
     - `cutoff` ≤ ~19 kHz net → **`Fake`** (transcodé d'un lossy, falaise visible)
     - entre les deux / falaise peu nette → **`Grey`** (soumis à revue, preuve = spectro)
   - **déclaré lossy (MP3/AAC/OGG)** :
     - jamais « fake » au sens transcode-vers-lossless ; on rapporte le **bitrate réel**
       cohérent ou non avec le déclaré (un « 320 » à coupure 16 kHz = sur-codé depuis du 128 →
       `Grey`/badge « bitrate suspect »).
4. **Seuil réglable** (Réglages, défaut 320 → réf. ~20 kHz) : on stocke `cutoff_hz` brut ; la
   **bande de décision** est appliquée par-dessus, donc reconfigurable sans réanalyse.

**Garde-fou anti-faux-positif** : certains morceaux authentiques (vieux masters, basé samples,
choix artistique) roulent off naturellement → ne jamais sur-affirmer ; ces cas tombent en
`Grey`, et M2c rend la **preuve visible** (coupure sur le spectro) pour que le DJ tranche.

## Le décodage (`ffmpeg-sidecar`)

- Une commande : décode `path` → `pcm_s16le`, downmix mono **et** garde les 2 canaux pour la
  phase (proposition : 2 passes logiques sur le même flux stéréo, downmix calculé en ligne ;
  PAS 2 décodages).
- Sortie streamée par blocs ; événements typés : `Progress` (ignoré en M2a, utile M2b),
  `Log(LogLevel::Error)` → `codec_error=true` + `container_ok=false`.
- Erreur de décodage **en fin** de flux alors que des données ont été lues → indice fort de
  **troncature** (croisé avec l'énergie de fin et la durée déclarée vs décodée).
- `FFMPEG_BINARY` déjà câblé en dev (M0). Aucun `auto_download`.

## Données

- **M2a n'écrit PAS en DB** (c'est M2b). `analyze()` retourne `AnalysisReport` en mémoire.
- Le mapping `AnalysisReport` → colonnes `tracks` (`verdict`, `bitrate`, `real_quality`,
  `clip_runs`, `clip_pct`, `true_peak_dbtp`, `dc_offset`, `phase_correlation`, `truncated`,
  `silence_head_ms`, `silence_tail_ms`, `has_cover`, `tags_cdj_ok`) — colonnes **déjà au
  schéma v1** — sera fait en M2b. (Le spectrogramme/peaks : à décider en M2b — cache fichier
  ou recalcul ; pas en DB.)

## IPC (M2a — debug uniquement)

- `analyze_path(path: string) -> AnalysisReport` — command de **debug** pour exercer le moteur
  depuis le front pendant le dev. Le câblage auto (worker) est M2b. Contrat typé dans
  `shared/contracts.ts`.

## Nouvelles dépendances (à valider)

- **`rustfft` 6.4.1** — FFT (spectre, cutoff, spectrogramme). Pur Rust, pas de C externe.
  Vérifié compatible Rust 1.77.2 (`cargo add --dry-run`).
- **`lofty` 0.22.2** — lecture tags/pochette unifiée (cf. question ouverte 4). ⚠️ le dernier
  (0.24) exige Rust 1.85 ; cargo retombe **auto sur 0.22.2** pour tenir le `rust-version`
  1.77.2 du projet — même contrainte MSRV que `notify-debouncer-full` en M1. Vérifié.
- (true-peak 4× : interpolation maison ou via une fonction simple — pas de crate dédié requis.)

## Tests (cœur du jalon — caractérisation)

- **`verdict.rs`** (pur, sans I/O) : table de cas `cutoff_hz` × format déclaré → `Verdict`
  attendu (Ok/Fake/Grey aux bornes des bandes). Le plus dense.
- **`spectrum.rs`** : signaux synthétiques générés en test (sinus purs, bruit blanc lowpassé
  à F connue) → `cutoff_hz` détecté ≈ F. Pas besoin de fichiers.
- **`dynamics.rs`** : buffers synthétiques (écrêté saturé, DC offset injecté) → valeurs exactes.
- **`structure.rs`** : silence en tête/queue synthétique → ms attendus.
- **`phase.rs`** : signal dupliqué (corr≈1), inversé (corr≈−1).
- **Caractérisation bout-en-bout** (dépend des **fixtures**, question ouverte 2) : sur le jeu
  connu, fige le verdict réel — vrai 320 ≠ fake 320, FLAC lossless = Ok, tronqué = truncated,
  etc. Snapshot des `AnalysisReport`.

## Risques / inconnues

- **Précision de la détection de coupure** sur du matériel réel (variabilité genre/mastering) —
  d'où le `Grey` et la preuve visuelle. À calibrer avec les fixtures.
- **`ffmpeg-sidecar` + blocs PCM** : valider le découpage en blocs et le mapping `s16le`→`f32`.
- **Perf** : viser un décodage proche temps-réel ou mieux ; mesurer sur un fichier long. Le
  throttling/batch est M2b, mais le moteur ne doit pas être pathologiquement lent.

## Hors périmètre M2a (rappel)

Worker auto + cache DB (M2b) · UI waveform/spectrogramme/badges (M2c) · player (M3) ·
conversion/rangement (M4) · empreinte/dédup (M5) · garde-fou Rekordbox.
