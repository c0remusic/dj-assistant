// Live Revue filing controller (Tauri only). Augments the mockup's Revue shell: renders
// real destination bins into the #fldz column (with a NoLibraryRoot picker gate), and the
// analysis report + a filing footer (editable canonical fields, format override, and the
// Ranger / Re-sourcer / Écarter actions) into the #mid pane. Drives the M4 backend via the
// IPC bindings; the plain-browser demo never loads this (see main.ts guard).
import {
  reconcile,
  fileTrack,
  rejectTrack,
  trashTrack,
  listBins,
  createBin,
  getSetting,
  setSetting,
  undoLast,
} from "./ipc";
import { open } from "@tauri-apps/plugin-dialog";
import { openReportInto } from "./report-view";
import type { Bin, Canonical, Target, QueueItem } from "../shared/contracts";

const LIBRARY_ROOT = "library_root";

const esc = (s: string) =>
  s.replace(/[&<>"]/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : "&quot;",
  );

/** Shared, mutable Revue state for the current filing session. */
interface RevueState {
  rootSet: boolean;
  rootPath: string | null; // absolute library root (for the root tree node label)
  bins: Bin[];
  binRel: string | null; // selected destination ("" = root, relative to root otherwise)
  creating: boolean; // "+ nouveau" inline input open
  track: QueueItem | null; // currently open track
  canonical: Canonical | null; // reconciled (then user-edited) metadata
  target: Target | null; // format override (null = backend rail default)
}

const state: RevueState = {
  rootSet: false,
  rootPath: null,
  bins: [],
  binRel: null,
  creating: false,
  track: null,
  canonical: null,
  target: null,
};

/** Refresh root + bin list from the backend. Call before rendering bins. */
async function loadBins(): Promise<void> {
  try {
    const root = await getSetting(LIBRARY_ROOT);
    state.rootPath = root ?? null;
    state.rootSet = !!(root && root.trim());
    state.bins = state.rootSet ? await listBins() : [];
    if (state.rootSet) expanded.add(""); // root open by default
    // Drop a stale selection (a real bin that vanished); "" (root) is always valid.
    if (state.binRel && state.binRel !== "" && !state.bins.some((b) => b.rel === state.binRel)) {
      state.binRel = null;
    }
    // Default to filing at the root until the user picks a sub-folder.
    if (state.rootSet && state.binRel === null) state.binRel = "";
  } catch (e) {
    console.error("loadBins failed", e);
    state.rootSet = false;
    state.bins = [];
  }
}

/** Prompt for, and persist, the library root, then refresh. */
async function pickRoot(fldz: HTMLElement): Promise<void> {
  const dir = await open({ directory: true, multiple: false });
  if (typeof dir !== "string") return;
  try {
    await setSetting(LIBRARY_ROOT, dir);
    await loadBins();
    renderBins(fldz);
  } catch (e) {
    console.error("setSetting(library_root) failed", e);
  }
}

/** Create a new bin under the current selection (or root) and select it. */
async function makeBin(fldz: HTMLElement, name: string): Promise<void> {
  const parent = ""; // M4-4b: create at root level; nested creation can come later
  try {
    const bin = await createBin(parent, name);
    await loadBins();
    state.binRel = bin.rel;
    state.creating = false;
    renderBins(fldz);
  } catch (e) {
    console.error("createBin failed", e);
    state.creating = false;
    renderBins(fldz);
  }
}

// Which folders are expanded in the tree. "" = the library root node.
const expanded = new Set<string>();

/** Display name of the library root (last path segment), for the root tree node. */
function rootName(): string {
  if (!state.rootPath) return "Bibliothèque";
  return state.rootPath.split(/[\\/]/).filter(Boolean).pop() || state.rootPath;
}

/** Human label for the current destination selection. */
function binLabel(): string {
  if (state.binRel === null) return "—";
  if (state.binRel === "") return rootName();
  return state.binRel;
}

/** Direct children of `rel` ("" = the root → its top-level bins). */
function childrenOf(rel: string): Bin[] {
  if (rel === "") return state.bins.filter((b) => b.depth === 1);
  const depth = rel.split("/").length;
  return state.bins.filter((b) => b.depth === depth + 1 && b.rel.startsWith(rel + "/"));
}

/** Recursive HTML for one tree node + its children when expanded. The root (depth 0,
 * rel "") sits at the top; folders nest under it, each with a caret when it has
 * sub-folders. Selecting a node sets it as the filing destination. */
function binNodeHtml(node: { rel: string; name: string; depth: number }): string {
  const kids = childrenOf(node.rel);
  const isOpen = expanded.has(node.rel);
  const on = node.rel === state.binRel ? " on" : "";
  const indent = node.depth * 13;
  const caret = kids.length
    ? `<span data-fil="caret" data-rel="${esc(node.rel)}" title="${isOpen ? "Replier" : "Déplier"}" style="display:inline-block;width:14px;text-align:center;cursor:pointer;color:var(--color-text-tertiary);transition:transform .2s;${
        isOpen ? "transform:rotate(90deg)" : ""
      }">▸</span>`
    : '<span style="display:inline-block;width:14px;flex:none"></span>';
  const icon = node.depth === 0 ? "ti-database" : "ti-folder";
  let html = `<div class="fld${on}" data-fil="bin" data-rel="${esc(node.rel)}" title="${esc(
    node.rel || node.name,
  )}" style="padding-left:${6 + indent}px;display:flex;align-items:center;gap:4px">${caret}<i class="ti ${icon}" style="font-size:13px;flex:none;color:var(--color-text-tertiary)"></i><span style="flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis">${esc(
    node.name,
  )}</span></div>`;
  if (kids.length && isOpen) html += kids.map(binNodeHtml).join("");
  return html;
}

/** Render the destination column (#fldz): root picker when unset, else a collapsible
 * folder tree (top-level always shown, sub-folders behind a caret toggle). */
export function renderBins(fldz: HTMLElement): void {
  if (!state.rootSet) {
    fldz.innerHTML =
      '<div style="font-size:11px;color:var(--color-text-tertiary);margin-bottom:8px">Choisis la racine de ta bibliothèque pour pouvoir ranger.</div>' +
      '<button data-fil="pickroot"><i class="ti ti-folder" style="font-size:13px;vertical-align:-2px"></i> Choisir la racine…</button>';
    fldz
      .querySelector('[data-fil="pickroot"]')
      ?.addEventListener("click", () => void pickRoot(fldz));
    return;
  }

  const tree = binNodeHtml({ rel: "", name: rootName(), depth: 0 });
  const emptyNote =
    state.bins.length === 0 && expanded.has("")
      ? '<div style="font-size:10px;color:var(--color-text-tertiary);padding:2px 0 2px 33px">vide — crée un dossier</div>'
      : "";

  const newRow = state.creating
    ? '<input data-fil="newin" placeholder="nom du dossier…" style="width:100%;font-size:12px;padding:5px 7px;margin-top:2px">'
    : '<div class="fld" data-fil="newbin" style="color:var(--color-text-tertiary)"><i class="ti ti-plus" style="font-size:14px"></i> nouveau</div>';

  fldz.innerHTML = tree + emptyNote + newRow;

  fldz.querySelectorAll<HTMLElement>('[data-fil="caret"]').forEach((el) =>
    el.addEventListener("click", (e) => {
      e.stopPropagation();
      const rel = el.dataset.rel || "";
      if (expanded.has(rel)) expanded.delete(rel);
      else expanded.add(rel);
      renderBins(fldz);
    }),
  );
  fldz.querySelectorAll<HTMLElement>('[data-fil="bin"]').forEach((el) =>
    el.addEventListener("click", () => {
      state.binRel = el.dataset.rel ?? null;
      renderBins(fldz);
      refreshFootButton();
    }),
  );
  fldz.querySelector('[data-fil="newbin"]')?.addEventListener("click", () => {
    state.creating = true;
    renderBins(fldz);
  });
  const input = fldz.querySelector<HTMLInputElement>('[data-fil="newin"]');
  if (input) {
    input.focus();
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        const v = input.value.trim();
        if (v) void makeBin(fldz, v);
      } else if (e.key === "Escape") {
        state.creating = false;
        renderBins(fldz);
      }
    });
  }
}

