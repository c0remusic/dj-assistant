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
import {
  openFilingInto,
  refreshBins,
  syncDetail,
  installUndoShortcut,
  installFilingKeys,
  renderBinsForBatch,
  refreshBinsForBatch,
  clearBinPick,
  defaultTarget,
  targetExt,
  TARGET_LABEL,
} from "./filing";
// Views/chrome extracted from this god-module (audit P-3) — kept stateless, wired here.
import { renderEcartes } from "./ecartes-view";
import { renderHomeSources, pickAndAddFolder } from "./home-sources";
import { installDragDrop, injectLeanStyle, injectTitlebar, installScrollAutohide } from "./chrome";
import type { QueueItem, BatchResult, FileProgress, Target } from "../shared/contracts";
import { FILE_IN_PLACE } from "../shared/contracts";
import { requireEl } from "./dom";

/** Human label for the batch destination (resolves the in-place sentinel to its prose). */
const IN_PLACE_LABEL = "Dossier source de chaque morceau";
import { setTask, clearTask, setCancelHandler } from "./progress-zone";
import {
  startBatchTracklist,
  previewBatchTracklist,
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
// Group keys (`kind:railKey`, e.g. "file:lossless"/"fake:fake") whose row list is COLLAPSED. Persists
// across re-renders like batchBin/batchSel. Collapsing hides rows only — never touches the selection.
const batchCollapsed = new Set<string>();
// Fakes ticked for DISCARD (never filed — Sift never ranges a fake lossless). Kept separate from
// batchSel (fileables → File) so the rail action button can be adaptive (File n / Discard n / both).
const batchFakeSel = new Set<number>();
// Batch "file in place" toggle (FILE_IN_PLACE). Kept apart from batchBin so the picked folder is
// remembered while in-place is on. Effective destination = batchInPlace ? FILE_IN_PLACE : batchBin.
let batchInPlace = false;
// Per-format-group encode target chosen via the group-header chips. Unset rail → its auto default
// (defaultTarget). Fed to the filer per track at submit (file_batch targets map).
const groupTarget: Partial<Record<"lossless" | "lossy" | "unknown", Target>> = {};
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

const VERDICT_DOT: Record<string, [string, string]> = {
  ok: ["#5bc08c", "authentic"],
  fake: ["#e2685e", "fake / over-encoded"],
  grey: ["#dda63f", "grey zone"],
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

  ql.innerHTML =
    (items
      .map(
        (it) =>
          `<div class="qi" data-id="${it.id}" data-path="${esc(it.path)}" title="Listen and file" style="display:flex;align-items:center;gap:8px;cursor:pointer">${verdictDot(
            it.verdict,
          )}<span style="flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis">${esc(
            it.filename || it.path,
          )}</span>${
            it.dup
              ? '<i class="ti ti-copy" title="Possible duplicate (same name)" style="flex:none;font-size:var(--text-md);color:var(--color-text-secondary)"></i>'
              : ""
          }<i class="ti ti-chevron-right" style="flex:none;color:var(--color-text-tertiary);font-size:var(--text-lg)"></i></div>`,
      )
      .join("") ||
      '<div style="font-size:var(--text-md);color:var(--color-text-tertiary);padding:6px 4px">Queue empty.</div>');

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
      on ? "rgba(255,255,255,.07)" : "transparent"
    };color:var(--color-text-${on ? "primary" : "tertiary"})"><i class="ti ${icon}" style="font-size:var(--text-base)"></i>${label}</button>`;
  };
  seg.innerHTML = tab("detail", "Detail", "ti-layout-list") + tab("batch", "Batch", "ti-table");
}

/** Batch triage view (board's Batch screen): splits the queue into a checkable READY list and a
 * read-only NEEDS REVIEW list, with a destination dropdown + File/Discard action bar (center,
 * #mid) and a selection summary rail (#filfoot). Every control is bound to a real command
 * (`fileBatch` / `rejectBatch`); nothing here is mocked. */
