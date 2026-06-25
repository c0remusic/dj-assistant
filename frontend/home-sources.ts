// Home "watched folders" panel (Tauri only): replaces the mockup's hardcoded sources block with
// the real watched sources + a Completed-vs-Incomplete warning, and the folder picker. Extracted
// from sift-live.ts (audit P-3). Row actions (toggle watch / remove) and the refresh after an add
// stay owned by sift-live; the picker takes `onChange` so this module never imports sift-live.
import { listSources, addSource } from "./ipc";
import { open } from "@tauri-apps/plugin-dialog";
import type { Source } from "../shared/contracts";
import { requireEl } from "./dom";

const esc = (s: string) =>
  s.replace(/[&<>"']/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : c === '"' ? "&quot;" : "&#39;",
  );

/** Replaces app.js's mockup "Dossiers surveillés" block with real sources + warning. */
export async function renderHomeSources() {
  const content = requireEl("#content", "renderHomeSources");
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
        : ' <span style="color:var(--color-text-danger);font-size:var(--text-sm)">⚠ inaccessible</span>';
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
      )}${warn}</span><span style="display:flex;align-items:center;gap:9px"><span style="font-size:var(--text-sm);color:${countColor}">${count}</span>${watch}<button data-sift="rmsrc" data-id="${s.id}" style="font-size:var(--text-sm);padding:2px 7px;color:var(--color-text-danger)">remove</button></span></div>`;
    })
    .join("");

  const panel = document.createElement("div");
  panel.id = "sift-sources";
  panel.innerHTML =
    '<div class="col-h" style="margin-top:12px">Watched folders</div>' +
    '<div style="display:flex;gap:8px;align-items:flex-start;background:var(--color-background-warning);border-radius:var(--border-radius-md);padding:8px 11px;margin:0 0 8px;font-size:var(--text-sm);color:var(--color-text-warning)"><i class="ti ti-info-circle" style="font-size:var(--text-lg);flex:none"></i><span>Point Sift at your <strong>Completed</strong> folder (not <em>Incomplete</em>) — files still downloading shouldn\'t enter the queue.</span></div>' +
    (rows || '<div style="font-size:var(--text-md);color:var(--color-text-tertiary)">No watched folder.</div>') +
    '<div style="margin:8px 0 0"><button data-sift="addsrc"><i class="ti ti-plus" style="font-size:var(--text-base);vertical-align:-2px"></i> add a folder</button></div>';

  // Hide the WHOLE mockup "Dossiers surveillés" block (its hardcoded counts never change):
  // the .col-h header + every following sibling up to the next .col-h. Insert the real
  // panel in its place.
  const left = requireEl(".home-left", "renderHomeSources", content);
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

/** Open the OS folder picker, add the chosen folder as a watched source, then `onChange`
 * (the caller's refresh). Kept out of sift-live so the picker has no app-state dependency. */
export async function pickAndAddFolder(onChange: () => void | Promise<void>) {
  const dir = await open({ directory: true, multiple: false });
  if (typeof dir === "string") {
    try {
      await addSource(dir);
      await onChange();
    } catch (e) {
      console.error("addSource failed", e);
    }
  }
}
