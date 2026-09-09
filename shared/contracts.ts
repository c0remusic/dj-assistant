// Wire contract — mirror of src-tauri/src/ipc.rs serde structs.
// Keep field names and types in sync with the Rust side. Bump when the Rust side changes.

/** Sentinel `binRel` meaning "file in place" (destination = each track's own source folder).
 * Mirror of Rust `filing::FILE_IN_PLACE`; the two literals MUST stay identical. Travels through
 * the normal `binRel` channel — no separate flag — and the backend resolves it in `plan_file`. */
export const FILE_IN_PLACE = "__SOURCE__";

/** Prefix marking `binRel` as a trusted absolute path OUTSIDE the library root ("Parcourir un
 * autre dossier…"). Mirror of Rust `filing::EXTERNAL_DEST_PREFIX`; the two literals MUST stay
 * identical. Trust boundary: build this value ONLY from a path returned by the Tauri directory
 * picker dialog (an existing directory the user navigated to and selected) — never from
 * free-typed text. The backend still re-validates the path exists before use. */
export const EXTERNAL_DEST_PREFIX = "__EXTERNAL__::";

/** Fragment marking an analysis error as "the file is gone from disk", and the only part of that
 * message anything may branch on. Mirror of Rust `analysis::decode::FILE_GONE`; the two literals
 * MUST stay identical — this is the one sentinel whose breakage DELETES a database row
 * (`ipc::analyze_path` → `scanner::forget_path`) as well as flipping this UI branch. Rewording the
 * sentence around it is safe; changing it is not. Held by
 * `filing.rs::file_gone_constant_matches_contracts_ts`. */
export const FILE_GONE = "n'existe plus";

/** `format_drive` refused: the expected drive is no longer connected. Mirror of Rust
 * `usb_format::DRIVE_VANISHED`. */
export const DRIVE_VANISHED = "DRIVE_VANISHED";

/** `format_drive` refused: a DIFFERENT drive now answers to that id — its volume serial no longer
 * matches the one the user confirmed. Mirror of Rust `usb_format::IDENTITY_MISMATCH`. A format is
 * irreversible, so this is the one error the UI must never answer with "try again". */
export const IDENTITY_MISMATCH = "IDENTITY_MISMATCH";

/** `format_drive` refused: the user dismissed the Windows elevation (UAC) prompt. Mirror of Rust
 * `usb_format::ELEVATION_DECLINED`. Formatting a disk requires administrator rights on every OS —
 * `diskpart` refuses even `list disk` from a normal user process — so Sift asks for elevation for
 * this one operation. Declining is a deliberate user choice, not a fault: nothing was touched, and
 * the UI must say so rather than blame the drive. */
export const ELEVATION_DECLINED = "ELEVATION_DECLINED";

/** `eject_drive` a échoué : le système refuse de démonter le volume, un programme le tient encore
 * ouvert. Miroir de Rust `usb_format::EJECT_BUSY`. C'est le cas FRÉQUENT d'une éjection, pas un cas
 * limite. Rien n'a été démonté : débrancher maintenant reste risqué, et le message doit dire quoi
 * fermer plutôt que d'inviter à réessayer. */
export const EJECT_BUSY = "EJECT_BUSY";

/** Default filename template — the value `filename_template` holds until the user changes it.
 * Mirror of Rust `settings::DEFAULT_TEMPLATE`; the two literals MUST stay identical, held by
 * `filing.rs::default_template_matches_contracts_ts`. Placeholders, and there are only these
 * three: `{artist}`, `{title}`, `{version}` — the last expands to " (Version)" when the track has
 * one and to "" when it doesn't (no empty parens). Rendering is `naming::render_filename`, which
 * also runs `sanitize()`; never re-implement either in TS — call `previewFilename` instead. */
export const DEFAULT_FILENAME_TEMPLATE = "{artist} - {title}{version}";

export interface AppInfo {
  name: string;
  version: string;
}