function renderBatch() {
  const mid = requireEl("#mid", "renderBatch");
  const ready = currentItems.filter((it) => it.verdict === "ok");
  const review = currentItems.filter((it) => it.verdict !== "ok");
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
        on ? "background:rgba(255,255,255,.045)" : ""
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
  const reviewRow = (it: QueueItem) => {
    const tone =
      it.verdict === "fake"
        ? ["FAKE", "var(--color-background-danger)", "var(--color-text-danger)"]
        : it.verdict === "grey"
          ? ["CHECK", "var(--color-background-warning)", "var(--color-text-warning)"]
          : ["UNANALYZED", "rgba(255,255,255,.06)", "var(--color-text-tertiary)"];
    return (
      `<div style="display:flex;align-items:center;gap:9px;padding:7px 9px">` +
      verdictDot(it.verdict) +
      nameCell(it, true) +
      (it.dup
        ? '<span style="flex:none;font-size:var(--text-2xs);font-weight:600;padding:2px 7px;border-radius:999px;background:var(--color-background-warning);color:var(--color-text-warning)">DUP</span>'
        : "") +
      `<span style="flex:none;font-size:var(--text-2xs);font-weight:600;letter-spacing:.03em;padding:2px 7px;border-radius:999px;background:${tone[1]};color:${tone[2]}">${tone[0]}</span>` +
      `<button data-sift="batchopen" data-id="${it.id}" style="flex:none;font-size:var(--text-xs);padding:2px 8px;color:var(--color-text-info)">open in Detail</button>` +
      `</div>`
    );
  };

  // Fakes are selectable to DISCARD (their own tick set), never to file. grey/unanalyzed stay
  // read-only (reviewRow) — they need a human in Detail first.
  const fakeRow = (it: QueueItem) => {
    const on = batchFakeSel.has(it.id);
    return (
      `<div class="bx-row" data-sift="batchpickfake" data-id="${it.id}" style="display:flex;align-items:center;gap:9px;padding:7px 9px;border-radius:var(--border-radius-md);cursor:pointer;${
        on ? "background:rgba(255,255,255,.045)" : ""
      }">` +
      `<span class="bx-ck" style="flex:none;width:15px;height:15px;border-radius:4px;display:inline-flex;align-items:center;justify-content:center;border:1.5px solid ${
        on ? "var(--color-text-danger)" : "var(--color-border-secondary)"
      };background:${on ? "var(--color-background-danger)" : "transparent"}">${
        on ? '<i class="ti ti-check" style="font-size:var(--text-xs);color:var(--color-text-danger)"></i>' : ""
      }</span>` +
      verdictDot(it.verdict) +
      nameCell(it, true) +
      '<span style="flex:none;font-size:var(--text-2xs);font-weight:600;letter-spacing:.03em;padding:2px 7px;border-radius:999px;background:var(--color-background-danger);color:var(--color-text-danger)">FAKE</span>' +
      `<button data-sift="batchopen" data-id="${it.id}" style="flex:none;font-size:var(--text-xs);padding:2px 8px;color:var(--color-text-info)">open in Detail</button>` +
      `</div>`
    );
  };

  const allOn = ready.length > 0 && batchSel.size === ready.length;
  const sectionHead = (label: string, n: number, extra = "") =>
    `<div style="display:flex;align-items:center;justify-content:space-between;margin:0 0 6px"><div class="col-h" style="margin:0">${label} · ${n}</div>${extra}</div>`;

  // READY rows grouped by rail (board's lossless/lossy separation), each labelled with the real
  // output format the filer will encode to (encode::target_for). Tracks file separately by rail.
  const railGroup = (rail: "lossless" | "lossy" | "unknown") => {
    const xs = ready.filter((it) => (it.rail ?? "unknown") === rail);
    if (!xs.length) return "";
    const collapsed = batchCollapsed.has(`file:${rail}`);
    return (
      `<div style="margin:2px 0 6px">` +
      groupHeaderHtml("file", rail, railLabel(rail), xs.map((it) => it.id), groupChipsHtml(rail)) +
      (collapsed ? "" : xs.map(readyRow).join("")) +
      `</div>`
    );
  };
  const fakes = review.filter((it) => it.verdict === "fake");
  const reviewRest = review.filter((it) => it.verdict !== "fake");
  // Only global control kept: a single Tout / Aucun toggle. Per-format selection now lives in the
  // group-header checkboxes (no redundant per-format quick buttons up here).
  const readyHead = sectionHead(
    "READY TO FILE",
    ready.length,
    `<button data-sift="batchall" style="font-size:var(--text-xs);padding:2px 8px;color:var(--color-text-info)">${
      allOn ? "Aucun (clear)" : "Tout"
    }</button>`,
  );

  // No center action bar: the destination + adaptive File/Discard/Stop button now live solely in the
  // right rail (renderBatchRail), mirroring the Detail screen's CTA-in-the-rail grammar.
  mid.innerHTML =
    `<div style="display:flex;flex-direction:column;height:100%;min-height:0">` +
    `<div style="flex:1;min-height:0;overflow-y:auto;padding-right:2px">` +
    (ready.length
      ? readyHead + railGroup("lossless") + railGroup("lossy") + railGroup("unknown")
      : '<div class="col-h" style="margin:0 0 6px">READY TO FILE · 0</div><div style="font-size:var(--text-md);color:var(--color-text-tertiary);padding:4px 9px 14px">Nothing clean to file yet.</div>') +
    (review.length
      ? `<div style="margin-top:16px"></div>` +
        sectionHead("NEEDS REVIEW", review.length) +
        (fakes.length
          ? groupHeaderHtml("fake", "fake", "Fakes", fakes.map((it) => it.id)) +
            (batchCollapsed.has("fake:fake") ? "" : fakes.map(fakeRow).join(""))
          : "") +
        reviewRest.map(reviewRow).join("")
      : "") +
    `</div></div>`;

  renderBatchRail(review.length);
}

