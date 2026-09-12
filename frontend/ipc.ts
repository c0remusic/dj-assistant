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
  PurgeResult,
  JournalEntry,
  Target,
  EcarteItem,
  DupMatch,
  LibraryTrack,
  LibraryFacets,
  LibraryFilter,
  MetadataEdit,
  TrackRelease,
  FileTags,
  DupGroup,
  DashboardStats,
  RekordboxLinkStatus,
  PendingMasterdbRepair,
  ApplyRepairOutcome,
  PendingMetadataSync,
  ApplyMetadataSyncOutcome,
  PendingArtworkSync,
  ApplyArtworkSyncOutcome,
  PlaylistDuplicateGroupDto,
  Spectrogram,
} from "../shared/contracts";
import { decodeB85 } from "./b85";

export const appInfo = (): Promise<AppInfo> => invoke("app_info");
export const dbHealth = (): Promise<DbHealth> => invoke("db_health");
export const ffmpegVersion = (): Promise<string> => invoke("ffmpeg_version");

export const addSource = (path: string): Promise<Source> =>
  invoke("add_source", { path });
export const listSources = (): Promise<Source[]> => invoke("list_sources");
export const removeSource = (id: number): Promise<void> =>
  invoke("remove_source", { id });
export const listQueue = (): Promise<QueueItem[]> => invoke("list_queue");
export const reanalyzeTracks = (trackIds: number[]): Promise<number> =>
  invoke("reanalyze_tracks", { trackIds });
export const rescanSource = (id: number): Promise<void> =>
  invoke("rescan_source", { id });
export const setSourceWatched = (id: number, watched: boolean): Promise<void> =>
  invoke("set_source_watched", { id, watched });
export const setSourceColor = (id: number, colorKey: string | null): Promise<void> =>
  invoke("set_source_color", { id, colorKey });

/** On-the-wire shape of `analyze_path`: identical to AnalysisReport except `mag_db`, which the
 *  backend serialises as an RFC1924 base85 string instead of an array of decimal integers. This
 *  type stays LOCAL to this file — the applicative contract (shared/contracts.ts) only knows the
 *  decoded Uint8Array. */
type WireAnalysisReport = Omit<AnalysisReport, "spectrogram"> & {
  spectrogram: Omit<AnalysisReport["spectrogram"], "mag_db"> & { mag_db: string };
};

/** Vérifie l'unique invariant de taille du spectrogramme : `mag_db.length === frames * bins`.
 *
 *  POURQUOI ICI. `decodeB85` sait dire « ces caractères ne sont pas du base85 » ; il ne sait pas
 *  combien d'octets l'appelant attendait — c'est un miroir littéral du crate Rust (`b85.ts:1-14`),
 *  pas un lecteur de rapport d'analyse. La longueur attendue n'existe qu'ici, où le champ
 *  `frames`/`bins` qui la définit arrive dans le même objet. Et `analyzePath` est le SEUL point de
 *  décodage du frontend, donc la seule garde à écrire.
 *
 *  POURQUOI ÇA LÈVE PLUTÔT QUE DE DÉGRADER. Aucun chemin backend légitime ne produit d'écart :
 *  - `spectrum.rs:230-238` — rien à afficher → `frames: 0, bins: 0, mag_db: vec![]`. `0 === 0*0`,
 *    donc la sentinelle « pas de spectrogramme » satisfait l'invariant, elle n'y échappe pas ;
 *  - `spectrum.rs:264-268` — sinon `Vec::with_capacity(frames * out_bins)` rempli colonne par
 *    colonne, exactement `frames*bins` octets ;
 *  - `ipc.rs::cache_json` retire la grille par `std::mem::take` sur la struct ENTIÈRE, donc un
 *    rapport lu du cache vaut `Default` complet — jamais des `frames`/`bins` orphelins.
 *  Un écart ne peut donc venir que d'une chaîne tronquée sur le fil, d'une divergence entre ce
 *  décodeur et le crate, ou d'un bug backend. C'est un bug, pas un état de fonctionnement.
 *
 *  CE QUE ÇA COÛTE. Le seul appelant qui demande la grille est `report-view.ts:1087` (ouverture du
 *  collapse Diagnostic), déjà enveloppé dans un `try/catch` qui affiche « échec — réessayer »
 *  (`report-view.ts:1091-1096`). L'écran Revue lui-même s'ouvre avec `withSpectrogram=false`
 *  (`report-view.ts:1174`), comme le prefetch, la modale et le self-test : lever ne leur retire
 *  rien.
 *
 *  CE QUE ÇA PROTÈGE. `spectroPointAt` et `drawSpectrogram` indexent `mag_db[f * bins + b]` avec
 *  `f` et `b` bornés à `frames-1`/`bins-1` — index maximum `frames*bins - 1`. Sous cet invariant
 *  l'accès est toujours défini, ce qui est la raison pour laquelle le `|| 0` de ces deux lignes a
 *  pu partir : il transformait un `undefined` en 0, c'est-à-dire en -100 dBFS, c'est-à-dire en
 *  silence — une grille décalée finissant en noir, sans une seule erreur nulle part. */
