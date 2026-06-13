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
  listEcartes,
  trashTrack,
  restoreTrack,
  purgeTrash,
  openUrl,
} from "./ipc";
import type { EcarteItem } from "../shared/contracts";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
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
          )}</span>${
            it.dup
              ? '<i class="ti ti-copy" title="Doublon possible (même nom)" style="flex:none;font-size:12px;color:var(--color-text-secondary)"></i>'
              : ""
          }<i class="ti ti-chevron-right" style="flex:none;color:var(--color-text-tertiary);font-size:14px"></i></div>`,
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

/** Reason pill for an écarté track (truncated → tronqué, fake → faux, else à re-sourcer). */
function ecReason(it: EcarteItem): string {
  if (it.truncated)
    return '<span class="pill" style="background:var(--color-background-warning);color:var(--color-text-warning);flex:none"><i class="ti ti-cut" style="font-size:9px"></i> tronqué</span>';
  if (it.verdict === "fake")
    return '<span class="pill" style="background:var(--color-background-danger);color:var(--color-text-danger);flex:none"><i class="ti ti-alert-triangle" style="font-size:9px"></i> faux</span>';
  return '<span class="pill" style="background:var(--color-background-danger);color:var(--color-text-danger);flex:none"><i class="ti ti-alert-circle" style="font-size:9px"></i> à re-sourcer</span>';
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
        )}<button class="lk" data-ec="trash" data-id="${it.id}" title="Envoyer à la corbeille"><i class="ti ti-trash" style="font-size:12px;color:var(--color-text-tertiary)"></i></button></div><div style="margin-top:5px;display:flex;flex-wrap:wrap;align-items:center;gap:4px"><button data-ec="slsk" data-q="${esc(
          ecSlsk(it),
        )}" title="Copier « Artiste Titre » pour rechercher sur Soulseek" style="font-size:10px;padding:2px 7px;color:var(--color-text-secondary)"><i class="ti ti-copy" style="font-size:10px;vertical-align:-1px"></i> Copier le nom</button><span style="color:var(--color-border-secondary)">·</span>${ecStoreLinks(
          it,
        )}</div></div>`,
    )
    .join("");

  const trashRows = trash
    .map(
      (it) =>
        `<div style="display:flex;align-items:center;gap:7px;padding:7px 4px;border-bottom:0.5px solid var(--color-border-tertiary)"><div style="flex:1;min-width:0"><div style="white-space:nowrap;overflow:hidden;text-overflow:ellipsis;font-size:12px">${name(
          it,
        )}</div>${fileLine(it)}</div><button data-ec="restore" data-id="${it.id}" style="font-size:10px;padding:2px 8px;color:var(--color-text-info)">restaurer</button></div>`,
    )
    .join("");

  content.innerHTML =
    '<div class="h1">Écartés</div>' +
    '<div style="display:flex;gap:7px;margin-bottom:12px;flex-wrap:wrap;align-items:center">' +
    `<span class="pill" style="background:var(--color-background-danger);color:var(--color-text-danger)"><i class="ti ti-alert-circle" style="font-size:10px"></i> ${res.length} à re-sourcer</span>` +
    `<span class="pill"><i class="ti ti-trash" style="font-size:10px"></i> ${trash.length} en corbeille</span>` +
    (trash.length
      ? `<button data-ec="purge" style="font-size:10px;padding:2px 8px;color:var(--color-text-danger)">Vider la corbeille (${trash.length})</button>`
      : "") +
    "</div>" +
    (res.length ? `<div class="col-h">À re-sourcer</div>${resRows}` : "") +
    (trash.length ? `<div class="col-h" style="margin-top:14px">Corbeille</div>${trashRows}` : "") +
    (items.length === 0
      ? '<div style="font-size:12px;color:var(--color-text-tertiary)">Aucun fichier écarté.</div>'
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
    // landing/demo copy in index.html: marketing pitch, demo disclaimer, feature cards row
    ".pitch,.sub,.frow{display:none!important}" +
    // unbuilt nav tabs (Biblio, Rekordbox, Clé USB, Réglages) — Écartés is live now
    '#nav .nv[data-view="biblio"],#nav .nv[data-view="rkb"],#nav .nv[data-view="cle"],' +
    '#nav .nv[data-view="reglages"]{display:none!important}' +
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
    '<button class="sift-win" data-win="min" title="Réduire"><i class="ti ti-minus"></i></button>' +
    '<button class="sift-win" data-win="max" title="Agrandir"><i class="ti ti-square"></i></button>' +
    '<button class="sift-win sift-win-close" data-win="close" title="Fermer"><i class="ti ti-x"></i></button>' +
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

export function installLiveWiring() {
  window.__siftHome = renderHomeSources;
  window.__siftQueue = renderQueue;
  window.__siftEcarts = renderEcartes;
  injectLeanStyle();
  injectTitlebar();
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
    // Écartés actions (Soulseek copy / send-to-bin / restore / empty bin)
    const ec = (e.target as HTMLElement).closest<HTMLElement>("[data-ec]");
    if (ec) {
      e.stopPropagation();
      const act = ec.dataset.ec;
      const id = Number(ec.dataset.id);
      if (act === "slsk") {
        void navigator.clipboard.writeText(ec.dataset.q || "").catch(() => {});
        const prev = ec.innerHTML;
        ec.innerHTML = '<i class="ti ti-check" style="font-size:10px;vertical-align:-1px"></i> Copié';
        setTimeout(() => {
          ec.innerHTML = prev;
        }, 1200);
      } else if (act === "trash") {
        void trashTrack(id).then(renderEcartes).catch((err) => console.error("trash failed", err));
      } else if (act === "restore") {
        void restoreTrack(id).then(renderEcartes).catch((err) => console.error("restore failed", err));
      } else if (act === "purge") {
        void purgeTrash().then(renderEcartes).catch((err) => console.error("purge failed", err));
      } else if (act === "store") {
        void openUrl(decodeURIComponent(ec.dataset.url || "")).catch((err) =>
          console.error("open_url failed", err),
        );
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
    __siftEcarts?: () => void;
  }
}
