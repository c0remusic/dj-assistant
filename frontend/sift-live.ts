// Live data wiring — ACTIVE ONLY inside the Tauri app. In a plain browser the hooks
// below are never installed, so app.js keeps its mockup (Vercel demo unaffected).
import {
  removeSource,
  listQueue,
  listBins,
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
} from "./filing";
// Views/chrome extracted from this god-module (audit P-3) — kept stateless, wired here.
import { renderEcartes } from "./ecartes-view";
import { renderHomeSources, pickAndAddFolder } from "./home-sources";
import { installDragDrop, injectLeanStyle, injectTitlebar, installScrollAutohide } from "./chrome";
import type { QueueItem, Bin, BatchResult, FileProgress } from "../shared/contracts";
import { requireEl } from "./dom";
import { setTask, clearTask, setCancelHandler } from "./progress-zone";

// Latest live queue items, kept so a queue-row click can recover the full item (id +
// verdict) the filing pane needs.
let currentItems: QueueItem[] = [];

// Review mode: "detail" = one track at a time (filing pane), "batch" = triage many at once
// (board's Detail|Batch segmented control). `batchSel` holds the ticked track ids; it is
// pruned to the currently-ready set on every batch render so a filed/removed id can't linger.
let reviewMode: "detail" | "batch" = "detail";
const batchSel = new Set<number>();
// Destination bin chosen in the batch action bar (forward-slash rel; "" = library root). Kept
// across renders so the dropdown doesn't reset while triaging.
let batchBin = "";
// Cached bins for the batch destination dropdown (loaded when entering batch mode).
let batchBins: Bin[] = [];

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
    fileClearTimer = setTimeout(() => clearTask("file"), 1200);
  }
}

/** Stop button on the global zone's Filing row → request a stop-net cancel (sous-étape 3). The
 * in-flight file finishes and no new one starts; nothing is rolled back. The row shows "Stopping…"
 * until `file:done` arrives (handled by onFileBatchDone). The first click already takes effect
 * (flag set, button removed), but the only feedback used to be the small "Stopping…" at the bottom
 * of the nav rail — far from where the user clicked. While a conversion encodes, the counter is
 * frozen, so the cancel looks ignored and the user re-clicks into the void. We also drop an
 * immediate note at #filfoot (where they clicked File) so the click visibly registers right there. */