export function assertSpectrogramLength(sg: Spectrogram): void {
  const expected = sg.frames * sg.bins;
  const actual = sg.mag_db.length;
  if (actual === expected) return;
  // Égalité stricte, pas `actual < expected` : une grille plus LONGUE s'indexerait sans erreur,
  // mais elle voudrait dire que `frames`/`bins` ne décrivent pas les octets qui les accompagnent
  // — donc que ce qui s'affiche n'est pas ce que l'analyse a mesuré. Même désaccord de contrat.
  //
  // Le message cite les deux longueurs ET leurs deux facteurs : sans eux on saurait qu'il y a un
  // écart sans pouvoir dire de quel côté il vient — chaîne tronquée sur le fil, ou `frames`/`bins`
  // qui mentent. Même exigence que `valueAt` (`b85.ts:33`), qui cite le caractère ET sa position.
  throw new Error(
    `assertSpectrogramLength: grille incohérente — ${expected} octets attendus ` +
      `(frames ${sg.frames} × bins ${sg.bins}), ${actual} reçus`,
  );
}

/** Debug: run the M2a analysis engine on a file path and return the full report.
 * `withSpectrogram` builds the heavy display grid (verdict/scalars are identical either way).
 * SINGLE base85 decode point for the whole frontend: every consumer of `spectrogram.mag_db`
 * (report-view spectroPointAt / drawSpectrogram) goes through here — et le seul endroit où la
 * taille de la grille décodée est confrontée aux `frames`/`bins` qui l'accompagnent. */
export const analyzePath = async (
  path: string,
  withSpectrogram = false,
  // Pass true ONLY from the genuine user-open path (openReportInto): on a confirmed gone file it
  // lets the backend drop the stale pending row. Background reads (prefetch, spectrogram re-fetch,
  // self-test) leave it false so an observation never silently deletes a queue row.
  allowForget = false,
): Promise<AnalysisReport> => {
  const wire = await invoke<WireAnalysisReport>("analyze_path", {
    path,
    withSpectrogram,
    allowForget,
  });
  const spectrogram = { ...wire.spectrogram, mag_db: decodeB85(wire.spectrogram.mag_db) };
  assertSpectrogramLength(spectrogram);
  return { ...wire, spectrogram };
};

/** Background-analysis progress (pending analysed / total pending). */
export const analysisProgress = (): Promise<AnalysisProgress> =>
  invoke("analysis_progress");

/** Subscribe to backend "queue:changed" pings. Returns an unlisten fn. */
export const onQueueChanged = (cb: () => void): Promise<UnlistenFn> =>
  listen("queue:changed", () => cb());

/** Subscribe to "analysis:changed" pings (a track just got analysed). */
export const onAnalysisChanged = (cb: () => void): Promise<UnlistenFn> =>
  listen("analysis:changed", () => cb());

/** Un scan de dossier surveillé n'a PAS eu lieu — payload `[source_id, raison]`.
 *
 *  Impasse A4 (issue #15) : `spawn_scan` avait quatre sorties silencieuses après lesquelles la
 *  source restait à `pending_count = 0`, que l'écran Accueil peint « À jour » en vert. Rien ne
 *  distinguait « rien de nouveau » de « le scan n'a jamais tourné ». */
export const onScanFailed = (cb: (sourceId: number, reason: string) => void): Promise<UnlistenFn> =>
  listen<[number, string]>("scan:failed", (e) => cb(e.payload[0], e.payload[1]));

