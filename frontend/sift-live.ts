// Live data wiring — ACTIVE ONLY inside the Tauri app. In a plain browser the hooks
// below are never installed, so app.js keeps its mockup (Vercel demo unaffected).
import {
  removeSource,
  listQueue,
  fileBatch,
  fileCancel,
  onFileDone,
  onFileProgress,
  rejectBatch,
  onQueueChanged,
  onAnalysisChanged,
  analysisProgress,
  setSourceWatched,
  trashTrack,
  restoreTrack,
  requeueTrack,
  purgeTrash,
  openUrl,
  getSetting,
  setSetting,
  listLibrary,
  libraryFolders,
} from "./ipc";
import type {
  LibraryTrack,
  LibraryFacets,
  LibraryFilter,
} from "../shared/contracts";
import { openLibraryDetailInto } from "./library-detail";
import { emptyStateHtml, wireEmptyState } from "./empty-state";
import {
  openFilingInto,
  refreshBins,
  syncDetail,
  installUndoShortcut,
  installFilingKeys,
  renderBinsForBatch,
  refreshBinsForBatch,
  ensureDestPopoverAutoClose,
  clearBinPick,
  setBinPickInert,
  targetExt,
  TARGET_LABEL,
  toggleDestPopover,
  repositionDestPopoverIfOpen,
} from "./filing";
// Views/chrome extracted from this god-module (audit P-3) — kept stateless, wired here.
import { renderEcartes } from "./ecartes-view";
import { renderHomeSources, pickAndAddFolder } from "./home-sources";
import { installDragDrop, injectLeanStyle, injectTitlebar, installScrollAutohide } from "./chrome";
import { initTheme, setTheme } from "./theme";
import type { ThemeChoice } from "./theme";
import type { QueueItem, BatchResult, FileProgress, Target } from "../shared/contracts";
import { FILE_IN_PLACE } from "../shared/contracts";
import { requireEl } from "./dom";
import { renderJournal } from "./journal";
import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";

/** Human label for the batch destination (resolves the in-place sentinel to its prose). */
const IN_PLACE_LABEL = "Dossier source de chaque morceau";
import {
  setTask,
  clearTask,
  setCancelHandler,
  mountProgressZone,
  homeProgressZone,
} from "./progress-zone";
import {
  startBatchTracklist,
  updateBatchTracklist,
  finishBatchTracklist,
  clearBatchTracklist,
} from "./batch-tracklist";

// Latest live queue items, kept so a queue-row click can recover the full item (id +
// verdict) the filing pane needs.
let currentItems: QueueItem[] = [];

// Review mode: "detail" = one track at a time (filing pane), "batch" = triage many at once
// (board's Detail|Batch segmented control). `batchSel` holds the ticked track ids; it is
// pruned to the currently-ready set on every batch render so a filed/removed id can't linger.
let reviewMode: "detail" | "batch" = "detail";
const batchSel = new Set<number>();
// Auto-fill the ticks to "all ready" ONCE, on the first batch render that has ready items. Without
// this guard renderBatch re-filled whenever batchSel hit 0, which silently undid "Aucun (clear)".
let batchSelInit = false;
// Fakes ticked for DISCARD (never filed — Sift never ranges a fake lossless). Kept separate from
// batchSel (fileables → File) so the rail action button can be adaptive (File n / Discard n / both).
const batchFakeSel = new Set<number>();
// Batch "file in place" toggle (FILE_IN_PLACE). Kept apart from batchBin so the picked folder is
// remembered while in-place is on. Effective destination = batchInPlace ? FILE_IN_PLACE : batchBin.
let batchInPlace = false;
// Single encode target for the whole "Prêts · lossless" selection (maquette: one segmented format
// control for the batch, not one per source rail — a lossy-sourced file can still be asked for
// AIFF/WAV here, unlike the Détail rail which keeps the no-upscale guard). Fed to the filer as the
// same target for every submitted id.
let batchFormat: Target = "aiff_16_44";
// The ordered ids submitted to the currently-running batch — drives the per-track tracklist (the
// nth `file:progress.done` maps to batchTrackIds[n]). Set at submit, used at file:done.
let batchTrackIds: number[] = [];
// Destination bin chosen in the batch folder tree (forward-slash rel; "" = library root). Kept
// across renders so the choice doesn't reset while triaging.
let batchBin = "";

// Bibliothèque browser state: active filter, which facet column (folder/genre) is shown,
// and the last fetched track list (so a row-click can recover the track's path).
const bibState: { filter: LibraryFilter; facet: "folder" | "genre"; tracks: LibraryTrack[] } = {
  filter: {},
  facet: "folder",
  tracks: [],
};

// Verdict = meaning only, vert/ambre uniquement (voir brief refonte 2026-07) — jamais un hex en
// dur ici (l'ancien `#e2685e` rouge cassait cette règle) : lire les tokens CSS, pas une 3e teinte.
const VERDICT_DOT: Record<string, [string, string]> = {
  ok: ["var(--color-text-success)", "authentic"],
  fake: ["var(--color-text-warning)", "fake / over-encoded"],
  grey: ["var(--color-text-warning)", "grey zone"],
};
function verdictDot(v: string | null): string {
  if (v && VERDICT_DOT[v]) {
    const [c, title] = VERDICT_DOT[v];
    return `<span title="${title}" style="flex:none;width:9px;height:9px;border-radius:50%;background:${c}"></span>`;
  }
  // not analysed yet
  return `<span title="awaiting analysis" style="flex:none;width:9px;height:9px;border-radius:50%;border:1.5px solid var(--color-text-tertiary);box-sizing:border-box"></span>`;
}