/** Human label for a rail. */
function railLabel(rail: "lossless" | "lossy" | "unknown"): string {
  return rail === "lossless" ? "Lossless" : rail === "lossy" ? "Lossy" : "Unknown rail";
}

/** Tri-state of a group's checkbox given its item ids and the active selection set. */
function groupState(ids: number[], sel: Set<number>): "empty" | "partial" | "full" {
  if (ids.length === 0) return "empty";
  let n = 0;
  for (const id of ids) if (sel.has(id)) n++;
  return n === 0 ? "empty" : n === ids.length ? "full" : "partial";
}

/** A group header row: tri-state checkbox + col-h label + count + optional right-aligned extra.
 *  `kind` routes the toggle to the right selection set ("file" → batchSel, "fake" → batchFakeSel). */
function groupHeaderHtml(
  kind: "file" | "fake",
  railKey: string,
  label: string,
  ids: number[],
  extra = "",
): string {
  const st = groupState(ids, kind === "file" ? batchSel : batchFakeSel);
  const box =
    st === "full"
      ? '<span class="sift-bgrp-box on"><i class="ti ti-check"></i></span>'
      : st === "partial"
        ? '<span class="sift-bgrp-box partial"><i class="ti ti-minus"></i></span>'
        : '<span class="sift-bgrp-box"></span>';
  // Collapse chevron: its own data-sift so the click resolves to "batchcollapse" (closest [data-sift])
  // and is swallowed there — never bubbling to the group-header tri-state toggle (same nesting trick
  // as the format chips). Collapsing hides rows only; the checkbox count stays driven by `ids`.
  const gkey = `${kind}:${railKey}`;
  const collapsed = batchCollapsed.has(gkey);
  const chev = `<span data-sift="batchcollapse" data-gkey="${esc(gkey)}" style="flex:none;display:inline-flex;align-items:center;cursor:pointer;color:var(--color-text-tertiary)"><i class="ti ti-chevron-${
    collapsed ? "right" : "down"
  }" style="font-size:var(--text-md)"></i></span>`;
  return (
    `<div class="sift-bgrp-head" data-sift="batchgroup" data-kind="${kind}" data-rail="${esc(railKey)}" style="cursor:pointer">` +
    chev +
    box +
    `<span class="col-h" style="margin:0">${esc(label)} · ${ids.length}</span>` +
    `<span style="flex:1"></span>${extra}</div>`
  );
}

/** The active encode target for a format group: the chip the user picked, else the rail's default. */
function railTarget(rail: "lossless" | "lossy" | "unknown"): Target {
  return groupTarget[rail] ?? defaultTarget(rail);
}

/** Per-group format chips (MP3 / AIFF / WAV), shown on the right of a group header. Same `chip`/
 *  `chip on` look as the détail (renderFoot). A lossy source can't be upscaled, so AIFF/WAV are hard-
 *  greyed (matching renderFoot's rule) — pointer-events:none so a greyed chip can't toggle the group. */