export interface DbHealth {
  schema_version: number;
  tables: number;
}

export interface Source {
  id: number;
  path: string;
  pending_count: number;
  /** Total de fichiers reconnus par le scan, tous statuts — 0 = « aucun fichier audio reconnu »
   *  (issue #55, badge « 0 audio » du rail). Un `pending_count` à 0 ne distingue pas ce cas
   *  d'un dossier entièrement traité. */
  track_count: number;
  accessible: boolean;
  watched: boolean;
  color_key: string | null;
}

export interface QueueItem {
  id: number;
  path: string;
  filename: string | null;
  source_id: number | null;
  verdict: "ok" | "fake" | "grey" | null;
  /** Declared rail, NULL until analysed. Drives batch grouping + output format. */
  rail: "lossless" | "lossy" | "unknown" | null;
  /** Identified artist/title (metadata table), NULL until identified — the "after" name. */
  artist: string | null;
  title: string | null;
  /** Shares a name with another pending/filed track (dedup name pre-filter). */
  dup: boolean;
  /** True when there's no current, usable verdict for this track: not-yet-analysed, due for
   *  re-analysis (content changed), OR a permanently-stuck decode failure the worker will never
   *  retry on its own (verdict stays null but analyzed_at/report_json get set — see
   *  queue.rs::QueueItem's own doc comment for the full backend rationale). Single source of
   *  truth for "offer a re-analyze affordance" — never re-derive this from `verdict` alone. */
  needs_analysis: boolean;
  /** Number of failed analyses so far. `>= MAX_ANALYSIS_ATTEMPTS` means terminally broken: still
   *  individually retryable (a per-row retry resets it to 0), but excluded from the count and the
   *  bulk "Réanalyser (N)" so a genuinely unrepairable file stops inflating "Non analysés (N)". */
  analysis_attempts: number;
  /** Duration in seconds, NULL until analysed. */
  duration: number | null;
  /** Bitrate in kbps, NULL until analysed. */
  bitrate: number | null;
  /** Declared format ("mp3" | "aac" | "flac" | …), NULL until analysed. */
  declared_fmt: string | null;
}

/** Mirror of queue.rs::MAX_ANALYSIS_ATTEMPTS. After this many failures a stuck track drops out of
 *  the unanalysed count / bulk retry (a manual per-row retry clears the counter for a fresh try). */
export const MAX_ANALYSIS_ATTEMPTS = 3;

/** Best duplicate match for a track. kind: "name" (names agree) or "both" (name + sound). */
export interface DupMatch {
  id: number;
  status: string;
  folder: string | null;
  filename: string | null;
  kind: "name" | "both";
  score: number;
}

export interface AnalysisProgress {
  done: number;
  total: number;
}

export interface Spectrogram {
  frames: number;
  bins: number;
  hz_per_bin: number;
  sec_per_frame: number;
  /** frames*bins, row-major, 0..255 (-100..0 dBFS). Travels on the wire as an RFC1924 base85
   *  string (Rust side: `#[serde(with = "crate::b85_bytes")]` on analysis/mod.rs Spectrogram),
   *  decoded EXACTLY ONCE in frontend/ipc.ts analyzePath — nothing downstream ever sees the
   *  string form. Indexing stays `mag_db[f * bins + b]`, unchanged. */
  mag_db: Uint8Array;
}