// ---- M4 filing loop (mirror of ipc_filing.rs) ----

/** Reconcile a track's tags + filename into the canonical record + confidence. */
export const reconcile = (trackId: number): Promise<Canonical> =>
  invoke("reconcile", { trackId });

/** Live filename preview via the SAME naming::render_filename the real filing path uses (FIX-12) —
 * real active template + real sanitize(), not a front-side reimplementation. */
/** `template` omis = le modèle ENREGISTRÉ. Le passer permet d'apercevoir un modèle candidat non
 *  encore enregistré (écran Réglages) sans réimplémenter `render_filename`/`sanitize` en TS. */
export const previewFilename = (
  edited: Canonical,
  ext: string,
  template?: string | null,
): Promise<string> => invoke("preview_filename", { edited, ext, template: template ?? null });

/** Read-only release facts (label/year/genres) from the persisted `metadata` table. Fast DB read,
 * no network — used to show label/year/genres on a cold open. */
export const trackRelease = (trackId: number): Promise<TrackRelease> =>
  invoke("track_release", { trackId });

/** Read the file's REAL tags once (artist/title/label/year/genre joined). Used in memory to flag
 * when the displayed identity hasn't been written to the file yet — no per-keystroke disk read. */
export const trackFileTags = (trackId: number): Promise<FileTags> =>
  invoke("track_file_tags", { trackId });

/** Write the edited tags onto the file IN PLACE (no move, no encode, no status change). Journaled
 * as a revertable `tag_edit`; resolves to the new batch_id so the front can offer a targeted undo. */
export const applyTags = (trackId: number, edited: Canonical): Promise<string> =>
  invoke("apply_tags", { trackId, edited });

/** File one track into `binRel`. `target` overrides the rail default; `edited` overrides
 * the reconciled metadata with the user's corrections. `allowRailMismatch` (FIX-1): pass `true`
 * only after the user has explicitly confirmed a `"RAIL_MISMATCH"` warning (the source's
 * extension claims lossless but its content is actually lossy — an MP3 renamed `.flac`).
 * Resolves to the filed path; rejects with `"RAIL_MISMATCH"` when the mismatch isn't confirmed. */
export const fileTrack = (
  trackId: number,
  binRel: string,
  target?: Target | null,
  edited?: Canonical | null,
  allowRailMismatch?: boolean,
): Promise<FileResult> =>
  invoke("file_track", {
    trackId,
    binRel,
    target: target ?? null,
    edited: edited ?? null,
    allowRailMismatch: allowRailMismatch ?? null,
  });

/** Launch background filing of `trackIds` into `binRel`. Resolves as soon as the background task
 * is STARTED (not when it finishes) — subscribe to `onFileDone` for the end-of-batch summary.
 * Rejects synchronously on NoLibraryRoot, or if the background task can't be started. */
export const fileBatch = (
  trackIds: number[],
  binRel: string,
  targets?: Record<number, Target>,
): Promise<void> => invoke("file_batch", { trackIds, binRel, targets: targets ?? null });

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

/** Reveal a track's file in Explorer (Windows) or Finder (macOS), the file itself selected.
 *  Takes an id, never a path: the backend resolves it from the database, so this command can't be
 *  pointed at an arbitrary location. A missing file rejects rather than opening its folder. */
export const revealTrack = (trackId: number): Promise<void> =>
  invoke("reveal_track", { trackId });

/** All destination bins (recursive subdirs of the library root). */
export const listBins = (): Promise<Bin[]> => invoke("list_bins");

// `createBin` / la commande IPC `create_bin` : retirées toutes deux depuis le retrait de la création
// de dossier in-library en Revue (décision B, #47, 2026-08-25 — les deux boutons du pied du popover
// ouvrent le picker natif). Le wrapper frontend a disparu ici, et la commande exposée
// `ipc_filing::create_bin` + son entrée dans l'`invoke_handler` de lib.rs ont été retirées côté Rust.
// La fonction `library::create_bin` reste vivante — elle auto-crée un bac au filing (cf. ipc.rs) —
// mais n'est plus atteignable par une commande Tauri.

/** Undo the most recent live batch (LIFO). Resolves to the reverted batch id, or null. */
export const undoLast = (): Promise<string | null> => invoke("undo_last");

