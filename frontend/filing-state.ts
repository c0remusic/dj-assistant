import { listen } from "@tauri-apps/api/event";
import type {
  Canonical,
  Target,
  QueueItem,
  FileTags,
  TrackFileOutcome,
} from "../shared/contracts";

/** Shared, mutable Revue state for the current filing session. Destination-selection state
 *  (library root, bin list, selected bin, "sur place" flag) moved to filing-bins.ts's own
 *  DestState (tech-debt audit F03 — god-file split, first tranche). */
export interface RevueState {
  track: QueueItem | null; // currently open track
  canonical: Canonical | null; // reconciled (then user-edited) metadata
  target: Target | null; // format override (null = backend rail default)
  // Analysed rail of the open track ("lossless" | "lossy" | "unknown"), set in openFilingInto. The
  // single source for the default format when target is null — used by BOTH the lit chip and the
  // Final-name preview (defaultTarget) so they never disagree on open.
  rail: string;
  // Read-only Discogs release facts for the open track. NOT part of Canonical (which drives the
  // filename/tags and is a Rust-mirrored contract) — kept here so the editor can show them. Loaded
  // from `releaseCache` on open, or set from `applied` on identify; null = unknown (no display).
  label: string | null;
  year: number | null;
  // Country/format of the applied release (e.g. "UK", "Vinyl, 12\", EP") — same session-cache-only
  // scope as label/year above, except there is no persisted backend column for these two (Rust
  // TrackRelease has none): they survive a close+reopen within this session (releaseCache) but not
  // an app restart, until/unless the metadata table grows matching columns (2026-07-06 annotation:
  // previously these were shown in the candidate list, then dropped the instant a candidate got
  // selected — kept here so the read-only release line below Genres keeps showing them afterwards).
  releaseCountry: string | null;
  releaseFormat: string | null;
  // Cover of the applied/persisted release — restoreCover() re-applies it to the hero on reopen of
  // an already-identified track (the analysis report doesn't carry the Discogs cover, so without the
  // stored path the cover would stay hidden until a re-identify).
  coverPath: string | null;
  // The would-write sub-genres for the open track (DB track_genres order), shown in .sift-genres and
  // compared (joined) against the file. Set on open from track_release, or from `applied.styles`.
  genres: string[];
  // The file's REAL tags, snapshotted ONCE on open (and re-read after an Apply/File). The marker
  // compares the displayed identity to THIS in-memory snapshot — never a per-keystroke disk read.
  // null until the open-time read resolves.
  fileTags: FileTags | null;
  // After a Detail-mode filing, the just-filed track's batch_id + bin label → drives the
  // persistent "Filed ↩" confirmation in #mid (targeted revert via the journal). Null = none up.
  filedConfirm: { batchId: string; bin: string } | null;
  // True once a Discogs identity is applied to the open track (fresh fetch OR persisted-identified
  // reopen). Gates the "rebuy on Beatport" link: searching a raw filename is useless — only a
  // confirmed artist+title is worth a store search.
  identified: boolean;
  // Nombre de morceaux RANGÉS dans cette session (filings, jamais un simple écarté). Pilote le fork
  // de l'empty-state : >0 → « Tout est trié » (→ Bibliothèque, il y a quelque chose à voir) ; 0 →
  // « Rien à revoir » (→ Accueil, rien de rangé, en ajouter). Remis à 0 seulement au rechargement.
  filedThisSession: number;
}

export const state: RevueState = {
  track: null,
  canonical: null,
  target: null,
  rail: "unknown",
  label: null,
  year: null,
  releaseCountry: null,
  releaseFormat: null,
  coverPath: null,
  genres: [],
  fileTags: null,
  filedConfirm: null,
  identified: false,
  filedThisSession: 0,
};

// ---- Background filing (P5 — PRD 2026-07-27, D3/D5) ----
//
// `file_track` now returns as soon as its plan is settled: the conversion itself keeps running on a
// backend thread and reports on `file:track:done`. Between those two moments the track is in a
// state the queue alone can't express — still `pending` backend-side (it only becomes `filed` when
// the encode commits), yet already OUT of the user's loop. The two maps below are that state.
//
// They live here, in the shared Revue state, rather than in filing-actions.ts, because BOTH the
// action (which starts a filing) and the queue panel (which must stop showing a track being
// converted, and must show one that failed) read them — and filing-actions.ts cannot import the
// queue panel without closing an import cycle (queue-panel → filing → filing-actions).

