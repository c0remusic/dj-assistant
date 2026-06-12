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
  JournalEntry,
  Target,
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

/** File every green track of `trackIds` into `binRel`; yellow/errored ones come back in
 * `needs_validation`. */
export const fileBatch = (
  trackIds: number[],
  binRel: string,
): Promise<BatchResult> => invoke("file_batch", { trackIds, binRel });

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