// Mirror of src-tauri/src/analysis/mod.rs AnalysisReport (M2a).
export interface AnalysisReport {
  path: string;
  sample_rate: number;
  channels: number;
  duration_sec: number;
  declared_format: string;
  declared_bitrate: number | null;
  declared_rail: "lossless" | "lossy" | "unknown";
  cutoff_hz: number;
  verdict: "ok" | "fake" | "grey";
  /** True when declared_rail is "lossless" but the real container (magic-byte sniffed) is
   *  lossy — the specific Fake cause where the cutoff can sit near Nyquist, unlike a genuine
   *  spectral-cliff transcode. Mirrors the Rust condition, don't recompute in TS. */
  container_mismatch: boolean;
  /** Equivalent lossy bitrate estimated from cutoff_hz (FIX-11: computed in Rust, single
   *  source of truth shared with the verdict logic — don't recompute this in TS). */
  est_kbps: number;
  peaks: number[];
  /** Mono samples represented by ONE entry of `peaks`. 512 for an uncapped envelope, a multiple
   *  of it once the backend max-pooled it (analysis::MAX_PEAKS). Always read this instead of
   *  assuming 512: on a capped track the constant under-reports the covered duration severalfold. */
  peaks_step: number;
  spectrogram: Spectrogram;
  clip_runs: number;
  clip_pct: number;
  true_peak_dbtp: number;
  dc_offset: number;
  phase_correlation: number;
  dual_mono: boolean;
  container_ok: boolean;
  codec_error: string | null;
  truncated: boolean;
  /** Platitude spectrale de la bande 16–20 kHz, médiane sur les trames, en dB. `null` quand la
   *  bande n'existe pas à ce taux d'échantillonnage.
   *
   *  Un FAIT sur le fichier tel qu'il est, pas une affirmation sur son histoire. « L'aigu est
   *  clairsemé » se mesure ; « ça a été un MP3 » ne s'en déduit pas — un master volontairement
   *  sombre donne la même valeur qu'un transcodage. C'est pourquoi le verdict ne la lit pas, et
   *  pourquoi un affichage ne doit jamais la présenter comme une accusation.
   *
   *  Repère mesuré (corpus étiqueté, 2026-08-18) : authentiques dans [−5,4 ; −2,6] dB,
   *  transcodages jusqu'à −43,8. Plus c'est négatif, plus l'aigu est clairsemé.
   *
   *  Ancien rapport en cache : `null` — traiter comme « pas mesuré », jamais comme 0. */
  hf_flatness_db: number | null;
  /** Même mesure sur une bande RELATIVE au Nyquist (0,80–0,98) plutôt qu'en Hz fixes.
   *
   *  Les deux sont nécessaires, chacune étant aveugle là où l'autre voit : un MP3 128 coupe à
   *  16,8 kHz, donc la bande relative tombe entièrement au-dessus de sa coupure sur un plancher
   *  uniforme — parfaitement plat, donc « authentique » ; un Opus est à 48 kHz et garde du contenu
   *  jusqu'à 20 kHz, donc la bande FIXE y est en pleine bande passante et ne voit rien.
   *
   *  Mesuré sur 150 transcodages, seuils au plancher de 32 authentiques : fixe 63 %, relative
   *  17 %, union 68 %. Opus 0/10 → 6/10. Sa plage de référence est très large (−23,8 à −2,5) parce
   *  que le haut du spectre d'un master ambient est légitimement clairsemé. */
  hf_flatness_top_db: number | null;
  /** Durée RÉELLEMENT décodée, à comparer à `duration_sec` qui vient de l'en-tête.
   *
   *  Les deux étaient une seule valeur — la déclarée — et rien ne vérifiait qu'elle correspondait
   *  au son présent. Un en-tête peut annoncer 6 minutes sur un fichier tronqué à 40 secondes :
   *  `truncated` ne l'attrape pas, il teste une coupure ABRUPTE du signal, pas un désaccord de
   *  comptage. Les deux échouent sur des cas différents.
   *
   *  Brut plutôt que booléen : la tolérance est une décision d'affichage, et un écart se lit mieux
   *  en secondes. Ancien rapport en cache : `0` (le `#[serde(default)]` côté Rust), donc traiter
   *  zéro comme « pas mesuré » et non comme « durée nulle ». */
  decoded_duration_sec: number;
  /** Vraisemblance que le signal soit DÉJÀ passé par la grille de quantification d'un codec AAC —
   *  le troisième signal du verdict (issue #52, méthode d'Olivier Derrien, JAES 67(3), 2019).
   *
   *  `null` = **pas mesurée**. Elle n'est calculée que là où elle peut trancher — lossless
   *  déclaré, conteneur non démenti, **bande pleine** (la falaise n'a alors plus rien à dire) —
   *  donc sur ~tout lossless sain, jamais sur un lossy ni sur un fichier déjà Faux par la falaise
   *  ou par le conteneur. Ailleurs elle n'est pas basse, elle n'a pas été demandée. Ne JAMAIS
   *  lire `null` comme 0 : 0 est une vraie valeur, celle d'un signal dont aucune cellule ne porte
   *  la grille.
   *
   *  Un FAIT, pas un verdict. Le seuil qui le juge est `verdict::QUANT_LAMBDA` (0,18 au
   *  2026-09-02, PROVISOIRE — calibré sur 10 authentiques seulement) et il n'a pas de miroir ici :
   *  le front n'a pas à re-seuiller une mesure, il affiche le verdict que Rust a rendu.
   *
   *  Ancien rapport en cache (`REPORT_CACHE_VERSION` ≤ 9) : `null`. */
  quant_likelihood: number | null;
  silence_head_ms: number;
  silence_tail_ms: number;
  /** TYPE(S) de tag présents dans le fichier, noms `lofty` triés par ordre alphabétique puis
   *  joints par `+` — « Id3v2 », « RiffInfo », « VorbisComments », « Id3v1+Id3v2 »… `null` si le
   *  fichier ne porte aucun tag. Le tri est fait par `analysis/tags.rs::read` : l'ordre de parcours
   *  du conteneur n'est garanti par rien, et cette chaîne est persistée en base.
   *
   *  Le nom est historique (colonne SQLite, issue #46) : il ne porte AUCUNE sous-version ID3.
   *  `lofty` ne l'expose pas sur un tag générique (il remonte tout en v2.4), et Pioneer déclare
   *  v1/v1.1/v2.2/v2.3/v2.4 indifféremment — la sous-version ne change donc aucun verdict.
   *  Jusqu'au 2026-09-01 ce champ valait la chaîne littérale « ID3 » pour toute extension `.mp3`
   *  et `null` sinon : un rapport en cache d'avant peut encore porter cette valeur. */
  id3_version: string | null;
  /** Artiste ET Titre vivent dans un tag d'un TYPE qu'une platine lit en navigation directe sur
   *  clé USB, sans base rekordbox — pas la seule PRÉSENCE d'Artiste+Titre (issue #46).
   *
   *  Matrice de `docs/cdj-metadata-formats.md`, appliquée par `analysis/tags.rs`
   *  (`tag_type_readable_on_cdj`) : MP3+Id3v2/Id3v1, AIFF+Id3v2, FLAC+VorbisComments. Tout le
   *  reste vaut `false` par prudence — WAV en tête, quel que soit le porteur (RIFF INFO ou ID3
   *  en chunk), parce que la platine y retombe sur le nom de fichier.
   *
   *  Ne juge QUE le porteur des tags : ni le codec, ni l'encodage du texte, ni le système de
   *  fichiers. */
  tags_cdj_ok: boolean;
  has_cover: boolean;
}

