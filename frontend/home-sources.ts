// Home "sources" screen (Tauri only). Two-column grammar matching the maquette
// (design_handoff_sift_refonte/Sift.dc.html:68-77 list rail, :594-633 inspector): a list
// of watched sources in the queue rail (#homequeue), and a detail inspector for the
// selected one (#homeinspector) — breadcrumb, "Dossier surveillé" card, watch toggle,
// bottom-bar "+ Ajouter un dossier". Extracted from sift-live.ts (audit P-3), rebuilt
// 2026-07-02 (docs/audit-fidelite-2026-07-02.md §1: the old single-column list was a
// confirmed structural gap vs the maquette).
import { listSources, addSource, removeSource, setSourceWatched, getSetting } from "./ipc";
import { open } from "@tauri-apps/plugin-dialog";
import type { Source } from "../shared/contracts";

const LIBRARY_ROOT = "library_root"; // same setting key filing.ts gates the destination tree on

const esc = (s: string) =>
  s.replace(/[&<>"']/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : c === '"' ? "&quot;" : "&#39;",
  );

/** Selected source persists across re-renders (watcher/refresh events) by id, not index —
 * the list can reorder/shrink under us. */
let selectedSourceId: number | null = null;

function baseName(path: string): string {
  const norm = path.replace(/[/\\]+$/, "");
  const idx = Math.max(norm.lastIndexOf("/"), norm.lastIndexOf("\\"));
  return idx >= 0 ? norm.slice(idx + 1) : norm;
}

interface StatusMeta {
  label: string;
  color: string;
}

function statusMeta(s: Source): StatusMeta {
  if (!s.accessible) return { label: "Inaccessible", color: "var(--color-text-danger)" };
  if (s.pending_count > 0) return { label: `${s.pending_count} nouveau${s.pending_count > 1 ? "x" : ""}`, color: "var(--color-text-info)" };
  if (!s.watched) return { label: "En pause", color: "var(--color-text-tertiary)" };
  return { label: "À jour", color: "var(--color-text-success)" };
}

function rowHtml(s: Source, active: boolean): string {
  const sm = statusMeta(s);
  return (
    `<div class="qi${active ? " cur" : ""}" data-sift="homerow" data-id="${s.id}" style="flex-direction:column;align-items:stretch;gap:3px;height:auto;padding:8px 9px">` +
    `<span style="font-size:var(--text-lg);font-weight:500;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">${esc(baseName(s.path))}</span>` +
    `<span style="display:flex;align-items:center;gap:6px;font-size:var(--text-sm);color:${sm.color}"><span style="width:5px;height:5px;border-radius:999px;background:${sm.color};flex:none"></span>${esc(sm.label)}</span>` +
    `</div>`
  );
}

function listColumnHtml(sources: Source[]): string {
  const header =
    `<div style="display:flex;align-items:center;justify-content:space-between;padding:0 2px 11px">` +
    `<span style="font-size:var(--text-lg);font-weight:600">Sources <span style="font-family:var(--font-mono);font-weight:400;font-size:var(--text-sm);color:var(--color-text-tertiary)">${sources.length}</span></span>` +
    `</div>`;
  const rows = sources.length
    ? sources.map((s) => rowHtml(s, s.id === selectedSourceId)).join("")
    : `<div style="font-size:var(--text-md);color:var(--color-text-tertiary);padding:4px 2px">Aucun dossier surveillé.</div>`;
  const bottomBar =
    `<div style="flex:none;border-top:0.5px solid var(--color-border-tertiary);margin-top:8px;padding-top:8px">` +
    `<button data-sift="addsrc" style="width:100%;background:var(--color-background-info);color:var(--color-text-info);font-weight:600"><i class="ti ti-plus" style="font-size:var(--text-base);vertical-align:-2px"></i> Ajouter un dossier</button>` +
    `</div>`;
  return header + `<div style="flex:1;min-height:0;overflow-y:auto;display:flex;flex-direction:column;gap:2px">${rows}</div>` + bottomBar;
}

function inspectorHtml(selected: Source | null, root: string | null): string {
  const rootGateHtml = root
    ? ""
    : '<div style="display:flex;gap:8px;align-items:flex-start;background:var(--color-background-warning);border-radius:var(--border-radius-md);padding:8px 11px;margin-bottom:16px;font-size:var(--text-sm);color:var(--color-text-warning)">' +
      '<i class="ti ti-alert-triangle" style="font-size:var(--text-lg);flex:none"></i>' +
      "<span><strong>Racine de bibliothèque non définie</strong> — les dossiers surveillés restent scannés, mais le rangement sera bloqué tant qu'aucune racine n'est choisie. " +
      '<button data-sift="gotoreglages" style="color:var(--color-text-warning);text-decoration:underline;padding:0;font:inherit">Ouvrir Réglages →</button></span></div>';

  if (!selected) {
    return (
      `<div style="flex:1;overflow-y:auto;padding:20px 30px">` +
      rootGateHtml +
      `<div style="font-size:var(--text-md);color:var(--color-text-tertiary)">Sélectionne un dossier surveillé pour voir son détail.</div>` +
      `</div>`
    );
  }

  const sm = statusMeta(selected);
  const name = esc(baseName(selected.path));
  const watchOn = selected.watched;

  return (
    `<div style="flex:1;overflow-y:auto;padding:20px 30px">` +
    `<div style="font-size:var(--text-sm);color:var(--color-text-tertiary);margin-bottom:20px">Accueil <span style="color:var(--color-text-tertiary);margin:0 3px">›</span> <span style="color:var(--color-text-primary)">${name}</span></div>` +
    rootGateHtml +
    `<div style="display:flex;align-items:center;gap:16px;margin-bottom:20px">` +
    `<div style="font-size:var(--text-xl);font-weight:600">${name}</div>` +
    `<span style="font-family:var(--font-mono);font-size:var(--text-sm);padding:4px 10px;border-radius:999px;background:var(--color-background-secondary);color:${sm.color}">${esc(sm.label)}</span>` +
    `</div>` +
    `<div style="background:var(--color-background-secondary);border:0.5px solid var(--color-border-tertiary);border-radius:var(--border-radius-lg);padding:16px 18px;max-width:560px;margin-bottom:16px">` +
    `<div style="font-size:var(--text-xs);letter-spacing:.09em;text-transform:uppercase;color:var(--color-text-tertiary);margin-bottom:6px">Dossier surveillé</div>` +
    `<div style="font-family:var(--font-mono);font-size:var(--text-md);color:var(--color-text-secondary);word-break:break-all">${esc(selected.path)}</div>` +
    (selected.accessible
      ? ""
      : `<div style="margin-top:8px;font-size:var(--text-sm);color:var(--color-text-danger)"><i class="ti ti-alert-triangle" style="vertical-align:-1px"></i> Dossier inaccessible.</div>`) +
    `</div>` +
    `<div style="display:flex;align-items:center;gap:10px">` +
    `<div data-sift="togglewatch" data-id="${selected.id}" data-watched="${watchOn ? "1" : "0"}" style="display:flex;align-items:center;gap:8px;font-size:var(--text-md);padding:8px 13px;border-radius:var(--border-radius-md);background:var(--color-background-secondary);border:0.5px solid var(--color-border-tertiary);cursor:pointer;color:var(--color-text-secondary)">` +
    `<span style="width:15px;height:15px;border-radius:4px;border:1px solid var(--color-border-secondary);background:${watchOn ? "var(--color-text-success)" : "transparent"};flex:none"></span>` +
    `Surveiller ce dossier</div>` +
    `<button data-sift="rmsrc" data-id="${selected.id}" style="color:var(--color-text-danger)"><i class="ti ti-trash" style="font-size:var(--text-md);vertical-align:-2px"></i> Retirer</button>` +
    `</div>` +
    `</div>`
  );
}

/** Replaces the Home shell's two columns (#homequeue list rail, #homeinspector detail)
 * with the real watched sources + selection detail + library-root warning. */
export async function renderHomeSources() {
  // Auto-guard (mirror of renderQueue's `if (!ql) return`): the shell only exists while the
  // Home view is mounted — no-op cleanly instead of throwing, so a blind refresh() from any
  // view skips Home safely.
  const queueCol = document.querySelector<HTMLElement>("#homequeue");
  const inspectorCol = document.querySelector<HTMLElement>("#homeinspector");
  if (!queueCol || !inspectorCol) return;

  let sources: Source[] = [];
  try {
    sources = await listSources();
  } catch (e) {
    console.error("listSources failed", e);
    return;
  }
  let root: string | null = null;
  try {
    root = await getSetting(LIBRARY_ROOT);
  } catch (e) {
    console.error("getSetting(library_root) failed", e);
  }

  if (selectedSourceId == null || !sources.some((s) => s.id === selectedSourceId)) {
    selectedSourceId = sources[0]?.id ?? null;
  }
  const selected = sources.find((s) => s.id === selectedSourceId) ?? null;

  queueCol.innerHTML = listColumnHtml(sources);
  inspectorCol.innerHTML = inspectorHtml(selected, root);

  queueCol.querySelectorAll<HTMLElement>('[data-sift="homerow"]').forEach((row) => {
    row.addEventListener("click", () => {
      selectedSourceId = Number(row.dataset.id);
      void renderHomeSources();
    });
  });
  queueCol.querySelector('[data-sift="addsrc"]')?.addEventListener("click", () => {
    void pickAndAddFolder(renderHomeSources);
  });

  inspectorCol.querySelector('[data-sift="gotoreglages"]')?.addEventListener("click", () => {
    document
      .querySelector('[data-view="reglages"]')
      ?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  });
  inspectorCol.querySelector('[data-sift="togglewatch"]')?.addEventListener("click", async (e) => {
    const el = e.currentTarget as HTMLElement;
    const id = Number(el.dataset.id);
    const next = el.dataset.watched !== "1";
    try {
      await setSourceWatched(id, next);
      await renderHomeSources();
    } catch (err) {
      console.error("setSourceWatched failed", err);
    }
  });
  inspectorCol.querySelector('[data-sift="rmsrc"]')?.addEventListener("click", async (e) => {
    const el = e.currentTarget as HTMLElement;
    const id = Number(el.dataset.id);
    try {
      await removeSource(id);
      if (selectedSourceId === id) selectedSourceId = null;
      await renderHomeSources();
    } catch (err) {
      console.error("removeSource failed", err);
    }
  });
}

/** Open the OS folder picker, add the chosen folder as a watched source, then `onChange`
 * (the caller's refresh). Kept out of sift-live so the picker has no app-state dependency. */
export async function pickAndAddFolder(onChange: () => void | Promise<void>) {
  const dir = await open({ directory: true, multiple: false });
  if (typeof dir === "string") {
    try {
      const added = await addSource(dir);
      selectedSourceId = added.id;
      await onChange();
    } catch (e) {
      console.error("addSource failed", e);
    }
  }
}