/** Default target from the analysed rail (lossless → AIFF, else MP3 320). */
function defaultTarget(rail: string): Target {
  return rail === "lossless" ? "aiff_16_44" : "mp3_320";
}

const TARGET_LABEL: Record<Target, string> = {
  mp3_320: "MP3 320",
  aiff_16_44: "AIFF",
  wav_16_44: "WAV",
};

function targetExt(t: Target): string {
  if (t === "mp3_320") return "mp3";
  if (t === "wav_16_44") return "wav";
  return "aiff";
}

/** Live filename preview from the edited canonical + chosen target. */
function previewName(): string {
  const c = state.canonical;
  if (!c) return "";
  const ver = c.version && c.version.trim() ? ` (${c.version.trim()})` : "";
  const ext = targetExt(state.target ?? "mp3_320");
  return `${c.artist} - ${c.title}${ver}.${ext}`;
}

/** Re-render just the Ranger button label (bin can change while a track is open). */
function refreshFootButton(): void {
  const btn = document.querySelector<HTMLElement>('[data-fil="ranger"] .sift-fil-bin');
  if (btn) btn.textContent = binLabel();
}

/** Render the filing footer (editor + format + actions) into `foot`. */
function renderFoot(foot: HTMLElement, mid: HTMLElement, rail: string): void {
  const c = state.canonical;
  if (!c) {
    foot.innerHTML = "";
    return;
  }
  const badge =
    c.confidence === "green"
      ? '<span style="display:inline-flex;align-items:center;gap:4px;font-size:10px;color:var(--color-text-success)"><i class="ti ti-circle-check" style="font-size:11px"></i> métadonnées sûres</span>'
      : '<span style="display:inline-flex;align-items:center;gap:4px;font-size:10px;color:var(--color-text-warning)"><i class="ti ti-alert-circle" style="font-size:11px"></i> à vérifier</span>';

  const chips = (["mp3_320", "aiff_16_44", "wav_16_44"] as Target[])
    .map((t) => {
      const on = (state.target ?? defaultTarget(rail)) === t ? " on" : "";
      return `<span class="chip${on}" data-fil="fmt" data-t="${t}">${TARGET_LABEL[t]}</span>`;
    })
    .join(" ");

  const fake = state.track?.verdict === "fake";
  const secondary = fake
    ? '<button data-fil="resource" style="color:var(--color-text-warning)" title="Fichier faux — ira dans les écartés"><i class="ti ti-alert-triangle" style="font-size:12px;vertical-align:-2px"></i> Re-sourcer</button>'
    : '<button data-fil="trash" style="color:var(--color-text-danger)" title="Envoyer à la corbeille"><i class="ti ti-trash" style="font-size:12px;vertical-align:-2px"></i> Écarter</button>';

  const inputCss =
    "font-size:12px;padding:4px 7px;background:var(--color-background-secondary);border:0.5px solid var(--color-border-tertiary);border-radius:var(--border-radius-md);color:var(--color-text-primary);min-width:0";

  foot.innerHTML =
    `<div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:6px">${badge}<div style="display:flex;align-items:center;gap:6px"><span style="font-size:10px;color:var(--color-text-tertiary)">Sortir en</span>${chips}</div></div>` +
    `<div style="display:grid;grid-template-columns:1fr 1fr auto;gap:5px;margin-bottom:5px">` +
    `<input data-fil="artist" placeholder="Artiste" value="${esc(c.artist)}" style="${inputCss}">` +
    `<input data-fil="title" placeholder="Titre" value="${esc(c.title)}" style="${inputCss}">` +
    `<input data-fil="version" placeholder="Version" value="${esc(c.version ?? "")}" style="${inputCss};width:96px">` +
    `</div>` +
    `<div class="sift-fil-prev" style="font-size:10px;color:var(--color-text-tertiary);font-family:var(--font-mono);word-break:break-all;line-height:1.5;margin-bottom:8px">→ ${esc(previewName())}</div>` +
    `<div style="margin-bottom:9px;padding-top:7px;border-top:0.5px solid var(--color-border-tertiary)"><div style="display:grid;grid-template-columns:auto 1fr auto 1fr;gap:3px 8px;font-size:10px;align-items:center"><span style="color:var(--color-text-tertiary)">Label</span><span style="color:var(--color-text-tertiary)">—</span><span style="color:var(--color-text-tertiary)">Année</span><span style="color:var(--color-text-tertiary)">—</span><span style="color:var(--color-text-tertiary)">Genre</span><span style="color:var(--color-text-tertiary)">—</span><span style="color:var(--color-text-tertiary)">BPM</span><span style="color:var(--color-text-tertiary)">—</span></div><div style="font-size:9px;color:var(--color-text-tertiary);margin-top:4px"><i class="ti ti-download" style="font-size:9px;vertical-align:-1px"></i> enrichissement Discogs à venir</div></div>` +
    `<div style="display:flex;gap:8px">` +
    `<button data-fil="ranger" style="flex:1;background:var(--color-background-info);color:var(--color-text-info);border:none;font-weight:500"><i class="ti ti-corner-down-left" style="font-size:12px;vertical-align:-2px"></i> Ranger → <span class="sift-fil-bin">${esc(binLabel())}</span></button>` +
    secondary +
    `</div>`;

  const upd = () => {
    const a = foot.querySelector<HTMLInputElement>('[data-fil="artist"]');
    const t = foot.querySelector<HTMLInputElement>('[data-fil="title"]');
    const v = foot.querySelector<HTMLInputElement>('[data-fil="version"]');
    if (!state.canonical) return;
    state.canonical.artist = a?.value ?? "";
    state.canonical.title = t?.value ?? "";
    state.canonical.version = v?.value.trim() ? v.value.trim() : null;
    const prev = foot.querySelector<HTMLElement>(".sift-fil-prev");
    if (prev) prev.textContent = `→ ${previewName()}`;
  };
  foot
    .querySelectorAll<HTMLInputElement>('[data-fil="artist"],[data-fil="title"],[data-fil="version"]')
    .forEach((el) => el.addEventListener("input", upd));

  foot.querySelectorAll<HTMLElement>('[data-fil="fmt"]').forEach((el) =>
    el.addEventListener("click", () => {
      state.target = (el.dataset.t as Target) || null;
      renderFoot(foot, mid, rail);
    }),
  );

  foot
    .querySelector('[data-fil="ranger"]')
    ?.addEventListener("click", () => void doRanger(mid));
  foot
    .querySelector('[data-fil="resource"]')
    ?.addEventListener("click", () => void doSecondary(mid, "resource"));
  foot
    .querySelector('[data-fil="trash"]')
    ?.addEventListener("click", () => void doSecondary(mid, "trash"));
}