// ---- M4 filing loop (mirror of naming.rs / encode.rs / library.rs / actions.rs) ----

/** Output rail shapes. Serde-renamed on the Rust side (see encode.rs Target). */
export type Target = "mp3_320" | "aiff_16_44" | "wav_16_44";

/** How sure reconciliation is about the metadata — green files in one click. */
export type Confidence = "green" | "yellow";

/** Canonical {artist,title,version} that drives BOTH the output filename and the tags. `label` is a
 *  tag-only member (the editable Discogs release label): it does NOT affect the filename — Rust's
 *  `render_filename`/`tag_title` ignore it — and rides here so the Revue pane edits it in place and
 *  `apply_tags` writes it. Mirror of Rust `naming::Canonical`; pinned field-for-field by
 *  `naming.rs::canonical_shape_matches_contracts_ts` (exhaustive destructure). */
export interface Canonical {
  artist: string;
  title: string;
  version: string | null;
  label: string | null;
  confidence: Confidence;
}

/** A destination folder under the library root (recursive). */
export interface Bin {
  rel: string; // forward-slash path relative to root, e.g. "House/Deep"
  name: string; // last component
  depth: number; // 1 = direct child
}

/** Result of filing one track. */
export interface FileResult {
  path: string;
  batch_id: string;
}

