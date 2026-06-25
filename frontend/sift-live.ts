// Live data wiring — ACTIVE ONLY inside the Tauri app. In a plain browser the hooks
// below are never installed, so app.js keeps its mockup (Vercel demo unaffected).
import {
  addSource,
  listSources,
  removeSource,
  listQueue,
  listBins,
  fileBatch,
  rejectBatch,
  onQueueChanged,
  onAnalysisChanged,
  analysisProgress,
  setSourceWatched,
  importPaths,
  listEcartes,
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
  EcarteItem,
  LibraryTrack,
  LibraryFacets,
  LibraryFilter,
} from "../shared/contracts";
import { openLibraryDetailInto } from "./library-detail";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  openFilingInto,
  refreshBins,
  syncDetail,
  installUndoShortcut,
  installFilingKeys,
} from "./filing";
import type { Source, QueueItem, Bin } from "../shared/contracts";

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

/** Replaces app.js's mockup "Dossiers surveillés" block with real sources + warning. */
async function renderHomeSources() {
  const content = document.getElementById("content");
  if (!content) return;
  let sources: Source[] = [];
  try {
    sources = await listSources();
  } catch (e) {
    console.error("listSources failed", e);
    return;
  }

  document.getElementById("sift-sources")?.remove();

  const rows = sources
    .map((s) => {
      const warn = s.accessible
        ? ""
        : ' <span style="color:var(--color-text-danger);font-size:11px">⚠ inaccessible</span>';
      const watch = `<span class="tog${s.watched ? "" : " off"}" data-sift="togglewatch" data-id="${
        s.id
      }" data-watched="${s.watched ? "1" : "0"}" title="${
        s.watched ? "Watching — click to pause" : "Paused — click to watch"
      }"></span>`;
      const count = s.pending_count
        ? `${s.pending_count} new`
        : "up to date";
      const countColor = s.pending_count ? "var(--color-text-info)" : "var(--color-text-tertiary)";
      return `<div class="srow"><span class="v"><i class="ti ti-folder"></i> ${esc(
        s.path,
      )}${warn}</span><span style="display:flex;align-items:center;gap:9px"><span style="font-size:11px;color:${countColor}">${count}</span>${watch}<button data-sift="rmsrc" data-id="${s.id}" style="font-size:11px;padding:2px 7px;color:var(--color-text-danger)">remove</button></span></div>`;
    })
    .join("");

  const panel = document.createElement("div");
  panel.id = "sift-sources";
  panel.innerHTML =
    '<div class="col-h" style="margin-top:12px">Watched folders</div>' +
    '<div style="display:flex;gap:8px;align-items:flex-start;background:var(--color-background-warning);border-radius:var(--border-radius-md);padding:8px 11px;margin:0 0 8px;font-size:11px;color:var(--color-text-warning)"><i class="ti ti-info-circle" style="font-size:14px;flex:none"></i><span>Point Sift at your <strong>Completed</strong> folder (not <em>Incomplete</em>) — files still downloading shouldn\'t enter the queue.</span></div>' +
    (rows || '<div style="font-size:12px;color:var(--color-text-tertiary)">No watched folder.</div>') +
    '<div style="margin:8px 0 0"><button data-sift="addsrc"><i class="ti ti-plus" style="font-size:13px;vertical-align:-2px"></i> add a folder</button></div>';

  // Hide the WHOLE mockup "Dossiers surveillés" block (its hardcoded counts never change):
  // the .col-h header + every following sibling up to the next .col-h. Insert the real
  // panel in its place.
  const left = content.querySelector(".home-left");
  if (!left) return;
  // Lean Tauri UI: keep only the page title + the real sources panel; hide all the mock
  // home content (fictional stat cards, pending banner, per-folder breakdown).
  let title: Element | null = null;
  for (const child of Array.from(left.children)) {
    if (!title && child.classList.contains("h1")) {
      title = child;
      continue;
    }
    (child as HTMLElement).style.display = "none";
  }
  left.insertBefore(panel, title ? title.nextSibling : left.firstChild);
}

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
  let progressHtml = "";
  try {
    const p = await analysisProgress();
    if (p.total > 0) {
      const pct = Math.round((p.done / p.total) * 100);
      const label =
        p.done >= p.total
          ? `${p.total} analyzed`
          : `${p.done} / ${p.total} analyzed`;
      progressHtml = `<div style="margin:0 0 8px"><div style="display:flex;justify-content:space-between;font-size:11px;color:var(--color-text-tertiary);margin-bottom:3px"><span>Background analysis</span><span>${label}</span></div><div style="height:4px;border-radius:2px;background:rgba(237,233,224,.12);overflow:hidden"><div style="height:100%;width:${pct}%;background:var(--color-text-info,#8ecce8);transition:width .3s"></div></div></div>`;
    }
  } catch (e) {
    console.error("analysisProgress failed", e);
  }

  ql.innerHTML =
    progressHtml +
    (items
      .map(
        (it) =>
          `<div class="qi" data-id="${it.id}" data-path="${esc(it.path)}" title="Listen and file" style="display:flex;align-items:center;gap:8px;cursor:pointer">${verdictDot(
            it.verdict,
          )}<span style="flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis">${esc(
            it.filename || it.path,
          )}</span>${
            it.dup
              ? '<i class="ti ti-copy" title="Possible duplicate (same name)" style="flex:none;font-size:12px;color:var(--color-text-secondary)"></i>'
              : ""
          }<i class="ti ti-chevron-right" style="flex:none;color:var(--color-text-tertiary);font-size:14px"></i></div>`,
      )
      .join("") ||
      '<div style="font-size:12px;color:var(--color-text-tertiary);padding:6px 4px">Queue empty.</div>');

  // Live destination bins + neutral detail prompt (replace the mockup's hardcoded ones).
  const fldz = document.getElementById("fldz");
  if (fldz) void refreshBins(fldz);
  // Only sync the detail pane on structural changes (nav, queue add/remove/file). A background
  // ANALYSIS finishing must NOT re-open / switch the open track — that thrashes and aborts the
  // player's audio load (waveform shows from peaks, but no sound). See touchDetail=false caller.
  if (touchDetail) {
    if (reviewMode === "batch") {
      renderBatch();
    } else {
      const mid = document.getElementById("mid");
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

/** Detail|Batch segmented control (board `topseg`), injected once at the top of the queue
 * column. Owned here (not app.js) so it works inside Tauri where the live wiring renders the
 * Revue. Reflects `reviewMode`; clicks are handled in the #pa delegate. */
function ensureReviewSeg() {
  const qcol = document.getElementById("qcol");
  if (!qcol) return;
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
    return `<button data-sift="reviewmode" data-m="${m}" style="flex:1;display:inline-flex;align-items:center;justify-content:center;gap:5px;padding:5px 0;border:none;border-radius:6px;font-size:11px;font-weight:${
      on ? 600 : 400
    };cursor:pointer;background:${
      on ? "rgba(255,255,255,.07)" : "transparent"
    };color:var(--color-text-${on ? "primary" : "tertiary"})"><i class="ti ${icon}" style="font-size:13px"></i>${label}</button>`;
  };
  seg.innerHTML = tab("detail", "Detail", "ti-layout-list") + tab("batch", "Batch", "ti-table");
}

/** Batch triage view (board's Batch screen): splits the queue into a checkable READY list and a
 * read-only NEEDS REVIEW list, with a destination dropdown + File/Discard action bar (center,
 * #mid) and a selection summary rail (#filfoot). Every control is bound to a real command
 * (`fileBatch` / `rejectBatch`); nothing here is mocked. */
function renderBatch() {
  const mid = document.getElementById("mid");
  if (!mid) return;
  const ready = currentItems.filter((it) => it.verdict === "ok");
  const review = currentItems.filter((it) => it.verdict !== "ok");
  // Prune ticks to the live ready set; default to all-ready selected the first time.
  const readyIds = new Set(ready.map((it) => it.id));
  for (const id of [...batchSel]) if (!readyIds.has(id)) batchSel.delete(id);
  if (batchSel.size === 0) for (const it of ready) batchSel.add(it.id);

  const name = (it: QueueItem) => esc(it.filename || it.path);
  const readyRow = (it: QueueItem) => {
    const on = batchSel.has(it.id);
    return (
      `<div class="bx-row" data-sift="batchpick" data-id="${it.id}" style="display:flex;align-items:center;gap:9px;padding:7px 9px;border-radius:var(--border-radius-md);cursor:pointer;${
        on ? "background:rgba(255,255,255,.045)" : ""
      }">` +
      `<span class="bx-ck" style="flex:none;width:15px;height:15px;border-radius:4px;display:inline-flex;align-items:center;justify-content:center;border:1.5px solid ${
        on ? "var(--color-text-success)" : "var(--color-border-secondary)"
      };background:${on ? "var(--color-text-success)" : "transparent"}">${
        on ? '<i class="ti ti-check" style="font-size:10px;color:#1a1a18"></i>' : ""
      }</span>` +
      verdictDot(it.verdict) +
      `<span style="flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;font-size:12px">${name(it)}</span>` +
      (it.dup
        ? '<span style="flex:none;font-size:9px;font-weight:600;letter-spacing:.03em;padding:2px 7px;border-radius:999px;background:var(--color-background-warning);color:var(--color-text-warning)">DUPLICATE</span>'
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
      `<span style="flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;font-size:12px;color:var(--color-text-secondary)">${name(it)}</span>` +
      (it.dup
        ? '<span style="flex:none;font-size:9px;font-weight:600;padding:2px 7px;border-radius:999px;background:var(--color-background-warning);color:var(--color-text-warning)">DUP</span>'
        : "") +
      `<span style="flex:none;font-size:9px;font-weight:600;letter-spacing:.03em;padding:2px 7px;border-radius:999px;background:${tone[1]};color:${tone[2]}">${tone[0]}</span>` +
      `<button data-sift="batchopen" data-id="${it.id}" style="flex:none;font-size:10px;padding:2px 8px;color:var(--color-text-info)">open in Detail</button>` +
      `</div>`
    );
  };

  const allOn = ready.length > 0 && batchSel.size === ready.length;
  const sectionHead = (label: string, n: number, extra = "") =>
    `<div style="display:flex;align-items:center;justify-content:space-between;margin:0 0 6px"><div class="col-h" style="margin:0">${label} · ${n}</div>${extra}</div>`;

  mid.innerHTML =
    `<div style="display:flex;flex-direction:column;height:100%;min-height:0">` +
    `<div style="flex:1;min-height:0;overflow-y:auto;padding-right:2px">` +
    (ready.length
      ? sectionHead(
          "READY TO FILE",
          ready.length,
          `<button data-sift="batchall" style="font-size:10px;padding:2px 8px;color:var(--color-text-info)">${
            allOn ? "Clear" : `Select all ${ready.length}`
          }</button>`,
        ) + ready.map(readyRow).join("")
      : '<div class="col-h" style="margin:0 0 6px">READY TO FILE · 0</div><div style="font-size:12px;color:var(--color-text-tertiary);padding:4px 9px 14px">Nothing clean to file yet.</div>') +
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
    `<button data-sift="batchdiscard" style="font-size:11px;padding:7px 12px;border-radius:var(--border-radius-md);background:var(--color-background-danger);color:var(--color-text-danger)">Discard (${batchSel.size})</button>` +
    `<button data-sift="batchfile" style="font-size:11px;font-weight:600;padding:7px 14px;border-radius:var(--border-radius-md);background:#2f6fe0;color:#e5eeff;${
      batchSel.size ? "" : "opacity:.5;pointer-events:none"
    }">File selection (${batchSel.size})</button>` +
    `</div></div>`;

  renderBatchRail(review.length);
}

/** Destination dropdown for the batch bar — a real <select> of the user's bins (root + each
 * folder), reflecting `batchBin`. */
function binSelectHtml(): string {
  const opt = (rel: string, label: string) =>
    `<option value="${esc(rel)}"${rel === batchBin ? " selected" : ""}>${esc(label)}</option>`;
  const opts =
    opt("", "Library root") +
    batchBins.map((b) => opt(b.rel, `${"  ".repeat(Math.max(0, b.depth - 1))}${b.name}`)).join("");
  return `<select data-sift="batchbin" style="font-size:11px;padding:6px 9px;border-radius:var(--border-radius-md);background:var(--color-background-secondary);color:var(--color-text-secondary);border:0.5px solid var(--color-border-tertiary);max-width:200px">${opts}</select>`;
}

/** Right-rail summary for batch mode (board's SELECTION / DESTINATION / WILL ENCODE / EXCLUDED).
 * Replaces the filing footer + hides the folder tree while batching. */
function renderBatchRail(reviewN: number) {
  const foot = document.getElementById("filfoot");
  const fldz = document.getElementById("fldz");
  if (fldz) fldz.style.display = reviewMode === "batch" ? "none" : "";
  if (!foot) return;
  const dest = batchBin || "Library root";
  const block = (label: string, body: string) =>
    `<div style="margin-bottom:14px"><div class="col-h" style="margin:0 0 4px">${label}</div><div style="font-size:12px;color:var(--color-text-secondary)">${body}</div></div>`;
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
  const fldz = document.getElementById("fldz");
  if (fldz) fldz.style.display = m === "batch" ? "none" : "";
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

/** File every ticked (green) track into the chosen bin. The backend emits queue:changed, which
 * drives the redraw; we only surface the count + any tracks it bounced back for validation. */
async function runBatchFile() {
  const ids = [...batchSel];
  if (ids.length === 0) return;
  try {
    const res = await fileBatch(ids, batchBin);
    for (const id of ids) if (!res.needs_validation.includes(id)) batchSel.delete(id);
    const foot = document.getElementById("filfoot");
    if (foot) {
      const msg = res.needs_validation.length
        ? `${res.filed} filed · ${res.needs_validation.length} need validation`
        : `${res.filed} filed`;
      foot.insertAdjacentHTML(
        "afterbegin",
        `<div style="font-size:11px;color:var(--color-text-success);margin-bottom:10px"><i class="ti ti-check" style="font-size:12px;vertical-align:-1px"></i> ${msg}</div>`,
      );
    }
  } catch (err) {
    console.error("file_batch failed", err);
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

/** Reason pill for an écarté track (truncated → tronqué, fake → faux, else à re-sourcer). */
function ecReason(it: EcarteItem): string {
  if (it.truncated)
    return '<span class="pill" style="background:var(--color-background-warning);color:var(--color-text-warning);flex:none"><i class="ti ti-cut" style="font-size:9px"></i> truncated</span>';
  if (it.verdict === "fake")
    return '<span class="pill" style="background:var(--color-background-danger);color:var(--color-text-danger);flex:none"><i class="ti ti-alert-triangle" style="font-size:9px"></i> fake</span>';
  return '<span class="pill" style="background:var(--color-background-danger);color:var(--color-text-danger);flex:none"><i class="ti ti-alert-circle" style="font-size:9px"></i> to re-source</span>';
}

/** The "Artiste Titre" string to paste into Soulseek (single space; no dash). */
function ecSlsk(it: EcarteItem): string {
  if (it.artist && it.title) return `${it.artist} ${it.title}`;
  return (it.filename || it.path).replace(/\.[^.]+$/, "");
}

// Buy-link stores: a search URL built from the track's query (q is already encoded).
const EC_STORES: [string, (q: string) => string][] = [
  ["Beatport", (q) => `https://www.beatport.com/search?q=${q}`],
  ["Traxsource", (q) => `https://www.traxsource.com/search?term=${q}`],
  ["Juno", (q) => `https://www.junodownload.com/search/?q%5Ball%5D%5B%5D=${q}`],
  ["Bandcamp", (q) => `https://bandcamp.com/search?q=${q}`],
  ["Amazon", (q) => `https://www.amazon.fr/s?k=${q}&i=digital-music`],
  ["Apple Music", (q) => `https://music.apple.com/fr/search?term=${q}`],
];

/** Buy-link row for a track: store names that open a search in the default browser. */
function ecStoreLinks(it: EcarteItem): string {
  const q = encodeURIComponent(ecSlsk(it));
  return EC_STORES.map(
    ([label, fn]) =>
      `<a data-ec="store" data-url="${encodeURIComponent(fn(q))}" style="font-size:10px;color:var(--color-text-info);cursor:pointer;text-decoration:none;white-space:nowrap">${label}</a>`,
  ).join('<span style="color:var(--color-border-secondary);margin:0 3px">·</span>');
}

/** Live Écartés view: replaces #content with the real rejected (à re-sourcer) + trashed
 * tracks. Soulseek copy + send-to-bin / restore / empty-bin wired via the #pa handler. */
async function renderEcartes() {
  const content = document.getElementById("content");
  if (!content) return;
  let items: EcarteItem[] = [];
  try {
    items = await listEcartes();
  } catch (e) {
    console.error("listEcartes failed", e);
    return;
  }
  const res = items.filter((i) => i.status === "resourcing");
  const trash = items.filter((i) => i.status === "trash");
  const name = (it: EcarteItem) =>
    esc(it.artist && it.title ? `${it.artist} — ${it.title}` : it.filename || it.path);
  const fileLine = (it: EcarteItem) =>
    `<div style="font-size:10px;color:var(--color-text-tertiary);font-family:var(--font-mono);white-space:nowrap;overflow:hidden;text-overflow:ellipsis">${esc(
      it.filename || it.path,
    )}</div>`;

  const resRows = res
    .map(
      (it) =>
        `<div style="padding:7px 4px;border-bottom:0.5px solid var(--color-border-tertiary)"><div style="display:flex;align-items:center;gap:7px"><div style="flex:1;min-width:0"><div style="white-space:nowrap;overflow:hidden;text-overflow:ellipsis;font-size:12px;font-weight:500">${name(
          it,
        )}</div>${fileLine(it)}</div>${ecReason(
          it,
        )}<button class="lk" data-ec="requeue" data-id="${it.id}" title="Put back in the queue"><i class="ti ti-arrow-back-up" style="font-size:13px;color:var(--color-text-tertiary)"></i></button><button class="lk" data-ec="trash" data-id="${it.id}" title="Send to trash"><i class="ti ti-trash" style="font-size:12px;color:var(--color-text-tertiary)"></i></button></div><div style="margin-top:5px;display:flex;flex-wrap:wrap;align-items:center;gap:4px"><button data-ec="slsk" data-q="${esc(
          ecSlsk(it),
        )}" title="Copy 'Artist Title' to search on Soulseek" style="font-size:10px;padding:2px 7px;color:var(--color-text-secondary)"><i class="ti ti-copy" style="font-size:10px;vertical-align:-1px"></i> Copy name</button><span style="color:var(--color-border-secondary)">·</span>${ecStoreLinks(
          it,
        )}</div></div>`,
    )
    .join("");

  const trashRows = trash
    .map(
      (it) =>
        `<div style="display:flex;align-items:center;gap:7px;padding:7px 4px;border-bottom:0.5px solid var(--color-border-tertiary)"><div style="flex:1;min-width:0"><div style="white-space:nowrap;overflow:hidden;text-overflow:ellipsis;font-size:12px">${name(
          it,
        )}</div>${fileLine(it)}</div><button data-ec="restore" data-id="${it.id}" style="font-size:10px;padding:2px 8px;color:var(--color-text-info)">restore</button></div>`,
    )
    .join("");

  content.innerHTML =
    '<div class="h1">Discarded</div>' +
    '<div style="display:flex;gap:7px;margin-bottom:12px;flex-wrap:wrap;align-items:center">' +
    `<span class="pill" style="background:var(--color-background-danger);color:var(--color-text-danger)"><i class="ti ti-alert-circle" style="font-size:10px"></i> ${res.length} to re-source</span>` +
    `<span class="pill"><i class="ti ti-trash" style="font-size:10px"></i> ${trash.length} in trash</span>` +
    (trash.length
      ? `<button data-ec="purge" style="font-size:10px;padding:2px 8px;color:var(--color-text-danger)">Empty trash (${trash.length})</button>`
      : "") +
    "</div>" +
    (res.length ? `<div class="col-h">To re-source</div>${resRows}` : "") +
    (trash.length ? `<div class="col-h" style="margin-top:14px">Trash</div>${trashRows}` : "") +
    (items.length === 0
      ? '<div style="font-size:12px;color:var(--color-text-tertiary)">No discarded file.</div>'
      : "");
}

async function pickAndAddFolder() {
  const dir = await open({ directory: true, multiple: false });
  if (typeof dir === "string") {
    try {
      await addSource(dir);
      await refresh();
    } catch (e) {
      console.error("addSource failed", e);
    }
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
  const badge = document.querySelector<HTMLElement>('.nav-badge[data-badge="revue"]');
  if (!badge) return;
  try {
    const n = (await listQueue()).length;
    badge.textContent = n ? String(n) : "";
  } catch {
    /* leave the badge as-is on a transient failure */
  }
}

// One-time style: while dragging, an existing zone gets an outline + an overlaid hint
// (::after with the zone's data-dz text). No permanent dashed box — the hint shows only
// during a drag, on the real folder/queue boxes, saving space.
function ensureDropStyle() {
  if (document.getElementById("sift-dz-style")) return;
  const s = document.createElement("style");
  s.id = "sift-dz-style";
  s.textContent =
    ".sift-dz-on{position:relative;outline:1.5px dashed var(--color-text-info);outline-offset:-4px;border-radius:var(--border-radius-md)}" +
    ".sift-dz-on::after{content:attr(data-dz);position:absolute;inset:0;display:flex;align-items:center;justify-content:center;text-align:center;padding:10px;font-size:11px;color:var(--color-text-info);background:rgba(20,20,24,.55);border-radius:var(--border-radius-md);pointer-events:none;z-index:50}";
  document.head.appendChild(s);
}

// Existing boxes that double as drop targets, with the hint each shows while dragging.
// ".dest" is the WHOLE "Où on va" column (header + #fldz) so a folder dropped anywhere in
// that column registers as a destination — not just on the inner bin list.
const DROP_ZONES: [string, string][] = [
  [".dest", "Drop a folder here — new destination"],
  ["#ql", "Drop audio files here"],
  ["#sift-sources", "Drop a folder to watch"],
];

/** Toggle the drag hint/outline on the relevant existing boxes. Falls back to #content
 * (e.g. Bibliothèque) when none of the named zones are on screen. */
function setDropActive(on: boolean) {
  ensureDropStyle();
  document.querySelectorAll<HTMLElement>(".sift-dz-on").forEach((el) => {
    el.classList.remove("sift-dz-on");
    el.removeAttribute("data-dz");
  });
  if (!on) return;
  const present = DROP_ZONES.filter(([sel]) => document.querySelector(sel));
  const targets: [string, string][] = present.length
    ? present
    : [["#content", "Drop files (→ queue) or folders (→ watched)"]];
  for (const [sel, label] of targets) {
    const el = document.querySelector<HTMLElement>(sel);
    if (el) {
      el.classList.add("sift-dz-on");
      el.dataset.dz = label;
    }
  }
}

/** "dest" when the cursor is over the bins column (#fldz), else "source". Tauri 2 emits the
 * drop position already in logical (CSS) pixels — exactly what elementFromPoint expects, so
 * no devicePixelRatio correction (dividing here double-corrected on HiDPI/scaled displays). */
function dropModeAt(pos: { x: number; y: number }): "source" | "dest" {
  const el = document.elementFromPoint(pos.x, pos.y);
  return el && el.closest(".dest") ? "dest" : "source";
}

/** OS drag-drop: audio files → queue; folders → watched source, or a destination bin when
 * dropped on the "Où on va" column. */
async function installDragDrop() {
  try {
    await getCurrentWebview().onDragDropEvent((ev) => {
      const p = ev.payload;
      if (p.type === "drop") {
        setDropActive(false);
        if (p.paths.length)
          void importPaths(p.paths, dropModeAt(p.position)).catch((e) =>
            console.error("import_paths failed", e),
          );
      } else if (p.type === "enter" || p.type === "over") {
        setDropActive(true);
      } else {
        setDropActive(false);
      }
    });
  } catch (e) {
    console.error("drag-drop init failed", e);
  }
}

/** Lean Tauri UI: hide the mockup's not-yet-real surfaces (nav tabs + Revue toggles) so the
 * app shows only what actually works — Accueil (sources) and Revue (queue/report/filing).
 * Injected once; the demo (plain browser) never runs this, so its full mockup is untouched. */
function injectLeanStyle() {
  if (document.getElementById("sift-lean-style")) return;
  const st = document.createElement("style");
  st.id = "sift-lean-style";
  st.textContent =
    // landing/demo copy in index.html: marketing pitch, demo disclaimer, feature cards row
    ".pitch,.sub,.frow{display:none!important}" +
    // unbuilt nav tabs (Rekordbox, Clé USB = M7) + their now-empty EXPORT group header —
    // Accueil/Revue/Écartés/Réglages/Bibliothèque are live
    '#nav .nv[data-view="rkb"],#nav .nv[data-view="cle"],#nav .nv-grp[data-grp="export"]{display:none!important}' +
    // Revue: batch mode + "traités" toggle aren't wired to the real backend yet
    '[data-act="revmode"],[data-act="togglequeue"]{display:none!important}' +
    // custom frameless titlebar (decorations are off in tauri.conf — Tauri only)
    "#sift-titlebar{height:30px;flex:none;display:flex;align-items:center;justify-content:space-between;" +
    "background:var(--color-background-tertiary);-webkit-user-select:none;user-select:none}" +
    "#sift-tb-title{padding-left:13px;font-size:11px;letter-spacing:.04em;color:var(--color-text-tertiary)}" +
    "#sift-tb-controls{display:flex;height:100%}" +
    ".sift-win{width:44px;height:100%;display:flex;align-items:center;justify-content:center;border:none;" +
    "background:transparent;color:var(--color-text-tertiary);cursor:pointer;border-radius:0;padding:0}" +
    ".sift-win:hover{background:var(--color-background-secondary);color:var(--color-text-primary)}" +
    ".sift-win-close:hover{background:#e81123;color:#fff}.sift-win i{font-size:15px}" +
    // make room for the 30px bar: shrink the app shell so nothing is clipped
    ".wrap{height:calc(100vh - 30px)!important}";
  document.head.appendChild(st);
}

/** Inject the custom window titlebar (the native one is off via decorations:false) and wire
 * its minimise / maximise / close buttons. The bar + its title are drag regions. */
function injectTitlebar() {
  if (document.getElementById("sift-titlebar")) return;
  const bar = document.createElement("div");
  bar.id = "sift-titlebar";
  bar.setAttribute("data-tauri-drag-region", "");
  bar.innerHTML =
    '<span id="sift-tb-title" data-tauri-drag-region>Sift</span>' +
    '<div id="sift-tb-controls">' +
    '<button class="sift-win" data-win="min" title="Minimize"><i class="ti ti-minus"></i></button>' +
    '<button class="sift-win" data-win="max" title="Maximize"><i class="ti ti-square"></i></button>' +
    '<button class="sift-win sift-win-close" data-win="close" title="Close"><i class="ti ti-x"></i></button>' +
    "</div>";
  document.body.insertBefore(bar, document.body.firstChild);

  const w = getCurrentWindow();
  bar.querySelectorAll<HTMLElement>(".sift-win").forEach((b) =>
    b.addEventListener("click", () => {
      const act = b.dataset.win;
      if (act === "min") void w.minimize();
      else if (act === "max") void w.toggleMaximize();
      else if (act === "close") void w.close();
    }),
  );
}

/** Reveal a scroll area's thumb while it scrolls, then hide it ~700ms after it stops (the
 * CSS keeps it hidden at rest). Capture-phase so it catches scrolling on any inner element. */
function installScrollAutohide() {
  const timers = new WeakMap<Element, ReturnType<typeof setTimeout>>();
  document.addEventListener(
    "scroll",
    (e) => {
      const el = e.target;
      if (!(el instanceof Element)) return;
      el.classList.add("sift-scrolling");
      const prev = timers.get(el);
      if (prev) clearTimeout(prev);
      timers.set(
        el,
        setTimeout(() => el.classList.remove("sift-scrolling"), 700),
      );
    },
    true,
  );
}

/** Live Réglages view: injects the real Discogs token field below the mockup rows. */
async function renderReglagesLive() {
  const content = document.getElementById("content");
  if (!content) return;

  // Remove any previous live-settings block so we don't duplicate on re-render.
  document.getElementById("sift-reglages-live")?.remove();

  let token: string | null = null;
  try {
    token = await getSetting("discogs_token");
  } catch (e) {
    console.error("getSetting(discogs_token) failed", e);
  }

  const inputCss =
    "font-size:12px;padding:4px 7px;background:var(--color-background-secondary);" +
    "border:0.5px solid var(--color-border-tertiary);border-radius:var(--border-radius-md);" +
    "color:var(--color-text-primary);width:100%;font-family:var(--font-mono)";

  const block = document.createElement("div");
  block.id = "sift-reglages-live";
  block.style.cssText = "margin-top:14px";
  block.innerHTML =
    '<div class="col-h">Discogs</div>' +
    '<div class="srow" style="flex-direction:column;align-items:flex-start;gap:6px;padding-bottom:10px">' +
    '<div style="display:flex;align-items:center;justify-content:space-between;width:100%">' +
    '<span style="font-size:12px">Authentication token</span>' +
    '<a id="sift-discogs-link" style="font-size:11px;color:var(--color-text-info);cursor:pointer;text-decoration:none">' +
    '<i class="ti ti-external-link" style="font-size:11px;vertical-align:-1px"></i> get a token</a>' +
    "</div>" +
    `<input id="sift-discogs-token" type="text" placeholder="Discogs token…" value="${esc(token ?? "")}" style="${inputCss}">` +
    '<div id="sift-discogs-status" style="font-size:11px;color:var(--color-text-tertiary);min-height:14px"></div>' +
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
  const content = document.getElementById("content");
  if (!content) return;
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
          `<div class="fld${activeFacetVal === b.name ? " on" : ""}" data-bib="pick" data-key="${sideKey}" data-val="${esc(b.name)}" style="justify-content:space-between"><span>${esc(b.name)}</span><span style="font-size:11px;opacity:.7">${b.count}</span></div>`,
      )
      .join("");

  const rows = bibState.tracks
    .map((t) => {
      const name = esc(t.artist && t.title ? `${t.artist} — ${t.title}` : t.path.split(/[\\/]/).pop() || t.path);
      const link = t.discogs_release_id
        ? `<button class="lk" data-bib="link" data-rid="${esc(t.discogs_release_id)}" aria-label="Discogs page"><i class="ti ti-external-link" style="font-size:13px;color:var(--color-text-tertiary)"></i></button>`
        : `<button class="lk" data-bib="identify" data-id="${t.id}" aria-label="Identify"><i class="ti ti-search" style="font-size:12px;color:var(--color-text-tertiary)"></i></button>`;
      return `<div class="lr" data-bib="row" data-id="${t.id}"><button class="pb" data-bib="play" data-id="${t.id}" aria-label="Listen"><i class="ti ti-player-play" style="font-size:12px"></i></button><span class="bib-name" style="flex:1;min-width:0;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">${name}</span>${verdictBadge(t.verdict)}${qualPill(t)}<span style="flex:none;width:40px;text-align:right;font-family:var(--font-mono);color:var(--color-text-tertiary)">${fmtDur(t.duration)}</span>${link}</div>`;
    })
    .join("");

  const header =
    `<div style="display:flex;align-items:center;gap:8px;margin-bottom:10px">` +
    `<div style="flex:1;display:flex;align-items:center;gap:7px;border:0.5px solid var(--color-border-secondary);border-radius:var(--border-radius-md);padding:6px 10px"><i class="ti ti-search" style="font-size:14px;color:var(--color-text-tertiary)"></i><input id="bibq" placeholder="Search…" value="${esc(bibState.filter.q || "")}" style="flex:1;border:0;background:transparent;color:inherit;font-size:12px;outline:none"></div>` +
    chips +
    `</div>`;

  content.innerHTML =
    header +
    `<div style="display:flex;gap:14px"><div style="width:150px;flex:none"><div class="col-h">Library</div>${side}</div>` +
    `<div style="flex:1;min-width:0"><div style="display:flex;justify-content:space-between;margin-bottom:5px"><span style="font-size:13px;font-weight:500">${esc(activeFacetVal || "All")}</span><span style="font-size:11px;color:var(--color-text-tertiary)">${bibState.tracks.length} tracks</span></div>` +
    (rows || `<div style="font-size:12px;color:var(--color-text-tertiary)">No filed track.</div>`) +
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
  const host = document.getElementById("bibplayer");
  if (!t || !host) return;
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

  document.getElementById("pa")?.addEventListener("click", (e) => {
    // queue item → open the live filing pane (report + editor + actions) in #mid
    const qi = (e.target as HTMLElement).closest<HTMLElement>(".qi[data-id]");
    if (qi?.dataset.id) {
      e.stopPropagation();
      // In batch mode a row-click means "inspect this one" → drop back to the detail pane.
      if (reviewMode === "batch") setReviewMode("detail");
      const id = Number(qi.dataset.id);
      const item = currentItems.find((it) => it.id === id);
      const mid = document.getElementById("mid");
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
        ec.innerHTML = '<i class="ti ti-check" style="font-size:10px;vertical-align:-1px"></i> Copied';
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
      void pickAndAddFolder();
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
      const mid = document.getElementById("mid");
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
  document.getElementById("pa")?.addEventListener("change", (e) => {
    const sel = (e.target as HTMLElement).closest<HTMLSelectElement>('select[data-sift="batchbin"]');
    if (sel) {
      batchBin = sel.value;
      renderBatchRail(currentItems.filter((it) => it.verdict !== "ok").length);
    }
  });

  void onQueueChanged(refresh);

  // Analysis pings can arrive several times per second — debounce the queue redraw.
  let t: ReturnType<typeof setTimeout> | undefined;
  void onAnalysisChanged(() => {
    // A report may have changed (re-analysed / replaced file) → drop the in-session cache so
    // the next open re-fetches from the DB (the source of truth) instead of serving it stale.
    void import("./report-view").then((m) => m.clearReportCache());
    clearTimeout(t);
    // touchDetail=false: redraw the queue list + progress only; never re-open the open track
    // (that aborts the player's audio load).
    t = setTimeout(() => void renderQueue(false), 300);
  });

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