const esc = (s: string) =>
  s.replace(/[&<>"']/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : c === '"' ? "&quot;" : "&#39;",
  );

/** Replaces the mockup queue list with real pending items (Revue screen). */
async function renderQueue(touchDetail = true) {
  const ql = document.getElementById("ql");
  if (!ql) return;
  let items: QueueItem[] = [];
  try {
    items = await listQueue();
  } catch (e) {
    console.error("listQueue failed", e);
    return;
  }
  currentItems = items;
  ensureReviewSeg();
  // Background-analysis progress moved to the global progress zone (bottom of #nav, persistent
  // across views) — see pushAnalyzeProgress, fed by the analysis:changed event below.

  const verdictWord = (v: string | null): [string, string] =>
    v === "fake"
      ? ["fake", "var(--color-text-warning)"]
      : v === "grey"
        ? ["à vérifier", "var(--color-text-warning)"]
        : v === "ok"
          ? ["lossless", "var(--color-text-success)"]
          : ["analyse…", "var(--color-text-tertiary)"];

  ql.innerHTML =
    (items
      .map((it) => {
        const [word, wordColor] = verdictWord(it.verdict);
        const title = esc(it.filename || it.path);
        const artist = it.artist ? esc(it.artist) : "";
        return (
          `<div class="qi" data-id="${it.id}" data-path="${esc(it.path)}" title="Listen and file" style="display:flex;align-items:center;gap:8px;cursor:pointer;padding:5px 7px">` +
          `<div style="flex:1;min-width:0;display:flex;flex-direction:column;gap:2px">` +
          `<div style="display:flex;align-items:center;gap:6px;min-width:0">` +
          verdictDot(it.verdict) +
          `<span style="flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;font-weight:500">${title}</span>` +
          (it.dup
            ? '<span title="Possible duplicate (same name)" style="flex:none;font-size:var(--text-sm);color:var(--color-text-secondary)">⧉</span>'
            : "") +
          `</div>` +
          (artist
            ? `<div style="padding-left:15px;font-size:var(--text-xs);color:var(--color-text-tertiary);overflow:hidden;text-overflow:ellipsis;white-space:nowrap">${artist}</div>`
            : "") +
          `</div>` +
          `<span style="flex:none;font-size:var(--text-xs);color:${wordColor}">${word}</span>` +
          `</div>`
        );
      })
      .join("") ||
      '<div style="font-size:var(--text-md);color:var(--color-text-tertiary);padding:6px 4px">File vide.</div>');

  // Live destination bins + neutral detail prompt (replace the mockup's hardcoded ones).
  const fldz = requireEl("#fldz", "renderQueue");
  void refreshBins(fldz);
  // Only sync the detail pane on structural changes (nav, queue add/remove/file). A background
  // ANALYSIS finishing must NOT re-open / switch the open track — that thrashes and aborts the
  // player's audio load (waveform shows from peaks, but no sound). See touchDetail=false caller.
  if (touchDetail) {
    if (reviewMode === "batch") {
      renderBatch();
    } else {
      const mid = requireEl("#mid", "renderQueue");
      if (mid) {
        // auto-load the current/first pending track into the main pane + highlight its row
        const curId = syncDetail(mid, items);
        document.querySelectorAll(".qi.cur").forEach((n) => n.classList.remove("cur"));
        if (curId != null) {
          document.querySelector(`.qi[data-id="${curId}"]`)?.classList.add("cur");
        }
      }
    }
  }
}

// Global progress zone — feed the "analyze" row from the EXISTING analysis poll/events (no engine
// rewrite). `analysis_progress` returns (done, total) over PENDING tracks; a track stays pending
// after it's analysed (until filed), so done==total is the RESTING state, not "busy". So we show
// the row only while done<total (actively analysing), then flash a brief 100% "done" before hiding.
let analyzeWasRunning = false;
let analyzeClearTimer: ReturnType<typeof setTimeout> | undefined;
async function pushAnalyzeProgress() {
  try {
    const p = await analysisProgress();
    if (p.total > 0 && p.done < p.total) {
      clearTimeout(analyzeClearTimer);
      analyzeWasRunning = true;
      setTask("analyze", { done: p.done, total: p.total, state: "running" });
    } else if (analyzeWasRunning) {
      // Reached done==total (or the queue drained): flash 100% then auto-hide the row.
      analyzeWasRunning = false;
      setTask("analyze", { done: p.total, total: p.total, state: "done" });
      clearTimeout(analyzeClearTimer);
      analyzeClearTimer = setTimeout(() => clearTask("analyze"), 1200);
    } else {
      clearTask("analyze");
    }
  } catch (e) {
    console.error("analysisProgress failed", e);
  }
}

// Global progress zone — feed the "file" row from the per-file filing events (sous-étape 2). Mirror
// of pushAnalyzeProgress, but here done/total arrive straight from the event (no poll). On
// done==total the row flashes 100% "done" then auto-hides after 1.2s, exactly like the analyze row.
let fileClearTimer: ReturnType<typeof setTimeout> | undefined;
let fileStopping = false;
// True from the moment a batch File/Discard launches until file:done (or discard completes) — drives
// the rail button between its adaptive state and "Stop".
let batchRunning = false;
let lastFileProgress: FileProgress | null = null;
function pushFileProgress(p: FileProgress) {
  lastFileProgress = p;
  if (p.total <= 0) {
    clearTask("file");
    return;
  }
  if (p.done < p.total) {
    clearTimeout(fileClearTimer);
    setTask("file", { done: p.done, total: p.total, state: "running", stopping: fileStopping });
  } else {
    setTask("file", { done: p.total, total: p.total, state: "done" });
    clearTimeout(fileClearTimer);
    fileClearTimer = setTimeout(() => {
      clearTask("file");
      clearBatchTracklist();
      refreshBatchTracksPreview(); // in-place: bring the source-folder preview back after the run
    }, 1200);
  }
  updateBatchTracklist(p.done); // first `done` rows = done, the (done)-th = running, rest = waiting
}

/** Stop button on the global zone's Filing row → request a stop-net cancel (sous-étape 3). The
 * in-flight file finishes and no new one starts; nothing is rolled back. The row shows "Stopping…"
 * until `file:done` arrives (handled by onFileBatchDone). The first click already takes effect
 * (flag set, button removed), but the only feedback used to be the small "Stopping…" at the bottom
 * of the nav rail — far from where the user clicked. While a conversion encodes, the counter is
 * frozen, so the cancel looks ignored and the user re-clicks into the void. We also drop an
 * immediate note at #filfoot (where they clicked File) so the click visibly registers right there. */
function onFileStop() {
  if (fileStopping) return;
  fileStopping = true;
  if (lastFileProgress) {
    setTask("file", {
      done: lastFileProgress.done,
      total: lastFileProgress.total,
      state: "running",
      stopping: true,
    });
  }
  // Local, immediate feedback next to the action — explains the unavoidable wait on the in-flight
  // file (its encode cannot be cut). Replaced by the run summary when `file:done` arrives.
  fileNote(
    '<i class="ti ti-loader sift-spin" style="font-size:var(--text-md);vertical-align:-1px"></i> Stop requested — finishing the current file…',
  );
  void fileCancel();
}

// No Rekordbox/USB backend exists yet (rbox/rekordcrate are still candidates, not integrated —
// see docs/ressources-externes.md). This drives a REAL "export" row in the progress zone (not a
// placeholder), but the work itself is simulated: a fake per-track tick, same ~450ms pace and
// done→auto-hide convention as the other rows (pushAnalyzeProgress/pushFileProgress above).
let exportTimer: ReturnType<typeof setInterval> | undefined;
let exportClearTimer: ReturnType<typeof setTimeout> | undefined;

/** Start (or ignore if one is already running) a simulated export of `total` filed tracks to
 * `target`. Ticks "export" done/total once per track, then flashes done and auto-hides — mirrors
 * pushFileProgress's done-state handling exactly. */
function startExportSim(target: "rekordbox" | "usb", total: number): void {
  if (exportTimer) return; // one export run at a time, like every other TaskKind
  if (total <= 0) return;
  clearTimeout(exportClearTimer);
  let done = 0;
  setTask("export", { done, total, state: "running" });
  exportTimer = setInterval(() => {
    done += 1;
    if (done >= total) {
      clearInterval(exportTimer);
      exportTimer = undefined;
      setTask("export", { done: total, total, state: "done" });
      exportClearTimer = setTimeout(() => clearTask("export"), 1200);
    } else {
      setTask("export", { done, total, state: "running" });
    }
  }, 450);
}

/** A transient bottom-right toast (mirrors filing.ts/library-detail.ts, no undo affordance). */
function toast(message: string): void {
  document.getElementById("sift-toast")?.remove();
  const el = document.createElement("div");
  el.id = "sift-toast";
  el.style.cssText =
    "position:fixed;right:18px;bottom:18px;z-index:9998;display:flex;align-items:center;gap:12px;background:var(--color-background-secondary);border:0.5px solid var(--color-border-secondary);border-radius:var(--border-radius-md);padding:9px 13px;font-size:var(--text-md);color:var(--color-text-primary);box-shadow:0 8px 28px rgba(0,0,0,.4)";
  el.textContent = message;
  document.body.appendChild(el);
  setTimeout(() => el.remove(), 4000);
}

/** Nav "Export" click (Rekordbox/Clé USB, index.html's `.nv-export` items) — the maquette's
 * `exportTo` action: guards on an empty library and on a run already in flight, else fetches the
 * real filed-track count and starts the simulated export. Doesn't switch screens (these are
 * one-click actions, not real screens yet — see the capture-phase click listener below, which
 * pre-empts app.js's mockup view switch for data-view="rkb"/"cle"). */
async function runNavExport(target: "rekordbox" | "usb"): Promise<void> {
  if (exportTimer) return; // one export run at a time
  let total = 0;
  try {
    total = (await listLibrary()).length;
  } catch (e) {
    console.error("listLibrary failed (nav export)", e);
    return;
  }
  if (total === 0) {
    toast("Bibliothèque vide — rien à exporter");
    return;
  }
  startExportSim(target, total);
}

/** Detail|Batch segmented control (board `topseg`), injected once at the top of the queue
 * column. Owned here (not app.js) so it works inside Tauri where the live wiring renders the
 * Revue. Reflects `reviewMode`; clicks are handled in the #pa delegate. */
function ensureReviewSeg() {
  const qcol = requireEl("#qcol", "ensureReviewSeg");
  let seg = document.getElementById("sift-revseg");
  if (!seg) {
    seg = document.createElement("div");
    seg.id = "sift-revseg";
    seg.style.cssText =
      "display:flex;gap:2px;padding:2px;margin-bottom:10px;background:var(--color-background-secondary);border-radius:var(--border-radius-md)";
    qcol.insertBefore(seg, qcol.firstChild);
  }
  const tab = (m: "detail" | "batch", label: string, icon: string) => {
    const on = reviewMode === m;
    return `<button data-sift="reviewmode" data-m="${m}" style="flex:1;display:inline-flex;align-items:center;justify-content:center;gap:5px;padding:5px 0;border:none;border-radius:6px;font-size:var(--text-sm);font-weight:${
      on ? 600 : 400
    };cursor:pointer;background:${
      on ? "var(--color-background-primary)" : "transparent"
    };color:var(--color-text-${on ? "primary" : "tertiary"})"><i class="ti ${icon}" style="font-size:var(--text-base)"></i>${label}</button>`;
  };
  seg.innerHTML = tab("detail", "Détail", "ti-layout-list") + tab("batch", "Lot", "ti-table");
}

/** Batch triage view (maquette "Mode Lot"): 3 flat groups by verdict — Prêts · lossless
 * (selectable → File), À vérifier · fake (selectable → Écarter, never filed — Sift ne range
 * jamais un fake lossless), En analyse (read-only, encore en cours d'analyse). One shared
 * format selector for the whole file-able selection (renderBatchRail) — no per-source-rail
 * split; a lossy-sourced file CAN be asked for AIFF/WAV here (see docs/refonte-ui-plan.md,
 * décision "maquette prime" du 2026-07-01 — seule la règle fakes-jamais-filés est gardée).
 * Every control is bound to a real command (`fileBatch` / `rejectBatch`); nothing is mocked. */
function renderBatch() {
  const mid = requireEl("#mid", "renderBatch");
  const ready = currentItems.filter((it) => it.verdict === "ok");
  const fakes = currentItems.filter((it) => it.verdict === "fake");
  const pending = currentItems.filter((it) => it.verdict !== "ok" && it.verdict !== "fake");
  // Prune ticks to the live ready set; default to all-ready selected ONCE (first render with ready
  // items). Guarded by batchSelInit so an explicit "Aucun (clear)" (batchSel→0) is NOT re-filled.
  const readyIds = new Set(ready.map((it) => it.id));
  for (const id of [...batchSel]) if (!readyIds.has(id)) batchSel.delete(id);
  if (!batchSelInit && ready.length) {
    batchSelInit = true;
    for (const it of ready) batchSel.add(it.id);
  }

  // BEFORE (file name) + AFTER (Discogs artist — title once identified). When not yet identified
  // only the filename shows; identifying it (Identify all) reveals the clean name above the file.
  const nameCell = (it: QueueItem, dim = false) => {
    const after = it.artist && it.title ? `${it.artist} — ${it.title}` : null;
    const before = it.filename || it.path;
    const topColor = after
      ? "var(--color-text-primary)"
      : dim
        ? "var(--color-text-secondary)"
        : "var(--color-text-primary)";
    return (
      `<div style="flex:1;min-width:0">` +
      `<div style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-size:var(--text-md);color:${topColor}">${esc(
        after ?? before,
      )}</div>` +
      (after
        ? `<div style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-size:var(--text-xs);color:var(--color-text-tertiary);font-family:var(--font-mono);margin-top:1px"><span style="opacity:.55">was</span> ${esc(
            before,
          )}</div>`
        : "") +
      `</div>`
    );
  };
  const readyRow = (it: QueueItem) => {
    const on = batchSel.has(it.id);
    return (
      `<div class="bx-row" data-sift="batchpick" data-id="${it.id}" style="display:flex;align-items:center;gap:9px;padding:7px 9px;border-radius:var(--border-radius-md);cursor:pointer;${
        on ? "background:var(--overlay-hover)" : ""
      }">` +
      `<span class="bx-ck" style="flex:none;width:15px;height:15px;border-radius:4px;display:inline-flex;align-items:center;justify-content:center;border:1.5px solid ${
        on ? "var(--color-text-success)" : "var(--color-border-secondary)"
      };background:${on ? "var(--color-text-success)" : "transparent"}">${
        on ? '<i class="ti ti-check" style="font-size:var(--text-xs);color:#1a1a18"></i>' : ""
      }</span>` +
      verdictDot(it.verdict) +
      nameCell(it) +
      (it.dup
        ? '<span style="flex:none;font-size:var(--text-2xs);font-weight:600;letter-spacing:.03em;padding:2px 7px;border-radius:999px;background:var(--color-background-warning);color:var(--color-text-warning)">DUPLICATE</span>'
        : "") +
      `</div>`
    );
  };
  // Read-only "En analyse" rows — no checkbox, matches the maquette's inert third group.
  const pendingRow = (it: QueueItem) => {
    const label = it.verdict === "grey" ? "CHECK" : "analyse…";
    return (
      `<div style="display:flex;align-items:center;gap:9px;padding:7px 9px;opacity:.6">` +
      verdictDot(it.verdict) +
      nameCell(it, true) +
      (it.dup
        ? '<span style="flex:none;font-size:var(--text-2xs);font-weight:600;padding:2px 7px;border-radius:999px;background:var(--color-background-warning);color:var(--color-text-warning)">DUP</span>'
        : "") +
      `<span style="flex:none;font-size:var(--text-2xs);color:var(--color-text-tertiary)">${label}</span>` +
      `<button data-sift="batchopen" data-id="${it.id}" style="flex:none;font-size:var(--text-xs);padding:2px 8px;color:var(--color-text-info)">Ouvrir en Détail</button>` +
      `</div>`
    );
  };

  // Fakes are selectable to DISCARD (their own tick set), never to file.
  const fakeRow = (it: QueueItem) => {
    const on = batchFakeSel.has(it.id);
    return (
      `<div class="bx-row" data-sift="batchpickfake" data-id="${it.id}" style="display:flex;align-items:center;gap:9px;padding:7px 9px;border-radius:var(--border-radius-md);cursor:pointer;${
        on ? "background:var(--overlay-hover)" : ""
      }">` +
      `<span class="bx-ck" style="flex:none;width:15px;height:15px;border-radius:4px;display:inline-flex;align-items:center;justify-content:center;border:1.5px solid ${
        on ? "var(--color-text-danger)" : "var(--color-border-secondary)"
      };background:${on ? "var(--color-background-danger)" : "transparent"}">${
        on ? '<i class="ti ti-check" style="font-size:var(--text-xs);color:var(--color-text-danger)"></i>' : ""
      }</span>` +
      verdictDot(it.verdict) +
      nameCell(it, true) +
      '<span style="flex:none;font-size:var(--text-2xs);font-weight:600;letter-spacing:.03em;padding:2px 7px;border-radius:999px;background:var(--color-background-danger);color:var(--color-text-danger)">FAKE</span>' +
      `<button data-sift="batchopen" data-id="${it.id}" style="flex:none;font-size:var(--text-xs);padding:2px 8px;color:var(--color-text-info)">Ouvrir en Détail</button>` +
      `</div>`
    );
  };

  // A group header row: tri-state checkbox + dot + label + count, mirroring the maquette's
  // `groupDefs` (label already reads "Prêts · lossless" / "À vérifier · fake" / "En analyse" —
  // the count is appended separately so it stays JetBrains Mono like every other counter).
  const groupHead = (
    kind: "file" | "fake" | "readonly",
    dotColor: string,
    label: string,
    ids: number[],
  ) => {
    const sel = kind === "file" ? batchSel : kind === "fake" ? batchFakeSel : null;
    const n = sel ? ids.filter((id) => sel.has(id)).length : 0;
    const st = !sel || ids.length === 0 ? "empty" : n === 0 ? "empty" : n === ids.length ? "full" : "partial";
    const box = !sel
      ? ""
      : st === "full"
        ? '<span class="sift-bgrp-box on"><i class="ti ti-check"></i></span>'
        : st === "partial"
          ? '<span class="sift-bgrp-box partial"><i class="ti ti-minus"></i></span>'
          : '<span class="sift-bgrp-box"></span>';
    const clickable = sel ? ` data-sift="batchgroup" data-kind="${kind}" style="cursor:pointer"` : "";
    return (
      `<div class="sift-bgrp-head"${clickable}>` +
      box +
      `<span style="width:6px;height:6px;border-radius:999px;background:${dotColor};flex:none"></span>` +
      `<span class="col-h" style="margin:0">${esc(label)} · ${ids.length}</span>` +
      `</div>`
    );
  };

  // No center action bar: the destination + adaptive File/Discard/Stop button now live solely in the
  // right rail (renderBatchRail), mirroring the Detail screen's CTA-in-the-rail grammar.
  mid.innerHTML =
    `<div style="display:flex;flex-direction:column;height:100%;min-height:0">` +
    `<div style="flex:1;min-height:0;overflow-y:auto;padding-right:2px">` +
    (ready.length
      ? `<div style="margin:2px 0 16px">` +
        groupHead("file", "var(--color-text-success)", "Prêts · lossless", ready.map((it) => it.id)) +
        ready.map(readyRow).join("") +
        `</div>`
      : '<div class="col-h" style="margin:0 0 6px">Prêts · lossless · 0</div><div style="font-size:var(--text-md);color:var(--color-text-tertiary);padding:4px 9px 14px">Rien à filer pour l’instant.</div>') +
    (fakes.length
      ? `<div style="margin:2px 0 16px">` +
        groupHead("fake", "var(--color-text-warning)", "À vérifier · fake", fakes.map((it) => it.id)) +
        fakes.map(fakeRow).join("") +
        `</div>`
      : "") +
    (pending.length
      ? `<div style="margin:2px 0 16px">` +
        groupHead("readonly", "var(--color-text-tertiary)", "En analyse", pending.map((it) => it.id)) +
        pending.map(pendingRow).join("") +
        `</div>`
      : "") +
    `</div></div>`;

  renderBatchRail(fakes.length + pending.length);
}


/** The destination actually passed to the filer (FILE_IN_PLACE sentinel, or the picked folder rel). */
function batchDest(): string {
  return batchInPlace ? FILE_IN_PLACE : batchBin;
}
/** Human label for the batch destination — shown in the rail récap + name preview. */
function batchDestLabel(): string {
  return batchInPlace ? IN_PLACE_LABEL : batchBin || "Racine de bibliothèque";
}
/** A folder click in the #fldz tree (batch pick mode) -> set batchBin, drop in-place, re-render. */
function onBatchBinPick(rel: string): void {
  batchBin = rel;
  batchInPlace = false; // choosing a folder turns off "file in place"
  const fldz = document.getElementById("fldz");
  if (fldz) renderBinsForBatch(fldz, batchBin, onBatchBinPick, batchInPlace);
  renderBatchRail(currentItems.filter((it) => it.verdict !== "ok").length);
}
/** Ensure the batch destination UI around #fldz: the tree is in batch pick mode. The "file in
 *  place" checkbox itself now renders as part of renderBins's own output (filing.ts) — same
 *  markup/attribute for both modes — so there's nothing left to create here, only the inert
 *  (greyed) state to keep in sync on every rail rebuild. */
function ensureBatchDestUI(): void {
  const fldz = document.getElementById("fldz");
  if (!fldz) return;
  // In-place GREYS the tree (visible but inert) — never hides it; the tree only picks a real folder.
  // ensureBatchDestUI runs on EVERY renderBatchRail (incl. run start and the post-run refresh), so
  // syncing binPick.inert here makes it the single source of truth: a later renderBins (queue refresh
  // during/after a run) re-asserts the SAME state via its own .sift-fldz-tree opacity logic.
  setBinPickInert(batchInPlace);
  fldz.style.display = ""; // belt-and-suspenders: a greyed tree must stay laid out, not collapse
  const treeWrap = fldz.querySelector<HTMLElement>(".sift-fldz-tree");
  if (treeWrap) {
    treeWrap.style.opacity = batchInPlace ? ".4" : "1";
    treeWrap.style.pointerEvents = batchInPlace ? "none" : "auto";
  }
}

/** The single rail action button. Adaptive before a run (Filer / Discarder / both / disabled),
 *  "Stop" during one. `running` swaps to the Stop affordance (wired to onFileStop). */
function actionButtonHtml(running: boolean): string {
  if (running) {
    return '<button data-sift="batchstop" class="sift-baction" style="background:var(--color-background-danger);color:var(--color-text-danger)"><i class="ti ti-player-stop" style="font-size:var(--text-md);vertical-align:-2px"></i> Stop</button>';
  }
  const fileN = batchSel.size;
  const fakeN = batchFakeSel.size;
  if (fileN === 0 && fakeN === 0)
    return '<button class="sift-baction" disabled style="background:var(--color-background-info);color:var(--color-text-info);opacity:.5;pointer-events:none">Filer (0)</button>';
  if (fakeN === 0)
    return `<button data-sift="batchaction" class="sift-baction" style="background:var(--color-background-info);color:var(--color-text-info)"><i class="ti ti-corner-down-left" style="font-size:var(--text-md);vertical-align:-2px"></i> Filer (${fileN})</button>`;
  if (fileN === 0)
    return `<button data-sift="batchaction" class="sift-baction" style="background:var(--color-background-danger);color:var(--color-text-danger)"><i class="ti ti-trash" style="font-size:var(--text-md);vertical-align:-2px"></i> Écarter (${fakeN})</button>`;
  return `<button data-sift="batchaction" class="sift-baction" style="background:var(--color-background-info);color:var(--color-text-info)">Filer (${fileN}) · Écarter (${fakeN})</button>`;
}

/** Right-rail summary for batch mode (board's SELECTION / DESTINATION / WILL ENCODE / EXCLUDED).
 * Replaces the filing footer + hides the folder tree while batching. */
function renderBatchRail(reviewN: number) {
  const foot = requireEl("#filfoot", "renderBatchRail");
  requireEl("#fldz", "renderBatchRail"); // fail-fast: asserts the popover host exists
  ensureBatchDestUI();
  // Preserve the LIVE run's progress list across this wholesale rebuild (renderBatch rebuilds the rail
  // on every selection change). Not while idle — the choice-time preview is rebuilt fresh below.
  const keepTracks = batchRunning ? foot.querySelector("#sift-batch-tracks") : null;
  const keepNote = foot.querySelector("[data-file-note]");
  // The progress zone may live in THIS rail from a prior render; park it back in its nav home before the
  // innerHTML wipe (which would destroy the node + its live rowCache), then re-mount into the fresh slot.
  if (foot.querySelector("#sift-progress-zone")) homeProgressZone();
  // Destination button in BOTH modes: a real folder (tree mode) or the in-place RULE label
  // ("Dossier source de chaque morceau") — batchDestLabel() resolves which. In-place states the rule
  // once here instead of listing each track's folder. Clickable — opens the #fldz popover (batch's
  // own rail doesn't go through filing.ts's renderFoot, so it wires the same toggle itself).
  const destBlock = `<button data-fil="destbtn" class="sift-dest-btn"><span class="sift-dest-btn-label">Destination</span><span class="sift-fil-bin">${esc(
    batchDestLabel(),
  )}</span><i class="ti ti-chevron-down sift-dest-btn-caret"></i></button>`;
  // "Excluded" is folded into Selection as a discreet (tertiary) suffix — no separate block.
  const jeter = batchFakeSel.size ? ` · ${batchFakeSel.size} à jeter` : "";
  const exclus = reviewN
    ? ` · <span style="color:var(--color-text-tertiary)">${reviewN} exclus (en review)</span>`
    : "";
  // Single global format selector (maquette `formats`) — applies to the whole file-able selection,
  // no per-source-rail split (décision "maquette prime" du 2026-07-01, docs/refonte-ui-plan.md).
  const formatBlock =
    `<div class="sift-rail-fmt-group"><span class="col-h">Format</span><div style="display:flex;background:var(--color-track);border-radius:8px;padding:2px;gap:2px">` +
    (["mp3_320", "aiff_16_44", "wav_16_44"] as Target[])
      .map(
        (t) =>
          `<span data-sift="batchformat" data-t="${t}" style="flex:1;text-align:center;font-family:var(--font-mono);font-size:var(--text-sm);padding:6px 0;border-radius:6px;cursor:pointer;background:${
            batchFormat === t ? "var(--color-surface-raised)" : "transparent"
          };color:var(--color-text-${batchFormat === t ? "primary" : "tertiary"})">${TARGET_LABEL[t]}</span>`,
      )
      .join("") +
    `</div></div>`;
  // Rail order (one row, matching the Detail rail restructure): Destination → Format → spacer →
  // Selection count → progress/tracks (each flex-basis:100% so they wrap to their own line, empty/
  // invisible while idle) → action. "Final name" motif dropped — redundant with Selection + the
  // per-track list once a run starts (see batchNameMotifHtml removal, Task 3).
  foot.innerHTML =
    destBlock +
    formatBlock +
    `<div class="sift-rail-spacer"></div>` +
    `<span style="font-size:var(--text-sm);color:var(--color-text-secondary);white-space:nowrap">${
      batchSel.size
    } à filer${jeter}${exclus}</span>` +
    `<div id="sift-batch-progress" style="flex-basis:100%"></div>` +
    `<div id="sift-batch-tracks" style="flex-basis:100%"></div>` +
    `<div class="sift-baction-slot">${actionButtonHtml(batchRunning)}</div>`;
  if (keepNote) foot.insertAdjacentElement("afterbegin", keepNote);
  if (keepTracks) foot.querySelector("#sift-batch-tracks")!.replaceWith(keepTracks);
  else refreshBatchTracksPreview(); // idle → keep the per-track list empty (it is a run-only artifact)
  foot.querySelector('[data-fil="destbtn"]')?.addEventListener("click", (e) => {
    e.stopPropagation();
    toggleDestPopover();
  });
  ensureDestPopoverAutoClose();
  // Move the single progress zone into the rail (batch). setTask/clearTask keep driving the same node,
  // so Filing X/N + Analysing render here with no duplicated logic. Detail restores it via setReviewMode.
  mountProgressZone(requireEl("#sift-batch-progress", "renderBatchRail progress slot"));
  repositionDestPopoverIfOpen(); // the destbtn above was just rebuilt — keep an open popover glued to it
}

/** Switch between detail and batch review. On entering batch the #fldz tree becomes the destination
 * explorer (batch pick mode); on leaving we restore the per-track filing pane. */
function setReviewMode(m: "detail" | "batch") {
  reviewMode = m;
  ensureReviewSeg();
  const fldz = requireEl("#fldz", "setReviewMode");
  // #fldz is now the destination popover (hidden by default, toggled by the rail's Destination
  // button in either mode — see renderFoot/renderBatchRail) — no static column visibility to manage.
  if (m === "batch") {
    renderBatch();
    // Drive the #fldz tree in batch pick mode (loads bins, clicks set batchBin via onBatchBinPick).
    void refreshBinsForBatch(fldz, batchBin, onBatchBinPick, batchInPlace);
  } else {
    // Leave batch pick mode: tree reverts to detail's state.binRel. No manual opacity/checkbox
    // cleanup needed — renderBins (filing.ts) always re-derives .sift-fldz-tree's opacity from
    // the current binPick (null in detail) and renders the one shared in-place checkbox itself.
    clearBinPick();
    // Return the progress zone to its left-sidebar home (it was relocated into the batch rail).
    homeProgressZone();
    void renderQueue(true);
  }
}

/** Launch background filing of every ticked (green) track into the chosen bin, then return — the
 * work runs off the main thread, so the UI stays responsive and analysis can keep running. A
 * spinner note is shown; the per-run summary AND the view refresh arrive later via the `file:done`
 * event (see `onFileBatchDone`). Filed tracks leave the queue, so the refresh prunes them from the
 * ticked set automatically. */
async function runBatchFile() {
  const ids = [...batchSel];
  if (ids.length === 0) return;
  fileStopping = false;
  lastFileProgress = null;
  // Flip the rail button to Stop and (re)build the rail so the progress slot exists before mounting.
  batchRunning = true;
  renderBatchRail(currentItems.filter((it) => it.verdict !== "ok").length);
  // Mount the per-track list (ordered like the submitted ids) before launching — the first row
  // shows "running" immediately; file:progress/file:done drive the rest. No backend event needed.
  batchTrackIds = ids;
  startBatchTracklist(ensureBatchTracklistHost(), ids.map(batchTrackItem));
  fileNote(
    '<i class="ti ti-loader sift-spin" style="font-size:var(--text-md);vertical-align:-1px"></i> Rangement en arrière-plan…',
  );
  // Single format applied to every submitted id (maquette's one segmented control for the batch).
  const targets: Record<number, Target> = {};
  for (const id of ids) targets[id] = batchFormat;
  try {
    // Resolves as soon as the background task STARTS; the summary comes via file:done.
    await fileBatch(ids, batchDest(), targets);
  } catch (err) {
    // Launch-time rejections only (NoLibraryRoot, or the task couldn't start).
    const code = String(err);
    fileNote(
      code.includes("NoLibraryRoot")
        ? "Aucune racine de bibliothèque configurée — à définir dans Réglages."
        : `Échec du lancement du rangement : ${esc(code)}`,
      "var(--color-text-danger)",
    );
    console.error("file_batch launch failed", err);
  }
}

/** Display name for a batch row: the queue item's artist — title, else its filename/path. */
function batchTrackName(id: number): string {
  const it = currentItems.find((q) => q.id === id);
  if (!it) return `#${id}`;
  return [it.artist, it.title].filter(Boolean).join(" — ") || it.filename || it.path;
}

/** A per-track list item (id + display name). In-place mode no longer attaches a source-folder
 *  suffix: the récap states the RULE once ("Dossier source de chaque morceau"), so the per-track
 *  list stays identical to normal mode (name + status pill, no per-file path inventory). */
function batchTrackItem(id: number): { id: number; name: string } {
  return { id, name: batchTrackName(id) };
}

/** At choice time the per-track list is NOT shown in either mode: dumping the (up to ~1752) selected
 *  filenames row-by-row only duplicates the "N à filer" count and the "Final name" motif, drowning the
 *  récap. The list is a RUN artifact — startBatchTracklist mounts it live when filing begins. So here we
 *  just keep #sift-batch-tracks empty. No-op during a run (the live list owns the container). */
function refreshBatchTracksPreview(): void {
  if (reviewMode !== "batch" || batchRunning) return;
  const host = document.getElementById("sift-batch-tracks");
  if (!host) return;
  host.innerHTML = "";
}

/** Stable container for the per-track list, mounted in the right rail (#filfoot) under the batch
 *  récap and above the action button, so all batch progress lives next to the controls that drive it. */
function ensureBatchTracklistHost(): HTMLElement {
  let el = document.getElementById("sift-batch-tracks");
  if (!el) {
    // The rail slot (renderBatchRail's #sift-batch-tracks div) isn't mounted yet — create a detached
    // node and append it to the rail; renderBatchRail preserves it (keepTracks) on its next rebuild.
    el = document.createElement("div");
    el.id = "sift-batch-tracks";
    document.getElementById("filfoot")?.appendChild(el);
  }
  return el;
}

/** Insert/replace a transient note at the top of the batch rail (#filfoot), if it is on screen. */
function fileNote(html: string, color = "var(--color-text-secondary)") {
  const foot = document.getElementById("filfoot");
  if (!foot) return;
  foot.querySelector("[data-file-note]")?.remove();
  foot.insertAdjacentHTML(
    "afterbegin",
    `<div data-file-note style="font-size:var(--text-sm);color:${color};margin-bottom:10px">${html}</div>`,
  );
}

/** End-of-(background-)filing handler, fired by the `file:done` event. Refreshes the view (as the
 * end-of-batch queue:changed used to) then shows the run summary — but only if the batch rail is
 * still on screen, since the user may have navigated away while the batch ran. */
async function onFileBatchDone(res: BatchResult) {
  fileStopping = false;
  batchRunning = false; // the later refresh() repaints the rail button back to its adaptive state
  // Final per-track reconcile: filed ids = done, needs_validation ids = failed. A cancelled run only
  // processed the first `lastFileProgress.done` ids; the rest never started (left at waiting).
  const processed = res.cancelled ? batchTrackIds.slice(0, lastFileProgress?.done ?? 0) : batchTrackIds;
  const failed = new Set(res.needs_validation);
  finishBatchTracklist(processed.filter((id) => !failed.has(id)), res.needs_validation);
  if (res.cancelled) {
    // Stop-net end: no 100% done-flash came from progress (done<total). Flash the partial then hide.
    clearTimeout(fileClearTimer);
    const lp = lastFileProgress;
    if (lp) {
      setTask("file", { done: lp.done, total: lp.total, state: "done" });
      fileClearTimer = setTimeout(() => {
        clearTask("file");
        clearBatchTracklist();
        refreshBatchTracksPreview();
      }, 1200);
    } else {
      clearTask("file");
      clearBatchTracklist();
      refreshBatchTracksPreview();
    }
  }
  const base = res.needs_validation.length
    ? `${res.filed} filed · ${res.needs_validation.length} need validation`
    : `${res.filed} filed`;
  // Refresh the view, then post the run summary at #filfoot — after refresh so it survives
  // renderBatch's wholesale rail rebuild (renderBatchRail sets #filfoot.innerHTML). refresh() no
  // longer throws on an unmounted view (each renderer no-ops when its root is absent), so the
  // earlier try/finally guard around it is no longer needed.
  await refresh();
  fileNote(
    `<i class="ti ti-check" style="font-size:var(--text-md);vertical-align:-1px"></i> ${
      res.cancelled ? `Filing cancelled · ${base}` : base
    }`,
    "var(--color-text-success)",
  );
}

/** Send every ticked track to Écartés for re-sourcing (backend emits queue:changed → redraw). */
async function runBatchDiscard() {
  const ids = [...batchFakeSel];
  if (ids.length === 0) return;
  batchRunning = true;
  renderBatchRail(currentItems.filter((it) => it.verdict !== "ok").length);
  try {
    await rejectBatch(ids);
    batchFakeSel.clear();
  } catch (err) {
    console.error("reject_batch failed", err);
  } finally {
    batchRunning = false;
    await refresh();
  }
}

async function refresh() {
  await renderHomeSources();
  await renderQueue();
  updateRevueBadge(currentItems.length);
}

/** Fill the Review nav badge with the pending count (board's "Revue [18]"). Runs from refresh()
 * — i.e. on every queue change, on any screen — so it's correct even off the Revue view. Empty
 * text collapses the pill via the `.nav-badge:empty` CSS rule. `count` is the queue length
 * `renderQueue` just fetched — no redundant `listQueue()` re-fetch here. */
function updateRevueBadge(count: number) {
  const badge = requireEl<HTMLElement>('.nav-badge[data-badge="revue"]', "updateRevueBadge");
  badge.textContent = count ? String(count) : "";
}

/** Live Réglages view: a single scrolling page of real cards (Discogs, Bibliothèque, Apparence),
 * replacing the mockup's static placeholder rows (Dossiers source, Format lossless…), which have
 * no backing data and led nowhere — same "lean Tauri UI" pattern as home-sources.ts (hide the mock
 * content, keep only the title, inject the real thing). One page, not tabs: every card is always
 * visible and reachable by scrolling, per the maquette's "PAS des onglets exclusifs" rule. */
async function renderReglagesLive() {
  const content = requireEl("#content", "renderReglagesLive");

  // Remove any previous live-settings block so we don't duplicate on re-render.
  document.getElementById("sift-reglages-live")?.remove();

  // Hide the mockup's static rows (no real data behind them); keep only the page title.
  let title: Element | null = null;
  for (const child of Array.from(content.children)) {
    if (!title && child.classList.contains("h1")) {
      title = child;
      continue;
    }
    (child as HTMLElement).style.display = "none";
  }

  let token: string | null = null;
  try {
    token = await getSetting("discogs_token");
  } catch (e) {
    console.error("getSetting(discogs_token) failed", e);
  }
  let theme: ThemeChoice = "auto";
  try {
    const v = await getSetting("ui_theme");
    if (v === "light" || v === "dark") theme = v;
  } catch (e) {
    console.error("getSetting(ui_theme) failed", e);
  }
  let root: string | null = null;
  try {
    root = await getSetting("library_root");
  } catch (e) {
    console.error("getSetting(library_root) failed", e);
  }

  const inputCss =
    "font-size:var(--text-md);padding:4px 7px;background:var(--color-background-secondary);" +
    "border:0.5px solid var(--color-border-tertiary);border-radius:var(--border-radius-md);" +
    "color:var(--color-text-primary);width:100%;font-family:var(--font-mono)";

  const block = document.createElement("div");
  block.id = "sift-reglages-live";
  block.dataset.section = "discogs";
  block.style.cssText = "margin-top:14px";
  block.innerHTML =
    '<div class="col-h">Discogs</div>' +
    '<div class="srow" style="flex-direction:column;align-items:flex-start;gap:6px;padding-bottom:10px">' +
    '<div style="display:flex;align-items:center;justify-content:space-between;width:100%">' +
    '<span style="font-size:var(--text-md)">Authentication token</span>' +
    '<a id="sift-discogs-link" style="font-size:var(--text-sm);color:var(--color-text-info);cursor:pointer;text-decoration:none">' +
    '<i class="ti ti-external-link" style="font-size:var(--text-sm);vertical-align:-1px"></i> get a token</a>' +
    "</div>" +
    `<input id="sift-discogs-token" type="text" placeholder="Discogs token…" value="${esc(token ?? "")}" style="${inputCss}">` +
    '<div id="sift-discogs-status" style="font-size:var(--text-sm);color:var(--color-text-tertiary);min-height:14px"></div>' +
    "</div>";

  const libBlock = document.createElement("div");
  libBlock.id = "sift-reglages-bibliotheque";
  libBlock.dataset.section = "bibliotheque";
  libBlock.style.cssText = "margin-top:14px";
  libBlock.innerHTML =
    '<div class="col-h">Bibliothèque</div>' +
    '<div class="srow" style="flex-direction:column;align-items:flex-start;gap:6px;padding-bottom:10px">' +
    '<div style="display:flex;align-items:center;justify-content:space-between;width:100%;gap:12px">' +
    `<span style="font-size:var(--text-md);font-family:var(--font-mono);color:${
      root ? "var(--color-text-primary)" : "var(--color-text-tertiary)"
    };overflow:hidden;text-overflow:ellipsis;white-space:nowrap">${esc(root || "Aucun dossier sélectionné")}</span>` +
    '<button id="sift-lib-root-change" style="flex:none;font-size:var(--text-sm);padding:2px 10px">Changer…</button>' +
    "</div>" +
    (root
      ? '<div id="sift-lib-root-forget" style="font-size:var(--text-sm);color:var(--color-text-tertiary);cursor:pointer;text-decoration:underline">Oublier le dossier racine</div>'
      : "") +
    "</div>";
  libBlock.querySelector("#sift-lib-root-change")?.addEventListener("click", () => {
    void (async () => {
      const dir = await openFolderDialog({ directory: true, multiple: false });
      if (typeof dir !== "string") return;
      try {
        await setSetting("library_root", dir);
        void renderReglagesLive();
      } catch (e) {
        console.error("setSetting(library_root) failed", e);
      }
    })();
  });
  libBlock.querySelector("#sift-lib-root-forget")?.addEventListener("click", () => {
    void (async () => {
      try {
        await setSetting("library_root", "");
        void renderReglagesLive();
      } catch (e) {
        console.error("setSetting(library_root) failed", e);
      }
    })();
  });

  const themeBlock = document.createElement("div");
  themeBlock.id = "sift-reglages-apparence";
  themeBlock.dataset.section = "apparence";
  themeBlock.style.cssText = "margin-top:14px";
  const themeBtn = (v: ThemeChoice, label: string) =>
    `<span class="chip${theme === v ? " on" : ""}" data-theme-choice="${v}">${label}</span>`;
  themeBlock.innerHTML =
    '<div class="col-h">Apparence</div>' +
    '<div class="srow" style="padding-bottom:10px">' +
    '<div style="display:flex;gap:5px">' +
    themeBtn("auto", "Auto") +
    themeBtn("light", "Clair") +
    themeBtn("dark", "Sombre") +
    "</div></div>";
  themeBlock.querySelectorAll<HTMLElement>("[data-theme-choice]").forEach((el) =>
    el.addEventListener("click", () => {
      const choice = el.dataset.themeChoice as ThemeChoice;
      void setTheme(choice);
      themeBlock.querySelectorAll("[data-theme-choice]").forEach((c) => c.classList.remove("on"));
      el.classList.add("on");
    }),
  );

  content.appendChild(block);
  content.appendChild(libBlock);
  content.appendChild(themeBlock);

  const inp = block.querySelector<HTMLInputElement>("#sift-discogs-token");
  const status = block.querySelector<HTMLElement>("#sift-discogs-status");
  const link = block.querySelector<HTMLElement>("#sift-discogs-link");

  link?.addEventListener("click", () =>
    void openUrl("https://www.discogs.com/settings/developers").catch((e) =>
      console.error("openUrl failed", e),
    ),
  );

  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  inp?.addEventListener("input", () => {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(async () => {
      const val = inp.value.trim();
      try {
        await setSetting("discogs_token", val);
        if (status) {
          status.textContent = val ? "Token saved." : "Token cleared.";
          setTimeout(() => {
            if (status) status.textContent = "";
          }, 2000);
        }
      } catch (e) {
        if (status) status.textContent = "Save error.";
        console.error("setSetting(discogs_token) failed", e);
      }
    }, 600);
  });
}

function fmtDur(sec: number | null): string {
  if (!sec || sec <= 0) return "—";
  const m = Math.floor(sec / 60),
    s = Math.round(sec % 60);
  return `${m}:${String(s).padStart(2, "0")}`;
}
function qualPill(t: LibraryTrack): string {
  const f = (t.format || "?").toUpperCase();
  return `<span class="pill" style="flex:none">${esc(f)}</span>`;
}
function verdictBadge(v: string | null): string {
  if (v === "fake")
    return `<span class="pill" style="background:var(--color-background-danger);color:var(--color-text-danger);flex:none">fake</span>`;
  if (v === "grey")
    return `<span class="pill" style="background:var(--color-background-warning);color:var(--color-text-warning);flex:none">?</span>`;
  return "";
}

/** Live Bibliothèque view: lists filed tracks with search + quality chips + folder/genre
 * facets, wired to real data. Actions go through the #pa delegated handler (data-bib). */
async function renderBiblioLive() {
  const content = requireEl("#content", "renderBiblioLive");
  let facets: LibraryFacets = { folders: [], genres: [] };
  try {
    [bibState.tracks, facets] = await Promise.all([
      listLibrary(bibState.filter),
      libraryFolders(),
    ]);
  } catch (e) {
    console.error("library load failed", e);
    return;
  }

  const chips = (["all", "lossless", "mp3"] as const)
    .map((q) => {
      const on = (bibState.filter.quality ?? "all") === q;
      const label = q === "all" ? "All" : q === "lossless" ? "Lossless" : "MP3";
      return `<span class="chip${on ? " on" : ""}" data-bib="qual" data-q="${q}">${label}</span>`;
    })
    .join("");

  const facetList = bibState.facet === "folder" ? facets.folders : facets.genres;
  const sideKey = bibState.facet === "folder" ? "folder" : "genre";
  const activeFacetVal = bibState.facet === "folder" ? bibState.filter.folder : bibState.filter.genre;
  const side =
    `<div style="display:flex;gap:4px;margin-bottom:8px">` +
    `<span class="chip${bibState.facet === "folder" ? " on" : ""}" data-bib="facet" data-f="folder">Folders</span>` +
    `<span class="chip${bibState.facet === "genre" ? " on" : ""}" data-bib="facet" data-f="genre">Genres</span></div>` +
    facetList
      .map(
        (b) =>
          `<div class="fld${activeFacetVal === b.name ? " on" : ""}" data-bib="pick" data-key="${sideKey}" data-val="${esc(b.name)}" style="justify-content:space-between"><span>${esc(b.name)}</span><span style="font-size:var(--text-sm);opacity:.7">${b.count}</span></div>`,
      )
      .join("");

  const rows = bibState.tracks
    .map((t) => {
      const name = esc(t.artist && t.title ? `${t.artist} — ${t.title}` : t.path.split(/[\\/]/).pop() || t.path);
      const link = t.discogs_release_id
        ? `<button class="lk" data-bib="link" data-rid="${esc(t.discogs_release_id)}" aria-label="Discogs page"><i class="ti ti-external-link" style="font-size:var(--text-base);color:var(--color-text-tertiary)"></i></button>`
        : `<button class="lk" data-bib="identify" data-id="${t.id}" aria-label="Identify"><i class="ti ti-search" style="font-size:var(--text-md);color:var(--color-text-tertiary)"></i></button>`;
      return `<div class="lr" data-bib="row" data-id="${t.id}"><button class="pb" data-bib="play" data-id="${t.id}" aria-label="Listen"><i class="ti ti-player-play" style="font-size:var(--text-md)"></i></button><span class="bib-name" style="flex:1;min-width:0;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">${name}</span>${verdictBadge(t.verdict)}${qualPill(t)}<span style="flex:none;width:40px;text-align:right;font-family:var(--font-mono);color:var(--color-text-tertiary)">${fmtDur(t.duration)}</span>${link}</div>`;
    })
    .join("");
  // Truly empty (no filed track at all, no filter narrowing it) vs. a filter that just matches
  // nothing right now — only the former is DESIGN.md's "État vide" dead-end with a back-to-Revue
  // link; the latter keeps the search/chips/facets on screen so the filter can be cleared.
  const noFilter =
    !bibState.filter.q && !bibState.filter.quality && !bibState.filter.folder && !bibState.filter.genre;
  const trulyEmpty = bibState.tracks.length === 0 && noFilter;

  // Export (Rekordbox/Clé USB) lives in the nav rail now, not here — matches the maquette's
  // persistent Export section (index.html nav-export items, wired in installLiveWiring below).
  const header =
    `<div style="display:flex;align-items:center;gap:8px;margin-bottom:10px">` +
    `<div style="flex:1;display:flex;align-items:center;gap:7px;border:0.5px solid var(--color-border-secondary);border-radius:var(--border-radius-md);padding:6px 10px"><i class="ti ti-search" style="font-size:var(--text-lg);color:var(--color-text-tertiary)"></i><input id="bibq" placeholder="Search…" value="${esc(bibState.filter.q || "")}" style="flex:1;border:0;background:transparent;color:inherit;font-size:var(--text-md);outline:none"></div>` +
    chips +
    `</div>`;

  content.innerHTML = trulyEmpty
    ? emptyStateHtml({
        title: "Bibliothèque vide",
        note: "Les pistes que tu ranges depuis Revue apparaissent ici, prêtes à exporter vers Rekordbox ou une clé USB.",
        backToRevue: true,
      })
    : header +
      `<div style="display:flex;gap:14px"><div style="width:150px;flex:none"><div class="col-h">Library</div>${side}</div>` +
      `<div style="flex:1;min-width:0"><div style="display:flex;justify-content:space-between;margin-bottom:5px"><span style="font-size:var(--text-base);font-weight:500">${esc(activeFacetVal || "All")}</span><span style="font-size:var(--text-sm);color:var(--color-text-tertiary)">${bibState.tracks.length} tracks</span></div>` +
      (rows ||
        `<div style="font-size:var(--text-md);color:var(--color-text-tertiary)">Aucun résultat pour ce filtre.</div>`) +
      `<div id="bibplayer"></div></div></div>`;
  wireEmptyState(content);

  if (trulyEmpty) return; // no header/search — nothing left to wire

  const q = document.getElementById("bibq") as HTMLInputElement | null;
  q?.addEventListener("input", () => {
    bibState.filter.q = q.value || undefined;
    clearTimeout((q as unknown as { _t?: number })._t);
    (q as unknown as { _t?: number })._t = window.setTimeout(() => void renderBiblioLive(), 250);
  });
}

/** Display name for a library row (artist — title, else filename). Mirrors the row template. */
function bibName(t: LibraryTrack): string {
  return t.artist && t.title ? `${t.artist} — ${t.title}` : t.path.split(/[\\/]/).pop() || t.path;
}

/** Open the unified detail/edit panel for a filed track into #bibplayer, highlighting its row.
 * On save, patch the row label in place (player stays alive); on delete, re-render the list. */
function openBiblioDetail(id: number): void {
  const t = bibState.tracks.find((x) => x.id === id);
  const host = requireEl("#bibplayer", "openBiblioDetail");
  if (!t) return;
  document.querySelectorAll(".lr.cur").forEach((n) => n.classList.remove("cur"));
  document.querySelector(`.lr[data-id="${id}"]`)?.classList.add("cur");
  openLibraryDetailInto(
    host,
    t,
    (updated) => {
      // Keep the in-memory list + the visible row label in sync without a full re-render.
      const i = bibState.tracks.findIndex((x) => x.id === updated.id);
      if (i >= 0) bibState.tracks[i] = updated;
      const span = document.querySelector(`.lr[data-id="${updated.id}"] .bib-name`);
      if (span) span.textContent = bibName(updated);
    },
    () => void renderBiblioLive(),
  );
}

export function installLiveWiring() {
  window.__siftHome = renderHomeSources;
  window.__siftQueue = renderQueue;
  window.__siftEcarts = renderEcartes;
  window.__siftReglages = () => void renderReglagesLive();
  window.__siftBiblio = () => void renderBiblioLive();
  window.__siftJournal = () => void renderJournal();
  injectLeanStyle();
  injectTitlebar();
  void initTheme();
  installUndoShortcut();
  installFilingKeys();
  installScrollAutohide();
  void installDragDrop();

  // Nav Export (Rekordbox/Clé USB) is a one-click action, not a real screen (renderRkb/renderCle
  // in app.js are unbuilt mock content) — capture phase so this runs BEFORE app.js's own bubble-
  // phase `#pa` listener (registered first, at import time) can switch `view` to the mock screen.
  // stopPropagation() during capture halts the whole path, including that bubble-phase listener.
  requireEl("#pa", "installLiveWiring").addEventListener(
    "click",
    (e) => {
      const exp = (e.target as HTMLElement).closest<HTMLElement>(
        '[data-view="rkb"],[data-view="cle"]',
      );
      if (!exp) return;
      e.stopPropagation();
      void runNavExport(exp.dataset.view === "cle" ? "usb" : "rekordbox");
    },
    { capture: true },
  );

  // Debounces the heavy report/audio-decode load triggered by a queue-row selection (click or
  // ↑/↓, which dispatches a real .click() — see installFilingKeys). Flicking through several
  // rows fast would otherwise fire a full fetch+decodeAudioData per row, most immediately
  // discarded. Row highlighting itself stays instant — only this load is deferred.
  let queueSelectTimer: ReturnType<typeof setTimeout> | undefined;

  requireEl("#pa", "installLiveWiring").addEventListener("click", (e) => {
    // queue item → open the live filing pane (report + editor + actions) in #mid
    const qi = (e.target as HTMLElement).closest<HTMLElement>(".qi[data-id]");
    if (qi?.dataset.id) {
      e.stopPropagation();
      // In batch mode a row-click means "inspect this one" → drop back to the detail pane.
      if (reviewMode === "batch") setReviewMode("detail");
      const id = Number(qi.dataset.id);
      const item = currentItems.find((it) => it.id === id);
      const mid = requireEl("#mid", "qi-click");
      // highlight the active row
      document.querySelectorAll(".qi.cur").forEach((n) => n.classList.remove("cur"));
      qi.classList.add("cur");
      clearTimeout(queueSelectTimer);
      queueSelectTimer = setTimeout(() => {
        if (item && mid) void openFilingInto(mid, item);
        else if (qi.dataset.path)
          void import("./report-view").then((m) => m.openReportModal(qi.dataset.path!));
      }, 150);
      return;
    }
    // Écartés actions (Soulseek copy / send-to-bin / restore / empty bin)
    const ec = (e.target as HTMLElement).closest<HTMLElement>("[data-ec]");
    if (ec) {
      e.stopPropagation();
      const act = ec.dataset.ec;
      const id = Number(ec.dataset.id);
      if (act === "slsk") {
        void navigator.clipboard.writeText(ec.dataset.q || "").catch(() => {});
        const prev = ec.innerHTML;
        ec.innerHTML = '<i class="ti ti-check" style="font-size:var(--text-xs);vertical-align:-1px"></i> Copied';
        setTimeout(() => {
          ec.innerHTML = prev;
        }, 1200);
      } else if (act === "trash") {
        void trashTrack(id).then(renderEcartes).catch((err) => console.error("trash failed", err));
      } else if (act === "restore") {
        void restoreTrack(id).then(renderEcartes).catch((err) => console.error("restore failed", err));
      } else if (act === "requeue") {
        void requeueTrack(id).then(renderEcartes).catch((err) => console.error("requeue failed", err));
      } else if (act === "purge") {
        void purgeTrash().then(renderEcartes).catch((err) => console.error("purge failed", err));
      } else if (act === "store") {
        void openUrl(decodeURIComponent(ec.dataset.url || "")).catch((err) =>
          console.error("open_url failed", err),
        );
      }
      return;
    }
    // Bibliothèque actions (quality chips / facet toggle / folder|genre pick / Discogs link / play)
    const bibEl = (e.target as HTMLElement).closest<HTMLElement>("[data-bib]");
    if (bibEl) {
      const act = bibEl.dataset.bib;
      if (act === "qual") {
        const q = bibEl.dataset.q;
        bibState.filter.quality = q === "all" ? undefined : (q as "lossless" | "mp3");
        void renderBiblioLive();
      } else if (act === "facet") {
        bibState.facet = bibEl.dataset.f === "genre" ? "genre" : "folder";
        void renderBiblioLive();
      } else if (act === "pick") {
        const key = bibEl.dataset.key as "folder" | "genre";
        const val = bibEl.dataset.val;
        // toggle off if re-clicking the active facet value
        const cur = key === "folder" ? bibState.filter.folder : bibState.filter.genre;
        const next = cur === val ? undefined : val;
        bibState.filter.folder = key === "folder" ? next : undefined;
        bibState.filter.genre = key === "genre" ? next : undefined;
        void renderBiblioLive();
      } else if (act === "link") {
        const rid = bibEl.dataset.rid;
        if (rid) void openUrl(`https://www.discogs.com/release/${rid}`);
      } else if (act === "play" || act === "row" || act === "identify") {
        // Open the unified detail/edit panel (report + inline editor + identify + actions).
        openBiblioDetail(Number(bibEl.dataset.id));
      }
      return;
    }
    const el = (e.target as HTMLElement).closest<HTMLElement>("[data-sift]");
    if (!el) return;
    const act = el.dataset.sift;
    if (act === "addsrc") {
      e.stopPropagation();
      void pickAndAddFolder(refresh);
    } else if (act === "rmsrc") {
      e.stopPropagation();
      void removeSource(Number(el.dataset.id)).then(refresh);
    } else if (act === "togglewatch") {
      e.stopPropagation();
      void setSourceWatched(
        Number(el.dataset.id),
        el.dataset.watched !== "1",
      ).then(refresh);
    } else if (act === "reviewmode") {
      e.stopPropagation();
      setReviewMode(el.dataset.m === "batch" ? "batch" : "detail");
    } else if (act === "batchpick") {
      e.stopPropagation();
      const id = Number(el.dataset.id);
      if (batchSel.has(id)) batchSel.delete(id);
      else batchSel.add(id);
      renderBatch();
    } else if (act === "batchgroup") {
      // Group-header tri-state toggle (maquette `onToggleAll`) — "file" selects/clears every
      // ready row, "fake" every fake row. Empty/partial → select all; full → clear.
      e.stopPropagation();
      const kind = el.dataset.kind === "fake" ? "fake" : "file";
      const ids =
        kind === "fake"
          ? currentItems.filter((it) => it.verdict === "fake").map((it) => it.id)
          : currentItems.filter((it) => it.verdict === "ok").map((it) => it.id);
      const sel = kind === "fake" ? batchFakeSel : batchSel;
      const full = ids.length > 0 && ids.every((id) => sel.has(id));
      for (const id of ids) if (full) sel.delete(id);
        else sel.add(id);
      renderBatch();
    } else if (act === "batchpickfake") {
      e.stopPropagation();
      const id = Number(el.dataset.id);
      if (batchFakeSel.has(id)) batchFakeSel.delete(id);
      else batchFakeSel.add(id);
      renderBatch();
    } else if (act === "batchformat") {
      e.stopPropagation();
      batchFormat = el.dataset.t as Target;
      renderBatchRail(currentItems.filter((it) => it.verdict !== "ok").length);
    } else if (act === "batchopen") {
      e.stopPropagation();
      const id = Number(el.dataset.id);
      const item = currentItems.find((it) => it.id === id);
      setReviewMode("detail");
      const mid = requireEl("#mid", "batchopen");
      if (item && mid) void openFilingInto(mid, item);
    } else if (act === "batchaction") {
      e.stopPropagation();
      // Adaptive dispatch. Combined (both ticked): file runs with its progress UI (Stop follows it);
      // discard fires in parallel as a fast fire-and-forget — IDs captured before clear.
      if (batchSel.size && batchFakeSel.size) {
        const discardIds = [...batchFakeSel];
        batchFakeSel.clear();
        void runBatchFile();
        void rejectBatch(discardIds).catch((err: unknown) =>
          console.error("reject_batch (combined) failed", err),
        );
      } else if (batchSel.size) {
        void runBatchFile();
      } else if (batchFakeSel.size) {
        void runBatchDiscard();
      }
    } else if (act === "batchstop") {
      e.stopPropagation();
      onFileStop();
    }
  });

  // "File in place" checkbox (under the #fldz tree, batch mode) — a checkbox, so it needs change.
  requireEl("#pa", "installLiveWiring").addEventListener("change", (e) => {
    const ip = (e.target as HTMLElement).closest<HTMLInputElement>('input[data-sift="inplace"]');
    if (ip) {
      batchInPlace = ip.checked;
      const fldz = document.getElementById("fldz");
      if (fldz) renderBinsForBatch(fldz, batchBin, onBatchBinPick, batchInPlace);
      renderBatchRail(currentItems.filter((it) => it.verdict !== "ok").length);
    }
  });

  // queue:changed fires once per burst source (watcher debounce window, each scanned source's
  // own background thread) — debounce the redraw the same way onAnalysisChanged does below.
  let queueChangeTimer: ReturnType<typeof setTimeout> | undefined;
  void onQueueChanged(() => {
    clearTimeout(queueChangeTimer);
    queueChangeTimer = setTimeout(() => void refresh(), 150);
  });
  void onFileDone(onFileBatchDone);
  void onFileProgress(pushFileProgress);
  // Stop button on the global zone's "file" row → stop-net cancel of the running filing batch.
  setCancelHandler("file", onFileStop);

  // Analysis pings can arrive several times per second — debounce the queue redraw.
  let t: ReturnType<typeof setTimeout> | undefined;
  // Throttle the progress-zone IPC+render: coalesce bursts to one RAF per frame (~16 ms).
  // Events are never dropped — only renders are coalesced. A trailing 350 ms timeout
  // guarantees a final render once pings stop (catches the done==total transition).
  let pendingAnalyzeRender = false;
  let analyzeTrailTimer: ReturnType<typeof setTimeout> | undefined;
  function scheduleAnalyzeRender() {
    // Reset the trailing timer on every event so it fires only after silence.
    clearTimeout(analyzeTrailTimer);
    analyzeTrailTimer = setTimeout(() => void pushAnalyzeProgress(), 350);
    if (pendingAnalyzeRender) return;
    pendingAnalyzeRender = true;
    requestAnimationFrame(() => {
      pendingAnalyzeRender = false;
      void pushAnalyzeProgress();
    });
  }
  void onAnalysisChanged(() => {
    // A report may have changed (re-analysed / replaced file) → drop the in-session cache so
    // the next open re-fetches from the DB (the source of truth) instead of serving it stale.
    void import("./report-view").then((m) => m.clearReportCache());
    // Throttle progress-zone update: IPC + DOM render at most once per RAF frame (~16 ms),
    // not once per event (can be dozens per second during a 4000-track analysis burst).
    scheduleAnalyzeRender();
    clearTimeout(t);
    // touchDetail=false: redraw the queue list only; never re-open the open track (that aborts
    // the player's audio load).
    t = setTimeout(() => void renderQueue(false), 300);
  });

  // Catch an analysis already in flight when the app opens (events only fire on each item after).
  void pushAnalyzeProgress();
  void refresh();
}

declare global {
  interface Window {
    __siftHome?: () => void;
    __siftQueue?: () => void;
    __siftEcarts?: () => void;
    __siftReglages?: () => void;
    __siftBiblio?: () => void;
    __siftJournal?: () => void;
  }
}