/** One track that errored during batch filing (mirror of filing.rs BatchError). */
export interface BatchError {
  track_id: number;
  message: string;
}

/** Result of filing a batch: how many filed, and the ids left needing validation. `cancelled` is
 *  true when the run was stop-net cancelled before processing every id (the result is then partial;
 *  nothing is rolled back). `filed_ids` lists which tracks filed (in batch order); `errors` carries
 *  per-track error messages for the subset of `needs_validation` that hit a real error. */
export interface BatchResult {
  filed: number;
  needs_validation: number[];
  cancelled: boolean;
  filed_ids: number[];
  errors: BatchError[];
}

/** Outcome of the BACKGROUND half of an interactive filing, delivered on the `file:track:done`
 *  event (mirror of ipc_filing.rs TrackFileOutcome). Since P5 `file_track` returns as soon as the
 *  plan is settled — the click is acknowledged, the multi-second encode keeps running behind it —
 *  so this payload is the only place the real result lands. `error !== null` means the filing did
 *  NOT happen and the track is still `pending`: the same bounce the batch path reports in bulk as
 *  `BatchResult.needs_validation`, for one interactive track. */
export interface TrackFileOutcome {
  track_id: number;
  batch_id: string;
  /** The filed path — non-null only when the filing actually committed. */
  path: string | null;
  /** Failure cause, null on success. */
  error: string | null;
}

/** Per-file filing progress (mirror of ipc_filing.rs FileProgress). `done` = files processed so
 *  far (filed or bounced), `total` = batch size. Drives the global progress zone's "file" row. */
export interface FileProgress {
  done: number;
  total: number;
}

/** Result of rejecting a batch: how many marked for re-sourcing, and the ids that failed. */
export interface RejectBatchResult {
  rejected: number;
  failed: number[];
}

/** Result of emptying the bin (mirror of ecartes.rs PurgeResult): how many files were really
 *  deleted, and the ids of the tracks whose file resisted deletion — those stay in the bin. */
export interface PurgeResult {
  purged: number;
  failed: number[];
}

/** One rejected/trashed track for the Écartés view. */
export interface EcarteItem {
  id: number;
  path: string;
  filename: string | null;
  status: "resourcing" | "trash";
  verdict: "ok" | "fake" | "grey" | null;
  truncated: boolean;
  artist: string;
  title: string;
}

/** One consultable undo-journal entry (a batch, summarized by its first action). */
export interface JournalEntry {
  batch_id: string;
  track_id: number | null;
  /** First action type of the batch — determines display category.
   *  "convert"|"move" → Filés; "trash" → Jetés; "reject" → Rejetés. */
  kind: "convert" | "move" | "trash" | "reject";
  from_path: string | null;
  to_path: string | null;
  ts: string;
  session_id: string | null;
  /** Distinct track count in the batch — used to gate the last-batch confirmation on > 10. */
  track_count: number;
  /** Whether the batch is undone AS A WHOLE (`MIN(undone) != 0` over its rows, actions.rs). A
   *  batch only PARTIALLY undone — a revert that failed mid-way, still retryable — reads false.
   *  Pinned by `actions.rs::journal_lists_undone_batches_with_their_flag`, which destructures
   *  `JournalEntry` exhaustively: a new Rust field breaks its compilation until this mirror
   *  follows. */
  undone: boolean;
}