/** Revert a specific batch by id (from the journal). */
export const revertBatch = (batchId: string): Promise<void> =>
  invoke("revert_batch", { batchId });

/** Recent live batches, newest first. `sessionId` = current session → Journal tab;
 *  omit (undefined) → all sessions → extended journal page. */
export const listJournal = (limit = 50, sessionId?: string): Promise<JournalEntry[]> =>
  invoke("list_journal", { limit, sessionId: sessionId ?? null });

/** The session ID generated at this app launch (from settings). Used to filter
 *  list_journal to the current session in the Journal tab. */
export const getSessionId = (): Promise<string> => invoke("get_session_id");

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

/** Permanently empty the bin. Tracks whose file could not be deleted stay in the bin and come
 *  back in `failed` — the count alone would look like a success with fewer tracks. */
export const purgeTrash = (): Promise<PurgeResult> => invoke("purge_trash");

/** Open an external http(s) URL in the default browser (Écartés buy links). */
export const openUrl = (url: string): Promise<void> => invoke("open_url", { url });

/** Best duplicate match for a track (by name; sound-confirmed when available), or null. */
export const findDuplicate = (trackId: number): Promise<DupMatch | null> =>
  invoke("find_duplicate", { trackId });

/** Import OS-dropped paths: directories become watched sources, audio files become pending
 * queue items. Returns how many of each were added, plus `blocked_by` — la raison quand zéro
 * n'est PAS explicable par le contenu déposé (impasse A5, issue #15 : pas de racine de
 * bibliothèque en mode dest, ou création de bac en échec). `blocked_by` prime sur les compteurs. */
export const importPaths = (
  paths: string[],
  mode: "source" | "dest" = "source",
): Promise<{ files_added: number; folders_added: number; blocked_by: string | null }> =>
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
  canonical: { artist: string; title: string; version: string | null; label: string | null; confidence: string };
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

/** Demande à Discogs si le jeton enregistré est accepté. Résout sur un jeton valide, rejette avec
 *  les MÊMES codes qu'`identify` — `identifyErrorHtml` les traduit déjà.
 *
 *  Le jeton n'est pas passé en argument : il est lu côté Rust dans les réglages, donc il ne
 *  traverse pas l'IPC pour un appel qui ne fait que le vérifier. */
export const verifyDiscogsToken = (): Promise<void> => invoke("verify_discogs_token", {});

// ---- M6b library browser (mirror of ipc_library.rs) ----

/** Filed tracks for the Bibliothèque list, with optional filters. */
export const listLibrary = (filter?: LibraryFilter): Promise<LibraryTrack[]> =>
  invoke("list_library", { filter: filter ?? null });

/** Folder + genre facet counts for the Bibliothèque sidebar. */
export const libraryFolders = (): Promise<LibraryFacets> =>
  invoke("library_folders");

/** Edit a filed track's metadata: writes the file tags first, then the DB. Preserves the
 * Discogs release link. Rejects (DB untouched) if the file write fails. Returns the batch_id for undo. */
export const updateMetadata = (trackId: number, edit: MetadataEdit): Promise<string> =>
  invoke("update_metadata", { trackId, edit });

/** Scan `filed` tracks for acoustic duplicates, grouped with a recommended keeper. */
export const scanLibraryDuplicates = (): Promise<DupGroup[]> =>
  invoke("scan_library_duplicates");

/** Dashboard aggregate stats for the Bibliothèque. */
export const libraryStats = (): Promise<DashboardStats> => invoke("library_stats");

// ---- M7 Rekordbox XML export + playlist path repair ----

/** Parse+validate a chosen Rekordbox XML file and persist it as the linked file. Rejects
 * (nothing persisted) if the file can't be read or parsed. */
export const linkRekordboxXml = (path: string): Promise<RekordboxLinkStatus> =>
  invoke("link_rekordbox_xml", { path });

/** Current linked-XML status (re-read fresh from disk each call). */
export const rekordboxStatus = (): Promise<RekordboxLinkStatus> => invoke("rekordbox_status");

/** Merge every filed track missing from the linked XML and rewrite it. Rejects if no XML is
 * linked yet, or if the linked file is unreadable/corrupt. */
