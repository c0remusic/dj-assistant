// Live data wiring — ACTIVE ONLY inside the Tauri app. In a plain browser the hooks
// below are never installed, so app.js keeps its mockup (Vercel demo unaffected).
import {
  addSource,
  listSources,
  removeSource,
  listQueue,
  onQueueChanged,
  onAnalysisChanged,
  analysisProgress,
  analyzePath,
  setSourceWatched,
} from "./ipc";
import { open } from "@tauri-apps/plugin-dialog";
import { renderReportInto } from "./report-view";
import type { Source, QueueItem, AnalysisReport } from "../shared/contracts";

// The currently-opened track in the Revue #mid pane (survives re-renders).
let selectedPath: string | null = null;
let selectedReport: AnalysisReport | null = null;

const MID_PLACEHOLDER =
  '<div style="flex:1;display:flex;flex-direction:column;align-items:center;justify-content:center;text-align:center;gap:6px;color:var(--color-text-tertiary);padding:20px"><i class="ti ti-click" style="font-size:24px"></i><div style="font-size:12px">Sélectionne un morceau dans la file<br>pour voir son analyse.</div></div>';

/** Owns the #mid pane in Tauri mode: shows the selected real report, or a placeholder —
 * never the mockup player/Discogs detail. Restores after the mockup's renderMid clobbers it. */
function paintMid(items: QueueItem[]) {
  const mid = document.getElementById("mid");
  if (!mid) return;
  // selection gone from the queue (filed/removed) → reset
  if (selectedPath && !items.some((it) => it.path === selectedPath)) {
    selectedPath = null;
    selectedReport = null;
  }
  if (selectedReport) {
    if (!mid.querySelector(".sift-wf")) renderReportInto(mid, selectedReport);
  } else {
    mid.innerHTML = MID_PLACEHOLDER;
  }
}

async function selectQueueItem(path: string) {
  const mid = document.getElementById("mid");
  if (!mid) return;
  selectedPath = path;
  selectedReport = null;
  const name = path.split(/[\\/]/).pop() || path;
  mid.innerHTML = `<div style="flex:1;display:flex;align-items:center;justify-content:center;color:var(--color-text-tertiary);font-size:13px">⏳ Analyse de ${esc(name)}…</div>`;
  try {
    const r = await analyzePath(path, false);
    if (selectedPath !== path) return; // a newer click won
    selectedReport = r;
    renderReportInto(mid, r);
  } catch (e) {
    console.error("analyze_path failed", e);
    if (selectedPath === path) {
      mid.innerHTML = `<div style="flex:1;display:flex;align-items:center;justify-content:center;color:var(--color-text-danger);font-size:13px">Analyse échouée.</div>`;
    }
  }
}

const VERDICT_DOT: Record<string, [string, string]> = {
  ok: ["#5cc97a", "authentique"],
  fake: ["#ff6b6b", "fake / sur-encodé"],
  grey: ["#f0c060", "zone grise"],
};
function verdictDot(v: string | null): string {
  if (v && VERDICT_DOT[v]) {
    const [c, title] = VERDICT_DOT[v];
    return `<span title="${title}" style="flex:none;width:9px;height:9px;border-radius:50%;background:${c}"></span>`;
  }
  // not analysed yet
  return `<span title="en attente d'analyse" style="flex:none;width:9px;height:9px;border-radius:50%;border:1.5px solid var(--color-text-tertiary);box-sizing:border-box"></span>`;
}

