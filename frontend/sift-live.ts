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
  setSourceWatched,
  importPaths,
} from "./ipc";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  openFilingInto,
  refreshBins,
  syncDetail,
  installUndoShortcut,
} from "./filing";
import type { Source, QueueItem } from "../shared/contracts";

// Latest live queue items, kept so a queue-row click can recover the full item (id +
// verdict) the filing pane needs.
let currentItems: QueueItem[] = [];

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
  currentItems = items;
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
          `<div class="qi" data-id="${it.id}" data-path="${esc(it.path)}" title="Écouter et ranger" style="display:flex;align-items:center;gap:8px;cursor:pointer">${verdictDot(
            it.verdict,
          )}<span style="flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis">${esc(
            it.filename || it.path,
          )}</span><i class="ti ti-chevron-right" style="flex:none;color:var(--color-text-tertiary);font-size:14px"></i></div>`,
      )
      .join("") ||
      '<div style="font-size:12px;color:var(--color-text-tertiary);padding:6px 4px">File vide.</div>');

  // Live destination bins + neutral detail prompt (replace the mockup's hardcoded ones).
  const fldz = document.getElementById("fldz");
  if (fldz) void refreshBins(fldz);
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
  [".dest", "Déposer un dossier ici — nouvelle destination"],
  ["#ql", "Déposer des fichiers audio ici"],
  ["#sift-sources", "Déposer un dossier à surveiller"],
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
    : [["#content", "Déposer des fichiers (→ file) ou dossiers (→ surveillés)"]];
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
    // unbuilt nav tabs (Biblio, Rekordbox, Clé USB, Écartés, Réglages)
    '#nav .nv[data-view="biblio"],#nav .nv[data-view="rkb"],#nav .nv[data-view="cle"],' +
    '#nav .nv[data-view="ecarts"],#nav .nv[data-view="reglages"]{display:none!important}' +
    // Revue: batch mode + "traités" toggle aren't wired to the real backend yet
    '[data-act="revmode"],[data-act="togglequeue"]{display:none!important}';
  document.head.appendChild(st);
}

export function installLiveWiring() {
  window.__siftHome = renderHomeSources;
  window.__siftQueue = renderQueue;
  injectLeanStyle();
  installUndoShortcut();
  void installDragDrop();

  document.getElementById("pa")?.addEventListener("click", (e) => {
    // queue item → open the live filing pane (report + editor + actions) in #mid
    const qi = (e.target as HTMLElement).closest<HTMLElement>(".qi[data-id]");
    if (qi?.dataset.id) {
      e.stopPropagation();
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