export const exportRekordboxXml = (): Promise<RekordboxLinkStatus> => invoke("export_rekordbox_xml");

// ---- M8 Tier 1 master.db path-repair candidates ----

/** Candidate master.db path repairs detected so far (excludes applied/dismissed). */
export const rekordboxMasterdbPendingRepairs = (): Promise<PendingMasterdbRepair[]> =>
  invoke("rekordbox_masterdb_pending_repairs");

/** Apply the given repair ids against the linked Rekordbox's master.db. Never automatic —
 * only call this after explicit user confirmation. A failure on one id doesn't stop the rest. */
export const rekordboxMasterdbApplyRepairs = (ids: number[]): Promise<ApplyRepairOutcome[]> =>
  invoke("rekordbox_masterdb_apply_repairs", { ids });

/** Mark a pending/ambiguous repair as dismissed — it stops appearing in pending_repairs. */
export const rekordboxMasterdbDismissRepair = (id: number): Promise<void> =>
  invoke("rekordbox_masterdb_dismiss_repair", { id });

/** Resolve an ambiguous repair by selecting the correct candidate track. */
export const rekordboxMasterdbResolveAmbiguous = (id: number, chosenTrackId: string): Promise<void> =>
  invoke("rekordbox_masterdb_resolve_ambiguous", { id, chosenTrackId });

export const rekordboxMasterdbPendingMetadataSyncs = (): Promise<PendingMetadataSync[]> =>
  invoke("rekordbox_masterdb_pending_metadata_syncs");

export const rekordboxMasterdbApplyMetadataSyncs = (ids: number[]): Promise<ApplyMetadataSyncOutcome[]> =>
  invoke("rekordbox_masterdb_apply_metadata_syncs", { ids });

export const rekordboxMasterdbDismissMetadataSync = (id: number): Promise<void> =>
  invoke("rekordbox_masterdb_dismiss_metadata_sync", { id });

export const rekordboxMasterdbResolveAmbiguousMetadataSync = (id: number, chosenTrackId: string): Promise<void> =>
  invoke("rekordbox_masterdb_resolve_ambiguous_metadata_sync", { id, chosenTrackId });

export const rekordboxMasterdbPendingArtworkSyncs = (): Promise<PendingArtworkSync[]> =>
  invoke("rekordbox_masterdb_pending_artwork_syncs");

export const rekordboxMasterdbApplyArtworkSyncs = (ids: number[]): Promise<ApplyArtworkSyncOutcome[]> =>
  invoke("rekordbox_masterdb_apply_artwork_syncs", { ids });

export const rekordboxMasterdbDismissArtworkSync = (id: number): Promise<void> =>
  invoke("rekordbox_masterdb_dismiss_artwork_sync", { id });

export const rekordboxMasterdbResolveAmbiguousArtworkSync = (id: number, chosenTrackId: string): Promise<void> =>
  invoke("rekordbox_masterdb_resolve_ambiguous_artwork_sync", { id, chosenTrackId });

// ---- M8 Tier 2 playlist duplicate-entry dedup ----

/** Scans the linked Rekordbox's master.db for playlists with the same track
 * added more than once. Read-only, called fresh on demand — nothing persists
 * between calls, unlike Tier 1's candidate repairs. */
export const rekordboxMasterdbScanPlaylistDuplicates = (): Promise<PlaylistDuplicateGroupDto[]> =>
  invoke("rekordbox_masterdb_scan_playlist_duplicates");

/** Removes every extra occurrence in group.remove, keeping group.keep untouched.
 * Pass back exactly the group object received from rekordboxMasterdbScanPlaylistDuplicates —
 * there is no separate id to reference. Never automatic — only call after explicit
 * user confirmation. */
export const rekordboxMasterdbDedupPlaylistGroup = (group: PlaylistDuplicateGroupDto): Promise<void> =>
  invoke("rekordbox_masterdb_dedup_playlist_group", { group });

// ---- M7 USB format utility (mirror of ipc_usb.rs) ----

