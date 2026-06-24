// Wire contract — mirror of src-tauri/src/ipc.rs serde structs.
// Keep field names and types in sync with the Rust side. Bump when the Rust side changes.
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
  accessible: boolean;
  watched: boolean;
}

export interface QueueItem {
  id: number;
  path: string;
  filename: string | null;
  source_id: number | null;
  verdict: "ok" | "fake" | "grey" | null;
  /** Shares a name with another pending/filed track (dedup name pre-filter). */
  dup: boolean;
}

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
  mag_db: number[]; // frames*bins, 0..255 (-100..0 dBFS)
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
  peaks: number[];
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
  silence_head_ms: number;
  silence_tail_ms: number;
  id3_version: string | null;
  tags_cdj_ok: boolean;
  has_cover: boolean;
}

// ---- M4 filing loop (mirror of naming.rs / encode.rs / library.rs / actions.rs) ----

/** Output rail shapes. Serde-renamed on the Rust side (see encode.rs Target). */
export type Target = "mp3_320" | "aiff_16_44" | "wav_16_44";

/** How sure reconciliation is about the metadata — green files in one click. */
export type Confidence = "green" | "yellow";

/** Canonical {artist,title,version} that drives BOTH the output filename and the tags. */
export interface Canonical {
  artist: string;
  title: string;
  version: string | null;
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

/** Result of filing a batch: how many filed, and the ids left needing validation. */
export interface BatchResult {
  filed: number;
  needs_validation: number[];
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

/** One consultable undo-journal entry (a live batch, summarized by its latest action). */
export interface JournalEntry {
  batch_id: string;
  track_id: number | null;
  kind: "convert" | "move" | "trash" | "reject";
  to_path: string | null;
  ts: string;
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

export interface LibraryFolder { name: string; count: number; }
export interface LibraryFacets { folders: LibraryFolder[]; genres: LibraryFolder[]; }

export interface LibraryFilter {
  folder?: string | null;
  quality?: "lossless" | "mp3" | null;
  genre?: string | null;
  q?: string | null;
}
