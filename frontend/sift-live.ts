// Live data wiring — ACTIVE ONLY inside the Tauri app. In a plain browser the hooks
// below are never installed, so app.js keeps its mockup (Vercel demo unaffected).
import {
  addSource,
  listSources,
  removeSource,
  listQueue,
  onQueueChanged,
} from "./ipc";
import { open } from "@tauri-apps/plugin-dialog";
import type { Source, QueueItem } from "../shared/contracts";

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
      return `<div class="srow"><span class="v"><i class="ti ti-folder"></i> ${esc(
        s.path,
      )}${warn}</span><span style="display:flex;align-items:center;gap:9px"><span style="font-size:11px;color:var(--color-text-info)">${
        s.pending_count
      } en file</span><button data-sift="rmsrc" data-id="${s.id}" style="font-size:11px;padding:2px 7px;color:var(--color-text-danger)">retirer</button></span></div>`;
    })
    .join("");

  const panel = document.createElement("div");
  panel.id = "sift-sources";
  panel.innerHTML =
    '<div class="col-h" style="margin-top:12px">Dossiers surveillés</div>' +
    '<div style="display:flex;gap:8px;align-items:flex-start;background:var(--color-background-warning);border-radius:var(--border-radius-md);padding:8px 11px;margin:0 0 8px;font-size:11px;color:var(--color-text-warning)"><i class="ti ti-info-circle" style="font-size:14px;flex:none"></i><span>Pointe Sift sur ton dossier <strong>Completed</strong> (pas <em>Incomplete</em>) — les fichiers en cours de téléchargement ne doivent pas entrer dans la file.</span></div>' +
    (rows || '<div style="font-size:12px;color:var(--color-text-tertiary)">Aucun dossier surveillé.</div>') +
    '<div style="margin:8px 0 0"><button data-sift="addsrc"><i class="ti ti-plus" style="font-size:13px;vertical-align:-2px"></i> ajouter un dossier</button></div>';

  content
    .querySelectorAll<HTMLButtonElement>("button")
    .forEach((b) => {
      if (b.textContent && b.textContent.includes("ajouter un dossier") && !b.dataset.sift) {
        b.closest("div")?.style.setProperty("display", "none");
      }
    });

  content.querySelector(".home-left")?.appendChild(panel);
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
  ql.innerHTML =
    items
      .map(
        (it) =>
          `<div class="qi"><i class="ti ti-circle"></i><span style="flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis">${esc(
            it.filename || it.path,
          )}</span></div>`,
      )
      .join("") ||
    '<div style="font-size:12px;color:var(--color-text-tertiary);padding:6px 4px">File vide.</div>';
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
    const el = (e.target as HTMLElement).closest<HTMLElement>("[data-sift]");
    if (!el) return;
    const act = el.dataset.sift;
    if (act === "addsrc") {
      e.stopPropagation();
      void pickAndAddFolder();
    } else if (act === "rmsrc") {
      e.stopPropagation();
      void removeSource(Number(el.dataset.id)).then(refresh);
    }
  });

  void onQueueChanged(refresh);
  void refresh();
}

declare global {
  interface Window {
    __siftHome?: () => void;
    __siftQueue?: () => void;
  }
}