export interface RemovableDrive {
  /** Physical disk path (`\\.\PHYSICALDRIVE2`, `/dev/disk4`) — never a drive letter. A new or
   * RAW key has no volume to name, and that is the main thing this screen formats. */
  id: string;
  label: string;
  /** Drive letter(s) if the disk has mounted volumes, `""` if it has none. Display only — an
   * empty string means "pas encore formatée", not "lookup failed". */
  mount: string;
  size_bytes: number;
  /** Octets libres sur les volumes montés, `0` si le disque n'en a aucun. Sert aussi de clé
   * d'invalidation au cache d'occupation côté Rust — ne pas le recalculer ici. */
  free_bytes: number;
  current_fs: string;
  /** Nom du volume actuel ("DJERMUSIQUE"), vide si le disque n'est pas formaté. Valeur par défaut
   * du champ de nom dans la modale. */
  volume_name: string;
  /** État de santé du volume, déjà formulé en français par le backend ("OK",
   * "Réparation complète nécessaire", "Avertissement (code N)"). Vide pour un disque RAW, qui n'a
   * pas de système de fichiers dont juger la santé. */
  health: string;
  /** `false` for an enumerated but empty card reader / drive bay. It still has a drive letter in
   * Explorer, so it must be listed and explained rather than hidden — but it cannot be formatted. */
  has_media: boolean;
  /** Opaque anti-race anchor — round-trip it to `formatDrive`, never parse it. */
  identity: string;
}

/** Matches `usb_format::TargetFs`'s `#[serde(rename_all = "snake_case")]`: `ExFat` -> "ex_fat". */
export type TargetFs = "fat32" | "ex_fat";

/** Drives Sift is confident are removable (conservative filter, backend-side). */
export const listRemovableDrives = (): Promise<RemovableDrive[]> =>
  invoke("list_removable_drives");

/** Étape courante du formatage privilégié, à interroger pendant que `formatDrive` est en vol.
 * Chaîne vide = rien en cours. Le travail se fait dans un processus élevé séparé, donc c'est le
 * seul moyen de savoir où il en est. */
export const formatStep = (): Promise<string> => invoke("format_step");

/** Démonte `driveId` pour qu'il puisse être débranché sans risque. Rejette avec `"EJECT_BUSY"`
 * quand le système refuse — rien n'a été démonté dans ce cas. */
export const ejectDrive = (driveId: string): Promise<void> => invoke("eject_drive", { driveId });

/** Une ligne du graphique d'occupation : un format, ce qu'il pèse, combien de fichiers.
 * Miroir de `volume_usage::ExtUsage`. */
export interface ExtUsage {
  /** `.wav`, `.mp3`, `PIONEER/` pour le bloc Rekordbox, `(sans extension)` sinon. */
  ext: string;
  bytes: number;
  file_count: number;
}

/** Miroir de `ipc_usage::UsageReport`. `free_bytes` vaut 0 pour la bibliothèque, qui n'est pas un
 * volume — ne pas y dessiner de segment « libre ». */
export interface UsageReport {
  total_bytes: number;
  free_bytes: number;
  file_count: number;
  buckets: ExtUsage[];
  /** Vrai quand rien n'a été parcouru. À afficher : sans ça un cache faux est indiscernable d'une
   * mesure fraîche, et personne ne sait quoi actualiser. */
  from_cache: boolean;
  /** Epoch en secondes du parcours qui a produit ces chiffres. */
  scanned_at: number;
}

/** Occupation d'un disque amovible, par format. Le backend parcourt le volume en métadonnées
 * seules et met le résultat en cache ; l'invalidation se fait sur l'espace libre, donc `false`
 * suffit dans le cas courant. `forceRescan` ne sert qu'au bouton « Actualiser ». */
export const driveUsage = (driveId: string, forceRescan = false): Promise<UsageReport> =>
  invoke("drive_usage", { driveId, forceRescan });

/** Occupation de la bibliothèque, par format. Aucune entrée/sortie disque : les tailles sont déjà
 * en base, c'est un agrégat. */
export const libraryUsage = (): Promise<UsageReport> => invoke("library_usage");

/** Format `driveId` to `fs`. `identity` must be the value last read for this drive — the backend
 * re-checks it against a fresh listing immediately before formatting and rejects with
 * "IDENTITY_MISMATCH"/"DRIVE_VANISHED" if the drive was swapped since the list was fetched. */
export const formatDrive = (
  driveId: string,
  identity: string,
  fs: TargetFs,
  label: string,
): Promise<void> => invoke("format_drive", { driveId, identity, fs, label });