function onFileStop() {
  // INSTRUMENTATION (cancel-bug-live): prove the click reaches the handler and whether it short-circuits.
  console.log("[cancel] STOP CLICKED — fileStopping(before)=", fileStopping);
  if (fileStopping) return;
  fileStopping = true;
  console.log("[cancel] fileStopping(after)=", fileStopping);
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
  // INSTRUMENTATION (cancel-bug-live): confirm invoke("file_cancel") is actually CALLED and resolves.
  console.log("[cancel] invoking file_cancel…");
  fileCancel()
    .then(() => console.log("[cancel] file_cancel resolved"))
    .catch((e) => console.error("[cancel] file_cancel FAILED", e));
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
  // Prune ticks to the live ready set; default to all-ready selected the first time.
  const readyIds = new Set(ready.map((it) => it.id));
  for (const id of [...batchSel]) if (!readyIds.has(id)) batchSel.delete(id);
  if (batchSel.size === 0) for (const it of ready) batchSel.add(it.id);

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

  const allOn = ready.length > 0 && batchSel.size === ready.length;
  const sectionHead = (label: string, n: number, extra = "") =>
    `<div style="display:flex;align-items:center;justify-content:space-between;margin:0 0 6px"><div class="col-h" style="margin:0">${label} · ${n}</div>${extra}</div>`;

  // READY rows grouped by rail (board's lossless/lossy separation), each labelled with the real
  // output format the filer will encode to (encode::target_for). Tracks file separately by rail.
  const railGroup = (rail: "lossless" | "lossy" | "unknown") => {
    const xs = ready.filter((it) => (it.rail ?? "unknown") === rail);
    if (!xs.length) return "";
    return (
      `<div style="margin:2px 0 6px"><div style="display:flex;align-items:center;justify-content:space-between;padding:3px 9px"><span style="font-size:var(--text-xs);letter-spacing:.04em;text-transform:uppercase;color:var(--color-text-tertiary)">${railLabel(
        rail,
      )} · ${xs.length}</span><span style="font-size:var(--text-xs);color:var(--color-text-tertiary)">→ ${outputFormat(
        rail,
      )}</span></div>${xs.map(readyRow).join("")}</div>`
    );
  };
  const readyHead = sectionHead(
    "READY TO FILE",
    ready.length,
    `<span style="display:flex;gap:8px">` +
      `<button data-sift="batchall" style="font-size:var(--text-xs);padding:2px 8px;color:var(--color-text-info)">${
        allOn ? "Clear" : `Select all ${ready.length}`
      }</button>` +
      `</span>`,
  );

  mid.innerHTML =
    `<div style="display:flex;flex-direction:column;height:100%;min-height:0">` +
    `<div style="flex:1;min-height:0;overflow-y:auto;padding-right:2px">` +
    (ready.length
      ? readyHead + railGroup("lossless") + railGroup("lossy") + railGroup("unknown")
      : '<div class="col-h" style="margin:0 0 6px">READY TO FILE · 0</div><div style="font-size:var(--text-md);color:var(--color-text-tertiary);padding:4px 9px 14px">Nothing clean to file yet.</div>') +
    (review.length
      ? `<div style="margin-top:16px"></div>` +
        sectionHead("NEEDS REVIEW", review.length) +
        review.map(reviewRow).join("")
      : "") +
    `</div>` +
    // Action bar
    `<div style="flex:none;display:flex;align-items:center;gap:9px;padding:11px 2px 2px;border-top:0.5px solid var(--color-border-tertiary);margin-top:8px">` +
    binSelectHtml() +
    `<div style="flex:1"></div>` +
    `<button data-sift="batchdiscard" style="font-size:var(--text-sm);padding:7px 12px;border-radius:var(--border-radius-md);background:var(--color-background-danger);color:var(--color-text-danger)">Discard (${batchSel.size})</button>` +
    `<button data-sift="batchfile" style="font-size:var(--text-sm);font-weight:600;padding:7px 14px;border-radius:var(--border-radius-md);background:#2f6fe0;color:#e5eeff;${
      batchSel.size ? "" : "opacity:.5;pointer-events:none"
    }">File selection (${batchSel.size})</button>` +
    `</div></div>`;

  renderBatchRail(review.length);
}

/** Human label for a rail. */
function railLabel(rail: "lossless" | "lossy" | "unknown"): string {
  return rail === "lossless" ? "Lossless" : rail === "lossy" ? "Lossy" : "Unknown rail";
}

/** The output format the filer encodes to for a rail — mirrors Rust `encode::target_for`
 * (lossless → AIFF 16/44.1; lossy/unknown → MP3 320). Shown so batch filing is not a black box. */
function outputFormat(rail: "lossless" | "lossy" | "unknown"): string {
  return rail === "lossless" ? "AIFF · 16-bit · 44.1 kHz" : "MP3 320";
}

/** Destination dropdown for the batch bar — a real <select> of the user's bins (root + each
 * folder), reflecting `batchBin`. */
function binSelectHtml(): string {
  const opt = (rel: string, label: string) =>
    `<option value="${esc(rel)}"${rel === batchBin ? " selected" : ""}>${esc(label)}</option>`;
  const opts =
    opt("", "Library root") +
    batchBins.map((b) => opt(b.rel, `${"  ".repeat(Math.max(0, b.depth - 1))}${b.name}`)).join("");
  return `<select data-sift="batchbin" style="font-size:var(--text-sm);padding:6px 9px;border-radius:var(--border-radius-md);background:var(--color-background-secondary);color:var(--color-text-secondary);border:0.5px solid var(--color-border-tertiary);max-width:200px">${opts}</select>`;
}

/** Right-rail summary for batch mode (board's SELECTION / DESTINATION / WILL ENCODE / EXCLUDED).
 * Replaces the filing footer + hides the folder tree while batching. */