// ---- M6b library browser (mirror of library.rs) ----

export interface LibraryTrack {
  id: number;
  path: string;
  artist: string | null;
  title: string | null;
  format: string | null;
  bitrate: number | null;
  duration: number | null;
  bpm: number | null;
  year: number | null;
  label: string | null;
  genres: string[];
  discogs_release_id: string | null;
  cover_path: string | null;
  has_cover: boolean;
  verdict: string | null;
  folder: string | null;
}

/** Read-only identity + release facts for a track, from the persisted `metadata` table. Mirror of
 * Rust `ipc_filing::TrackRelease`. `identified` = a Discogs release was chosen → the front trusts
 * `artist`/`title` here over reconcile (which recomputes from the still-untouched file tags). All
 * null / `identified:false` when there is no metadata row. `version` is the remix/dub split off the
 * chosen Discogs title and persisted in the `metadata.version` column, so the picked release survives
 * a close+reopen; the front falls back to reconcile's version when it is null. NOT folded into
 * `Canonical`. */
export interface TrackRelease {
  artist: string | null;
  title: string | null;
  version: string | null;
  label: string | null;
  year: number | null;
  cover_path: string | null;
  /** The track's sub-genres in stored order — the same list write_tags_full joins into the file's
   * Genre field. Shown on open; the joined form feeds the file-vs-display discrepancy check. */
  genres: string[];
  identified: boolean;
}

/** The file's REAL tag values (the fields write_tags_full owns), read once on open via
 * `track_file_tags`. Compared IN MEMORY against the displayed identity to flag tags not yet written
 * to the file (no per-keystroke disk read). `genre_joined` is the single Genre field exactly as the
 * file holds it (write_tags_full's joined "A; B" form). Mirror of Rust `ipc_filing::FileTags`. */
export interface FileTags {
  artist: string | null;
  title: string | null;
  label: string | null;
  year: number | null;
  genre_joined: string | null;
}

export interface LibraryFolder { name: string; count: number; }
export interface LibraryFacets { folders: LibraryFolder[]; genres: LibraryFolder[]; artists: LibraryFolder[]; }

export interface LibraryFilter {
  folder?: string | null;
  quality?: "lossless" | "mp3" | null;
  genre?: string | null;
  q?: string | null;
  verdict?: "fake" | null;
  artist?: string | null;
}

/** User-edited metadata for a filed track (Bibliothèque inline edit). Mirror of
 * src-tauri/src/metadata/mod.rs MetadataEdit. The backend writes the file tags first,
 * then the DB — and preserves discogs_release_id/source (a manual edit never wipes the link). */
export interface MetadataEdit {
  artist: string;
  title: string;
  label: string | null;
  year: number | null;
  genres: string[];
  cover_path: string | null;
}

// ---- M6b Lot 3: internal duplicates (mirror of src-tauri/src/dedup.rs) ----

export interface DupGroupMember {
  id: number;
  path: string;
  filename: string | null;
  folder: string | null;
  format: string | null;
  bitrate: number | null;
  duration: number | null;
  truncated: boolean;
  recommend_keep: boolean;
  reason: string | null;
}

export interface DupGroup {
  members: DupGroupMember[];
  similarity: number;
}

// ---- M6b Lot 4: dashboard (mirror of src-tauri/src/library.rs) ----

export interface GenreCount { genre: string; count: number; }

export interface DashboardStats {
  total: number;
  lossless: number;
  mp3: number;
  duplicates: number;
  fake: number;
  genres: GenreCount[];
}

// ---- M7 Rekordbox XML export + playlist path repair (mirror of src-tauri/src/ipc_library.rs) ----

