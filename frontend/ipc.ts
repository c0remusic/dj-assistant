import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppInfo,
  DbHealth,
  Source,
  QueueItem,
  AnalysisReport,
  AnalysisProgress,
  Canonical,
  Bin,
  FileResult,
  BatchResult,
  FileProgress,
  RejectBatchResult,
  JournalEntry,
  Target,
  EcarteItem,
  DupMatch,
  LibraryTrack,
  LibraryFacets,
  LibraryFilter,
  MetadataEdit,
} from "../shared/contracts";

export const appInfo = (): Promise<AppInfo> => invoke("app_info");
export const dbHealth = (): Promise<DbHealth> => invoke("db_health");
export const ffmpegVersion = (): Promise<string> => invoke("ffmpeg_version");

export const addSource = (path: string): Promise<Source> =>
  invoke("add_source", { path });
export const listSources = (): Promise<Source[]> => invoke("list_sources");
export const removeSource = (id: number): Promise<void> =>
  invoke("remove_source", { id });
export const listQueue = (): Promise<QueueItem[]> => invoke("list_queue");
export const rescanSource = (id: number): Promise<void> =>
  invoke("rescan_source", { id });
export const setSourceWatched = (id: number, watched: boolean): Promise<void> =>
  invoke("set_source_watched", { id, watched });

/** Debug: run the M2a analysis engine on a file path and return the full report.
 * `withSpectrogram` builds the heavy display grid (verdict/scalars are identical either way). */
export const analyzePath = (
  path: string,
  withSpectrogram = false,
): Promise<AnalysisReport> =>
  invoke("analyze_path", { path, withSpectrogram });

/** Background-analysis progress (pending analysed / total pending). */
export const analysisProgress = (): Promise<AnalysisProgress> =>
  invoke("analysis_progress");

/** Subscribe to backend "queue:changed" pings. Returns an unlisten fn. */
export const onQueueChanged = (cb: () => void): Promise<UnlistenFn> =>
  listen("queue:changed", () => cb());

/** Subscribe to "analysis:changed" pings (a track just got analysed). */
export const onAnalysisChanged = (cb: () => void): Promise<UnlistenFn> =>
  listen("analysis:changed", () => cb());

// ---- M4 filing loop (mirror of ipc_filing.rs) ----

/** Reconcile a track's tags + filename into the canonical record + confidence. */
export const reconcile = (trackId: number): Promise<Canonical> =>
  invoke("reconcile", { trackId });

/** File one track into `binRel`. `target` overrides the rail default; `edited` overrides
 * the reconciled metadata with the user's corrections. Resolves to the filed path. */
export const fileTrack = (
  trackId: number,
  binRel: string,
  target?: Target | null,
  edited?: Canonical | null,
): Promise<FileResult> =>
  invoke("file_track", {
    trackId,
    binRel,
    target: target ?? null,
    edited: edited ?? null,
  });

/** Launch background filing of `trackIds` into `binRel`. Resolves as soon as the background task
 * is STARTED (not when it finishes) — subscribe to `onFileDone` for the end-of-batch summary.
 * Rejects synchronously on NoLibraryRoot, or if the background task can't be started. */
export const fileBatch = (trackIds: number[], binRel: string): Promise<void> =>
  invoke("file_batch", { trackIds, binRel });

/** Subscribe to "file:done" (the background filing batch finished). Payload = run summary.
 * Returns an unlisten fn. */
export const onFileDone = (cb: (r: BatchResult) => void): Promise<UnlistenFn> =>
  listen<BatchResult>("file:done", (e) => cb(e.payload));

/** Subscribe to "file:progress" (one ping per file as the background filing advances).
 * Payload = { done, total }. Returns an unlisten fn. */
export const onFileProgress = (cb: (p: FileProgress) => void): Promise<UnlistenFn> =>
  listen<FileProgress>("file:progress", (e) => cb(e.payload));

/** Request a stop-net cancel of the running filing batch: the in-flight file finishes, then no new
 * one starts. Nothing is rolled back. No-op if nothing is running. */
export const fileCancel = (): Promise<void> => invoke("file_cancel");

/** Reject a batch of tracks for re-sourcing (each → Écartés). Returns how many were marked and
 * which ids failed (a misfire is reported, never aborts the rest). */
export const rejectBatch = (trackIds: number[]): Promise<RejectBatchResult> =>
  invoke("reject_batch", { trackIds });

