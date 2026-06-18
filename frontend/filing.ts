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
  findDuplicate,
  identify,
  applyIdentity,
} from "./ipc";
import type { Candidate, AppliedIdentity } from "./ipc";
import type { DupMatch } from "../shared/contracts";
import { open } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { openReportInto, togglePlay } from "./report-view";
import type { Bin, Canonical, Target, QueueItem } from "../shared/contracts";

const LIBRARY_ROOT = "library_root";

const esc = (s: string) =>
  s.replace(/[&<>"']/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : c === '"' ? "&quot;" : "&#39;",
  );

/** Capitalise the first letter of each word ("original mix" → "Original Mix"), leaving the rest
 *  as-is so existing caps/acronyms ("2WFU Dub", "Knee Deep Remix") survive. */
const titleCase = (s: string): string =>
  s.replace(/(^|[\s(/-])([\p{L}\p{N}])/gu, (_, sep: string, ch: string) => sep + ch.toUpperCase());

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

/** Clean display name from the (edited) canonical — what the file will be called. */
function displayName(): string {
  const c = state.canonical;
  if (!c) return "";
  const ver = c.version && c.version.trim() ? ` (${c.version.trim()})` : "";
  return c.artist ? `${c.artist} — ${c.title}${ver}` : `${c.title}${ver}`;
}

/** Replace the report header's filename with the clean proposed name (raw path stays as the
 * grey subtitle), so a messy source file shows its tidy target name. */
function updateHeaderName(mid: HTMLElement): void {
  const el = mid.querySelector<HTMLElement>(".sift-report-name");
  const name = displayName();
  if (el && name) el.textContent = name;
}

/** Re-render just the Ranger button label (bin can change while a track is open). */
function refreshFootButton(): void {
  const btn = document.querySelector<HTMLElement>('[data-fil="ranger"] .sift-fil-bin');
  if (btn) btn.textContent = binLabel();
}

/** Build the cover thumbnail (or placeholder) for a candidate row. */
function candCoverHtml(c: Candidate): string {
  if (c.cover_url) {
    return `<img src="${esc(c.cover_url)}" alt="" class="sift-cand-noart" loading="lazy">`;
  }
  return '<span class="sift-cand-noart"><i class="ti ti-vinyl" style="font-size:18px;color:var(--color-text-tertiary)"></i></span>';
}

/** Render one candidate button row. */
function candRowHtml(c: Candidate, idx: number): string {
  // [I3] drop styles from the sub-line — they clutter pressing-identification scanning
  // (shown as genre chips after applying; keep label, year, country, format)
  const sub = [c.label, c.year != null ? String(c.year) : null, c.country, c.format]
    .filter(Boolean)
    .join(" · ");
  return (
    `<button class="sift-cand" data-cand="${idx}">` +
    candCoverHtml(c) +
    `<span class="sift-cand-meta"><span>${esc(c.artist)} — ${esc(c.title)}</span>` +
    (sub ? `<small>${esc(sub)}</small>` : "") +
    `</span></button>`
  );
}

/** Render candidates into the host container. */
function renderCandidates(host: HTMLElement, list: Candidate[], isError = false): void {
  if (list.length === 0) {
    // [m10] neutral "no results" message — no warning styling
    host.innerHTML = '<div class="sift-cands-msg">Rien sur Discogs.</div>';
    return;
  }
  const [first, ...rest] = list;
  // [I4] "N autres résultats" with a chevron and interactive affordance
  const moreHtml = rest.length
    ? `<details class="sift-cand-more"><summary class="sift-cand-more-summary">▸ ${rest.length} autre${rest.length > 1 ? "s" : ""} résultat${rest.length > 1 ? "s" : ""}</summary>${rest.map((c, i) => candRowHtml(c, i + 1)).join("")}</details>`
    : "";
  host.innerHTML = candRowHtml(first, 0) + moreHtml;
}

/** Apply an identity result to the editing fields + filename preview.
 * [C3] `host` + `allCandidates` are kept so we can show a "changer" confirmation row
 * instead of dead-ending (no new API call needed — re-renders from in-memory list). */
function onIdentityApplied(
  applied: AppliedIdentity,
  foot: HTMLElement,
  mid: HTMLElement,
  host: HTMLElement,
  allCandidates: Candidate[],
  idBtn: HTMLButtonElement,
): void {
  if (!state.canonical) return;
  state.canonical.artist = applied.canonical.artist;
  // Split a trailing "(Version)" out of the Discogs title so it's never duplicated: the title
  // field gets the clean base, the version field gets the mix. Prefer the version Discogs put
  // in the title; otherwise keep the one parsed from the local name (Discogs search doesn't
  // always expose a per-track version). Fixes e.g. "Love Foolosophy (Knee Deep Remix) (Knee
  // Deep Remix)".
  const m = applied.canonical.title.match(/^(.*?)\s*\(([^()]+)\)\s*$/);
  const baseTitle = m ? m[1].trim() : applied.canonical.title.trim();
  const rawVersion = (m ? m[2].trim() : null) ?? state.canonical.version;
  const version = rawVersion ? titleCase(rawVersion) : null;
  state.canonical.title = baseTitle;
  state.canonical.version = version;

  // Update the editable inputs directly.
  const aInp = foot.querySelector<HTMLInputElement>('[data-fil="artist"]');
  const tInp = foot.querySelector<HTMLInputElement>('[data-fil="title"]');
  const vInp = foot.querySelector<HTMLInputElement>('[data-fil="version"]');
  if (aInp) aInp.value = applied.canonical.artist;
  if (tInp) tInp.value = baseTitle;
  if (vInp) vInp.value = version ?? "";

  // Refresh the filename preview using the same logic as the input handler.
  const prev = foot.querySelector<HTMLElement>(".sift-fil-prev");
  if (prev) prev.textContent = `→ ${previewName()}`;
  updateHeaderName(mid);

  // Show the cover if we have a local path.
  if (applied.cover_path) {
    const covEl = mid.querySelector<HTMLImageElement>(".sift-report-cover");
    if (covEl) {
      covEl.src = convertFileSrc(applied.cover_path);
      covEl.hidden = false;
    }
  }

  // [m11] Render genre/style chips with tooltip so they read as informational Discogs tags
  const genEl = mid.querySelector<HTMLElement>(".sift-genres");
  if (genEl) {
    genEl.innerHTML = applied.styles
      .map((s) => `<span class="sift-genre-chip" title="Sous-genres Discogs">${esc(s)}</span>`)
      .join("");
  }

  // [C3] Collapse candidate zone to a confirmation row + "changer" link (no dead-end).
  // Re-labelling the Identifier button to "Ré-identifier" is also handled here.
  const coverThumb = applied.cover_path
    ? `<img src="${esc(convertFileSrc(applied.cover_path))}" alt="" style="width:28px;height:28px;border-radius:3px;object-fit:cover;flex:none">`
    : `<span style="width:28px;height:28px;border-radius:3px;background:var(--color-background-secondary);display:inline-flex;align-items:center;justify-content:center;flex:none"><i class="ti ti-vinyl" style="font-size:14px;color:var(--color-text-tertiary)"></i></span>`;
  host.hidden = false;
  host.innerHTML =
    `<div style="display:flex;align-items:center;gap:7px;padding:4px 2px">` +
    coverThumb +
    `<span style="flex:1;min-width:0;font-size:12px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">` +
    `<span style="color:var(--color-text-secondary)">Identifié :</span> ${esc(applied.canonical.artist)} — ${esc(applied.canonical.title)}` +
    `</span>` +
    `<button class="sift-cand-jump" data-fil="cand-changer" style="font-size:11px;padding:2px 8px;flex:none">changer</button>` +
    `</div>`;

  const changerBtn = host.querySelector<HTMLElement>('[data-fil="cand-changer"]');
  changerBtn?.addEventListener("click", () => {
    // Re-show the full candidate list from memory (no new API call).
    host.innerHTML = "";
    renderCandidates(host, allCandidates);
    wireCandidateClicks(host, allCandidates, foot, mid, idBtn);
  });

  // [C1] Relabel Identifier → Ré-identifier once an identity has been applied.
  idBtn.innerHTML = '<i class="ti ti-refresh" style="font-size:11px;vertical-align:-1px"></i> Ré-identifier';
}

/** Wire clicks on rendered candidate buttons.
 * Extracted so it can be called after initial render AND after "changer" re-shows the list. */
function wireCandidateClicks(
  host: HTMLElement,
  candidates: Candidate[],
  foot: HTMLElement,
  mid: HTMLElement,
  idBtn: HTMLButtonElement,
): void {
  host.querySelectorAll<HTMLElement>("[data-cand]").forEach((el) => {
    const idx = Number(el.dataset.cand);
    el.addEventListener("click", () => {
      const c = candidates[idx];
      if (!c || !state.track) return;
      el.style.opacity = "0.5";
      el.style.pointerEvents = "none";
      void applyIdentity(state.track.id, c)
        .then((applied) => {
          onIdentityApplied(applied, foot, mid, host, candidates, idBtn);
        })
        .catch((e) => {
          el.style.opacity = "";
          el.style.pointerEvents = "";
          // [m10] errors get a warning icon to distinguish from "no results"
          host.innerHTML = `<div class="sift-cands-msg sift-cands-error"><i class="ti ti-alert-triangle" style="font-size:12px;vertical-align:-2px;margin-right:4px"></i>${esc(String(e))}</div>`;
        });
    });
  });
}

/** Run the Discogs identify flow for the current track. */
async function doIdentify(
  btn: HTMLButtonElement,
  host: HTMLElement,
  foot: HTMLElement,
  mid: HTMLElement,
): Promise<void> {
  if (!state.track) return;
  const trackId = state.track.id;
  const origLabel = btn.innerHTML;
  btn.disabled = true;
  btn.innerHTML = '<i class="ti ti-loader-2 sift-spin" style="font-size:11px;vertical-align:-1px"></i> Recherche…';
  host.hidden = false;
  host.innerHTML = '<div class="sift-cands-msg">Recherche en cours…</div>';

  let candidates: Candidate[] = [];
  try {
    candidates = await identify(trackId);
    renderCandidates(host, candidates);
    wireCandidateClicks(host, candidates, foot, mid, btn);
  } catch (err) {
    const msg = String(err);
    if (msg.includes("NO_TOKEN")) {
      // [C2/m5] explain WHY + give a direct action to open Réglages
      host.innerHTML =
        `<div class="sift-cands-msg">Discogs limite les recherches anonymes — ajoute ton token (gratuit) dans Réglages.</div>` +
        `<button class="sift-cand-jump" data-fil="goto-reglages" style="margin-top:5px;font-size:11px;padding:3px 9px">Ouvrir Réglages →</button>`;
      const gotoBtn = host.querySelector<HTMLElement>('[data-fil="goto-reglages"]');
      gotoBtn?.addEventListener("click", () => {
        // Navigate to the Réglages view via the existing nav click handler in app.js
        document.querySelector<HTMLElement>('[data-view="reglages"]')?.dispatchEvent(
          new MouseEvent("click", { bubbles: true }),
        );
      });
    } else {
      const rl = msg.match(/RATE_LIMITED:(\d+)/);
      if (rl) {
        host.innerHTML = `<div class="sift-cands-msg">Discogs limite les requêtes — réessaie dans ${rl[1]}s.</div>`;
      } else {
        // [m10] network/server errors get a warning icon to distinguish from "no results"
        host.innerHTML = `<div class="sift-cands-msg sift-cands-error"><i class="ti ti-alert-triangle" style="font-size:12px;vertical-align:-2px;margin-right:4px"></i>Discogs injoignable.</div>`;
      }
    }
  } finally {
    btn.disabled = false;
    btn.innerHTML = origLabel;
  }
}

/** Render the filing footer (editor + format + actions) into `foot`. */
function renderFoot(foot: HTMLElement, mid: HTMLElement, rail: string): void {
  const c = state.canonical;
  if (!c) {
    foot.innerHTML = "";
    return;
  }
  // [I6] Add tooltip to confidence badge so the colour is self-explanatory
  const badge =
    c.confidence === "green"
      ? '<span title="Titre et artiste extraits avec confiance" style="display:inline-flex;align-items:center;gap:4px;font-size:10px;color:var(--color-text-success)"><i class="ti ti-circle-check" style="font-size:11px"></i> métadonnées sûres</span>'
      : '<span title="Le titre ou l\'artiste n\'a pas pu être extrait avec certitude — vérifie les champs" style="display:inline-flex;align-items:center;gap:4px;font-size:10px;color:var(--color-text-warning)"><i class="ti ti-alert-circle" style="font-size:11px"></i> à vérifier</span>';

  const lossy = rail === "lossy";
  const chips = (["mp3_320", "aiff_16_44", "wav_16_44"] as Target[])
    .map((t) => {
      // a lossy source can't be upscaled to lossless — disable AIFF/WAV (the backend refuses
      // it anyway; greying it out prevents the dead-end click).
      if (lossy && t !== "mp3_320")
        return `<span class="chip" title="Pas d'upscale depuis un fichier lossy" style="opacity:.4;cursor:not-allowed">${TARGET_LABEL[t]}</span>`;
      const on = (state.target ?? defaultTarget(rail)) === t ? " on" : "";
      return `<span class="chip${on}" data-fil="fmt" data-t="${t}">${TARGET_LABEL[t]}</span>`;
    })
    .join(" ");

  const fake = state.track?.verdict === "fake";
  const secondary = fake
    ? '<button data-fil="resource" style="color:var(--color-text-warning)" title="Fichier faux — ira dans les écartés (touche X)"><span class="kbd">X</span> <i class="ti ti-alert-triangle" style="font-size:12px;vertical-align:-2px"></i> Re-sourcer</button>'
    : '<button data-fil="trash" style="color:var(--color-text-danger)" title="Envoyer à la corbeille (touche X)"><span class="kbd">X</span> <i class="ti ti-trash" style="font-size:12px;vertical-align:-2px"></i> Écarter</button>';

  const inputCss =
    "font-size:12px;padding:4px 7px;background:var(--color-background-secondary);border:0.5px solid var(--color-border-tertiary);border-radius:var(--border-radius-md);color:var(--color-text-primary);min-width:0";

  // [C1] Identifier is the first action visible — placed above the inputs with a gold filled
  // style so it reads as the primary entry point when reviewing a new track.
  // [C2] title= explains what it does; label shows keyboard shortcut hint (I).
  foot.innerHTML =
    `<div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:6px">${badge}<div style="display:flex;align-items:center;gap:6px"><span style="font-size:10px;color:var(--color-text-tertiary)">Sortir en</span>${chips}</div></div>` +
    `<div style="display:flex;align-items:center;gap:6px;margin-bottom:7px">` +
    `<button data-fil="identifier" class="sift-id-btn" title="Rechercher les métadonnées sur Discogs (cover, label, année, genres)"><i class="ti ti-search" style="font-size:12px;vertical-align:-1px"></i> Identifier <span class="kbd" style="font-size:9px;border-color:rgba(0,0,0,.18);color:rgba(0,0,0,.5)">I</span></button>` +
    `</div>` +
    `<div class="sift-cands" hidden></div>` +
    `<div style="display:grid;grid-template-columns:1fr 1fr auto;gap:5px;margin-bottom:5px">` +
    `<input data-fil="artist" placeholder="Artiste" value="${esc(c.artist)}" style="${inputCss}">` +
    `<input data-fil="title" placeholder="Titre" value="${esc(c.title)}" style="${inputCss}">` +
    `<input data-fil="version" placeholder="Version" value="${esc(c.version ?? "")}" style="${inputCss};width:96px">` +
    `</div>` +
    `<div class="sift-fil-prev" style="font-size:10px;color:var(--color-text-tertiary);font-family:var(--font-mono);word-break:break-all;line-height:1.5;margin-bottom:6px">→ ${esc(previewName())}</div>` +
    `<div class="sift-genres" style="margin-bottom:4px"></div>` +
    `<div style="display:flex;gap:8px">` +
    `<button data-fil="ranger" style="flex:1;background:var(--color-background-info);color:var(--color-text-info);border:none;font-weight:500"><i class="ti ti-corner-down-left" style="font-size:12px;vertical-align:-2px"></i> Ranger → <span class="sift-fil-bin">${esc(binLabel())}</span> <span class="kbd">⏎</span></button>` +
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
    updateHeaderName(mid); // keep the report header's clean name in sync with edits
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

  const idBtn = foot.querySelector<HTMLButtonElement>('[data-fil="identifier"]');
  const candsHost = foot.querySelector<HTMLElement>(".sift-cands");
  if (idBtn && candsHost) {
    idBtn.addEventListener("click", () => void doIdentify(idBtn, candsHost, foot, mid));
  }
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

// One filing action at a time — guards against a double-click firing two encodes.
let acting = false;

/** Disable/enable the footer action buttons (visible feedback while an action runs). */
function setActionsDisabled(mid: HTMLElement, disabled: boolean): void {
  mid
    .querySelectorAll<HTMLButtonElement>('[data-fil="ranger"],[data-fil="resource"],[data-fil="trash"]')
    .forEach((b) => {
      b.disabled = disabled;
      b.style.opacity = disabled ? "0.55" : "";
      b.style.pointerEvents = disabled ? "none" : "";
    });
}

/** Ranger the current track into the selected bin. */
async function doRanger(mid: HTMLElement): Promise<void> {
  if (!state.track || !state.canonical || acting) return;
  if (state.binRel === null) {
    toast("Choisis un dossier de destination.", false);
    return;
  }
  const ranger = mid.querySelector<HTMLElement>('[data-fil="ranger"]');
  const orig = ranger?.innerHTML ?? null;
  acting = true;
  setActionsDisabled(mid, true);
  if (ranger)
    ranger.innerHTML =
      '<i class="ti ti-loader-2 sift-spin" style="font-size:12px;vertical-align:-2px"></i> Rangement…';
  try {
    await fileTrack(state.track.id, state.binRel, state.target, state.canonical);
    toast(`Rangé → ${binLabel()}`, true);
    clearPane(mid);
  } catch (e) {
    const msg = String(e);
    if (msg.includes("NoLibraryRoot")) toast("Aucune racine de bibliothèque configurée.", false);
    else if (msg.toLowerCase().includes("upscale")) toast("Refusé : pas d'upscale lossy → lossless.", false);
    else toast(`Échec du rangement : ${msg}`, false);
    console.error("file_track failed", e);
    setActionsDisabled(mid, false);
    if (ranger && orig != null) ranger.innerHTML = orig;
  } finally {
    acting = false;
  }
}

/** Re-sourcer (fake) or Écarter (trash) the current track. */
async function doSecondary(mid: HTMLElement, kind: "resource" | "trash"): Promise<void> {
  if (!state.track || acting) return;
  acting = true;
  setActionsDisabled(mid, true);
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
    setActionsDisabled(mid, false);
  } finally {
    acting = false;
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

/** Banner HTML for a duplicate match (filed = already in library, pending = dupe in queue;
 * `both` = sound-confirmed, `name` = same name only → cautious wording). */
function dupBanner(m: DupMatch): string {
  const where =
    m.status === "filed"
      ? `Déjà rangé : ${esc((m.folder ? m.folder + "/" : "") + (m.filename || ""))}`
      : `Doublon d'un fichier en file : ${esc(m.filename || "")}`;
  const sure = m.kind === "both";
  const fg = sure ? "var(--color-text-warning)" : "var(--color-text-tertiary)";
  const bg = sure ? "var(--color-background-warning)" : "var(--color-background-secondary)";
  const head = sure ? "Doublon" : "Possible doublon (même nom — à vérifier)";
  return `<div style="display:flex;align-items:flex-start;gap:8px;background:${bg};border-radius:var(--border-radius-md);padding:8px 11px;margin-bottom:10px;font-size:11px"><i class="ti ti-copy" style="font-size:14px;flex:none;color:${fg}"></i><div style="min-width:0"><div style="font-weight:500;color:${fg}">${head}</div><div style="color:var(--color-text-tertiary);white-space:nowrap;overflow:hidden;text-overflow:ellipsis">${where}</div></div></div>`;
}

// Bumped on every open; an in-flight open bails at its await points if a newer one started
// (prevents a slow analyze/reconcile from clobbering the pane of a track opened since).
let openSeq = 0;

/** Render the analysis report + filing footer for `item` into the #mid pane. */
export async function openFilingInto(mid: HTMLElement, item: QueueItem): Promise<void> {
  const myseq = ++openSeq;
  state.track = item;
  state.target = null;
  state.canonical = null;

  mid.innerHTML =
    '<div class="sift-fil" style="display:flex;flex-direction:column;height:100%;min-height:0">' +
    '<div class="sift-fil-dup" style="flex:none"></div>' +
    '<div class="sift-fil-report" style="flex:1;min-height:0;overflow:auto"></div>' +
    '<div class="sift-fil-foot" style="flex:none;padding:10px 2px 2px;border-top:0.5px solid var(--color-border-tertiary)"></div>' +
    "</div>";
  const reportEl = mid.querySelector<HTMLElement>(".sift-fil-report");
  const footEl = mid.querySelector<HTMLElement>(".sift-fil-foot");
  if (!reportEl || !footEl) return;

  // Duplicate check (by name, sound-confirmed when available) — fill the banner slot async.
  void findDuplicate(item.id)
    .then((m) => {
      if (!m || state.track?.id !== item.id) return;
      const slot = mid.querySelector<HTMLElement>(".sift-fil-dup");
      if (slot) slot.innerHTML = dupBanner(m);
    })
    .catch((e) => console.error("find_duplicate failed", e));

  // Analysis report and metadata reconcile are independent DB reads — run them in
  // parallel so the footer renders as soon as both complete rather than sequentially.
  const [, canonical] = await Promise.all([
    openReportInto(reportEl, item.path),
    reconcile(item.id).catch((e): Canonical => {
      console.error("reconcile failed", e);
      return { artist: "", title: "", version: null, confidence: "yellow" };
    }),
  ]);
  if (myseq !== openSeq) return;

  state.canonical = canonical;
  // Tidy the casing of a version parsed from a (often lowercase) filename: "original mix"
  // → "Original Mix". Title/artist are left as reconciled.
  if (state.canonical.version) state.canonical.version = titleCase(state.canonical.version);

  // Default rail by extension (analysis data attribute not available cross-module).
  const ext = (item.path.split(".").pop() || "").toLowerCase();
  let rail = "unknown";
  if (["flac", "wav", "aif", "aiff", "alac"].includes(ext)) rail = "lossless";
  else if (["mp3", "m4a", "aac", "ogg"].includes(ext)) rail = "lossy";

  renderFoot(footEl, mid, rail);
  updateHeaderName(mid); // show the clean proposed name in the report header
}

/** Keyboard shortcuts for the open track (Revue): Space = play/pause, Enter = Ranger,
 * X = Écarter/Re-sourcer, I = Identifier. Ignored while typing in a field, and only when
 * a track is open. */
export function installFilingKeys(): void {
  document.addEventListener("keydown", (e) => {
    const t = e.target as HTMLElement | null;
    if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA")) return;
    if (!state.track) return; // only with a track open (i.e. on Revue)
    if (e.key === " ") {
      e.preventDefault(); // also stops Space from activating a focused button
      togglePlay();
    } else if (e.key === "Enter") {
      e.preventDefault();
      document.querySelector<HTMLElement>('[data-fil="ranger"]')?.click();
    } else if (e.key === "x" || e.key === "X") {
      document.querySelector<HTMLElement>('[data-fil="resource"],[data-fil="trash"]')?.click();
    } else if (e.key === "i" || e.key === "I") {
      // [m9] I = trigger Identifier (same as clicking the button)
      document.querySelector<HTMLButtonElement>('[data-fil="identifier"]')?.click();
    }
  });
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

/** Keep the detail pane in sync with the queue: if the open track is still pending, leave it
 * untouched; otherwise auto-load the first pending track into #mid — so tracks load without a
 * click, and after filing one the next opens automatically. Empty queue → neutral prompt.
 * Returns the id now shown (for the caller to highlight its row), or null. */
export function syncDetail(mid: HTMLElement, items: QueueItem[]): number | null {
  // Is our filing pane still in #mid? On navigation back to Revue, app.js re-draws its mock
  // detail into #mid, so the pane is no longer ours and must be re-rendered — but on a mere
  // queue/analysis refresh it's intact and we must NOT disrupt it (would restart playback).
  const paneIsOurs = !!mid.querySelector(".sift-fil");
  // If a track is open and our pane is intact, NEVER switch away from it — not even if it has
  // left the pending list (e.g. just analysed). Switching would destroy the player mid-load and
  // abort its audio (waveform shows from peaks, but no sound). This is the rule that keeps the
  // user's selection stable while the background worker churns through the queue.
  if (state.track && paneIsOurs) return state.track.id;
  // Pane was wiped (e.g. nav back to Revue re-draws app.js's mock) but we still have a track →
  // restore the real pane for it.
  if (state.track) {
    void openFilingInto(mid, state.track);
    return state.track.id;
  }
  // No track open → load the first pending one.
  if (items.length) {
    void openFilingInto(mid, items[0]);
    return items[0].id;
  }
  clearPane(mid);
  return null;
}