function groupChipsHtml(rail: "lossless" | "lossy" | "unknown"): string {
  const active = railTarget(rail);
  return (["mp3_320", "aiff_16_44", "wav_16_44"] as Target[])
    .map((t) => {
      // Greyed chip still carries data-sift="batchfmt" (no data-t) so its click is swallowed by the
      // batchfmt handler instead of falling through to the group-header toggle.
      if (rail === "lossy" && t !== "mp3_320")
        return `<span class="chip" data-sift="batchfmt" data-rail="${rail}" title="Pas d'upscale depuis une source lossy" style="opacity:.4;cursor:not-allowed">${TARGET_LABEL[t]}</span>`;
      return `<span class="chip${active === t ? " on" : ""}" data-sift="batchfmt" data-rail="${rail}" data-t="${t}">${TARGET_LABEL[t]}</span>`;
    })
    .join(" ");
}


/** The destination actually passed to the filer (FILE_IN_PLACE sentinel, or the picked folder rel). */
function batchDest(): string {
  return batchInPlace ? FILE_IN_PLACE : batchBin;
}
/** Human label for the batch destination — shown in the rail récap + name preview. */
function batchDestLabel(): string {
  return batchInPlace ? IN_PLACE_LABEL : batchBin || "Library root";
}
/** A folder click in the #fldz tree (batch pick mode) -> set batchBin, drop in-place, re-render. */
function onBatchBinPick(rel: string): void {
  batchBin = rel;
  batchInPlace = false; // choosing a folder turns off "file in place"
  const fldz = document.getElementById("fldz");
  if (fldz) renderBinsForBatch(fldz, batchBin, onBatchBinPick);
  renderBatchRail(currentItems.filter((it) => it.verdict !== "ok").length);
}
/** The naming MOTIF (not 1752 computed names): the front's filename convention rendered as a
 *  placeholder `Artiste - Titre.<ext>`, one line per DISTINCT chosen extension among the selected
 *  groups (so lossless→aiff + lossy→mp3 shows two lines). Returns READY HTML (`<br>`-joined; dest is
 *  esc'd, the rest is literal). Convention = the default template `{artist} - {title}{version}`
 *  (settings DEFAULT_TEMPLATE); a customized FILENAME_TEMPLATE isn't exposed to the front, so this
 *  would go stale exactly like the detail "Final name" preview already does. */
function batchNameMotifHtml(): string {
  if (batchSel.size === 0) return "—";
  const dest = esc(batchDestLabel());
  const exts: string[] = [];
  for (const rail of ["lossless", "lossy", "unknown"] as const) {
    const has = currentItems.some(
      (it) => it.verdict === "ok" && (it.rail ?? "unknown") === rail && batchSel.has(it.id),
    );
    if (!has) continue;
    const ext = targetExt(railTarget(rail));
    if (!exts.includes(ext)) exts.push(ext);
  }
  if (!exts.length) return "—";
  return exts.map((ext) => `${dest}/Artiste - Titre.${ext}`).join("<br>");
}
/** Ensure the batch destination UI around #fldz: the tree is in batch pick mode, and a "file in
 *  place" checkbox sits right under it (a sibling, so renderBins' innerHTML rebuild can't wipe it). */
function ensureBatchDestUI(): void {
  const fldz = document.getElementById("fldz");
  if (!fldz) return;
  fldz.style.opacity = batchInPlace ? ".45" : "";
  fldz.style.pointerEvents = batchInPlace ? "none" : "";
  let box = document.getElementById("sift-inplace");
  if (!box) {
    box = document.createElement("label");
    box.id = "sift-inplace";
    box.style.cssText =
      "display:flex;align-items:center;gap:7px;margin-top:8px;font-size:var(--text-sm);color:var(--color-text-secondary);cursor:pointer";
    fldz.parentElement?.insertBefore(box, fldz.nextSibling);
  }
  box.innerHTML = `<input type="checkbox" data-sift="inplace"${
    batchInPlace ? " checked" : ""
  } style="accent-color:var(--color-text-info)"> ${esc(IN_PLACE_LABEL)}`;
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
    return `<button data-sift="batchaction" class="sift-baction" style="background:var(--color-background-danger);color:var(--color-text-danger)"><i class="ti ti-trash" style="font-size:var(--text-md);vertical-align:-2px"></i> Discarder (${fakeN})</button>`;
  return `<button data-sift="batchaction" class="sift-baction" style="background:var(--color-background-info);color:var(--color-text-info)">Filer (${fileN}) · Discarder (${fakeN})</button>`;
}

/** Right-rail summary for batch mode (board's SELECTION / DESTINATION / WILL ENCODE / EXCLUDED).
 * Replaces the filing footer + hides the folder tree while batching. */