/** A filing whose acknowledgement is back but whose conversion is still running. */
export interface InFlightFiling {
  batchId: string;
  /** Destination path decided at plan time (where the file WILL land). */
  path: string;
  /** Destination label shown to the user (bin name, or "source folder"). */
  bin: string;
  /** Final filename, for messages that land after the banner is gone. */
  name: string;
}

const filingInFlight = new Map<number, InFlightFiling>();

// Tracks whose background conversion FAILED, with the reason. Module state on purpose: it survives
// every navigation inside the app (the queue rail is re-rendered from it on each paint), which is
// the whole point of D5 — by the time a late failure lands, the user is two tracks further and a
// toast alone would inform nobody. Cleared when that track is filed again (markFilingStarted).
const filingFailed = new Map<number, string>();

/** Record that `trackId`'s conversion just started in the background. Clears any previous failure
 *  marker for it: this IS the retry. */
export function markFilingStarted(trackId: number, info: InFlightFiling): void {
  filingFailed.delete(trackId);
  filingInFlight.set(trackId, info);
}

/** True while `trackId`'s conversion is still running — the queue must not offer it again. */
export function isFilingInFlight(trackId: number): boolean {
  return filingInFlight.has(trackId);
}

/** Why `trackId`'s last conversion failed, or null. Drives the persistent queue-row marker. */
export function filingFailure(trackId: number): string | null {
  return filingFailed.get(trackId) ?? null;
}

/** Notified once per settled background filing, after the maps above are updated. `started` is the
 *  entry that was in flight (null if this front never saw the start, e.g. after a reload). */
export type FilingOutcomeListener = (o: TrackFileOutcome, started: InFlightFiling | null) => void;

const outcomeListeners: FilingOutcomeListener[] = [];

/** Subscribe to background filing outcomes. Pure bookkeeping (no Tauri call) so it is safe to call
 *  at module scope, including in the browser mockup build where there is no backend. */
export function onFilingOutcome(cb: FilingOutcomeListener): void {
  outcomeListeners.push(cb);
}

let filingWatcher: Promise<unknown> | null = null;

/** Subscribe ONCE to the backend's `file:track:done`. Called from the filing action rather than at
 *  module load: this module is also imported by the mockup build, where `listen` has no backend to
 *  talk to. On a subscribe failure the handle is cleared so the next conversion retries.
 *
 *  AWAITABLE on purpose: `listen()` is itself an invoke, so the registration is not necessarily
 *  live when it returns. The caller must await this BEFORE calling `file_track` — those are two
 *  independent invokes with no ordering between them, and a conformant file is only tagged+moved
 *  (no encode), so the backend can emit `file:track:done` within milliseconds. A missed event is
 *  unrecoverable in-session: the track would stay in `filingInFlight` forever, hidden from the
 *  queue and stuck behind an eternal spinner. */
export async function ensureFilingWatcher(): Promise<void> {
  if (!filingWatcher) {
    filingWatcher = listen<TrackFileOutcome>("file:track:done", (e) =>
      settleFiling(e.payload),
    ).catch((e) => {
      console.error("file:track:done subscribe failed", e);
      filingWatcher = null;
    });
  }
  await filingWatcher;
}

function settleFiling(o: TrackFileOutcome): void {
  const started = filingInFlight.get(o.track_id) ?? null;
  filingInFlight.delete(o.track_id);
  if (o.error) filingFailed.set(o.track_id, o.error);
  for (const cb of outcomeListeners) {
    // One listener throwing must not swallow the others (the queue marker and the banner are
    // independent consumers of the same event) — reported, never silent.
    try {
      cb(o, started);
    } catch (e) {
      console.error("filing outcome listener failed", e);
    }
  }
}

// Bumped on every open; an in-flight open/action bails at its await points if a newer one started
// (prevents a slow analyze/reconcile/applyIdentity/applyTags/revert from clobbering the pane of a
// track opened since). `acting` guards against a double-click firing two encodes (one action at a
// time). Grouped into one object (not two module-level `let`s) — see file header comment above.
export const openState = { openSeq: 0, acting: false };