function renderBatchRail(reviewN: number) {
  const foot = requireEl("#filfoot", "renderBatchRail");
  const fldz = requireEl("#fldz", "renderBatchRail");
  fldz.style.display = reviewMode === "batch" ? "none" : "";
  const dest = batchBin || "Library root";
  const block = (label: string, body: string) =>
    `<div style="margin-bottom:14px"><div class="col-h" style="margin:0 0 4px">${label}</div><div style="font-size:var(--text-md);color:var(--color-text-secondary)">${body}</div></div>`;
  foot.innerHTML =
    block("Selection", `${batchSel.size} selected`) +
    block("Destination", esc(dest)) +
    block("Will file", `${batchSel.size} clean track${batchSel.size === 1 ? "" : "s"} → ${esc(dest)}`) +
    block("Excluded", `${reviewN} need review · filed safely only when clean`);
}

/** Switch between detail and batch review. On entering batch we (re)load the bins for the
 * destination dropdown; on leaving we restore the per-track filing pane. */
function setReviewMode(m: "detail" | "batch") {
  reviewMode = m;
  ensureReviewSeg();
  const fldz = requireEl("#fldz", "setReviewMode");
  fldz.style.display = m === "batch" ? "none" : "";
  if (m === "batch") {
    listBins()
      .then((b) => {
        batchBins = b;
        renderBatch();
      })
      .catch((err) => {
        console.error("listBins failed", err);
        batchBins = [];
        renderBatch();
      });
  } else {
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
  fileNote(
    '<i class="ti ti-loader sift-spin" style="font-size:var(--text-md);vertical-align:-1px"></i> Filing in the background…',
  );
  try {
    // Resolves as soon as the background task STARTS; the summary comes via file:done.
    await fileBatch(ids, batchBin);
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
  if (res.cancelled) {
    // Stop-net end: no 100% done-flash came from progress (done<total). Flash the partial then hide.
    clearTimeout(fileClearTimer);
    const lp = lastFileProgress;
    if (lp) {
      setTask("file", { done: lp.done, total: lp.total, state: "done" });
      fileClearTimer = setTimeout(() => clearTask("file"), 1200);
    } else {
      clearTask("file");
    }
  }
  const base = res.needs_validation.length
    ? `${res.filed} filed · ${res.needs_validation.length} need validation`
    : `${res.filed} filed`;
  // Refresh the view, then post the run summary at #filfoot. It MUST go after refresh so it survives
  // renderBatch's wholesale rail rebuild (renderBatchRail sets #filfoot.innerHTML) — but in a
  // `finally`, because a rejecting refresh (renderHomeSources/renderQueue use requireEl) would
  // otherwise skip it and strand the "Filing in the background…" spinner note. The summary must
  // always replace that note on file:done, exactly as the sidebar row clears on file:done.
  try {
    await refresh();
  } finally {
    fileNote(
      `<i class="ti ti-check" style="font-size:var(--text-md);vertical-align:-1px"></i> ${
        res.cancelled ? `Filing cancelled · ${base}` : base
      }`,
      "var(--color-text-success)",
    );
  }
}

/** Send every ticked track to Écartés for re-sourcing (backend emits queue:changed → redraw). */
async function runBatchDiscard() {
  const ids = [...batchSel];
  if (ids.length === 0) return;
  try {
    await rejectBatch(ids);
    batchSel.clear();
  } catch (err) {
    console.error("reject_batch failed", err);
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
      if (batchSel.size === ready.length) batchSel.clear();
      else for (const it of ready) batchSel.add(it.id);
      renderBatch();
    } else if (act === "batchopen") {
      e.stopPropagation();
      const id = Number(el.dataset.id);
      const item = currentItems.find((it) => it.id === id);
      setReviewMode("detail");
      const mid = requireEl("#mid", "batchopen");
      if (item && mid) void openFilingInto(mid, item);
    } else if (act === "batchfile") {
      e.stopPropagation();
      void runBatchFile();
    } else if (act === "batchdiscard") {
      e.stopPropagation();
      void runBatchDiscard();
    }
  });

  // Destination dropdown (batch bar) — a <select>, so it needs change, not click.
  requireEl("#pa", "installLiveWiring").addEventListener("change", (e) => {
    const sel = (e.target as HTMLElement).closest<HTMLSelectElement>('select[data-sift="batchbin"]');
    if (sel) {
      batchBin = sel.value;
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