function renderBatchRail(reviewN: number) {
  const foot = requireEl("#filfoot", "renderBatchRail");
  const fldz = requireEl("#fldz", "renderBatchRail");
  fldz.style.display = ""; // batch now shows the #fldz folder tree (the destination explorer)
  ensureBatchDestUI();
  const head = (label: string) => `<div class="col-h" style="margin:0 0 4px">${label}</div>`;
  // Preserve the LIVE run's progress list across this wholesale rebuild (renderBatch rebuilds the rail
  // on every selection change). Not while idle — the choice-time preview is rebuilt fresh below.
  const keepTracks = batchRunning ? foot.querySelector("#sift-batch-tracks") : null;
  const keepNote = foot.querySelector("[data-file-note]");
  // In-place mode has NO single destination (each track returns to its own source folder), so the
  // unique "Destination" line would lie — drop it; the per-track preview shows the real folders instead.
  const destBlock = batchInPlace
    ? ""
    : `<div style="margin-bottom:14px;background:var(--color-background-secondary);border-radius:var(--border-radius-md);padding:9px 11px">${head(
        "Destination",
      )}<div style="font-size:var(--text-md);color:var(--color-text-secondary)">${esc(
        batchDestLabel(),
      )}</div></div>`;
  // "Excluded" is folded into Selection as a discreet (tertiary) suffix — no separate block.
  const jeter = batchFakeSel.size ? ` · ${batchFakeSel.size} à jeter` : "";
  const exclus = reviewN
    ? ` · <span style="color:var(--color-text-tertiary)">${reviewN} exclus (en review)</span>`
    : "";
  // Order: Destination (pill, top) → Selection (+ exclus) → Final name (motif) → tracks → action.
  foot.innerHTML =
    destBlock +
    `<div style="margin-bottom:14px">${head("Selection")}<div style="font-size:var(--text-md);color:var(--color-text-primary);font-weight:500">${
      batchSel.size
    } à filer${jeter}${exclus}</div></div>` +
    `<div style="margin-bottom:14px">${head("Final name")}<div class="sift-fil-prev" style="font-size:var(--text-xs);color:var(--color-text-tertiary);font-family:var(--font-mono);word-break:break-all;line-height:1.5">${batchNameMotifHtml()}</div></div>` +
    `<div id="sift-batch-tracks"></div>` +
    `<div class="sift-baction-slot">${actionButtonHtml(batchRunning)}</div>`;
  if (keepNote) foot.insertAdjacentElement("afterbegin", keepNote);
  if (keepTracks) foot.querySelector("#sift-batch-tracks")!.replaceWith(keepTracks);
  else refreshBatchTracksPreview(); // in-place → show the per-track source folders at choice time
}

/** Switch between detail and batch review. On entering batch the #fldz tree becomes the destination
 * explorer (batch pick mode); on leaving we restore the per-track filing pane. */