/** Mark a track for re-sourcing (Écartés). */
export const rejectTrack = (trackId: number): Promise<void> =>
  invoke("reject_track", { trackId });

/** Move a track's file to .sift-trash (reversible via undo). */
export const trashTrack = (trackId: number): Promise<void> =>
  invoke("trash_track", { trackId });

/** All destination bins (recursive subdirs of the library root). */
export const listBins = (): Promise<Bin[]> => invoke("list_bins");

/** Create a new bin under `parentRel` ("" = root level). */
export const createBin = (parentRel: string, name: string): Promise<Bin> =>
  invoke("create_bin", { parentRel, name });

/** Undo the most recent live batch (LIFO). Resolves to the reverted batch id, or null. */
export const undoLast = (): Promise<string | null> => invoke("undo_last");

/** Revert a specific batch by id (from the journal). */
export const revertBatch = (batchId: string): Promise<void> =>
  invoke("revert_batch", { batchId });

/** Recent live (not-yet-undone) batches, newest first. */
export const listJournal = (limit = 20): Promise<JournalEntry[]> =>
  invoke("list_journal", { limit });

/** Read one app setting (null when unset). */
export const getSetting = (key: string): Promise<string | null> =>
  invoke("get_setting", { key });

/** Write one app setting (e.g. the library root). */
export const setSetting = (key: string, value: string): Promise<void> =>
  invoke("set_setting", { key, value });

/** Rejected/trashed tracks for the Écartés view. */
export const listEcartes = (): Promise<EcarteItem[]> => invoke("list_ecartes");

/** Restore a trashed track's file and re-queue it. */
export const restoreTrack = (trackId: number): Promise<void> =>
  invoke("restore_track", { trackId });

/** Put a re-sourcing track back into the queue (undo a "Re-sourcer" misclick). */
export const requeueTrack = (trackId: number): Promise<void> =>
  invoke("requeue_track", { trackId });

/** Permanently empty the bin. Returns how many tracks were purged. */
export const purgeTrash = (): Promise<number> => invoke("purge_trash");

/** Open an external http(s) URL in the default browser (Écartés buy links). */
export const openUrl = (url: string): Promise<void> => invoke("open_url", { url });

/** Best duplicate match for a track (by name; sound-confirmed when available), or null. */
export const findDuplicate = (trackId: number): Promise<DupMatch | null> =>
  invoke("find_duplicate", { trackId });

/** Import OS-dropped paths: directories become watched sources, audio files become pending
 * queue items. Returns how many of each were added. */
export const importPaths = (
  paths: string[],
  mode: "source" | "dest" = "source",
): Promise<{ files_added: number; folders_added: number }> =>
  invoke("import_paths", { paths, mode });

// ---- M6a Discogs identification ----

export interface Candidate {
  artist: string;
  title: string;
  label: string | null;
  year: number | null;
  styles: string[];
  country: string | null;
  format: string | null;
  cover_url: string | null;
  release_id: string;
  source: string;
}

export interface AppliedIdentity {
  canonical: { artist: string; title: string; version: string | null; confidence: string };
  label: string | null;
  year: number | null;
  styles: string[];
  cover_path: string | null;
}

/** Search Discogs for candidates matching the track. May reject with error codes:
 * "NO_TOKEN", "RATE_LIMITED:<seconds>", "NETWORK:<msg>", "PARSE:<msg>". */
export const identify = (trackId: number): Promise<Candidate[]> =>
  invoke("identify", { trackId });

/** Apply a chosen candidate: writes tags + downloads cover. Returns the applied identity. */
export const applyIdentity = (trackId: number, candidate: Candidate): Promise<AppliedIdentity> =>
  invoke("apply_identity_cmd", { trackId, candidate });

// ---- M6b library browser (mirror of ipc_library.rs) ----

/** Filed tracks for the Bibliothèque list, with optional filters. */
export const listLibrary = (filter?: LibraryFilter): Promise<LibraryTrack[]> =>
  invoke("list_library", { filter: filter ?? null });

/** Folder + genre facet counts for the Bibliothèque sidebar. */
export const libraryFolders = (): Promise<LibraryFacets> =>
  invoke("library_folders");

/** Edit a filed track's metadata: writes the file tags first, then the DB. Preserves the
 * Discogs release link. Rejects (DB untouched) if the file write fails. */
export const updateMetadata = (trackId: number, edit: MetadataEdit): Promise<void> =>
  invoke("update_metadata", { trackId, edit });