/** A transient toast at the bottom-right with an optional "Annuler" action. */
function toast(message: string, undo: boolean): void {
  document.getElementById("sift-toast")?.remove();
  const el = document.createElement("div");
  el.id = "sift-toast";
  el.style.cssText =
    "position:fixed;right:18px;bottom:18px;z-index:9998;display:flex;align-items:center;gap:12px;background:var(--color-background-secondary);border:0.5px solid var(--color-border-secondary);border-radius:var(--border-radius-md);padding:9px 13px;font-size:12px;color:var(--color-text-primary);box-shadow:0 8px 28px rgba(0,0,0,.4)";
  el.innerHTML =
    `<span>${esc(message)}</span>` +
    (undo
      ? '<button data-fil="undo" style="font-size:11px;padding:2px 9px">Annuler</button>'
      : "");
  document.body.appendChild(el);
  el.querySelector('[data-fil="undo"]')?.addEventListener("click", () => {
    el.remove();
    void undoLast()
      .then(() => {
        // the just-filed track is back in the queue — clear the stale detail pane
        const mid = document.getElementById("mid");
        if (mid) clearPane(mid);
      })
      .catch((e) => console.error("undo failed", e));
  });
  setTimeout(() => el.remove(), 6000);
}

/** Ranger the current track into the selected bin. */
async function doRanger(mid: HTMLElement): Promise<void> {
  if (!state.track || !state.canonical) return;
  if (state.binRel === null) {
    toast("Choisis un dossier de destination.", false);
    return;
  }
  try {
    await fileTrack(state.track.id, state.binRel, state.target, state.canonical);
    toast(`Rangé → ${state.binRel}`, true);
    clearPane(mid);
  } catch (e) {
    const msg = String(e);
    if (msg.includes("NoLibraryRoot")) toast("Aucune racine de bibliothèque configurée.", false);
    else if (msg.toLowerCase().includes("upscale")) toast("Refusé : pas d'upscale lossy → lossless.", false);
    else toast(`Échec du rangement : ${msg}`, false);
    console.error("file_track failed", e);
  }
}