function setReviewMode(m: "detail" | "batch") {
  reviewMode = m;
  ensureReviewSeg();
  const fldz = requireEl("#fldz", "setReviewMode");
  // Batch now SHOWS the #fldz tree (it is the destination explorer); detail keeps showing it too.
  fldz.style.display = "";
  if (m === "batch") {
    renderBatch();
    // Drive the #fldz tree in batch pick mode (loads bins, clicks set batchBin via onBatchBinPick).
    void refreshBinsForBatch(fldz, batchBin, onBatchBinPick);
  } else {
    // Leave batch pick mode: tree reverts to detail's state.binRel, remove the in-place checkbox.
    clearBinPick();
    document.getElementById("sift-inplace")?.remove();
    fldz.style.opacity = "";
    fldz.style.pointerEvents = "";
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
    '<i class="ti ti-loader sift-spin" style="font-size:var(--text-md);vertical-align:-1px"></i> Filing in the background…',
  );
  // Per-track encode target from the group chips (each id → its rail's chosen/default target).
  const targets: Record<number, Target> = {};
  for (const id of ids) {
    const it = currentItems.find((q) => q.id === id);
    targets[id] = railTarget((it?.rail ?? "unknown") as "lossless" | "lossy" | "unknown");
  }
  try {
    // Resolves as soon as the background task STARTS; the summary comes via file:done.
    await fileBatch(ids, batchDest(), targets);
  } catch (err) {
    // Launch-time rejections only (NoLibraryRoot, or the task couldn't start).
    const code = String(err);
    fileNote(
      code.includes("NoLibraryRoot")
        ? "No library root configured — set one in Settings."
        : `Filing failed to start: ${esc(code)}`,
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

/** Readable source-folder label for "file in place" mode: the parent folder NAME of `path` (its
 *  immediate containing directory), prefixed with `…<sep>`. Front-only, no disk read — handles both
 *  Windows `\` and POSIX `/` separators. E.g. `C:\Music\Crate Diggin\x.flac` → `…\Crate Diggin`. */
function sourceFolderLabel(path: string): string {
  const sep = path.includes("\\") ? "\\" : "/";
  const parts = path.split(/[\\/]/).filter(Boolean);
  if (parts.length < 2) return path; // no parent folder to show — fall back to the raw path
  return `…${sep}${parts[parts.length - 2]}`;
}

/** A per-track list item; the source-folder suffix is attached only in "file in place" mode (the one
 *  case where each track's destination differs — its own source folder — so a single récap line lies). */
function batchTrackItem(id: number): { id: number; name: string; suffix?: string } {
  const it = currentItems.find((q) => q.id === id);
  const suffix = batchInPlace && it ? sourceFolderLabel(it.path) : undefined;
  return { id, name: batchTrackName(id), suffix };
}

/** (Re)render the choice-time preview of the per-track list into #sift-batch-tracks: the in-place
 *  source folders, visible BEFORE a run. No-op during a run (the live list owns the container) or
 *  when not in-place (no per-track destinations to show). */
function refreshBatchTracksPreview(): void {
  if (reviewMode !== "batch" || batchRunning) return;
  const host = document.getElementById("sift-batch-tracks");
  if (!host) return;
  if (batchInPlace) previewBatchTracklist(host, [...batchSel].map(batchTrackItem));
  else host.innerHTML = "";
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
  await updateRevueBadge();
}

/** Fill the Review nav badge with the pending count (board's "Revue [18]"). Runs from refresh()
 * — i.e. on every queue change, on any screen — so it's correct even off the Revue view. Empty
 * text collapses the pill via the `.nav-badge:empty` CSS rule. */
async function updateRevueBadge() {
  const badge = requireEl<HTMLElement>('.nav-badge[data-badge="revue"]', "updateRevueBadge");
  try {
    const n = (await listQueue()).length;
    badge.textContent = n ? String(n) : "";
  } catch {
    /* leave the badge as-is on a transient failure */
  }
}

/** Live Réglages view: injects the real Discogs token field below the mockup rows. */
async function renderReglagesLive() {
  const content = requireEl("#content", "renderReglagesLive");

  // Remove any previous live-settings block so we don't duplicate on re-render.
  document.getElementById("sift-reglages-live")?.remove();

  let token: string | null = null;
  try {
    token = await getSetting("discogs_token");
  } catch (e) {
    console.error("getSetting(discogs_token) failed", e);
  }

  const inputCss =
    "font-size:var(--text-md);padding:4px 7px;background:var(--color-background-secondary);" +
    "border:0.5px solid var(--color-border-tertiary);border-radius:var(--border-radius-md);" +
    "color:var(--color-text-primary);width:100%;font-family:var(--font-mono)";

  const block = document.createElement("div");
  block.id = "sift-reglages-live";
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

  content.appendChild(block);

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

  const header =
    `<div style="display:flex;align-items:center;gap:8px;margin-bottom:10px">` +
    `<div style="flex:1;display:flex;align-items:center;gap:7px;border:0.5px solid var(--color-border-secondary);border-radius:var(--border-radius-md);padding:6px 10px"><i class="ti ti-search" style="font-size:var(--text-lg);color:var(--color-text-tertiary)"></i><input id="bibq" placeholder="Search…" value="${esc(bibState.filter.q || "")}" style="flex:1;border:0;background:transparent;color:inherit;font-size:var(--text-md);outline:none"></div>` +
    chips +
    `</div>`;

  content.innerHTML =
    header +
    `<div style="display:flex;gap:14px"><div style="width:150px;flex:none"><div class="col-h">Library</div>${side}</div>` +
    `<div style="flex:1;min-width:0"><div style="display:flex;justify-content:space-between;margin-bottom:5px"><span style="font-size:var(--text-base);font-weight:500">${esc(activeFacetVal || "All")}</span><span style="font-size:var(--text-sm);color:var(--color-text-tertiary)">${bibState.tracks.length} tracks</span></div>` +
    (rows || `<div style="font-size:var(--text-md);color:var(--color-text-tertiary)">No filed track.</div>`) +
    `<div id="bibplayer"></div></div></div>`;

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
  injectLeanStyle();
  injectTitlebar();
  installUndoShortcut();
  installFilingKeys();
  installScrollAutohide();
  void installDragDrop();

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
      if (item && mid) void openFilingInto(mid, item);
      else if (qi.dataset.path)
        void import("./report-view").then((m) => m.openReportModal(qi.dataset.path!));
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
    } else if (act === "batchall") {
      e.stopPropagation();
      const ready = currentItems.filter((it) => it.verdict === "ok");
      // "Clear" state (all ready ticked) → wipe BOTH selections so the action button falls back to the
      // disabled "Filer (0)"; otherwise select all ready.
      if (batchSel.size === ready.length) {
        batchSel.clear();
        batchFakeSel.clear();
      } else for (const it of ready) batchSel.add(it.id);
      renderBatch();
    } else if (act === "batchgroup") {
      e.stopPropagation();
      const kind = el.dataset.kind === "fake" ? "fake" : "file";
      const railKey = el.dataset.rail ?? "";
      const ids =
        kind === "fake"
          ? currentItems.filter((it) => it.verdict === "fake").map((it) => it.id)
          : currentItems
              .filter((it) => it.verdict === "ok" && (it.rail ?? "unknown") === railKey)
              .map((it) => it.id);
      const sel = kind === "fake" ? batchFakeSel : batchSel;
      // empty/partial → check all; full → clear all (tri-state toggle).
      const full = ids.length > 0 && ids.every((id) => sel.has(id));
      for (const id of ids) if (full) sel.delete(id);
        else sel.add(id);
      renderBatch();
    } else if (act === "batchcollapse") {
      e.stopPropagation(); // don't bubble to the group-header tri-state toggle
      const gkey = el.dataset.gkey ?? "";
      if (batchCollapsed.has(gkey)) batchCollapsed.delete(gkey);
      else batchCollapsed.add(gkey);
      renderBatch();
    } else if (act === "batchpickfake") {
      e.stopPropagation();
      const id = Number(el.dataset.id);
      if (batchFakeSel.has(id)) batchFakeSel.delete(id);
      else batchFakeSel.add(id);
      renderBatch();
    } else if (act === "batchfmt") {
      e.stopPropagation(); // don't let the chip click bubble to the group-header toggle
      const t = el.dataset.t; // greyed (disabled) chips carry no data-t → no-op
      if (t) {
        groupTarget[el.dataset.rail as "lossless" | "lossy" | "unknown"] = t as Target;
        renderBatch();
      }
    } else if (act === "batchopen") {
      e.stopPropagation();
      const id = Number(el.dataset.id);
      const item = currentItems.find((it) => it.id === id);
      setReviewMode("detail");
      const mid = requireEl("#mid", "batchopen");
      if (item && mid) void openFilingInto(mid, item);
    } else if (act === "batchaction") {
      e.stopPropagation();
      // Adaptive dispatch: file the ticked fileables, else discard the ticked fakes. When both are
      // ticked the button reads "Filer · Discarder" and runs the File batch (Stop/progress follows it).
      if (batchSel.size) void runBatchFile();
      else if (batchFakeSel.size) void runBatchDiscard();
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
      if (fldz) renderBinsForBatch(fldz, batchBin, onBatchBinPick);
      renderBatchRail(currentItems.filter((it) => it.verdict !== "ok").length);
    }
  });

  void onQueueChanged(refresh);
  void onFileDone(onFileBatchDone);
  void onFileProgress(pushFileProgress);
  // Stop button on the global zone's "file" row → stop-net cancel of the running filing batch.
  setCancelHandler("file", onFileStop);

  // Analysis pings can arrive several times per second — debounce the queue redraw.
  let t: ReturnType<typeof setTimeout> | undefined;
  void onAnalysisChanged(() => {
    // A report may have changed (re-analysed / replaced file) → drop the in-session cache so
    // the next open re-fetches from the DB (the source of truth) instead of serving it stale.
    void import("./report-view").then((m) => m.clearReportCache());
    // Update the global progress zone immediately (cheap count poll), decoupled from the queue
    // redraw debounce so the bar advances live during a continuous analysis burst.
    void pushAnalyzeProgress();
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
  }
}
