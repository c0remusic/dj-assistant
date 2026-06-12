// Live data wiring — ACTIVE ONLY inside the Tauri app. In a plain browser the hooks
// below are never installed, so app.js keeps its mockup (Vercel demo unaffected).
import {
  addSource,
  listSources,
  removeSource,
  listQueue,
  onQueueChanged,
  setSourceWatched,
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
    } else if (act === "togglewatch") {
      e.stopPropagation();
      void setSourceWatched(
        Number(el.dataset.id),
        el.dataset.watched !== "1",
      ).then(refresh);
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