/** Re-sourcer (fake) or Écarter (trash) the current track. */
async function doSecondary(mid: HTMLElement, kind: "resource" | "trash"): Promise<void> {
  if (!state.track) return;
  try {
    if (kind === "resource") {
      await rejectTrack(state.track.id);
      toast("Marqué à re-sourcer", true);
    } else {
      await trashTrack(state.track.id);
      toast("Envoyé à la corbeille", true);
    }
    clearPane(mid);
  } catch (e) {
    toast(`Échec : ${String(e)}`, false);
    console.error(`${kind} failed`, e);
  }
}

/** Empty the detail pane back to a neutral prompt (after an action). */
function clearPane(mid: HTMLElement): void {
  state.track = null;
  state.canonical = null;
  state.target = null;
  mid.innerHTML =
    '<div style="flex:1;display:flex;align-items:center;justify-content:center;color:var(--color-text-tertiary);font-size:12px;padding:20px;text-align:center">Sélectionne un morceau dans la file pour l\'écouter et le ranger.</div>';
}

/** Render the analysis report + filing footer for `item` into the #mid pane. */
export async function openFilingInto(mid: HTMLElement, item: QueueItem): Promise<void> {
  state.track = item;
  state.target = null;
  state.canonical = null;

  mid.innerHTML =
    '<div class="sift-fil" style="display:flex;flex-direction:column;height:100%;min-height:0">' +
    '<div class="sift-fil-report" style="flex:1;min-height:0;overflow:auto"></div>' +
    '<div class="sift-fil-foot" style="flex:none;padding:10px 2px 2px;border-top:0.5px solid var(--color-border-tertiary)"></div>' +
    "</div>";
  const reportEl = mid.querySelector<HTMLElement>(".sift-fil-report");
  const footEl = mid.querySelector<HTMLElement>(".sift-fil-foot");
  if (!reportEl || !footEl) return;

  // Analysis report (player, spectrogram) — reuses the finished report view.
  await openReportInto(reportEl, item.path);

  // Reconcile metadata for the editable fields + confidence badge.
  let rail = "unknown";
  try {
    state.canonical = await reconcile(item.id);
  } catch (e) {
    console.error("reconcile failed", e);
    state.canonical = { artist: "", title: "", version: null, confidence: "yellow" };
  }
  // Pull the analysed rail off the rendered report (data attribute set by report-view is
  // not available; default by extension instead).
  const ext = (item.path.split(".").pop() || "").toLowerCase();
  if (["flac", "wav", "aif", "aiff", "alac"].includes(ext)) rail = "lossless";
  else if (["mp3", "m4a", "aac", "ogg"].includes(ext)) rail = "lossy";

  renderFoot(footEl, mid, rail);
}

/** Wire a one-time global Ctrl+Z → undo (ignored while editing a field). */
export function installUndoShortcut(): void {
  document.addEventListener("keydown", (e) => {
    if (!(e.ctrlKey && (e.key === "z" || e.key === "Z"))) return;
    const t = e.target as HTMLElement;
    if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA")) return;
    e.preventDefault();
    void undoLast()
      .then((b) => {
        if (b) toast("Action annulée", false);
      })
      .catch((err) => console.error("undo failed", err));
  });
}

/** Load root+bins and render the destination column. Called from the live queue refresh. */
export async function refreshBins(fldz: HTMLElement): Promise<void> {
  await loadBins();
  renderBins(fldz);
}

/** Show the neutral "pick a track" prompt in #mid when nothing is open (replaces the
 * mockup's stale detail). No-op while a track is open so a queue refresh doesn't disrupt it. */
export function ensureMidPrompt(mid: HTMLElement): void {
  if (!state.track) clearPane(mid);
}