export interface RekordboxLinkStatus {
  path: string | null;
  linked: boolean;
  playlist_count: number;
  track_count: number;
  error: string | null;
  /** True when a prior filing/move's Rekordbox repair hit an ambiguous match and could not
   *  safely patch the linked XML — surfaced as a warning banner (see Task 3). */
  drift_detected: boolean;
  /** Pourquoi les 4 cartes de synchronisation M8 ne peuvent rien faire, quand c'est le cas.
   *  `error` juste au-dessus ne parle QUE du XML lié ; les détecteurs M8 lisent `master.db`, un
   *  autre fichier à un autre endroit. Sans ce champ, `master.db` introuvable donnait quatre
   *  cartes vides que l'écran peignait « à jour » — impasse A13, issue #15.
   *  Portée : présence du fichier à l'endroit attendu. PAS sa lisibilité — le déchiffrer coûterait
   *  plusieurs Mo de SQLCipher à chaque affichage de l'écran. */
  masterdb_error: string | null;
}

// ---- M8 Tier 1 master.db path-repair candidates (mirror of src-tauri/src/ipc_library.rs) ----

export interface CandidateTrack {
  track_id: string;
  folder_path: string | null;
}

export interface PendingMasterdbRepair {
  id: number;
  track_id: string | null;
  candidate_track_ids: string | null;
  candidate_tracks: CandidateTrack[] | null;
  from_path: string;
  to_path: string;
  status: "pending" | "ambiguous";
  detected_at: string;
  /** `metadata.artist` / `.title` of the moved track — display only, null without a metadata row
   *  (2026-09-08, pinned by `pending_masterdb_repair_shape_matches_contracts_ts`). */
  artist: string | null;
  title: string | null;
  /** The Sift app session that produced this candidate — null for pre-migration rows. */
  session_id: string | null;
}

export interface ApplyRepairOutcome {
  id: number;
  ok: boolean;
  error: string | null;
}

// ---- M8 Tier 3 master.db metadata sync candidates (mirror of src-tauri/src/ipc_library.rs) ----

export interface PendingMetadataSync {
  id: number;
  track_id: number;
  sift_path: string;
  rekordbox_track_id: string | null;
  candidate_track_ids: string | null;
  candidate_tracks: CandidateTrack[] | null;
  new_artist: string | null;
  new_title: string | null;
  new_label: string | null;
  new_year: number | null;
  new_genre: string | null;
  status: "pending" | "ambiguous";
  detected_at: string;
  /** The Sift app session that produced this candidate — null for pre-migration rows. */
  session_id: string | null;
}

export interface ApplyMetadataSyncOutcome {
  id: number;
  ok: boolean;
  error: string | null;
}

// ---- M8 Tier 3 master.db artwork sync candidates (mirror of src-tauri/src/rekordbox_repairs.rs) ----

export interface PendingArtworkSync {
  id: number;
  track_id: number;
  sift_path: string;
  rekordbox_track_id: string | null;
  candidate_track_ids: string | null;
  candidate_tracks: CandidateTrack[] | null;
  cover_path: string;
  status: "pending" | "ambiguous";
  detected_at: string;
  /** `metadata.artist` / `.title` of the track — display only, null without a metadata row
   *  (2026-09-08, pinned by `pending_artwork_sync_shape_matches_contracts_ts`). */
  artist: string | null;
  title: string | null;
  /** The Sift app session that produced this candidate — null for pre-migration rows. */
  session_id: string | null;
}

export interface ApplyArtworkSyncOutcome {
  id: number;
  ok: boolean;
  error: string | null;
}

// ---- M8 Tier 2 playlist duplicate-entry dedup (mirror of src-tauri/src/ipc_library.rs) ----

export interface PlaylistDuplicateEntryDto {
  song_playlist_id: string;
  track_no: number;
}

export interface PlaylistDuplicateGroupDto {
  playlist_id: string;
  playlist_name: string | null;
  content_id: string;
  track_path: string | null;
  keep: PlaylistDuplicateEntryDto;
  remove: PlaylistDuplicateEntryDto[];
}