const esc = (s: string) =>
  s.replace(/[&<>"]/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : "&quot;",
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
        s.watched ? "Surveillance active — cliquer pour suspendre" : "Surveillance suspendue — cliquer pour activer"
      }"></span>`;
      const count = s.pending_count
        ? `${s.pending_count} nouveau${s.pending_count > 1 ? "x" : ""}`
        : "à jour";
      const countColor = s.pending_count ? "var(--color-text-info)" : "var(--color-text-tertiary)";
      return `<div class="srow"><span class="v"><i class="ti ti-folder"></i> ${esc(
        s.path,
      )}${warn}</span><span style="display:flex;align-items:center;gap:9px"><span style="font-size:11px;color:${countColor}">${count}</span>${watch}<button data-sift="rmsrc" data-id="${s.id}" style="font-size:11px;padding:2px 7px;color:var(--color-text-danger)">retirer</button></span></div>`;
    })
    .join("");

  const panel = document.createElement("div");
  panel.id = "sift-sources";
  panel.innerHTML =
    '<div class="col-h" style="margin-top:12px">Dossiers surveillés</div>' +
    '<div style="display:flex;gap:8px;align-items:flex-start;background:var(--color-background-warning);border-radius:var(--border-radius-md);padding:8px 11px;margin:0 0 8px;font-size:11px;color:var(--color-text-warning)"><i class="ti ti-info-circle" style="font-size:14px;flex:none"></i><span>Pointe Sift sur ton dossier <strong>Completed</strong> (pas <em>Incomplete</em>) — les fichiers en cours de téléchargement ne doivent pas entrer dans la file.</span></div>' +
    (rows || '<div style="font-size:12px;color:var(--color-text-tertiary)">Aucun dossier surveillé.</div>') +
    '<div style="margin:8px 0 0"><button data-sift="addsrc"><i class="ti ti-plus" style="font-size:13px;vertical-align:-2px"></i> ajouter un dossier</button></div>';

  // Hide the WHOLE mockup "Dossiers surveillés" block (its hardcoded counts never change):
  // the .col-h header + every following sibling up to the next .col-h. Insert the real
  // panel in its place.
  const left = content.querySelector(".home-left");
  if (!left) return;
  let insertBefore: Element | null = null;
  let hiding = false;
  for (const child of Array.from(left.children)) {
    const isColH = child.classList.contains("col-h");
    if (isColH && child.textContent?.trim() === "Dossiers surveillés") {
      hiding = true;
      (child as HTMLElement).style.display = "none";
      continue;
    }
    if (hiding && isColH) {
      // reached the next section ("Répartition par dossier") — stop, drop the panel here.
      insertBefore = child;
      hiding = false;
      continue;
    }
    if (hiding) (child as HTMLElement).style.display = "none";
  }
  left.insertBefore(panel, insertBefore);
}

/** Replaces the mockup queue list with real pending items (Revue screen). */
async function renderQueue() {
  const ql = document.getElementById("ql");
  if (!ql) return;
  let items: QueueItem[] = [];
  try {
    items = await listQueue();
  } catch (e) {
    console.error("listQueue failed", e);
    return;
  }
  let progressHtml = "";
  try {
    const p = await analysisProgress();
    if (p.total > 0) {
      const pct = Math.round((p.done / p.total) * 100);
      const label =
        p.done >= p.total
          ? `${p.total} analysé${p.total > 1 ? "s" : ""}`
          : `${p.done} / ${p.total} analysés`;
      progressHtml = `<div style="margin:0 0 8px"><div style="display:flex;justify-content:space-between;font-size:11px;color:var(--color-text-tertiary);margin-bottom:3px"><span>Analyse en fond</span><span>${label}</span></div><div style="height:4px;border-radius:2px;background:rgba(237,233,224,.12);overflow:hidden"><div style="height:100%;width:${pct}%;background:var(--color-text-info,#8ecce8);transition:width .3s"></div></div></div>`;
    }
  } catch (e) {
    console.error("analysisProgress failed", e);
  }

  ql.innerHTML =
    progressHtml +
    (items
      .map(
        (it) =>
          `<div class="qi" data-path="${esc(it.path)}" title="Voir le rapport d'analyse" style="display:flex;align-items:center;gap:8px;cursor:pointer">${verdictDot(
            it.verdict,
          )}<span style="flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis">${esc(
            it.filename || it.path,
          )}</span><i class="ti ti-chevron-right" style="flex:none;color:var(--color-text-tertiary);font-size:14px"></i></div>`,
      )
      .join("") ||
      '<div style="font-size:12px;color:var(--color-text-tertiary);padding:6px 4px">File vide.</div>');

  // keep the clicked row highlighted across re-renders
  if (selectedPath) {
    ql.querySelector(`.qi[data-path="${cssEscape(selectedPath)}"]`)?.classList.add("cur");
  }
  paintMid(items);
}

/** Minimal CSS.escape fallback for attribute selectors (paths contain \ and spaces). */
function cssEscape(s: string): string {
  return (window.CSS && CSS.escape ? CSS.escape(s) : s.replace(/["\\]/g, "\\$&"));
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
}

export function installLiveWiring() {
  window.__siftHome = renderHomeSources;
  window.__siftQueue = renderQueue;

  document.getElementById("pa")?.addEventListener("click", (e) => {
    // queue item → render its analysis report inline in the #mid pane
    const qi = (e.target as HTMLElement).closest<HTMLElement>(".qi[data-path]");
    if (qi?.dataset.path) {
      e.stopPropagation();
      document.querySelectorAll(".qi.cur").forEach((n) => n.classList.remove("cur"));
      qi.classList.add("cur");
      void selectQueueItem(qi.dataset.path);
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
    }
  });

  void onQueueChanged(refresh);

  // Analysis pings can arrive several times per second — debounce the queue redraw.
  let t: ReturnType<typeof setTimeout> | undefined;
  void onAnalysisChanged(() => {
    clearTimeout(t);
    t = setTimeout(() => void renderQueue(), 300);
  });

  void refresh();
}

declare global {
  interface Window {
    __siftHome?: () => void;
    __siftQueue?: () => void;
  }
}
