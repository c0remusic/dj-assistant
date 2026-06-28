// Live Revue filing controller (Tauri only). Augments the mockup's Revue shell: renders the
// son-first analysis detail into the #mid pane, and the validation rail into the right .dest
// column — destination tree into #fldz (with a NoLibraryRoot picker gate) and the filing footer
// (editable canonical fields, format override, Identify, File / Re-source / Discard) into
// #filfoot below it. Drives the M4 backend via the IPC bindings; the plain-browser demo never
// loads this (see main.ts guard).
import {
  reconcile,
  fileTrack,
  listQueue,
  rejectTrack,
  trashTrack,
  listBins,
  createBin,
  getSetting,
  setSetting,
  undoLast,
  revertBatch,
  findDuplicate,
  identify,
  applyIdentity,
  trackRelease,
} from "./ipc";
import type { Candidate, AppliedIdentity } from "./ipc";
import type { DupMatch, TrackRelease } from "../shared/contracts";
import { open } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { openReportInto, togglePlay, vchipHtml } from "./report-view";
import { renderCandidates } from "./identify-shared";
import type { Bin, Canonical, Target, QueueItem } from "../shared/contracts";
import { FILE_IN_PLACE } from "../shared/contracts";
import { requireEl } from "./dom";

/** Banner label when a track was filed in place (its own source folder, not a tree bin). */
const IN_PLACE_BIN_LABEL = "source folder";

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
  binFilter: string; // folder search text (empty = show the full tree)
  track: QueueItem | null; // currently open track
  canonical: Canonical | null; // reconciled (then user-edited) metadata
  target: Target | null; // format override (null = backend rail default)
  // Analysed rail of the open track ("lossless" | "lossy" | "unknown"), set in openFilingInto. The
  // single source for the default format when target is null — used by BOTH the lit chip and the
  // Final-name preview (defaultTarget) so they never disagree on open.
  rail: string;
  // Read-only Discogs release facts for the open track. NOT part of Canonical (which drives the
  // filename/tags and is a Rust-mirrored contract) — kept here so the editor can show them. Loaded
  // from `releaseCache` on open, or set from `applied` on identify; null = unknown (no display).
  label: string | null;
  year: number | null;
  // After a Detail-mode filing, the just-filed track's batch_id + bin label → drives the
  // persistent "Filed ↩" confirmation in #mid (targeted revert via the journal). Null = none up.
  filedConfirm: { batchId: string; bin: string } | null;
}

const state: RevueState = {
  rootSet: false,
  rootPath: null,
  bins: [],
  binRel: null,
  creating: false,
  binFilter: "",
  track: null,
  canonical: null,
  target: null,
  rail: "unknown",
  label: null,
  year: null,
  filedConfirm: null,
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

/** Create a new bin under the current selection (or root when nothing is selected) and select
 * it. Nested creation: the parent is the folder currently highlighted, so "+ nouveau" while
 * "House" is selected makes "House/<name>". */
async function makeBin(fldz: HTMLElement, name: string): Promise<void> {
  const parent = state.binRel ?? ""; // "" = root; otherwise nest under the selected folder
  try {
    const bin = await createBin(parent, name);
    await loadBins();
    if (parent) expanded.add(parent); // reveal the freshly-created child
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
  if (!state.rootPath) return "Library";
  return state.rootPath.split(/[\\/]/).filter(Boolean).pop() || state.rootPath;
}

/** Absolute filesystem path of a bin (for the hover tooltip — "where on disk does this go?"),
 * using the library root's own path separator. */
function absPath(rel: string): string {
  const root = state.rootPath ?? "";
  if (!rel) return root || rootName();
  const sep = root.includes("\\") ? "\\" : "/";
  return `${root}${sep}${rel.replace(/\//g, sep)}`;
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
    ? `<span data-fil="caret" data-rel="${esc(node.rel)}" title="${isOpen ? "Collapse" : "Expand"}" style="display:inline-block;width:14px;text-align:center;cursor:pointer;color:var(--color-text-tertiary);transition:transform .2s;${
        isOpen ? "transform:rotate(90deg)" : ""
      }">▸</span>`
    : '<span style="display:inline-block;width:14px;flex:none"></span>';
  const icon = node.depth === 0 ? "ti-database" : "ti-folder";
  // Explicit highlight for the selected destination (don't rely on inherited .on CSS): tinted
  // background + an info-coloured folder icon + medium weight so the active bin is unmistakable.
  const sel = on
    ? "background:var(--color-background-info);border-radius:var(--border-radius-sm,4px)"
    : "";
  const iconColor = on ? "var(--color-text-info)" : "var(--color-text-tertiary)";
  const weight = on ? "font-weight:500;" : "";
  let html = `<div class="fld${on}" data-fil="bin" data-rel="${esc(node.rel)}" title="${esc(
    absPath(node.rel),
  )}" style="${sel};${weight}padding-left:${6 + indent}px;display:flex;align-items:center;gap:4px">${caret}<i class="ti ${icon}" style="font-size:var(--text-base);flex:none;color:${iconColor}"></i><span style="flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis">${esc(
    node.name,
  )}</span></div>`;
  if (kids.length && isOpen) html += kids.map(binNodeHtml).join("");
  return html;
}

/** Flat selectable row for the filtered view: shows the full relative path so the location is
 * obvious without the tree context, with the same highlight + absolute-path tooltip as the tree. */
function flatBinHtml(b: Bin): string {
  const on = b.rel === state.binRel ? " on" : "";
  const sel = on ? "background:var(--color-background-info);border-radius:var(--border-radius-sm,4px);" : "";
  const color = on ? "var(--color-text-info)" : "var(--color-text-tertiary)";
  return `<div class="fld${on}" data-fil="bin" data-rel="${esc(b.rel)}" title="${esc(
    absPath(b.rel),
  )}" style="${sel}padding:3px 6px;display:flex;align-items:center;gap:5px"><i class="ti ti-folder" style="font-size:var(--text-base);flex:none;color:${color}"></i><span style="flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis">${esc(
    b.rel,
  )}</span></div>`;
}

/** Render the destination column (#fldz): root picker when unset, else a folder filter + either
 * the collapsible tree (no filter) or a flat list of matching folders (filter active). */
export function renderBins(fldz: HTMLElement): void {
  if (!state.rootSet) {
    fldz.innerHTML =
      '<div style="font-size:var(--text-sm);color:var(--color-text-tertiary);margin-bottom:8px">Choose your library root to start filing.</div>' +
      '<button data-fil="pickroot"><i class="ti ti-folder" style="font-size:var(--text-base);vertical-align:-2px"></i> Choose root…</button>';
    fldz
      .querySelector('[data-fil="pickroot"]')
      ?.addEventListener("click", () => void pickRoot(fldz));
    return;
  }

  const filtering = state.binFilter.trim().length > 0;

  // Folder filter (only worth showing once there are sub-folders to sift through).
  const filterRow = state.bins.length
    ? `<input data-fil="binfilter" placeholder="Filter folders…" value="${esc(
        state.binFilter,
      )}" style="width:100%;font-size:var(--text-sm);padding:4px 7px;margin-bottom:6px;background:var(--color-background-secondary);border:0.5px solid var(--color-border-tertiary);border-radius:var(--border-radius-md);color:var(--color-text-primary);box-sizing:border-box">`
    : "";

  let body: string;
  if (filtering) {
    // Flat list of matches (path or name contains the query), case-insensitive.
    const q = state.binFilter.trim().toLowerCase();
    const matches = state.bins.filter(
      (b) => b.rel.toLowerCase().includes(q) || b.name.toLowerCase().includes(q),
    );
    body = matches.length
      ? matches.map(flatBinHtml).join("")
      : '<div style="font-size:var(--text-xs);color:var(--color-text-tertiary);padding:4px 0">No matching folder.</div>';
  } else {
    const tree = binNodeHtml({ rel: "", name: rootName(), depth: 0 });
    const emptyNote =
      state.bins.length === 0 && expanded.has("")
        ? '<div style="font-size:var(--text-xs);color:var(--color-text-tertiary);padding:2px 0 2px 33px">empty — create a folder</div>'
        : "";
    body = tree + emptyNote;
  }

  // "+ nouveau" creates under the selected folder (nested). Hidden while filtering.
  const nestLabel = state.binRel ? ` in ${binLabel()}` : "";
  const newRow = filtering
    ? ""
    : state.creating
      ? `<input data-fil="newin" placeholder="${esc(
          state.binRel ? `folder in ${binLabel()}…` : "folder name…",
        )}" style="width:100%;font-size:var(--text-md);padding:5px 7px;margin-top:2px;box-sizing:border-box">`
      : `<div class="fld" data-fil="newbin" style="color:var(--color-text-tertiary)"><i class="ti ti-plus" style="font-size:var(--text-lg)"></i> new${esc(
          nestLabel,
        )}</div>`;

  fldz.innerHTML = filterRow + body + newRow;

  // Re-render on every keystroke loses focus — restore it (caret at end) while filtering.
  if (filtering) {
    const fi = fldz.querySelector<HTMLInputElement>('[data-fil="binfilter"]');
    if (fi && document.activeElement !== fi) {
      fi.focus();
      fi.setSelectionRange(fi.value.length, fi.value.length);
    }
  }
  fldz.querySelector<HTMLInputElement>('[data-fil="binfilter"]')?.addEventListener("input", (e) => {
    state.binFilter = (e.target as HTMLInputElement).value;
    renderBins(fldz);
  });

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
  // Same default as the lit format chip (renderFoot): state.target when set, else defaultTarget(rail).
  // A hard-coded "mp3_320" here made an AIFF-source preview show ".mp3" while the AIFF chip was lit.
  const ext = targetExt(state.target ?? defaultTarget(state.rail));
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
  const c = state.canonical;
  if (!c) return; // before reconcile: keep the filename the report set
  // "Is the report pane still mounted?" is a normal question (mid may have been replaced after an
  // await / a navigation) → probe non-throw and bail, like renderQueue's `if (!ql) return`.
  const nameEl = mid.querySelector<HTMLElement>(".sift-report-name");
  if (!nameEl) return;
  // Board hero: big TITLE on top, "artist · version" subtitle below (not the full filename).
  nameEl.textContent = c.title || displayName();
  const subEl = mid.querySelector<HTMLElement>(".sift-report-sub");
  if (subEl) {
    const ver = c.version && c.version.trim() ? c.version.trim() : "";
    subEl.textContent = [c.artist, ver].filter(Boolean).join(" · ");
  }
}

/** Re-render just the Ranger button label (bin can change while a track is open). */
function refreshFootButton(): void {
  const btn = document.querySelector<HTMLElement>('[data-fil="ranger"] .sift-fil-bin');
  if (btn) btn.textContent = binLabel();
}

/** Re-sync the filename preview from the current canonical + target. The preview lives in the rail
 *  (#filfoot) right below the format chips, so a format change or a field edit must refresh this
 *  node (the extension follows state.target). Probe non-throw: the rail may be gone. */
function refreshPreview(): void {
  const prev = document.querySelector<HTMLElement>(".sift-fil-prev");
  if (prev) prev.textContent = `→ ${previewName()}`;
}

// Per-track Discogs release facts (label/year), captured when an identity is applied so they
// survive a close+reopen of the SAME track within the session. `reconcile` (the only open-time
// read) doesn't return label/year, and re-reading would need a new IPC — so we hold them in memory.
// Keyed by track id. Cross-session reopen won't repopulate this (a fresh process starts empty).
const releaseCache = new Map<number, { label: string | null; year: number | null }>();

/** Fill (or clear) the read-only "Label · Année" line from state.label/year. Shows only what
 *  exists — nothing at all when both are absent (no empty "—"). Mutates a stable `.sift-release`
 *  container (create-once), so an identify can refresh it without re-rendering the whole editor. */
function refreshReleaseLine(): void {
  const el = document.querySelector<HTMLElement>(".sift-release");
  if (!el) return; // editor not mounted (navigated away)
  const label = state.label && state.label.trim() ? state.label.trim() : null;
  const year = state.year != null ? String(state.year) : null;
  if (!label && !year) {
    el.innerHTML = "";
    return;
  }
  const value = [label, year].filter(Boolean).map((s) => esc(s as string)).join(" · ");
  el.innerHTML =
    `<div style="display:flex;align-items:center;gap:6px;font-size:var(--text-sm);color:var(--color-text-secondary);margin-bottom:10px">` +
    `<i class="ti ti-tag" style="font-size:var(--text-md);color:var(--color-text-tertiary)" title="Release (Discogs)"></i>` +
    `<span>${value}</span></div>`;
}

/** Apply an identity result to the editing fields + filename preview.
 * [C3] `host` + `allCandidates` are kept so we can show a "changer" confirmation row
 * instead of dead-ending (no new API call needed — re-renders from in-memory list). */
function onIdentityApplied(
  applied: AppliedIdentity,
  editor: HTMLElement,
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
  const aInp = editor.querySelector<HTMLInputElement>('[data-fil="artist"]');
  const tInp = editor.querySelector<HTMLInputElement>('[data-fil="title"]');
  const vInp = editor.querySelector<HTMLInputElement>('[data-fil="version"]');
  if (aInp) aInp.value = applied.canonical.artist;
  if (tInp) tInp.value = baseTitle;
  if (vInp) vInp.value = version ?? "";

  // Refresh the filename preview using the same logic as the input handler.
  refreshPreview();
  updateHeaderName(mid);

  // Read-only release facts from the chosen Discogs release. Cache them on the track so a
  // close+reopen within the session re-shows them (reconcile doesn't carry label/year). Choosing a
  // different candidate re-enters here with the new release → the line updates in place.
  state.label = applied.label;
  state.year = applied.year;
  if (state.track) releaseCache.set(state.track.id, { label: applied.label, year: applied.year });
  refreshReleaseLine();

  // Verdict-panel MATCH chip — qualitative (the backend has no % score): green confidence reads
  // as a confident MATCH, yellow as CHECK MATCH. Replaces any prior MATCH chip on re-identify.
  const vchips = mid.querySelector<HTMLElement>(".sift-vchips");
  if (vchips) {
    vchips.querySelector('[data-chip="match"]')?.remove();
    const green = applied.canonical.confidence === "green";
    vchips.insertAdjacentHTML(
      "beforeend",
      vchipHtml(green ? "MATCH" : "CHECK MATCH", green ? "success" : "warning").replace("<span ", '<span data-chip="match" '),
    );
  }

  // Show the cover if we have a local path. Probe non-throw — the report pane may be gone after the
  // identify await / a navigation.
  if (applied.cover_path) {
    const covEl = mid.querySelector<HTMLImageElement>(".sift-report-cover");
    if (covEl) {
      covEl.src = convertFileSrc(applied.cover_path);
      covEl.hidden = false;
    }
  }

  // [m11] Render genre/style chips with tooltip so they read as informational Discogs sub-genres.
  // .sift-genres lives in the center editor host now, so query `editor` rather than the #mid pane.
  const genEl = editor.querySelector<HTMLElement>(".sift-genres");
  if (genEl) {
    genEl.innerHTML = applied.styles
      .map((s) => `<span class="sift-genre-chip" title="Discogs sub-genres">${esc(s)}</span>`)
      .join("");
  }

  // [C3] Collapse candidate zone to a confirmation row + "changer" link (no dead-end).
  // Re-labelling the Identifier button to "Ré-identifier" is also handled here.
  host.hidden = false;
  host.innerHTML = identifiedLineHtml(applied.canonical.artist, applied.canonical.title, applied.cover_path);

  const changerBtn = host.querySelector<HTMLElement>('[data-fil="cand-changer"]');
  changerBtn?.addEventListener("click", () => {
    // Re-show the full candidate list from memory (no new API call).
    host.innerHTML = "";
    renderCandidates(host, allCandidates);
    wireCandidateClicks(host, allCandidates, editor, mid, idBtn);
  });

  // [C1] Relabel Identifier → Ré-identifier once an identity has been applied.
  idBtn.innerHTML = '<i class="ti ti-refresh" style="font-size:var(--text-sm);vertical-align:-1px"></i> Re-identify';
}

/** Markup for the "Identified: artist — title" confirmation line (cover thumb + "change" button).
 *  Single source of truth, reused by a fresh fetch (onIdentityApplied) and by the reopen of an
 *  already-identified track (restoreIdentifiedLine) so both render identically. */
function identifiedLineHtml(artist: string, title: string, coverPath: string | null): string {
  const coverThumb = coverPath
    ? `<img src="${esc(convertFileSrc(coverPath))}" alt="" style="width:28px;height:28px;border-radius:3px;object-fit:cover;flex:none">`
    : `<span style="width:28px;height:28px;border-radius:3px;background:var(--color-background-secondary);display:inline-flex;align-items:center;justify-content:center;flex:none"><i class="ti ti-vinyl" style="font-size:var(--text-lg);color:var(--color-text-tertiary)"></i></span>`;
  return (
    `<div style="display:flex;align-items:center;gap:7px;padding:4px 2px">` +
    coverThumb +
    `<span style="flex:1;min-width:0;font-size:var(--text-md);overflow:hidden;text-overflow:ellipsis;white-space:nowrap">` +
    `<span style="color:var(--color-text-secondary)">Identified:</span> ${esc(artist)} — ${esc(title)}` +
    `</span>` +
    `<button class="sift-cand-jump" data-fil="cand-changer" style="font-size:var(--text-sm);padding:2px 8px;flex:none">change</button>` +
    `</div>`
  );
}

/** On (re)open of an already-identified track (track_release.identified), show the "Identified" line
 *  in place of the bare Fetch button — same markup as a fresh fetch, rebuilt from `metadata` (cover
 *  included), ZERO network. The original candidate list is gone after a close / cold start, so here
 *  "change" re-runs a Discogs fetch (Antoine's call) rather than re-showing a list we no longer have.
 *  `editor` is the center editor host; `.sift-cands` + the Identifier button live inside it. */
function restoreIdentifiedLine(
  editor: HTMLElement,
  mid: HTMLElement,
  artist: string,
  title: string,
  coverPath: string | null,
): void {
  const host = editor.querySelector<HTMLElement>(".sift-cands");
  const idBtn = editor.querySelector<HTMLButtonElement>('[data-fil="identifier"]');
  if (!host || !idBtn) return;
  host.hidden = false;
  host.innerHTML = identifiedLineHtml(artist, title, coverPath);
  // [C1] Match the post-fetch state: the primary button reads "Re-identify".
  idBtn.innerHTML = '<i class="ti ti-refresh" style="font-size:var(--text-sm);vertical-align:-1px"></i> Re-identify';
  // Cold-start "change": the original candidates aren't in memory → re-run a Discogs fetch.
  host.querySelector<HTMLElement>('[data-fil="cand-changer"]')?.addEventListener("click", () => {
    void doIdentify(idBtn, host, editor, mid);
  });
}

/** Wire clicks on rendered candidate buttons.
 * Extracted so it can be called after initial render AND after "changer" re-shows the list. */
function wireCandidateClicks(
  host: HTMLElement,
  candidates: Candidate[],
  editor: HTMLElement,
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
          onIdentityApplied(applied, editor, mid, host, candidates, idBtn);
        })
        .catch((e) => {
          el.style.opacity = "";
          el.style.pointerEvents = "";
          // [m10] errors get a warning icon to distinguish from "no results"
          host.innerHTML = `<div class="sift-cands-msg sift-cands-error"><i class="ti ti-alert-triangle" style="font-size:var(--text-md);vertical-align:-2px;margin-right:4px"></i>${esc(String(e))}</div>`;
        });
    });
  });
}

/** Run the Discogs identify flow for the current track. */
async function doIdentify(
  btn: HTMLButtonElement,
  host: HTMLElement,
  editor: HTMLElement,
  mid: HTMLElement,
): Promise<void> {
  if (!state.track) return;
  const trackId = state.track.id;
  const origLabel = btn.innerHTML;
  btn.disabled = true;
  btn.innerHTML = '<i class="ti ti-loader-2 sift-spin" style="font-size:var(--text-sm);vertical-align:-1px"></i> Searching…';
  host.hidden = false;
  host.innerHTML = '<div class="sift-cands-msg">Searching…</div>';

  let candidates: Candidate[] = [];
  try {
    candidates = await identify(trackId);
    renderCandidates(host, candidates);
    wireCandidateClicks(host, candidates, editor, mid, btn);
  } catch (err) {
    const msg = String(err);
    if (msg.includes("NO_TOKEN")) {
      // [C2/m5] explain WHY + give a direct action to open Réglages
      host.innerHTML =
        `<div class="sift-cands-msg">Discogs throttles anonymous searches — add your (free) token in Settings.</div>` +
        `<button class="sift-cand-jump" data-fil="goto-reglages" style="margin-top:5px;font-size:var(--text-sm);padding:3px 9px">Open Settings →</button>`;
      const gotoBtn = host.querySelector<HTMLElement>('[data-fil="goto-reglages"]');
      gotoBtn?.addEventListener("click", () => {
        // Navigate to the Réglages view via the existing nav click handler in app.js
        requireEl('[data-view="reglages"]', "filing goto-reglages").dispatchEvent(
          new MouseEvent("click", { bubbles: true }),
        );
      });
    } else {
      const rl = msg.match(/RATE_LIMITED:(\d+)/);
      if (rl) {
        host.innerHTML = `<div class="sift-cands-msg">Discogs is rate-limiting — retry in ${rl[1]}s.</div>`;
      } else {
        // [m10] network/server errors get a warning icon to distinguish from "no results"
        host.innerHTML = `<div class="sift-cands-msg sift-cands-error"><i class="ti ti-alert-triangle" style="font-size:var(--text-md);vertical-align:-2px;margin-right:4px"></i>Discogs unreachable.</div>`;
      }
    }
  } finally {
    btn.disabled = false;
    btn.innerHTML = origLabel;
  }
}

/** Render the filing rail (format + actions) into `foot`. The metadata editor (Identify + editable
 *  fields + final-name preview + genres) lives in the center now — see `renderEditor`. */
function renderFoot(foot: HTMLElement, mid: HTMLElement, rail: string): void {
  // Preserve the "Filed" banner across re-renders: it is appended at the BOTTOM of #filfoot (étape 2)
  // and must survive renderFoot's innerHTML rewrite (e.g. a format-chip click) until the next filing or ✕.
  const filedBanner = foot.querySelector(".sift-filed-banner");
  if (!state.canonical) {
    foot.innerHTML = "";
    if (filedBanner) foot.append(filedBanner);
    return;
  }

  const lossy = rail === "lossy";
  const chips = (["mp3_320", "aiff_16_44", "wav_16_44"] as Target[])
    .map((t) => {
      // a lossy source can't be upscaled to lossless — disable AIFF/WAV (the backend refuses
      // it anyway; greying it out prevents the dead-end click).
      if (lossy && t !== "mp3_320")
        return `<span class="chip" title="No upscale from a lossy file" style="opacity:.4;cursor:not-allowed">${TARGET_LABEL[t]}</span>`;
      const on = (state.target ?? defaultTarget(rail)) === t ? " on" : "";
      return `<span class="chip${on}" data-fil="fmt" data-t="${t}">${TARGET_LABEL[t]}</span>`;
    })
    .join(" ");

  const fake = state.track?.verdict === "fake";
  const secondary = fake
    ? '<button data-fil="resource" style="width:100%;color:var(--color-text-warning)" title="Fake file — goes to Discarded (⌫)"><span class="kbd">⌫</span> <i class="ti ti-alert-triangle" style="font-size:var(--text-md);vertical-align:-2px"></i> Re-source</button>'
    : '<button data-fil="trash" style="width:100%;color:var(--color-text-danger)" title="Send to trash (⌫)"><span class="kbd">⌫</span> <i class="ti ti-trash" style="font-size:var(--text-md);vertical-align:-2px"></i> Discard</button>';

  // The rail keeps the system.md stack tail: FORMAT → Final name → CTA (File) → secondary. The
  // filename preview sits right above File so you see the name about to be produced next to the
  // action that produces it. Rebuilt inside this innerHTML so a format-chip re-render keeps it.
  foot.innerHTML =
    `<div class="col-h" style="margin-bottom:4px">Format</div>` +
    `<div style="display:flex;flex-wrap:wrap;gap:5px;margin-bottom:12px">${chips}</div>` +
    `<div class="col-h" style="margin-bottom:4px">Final name</div>` +
    `<div class="sift-fil-prev" style="font-size:var(--text-xs);color:var(--color-text-tertiary);font-family:var(--font-mono);word-break:break-all;line-height:1.5;margin-bottom:12px">→ ${esc(previewName())}</div>` +
    `<button data-fil="ranger" style="width:100%;background:var(--color-background-info);color:var(--color-text-info);border:none;font-weight:500;margin-bottom:6px"><i class="ti ti-corner-down-left" style="font-size:var(--text-md);vertical-align:-2px"></i> File → <span class="sift-fil-bin">${esc(binLabel())}</span> <span class="kbd">⏎</span></button>` +
    secondary;
  if (filedBanner) foot.append(filedBanner); // restore the banner below the freshly-rendered controls

  foot.querySelectorAll<HTMLElement>('[data-fil="fmt"]').forEach((el) =>
    el.addEventListener("click", () => {
      state.target = (el.dataset.t as Target) || null;
      renderFoot(foot, mid, rail);
      refreshPreview(); // the chosen format sets the filename extension shown in the rail preview
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

/** Render the center metadata editor (Identify + editable fields + genres) into `host`, below the
 *  analysis report. The final-name preview lives in the rail (`renderFoot`) next to the File button;
 *  this pane ends with genres. One-shot innerHTML — called once per track open, not on a
 *  burst event, so create-once/update-in-place is not required here. `rail` is accepted for symmetry
 *  with renderFoot; the editor itself is format-agnostic (the extension comes from state.target). */
function renderEditor(host: HTMLElement, mid: HTMLElement, rail: string): void {
  void rail;
  const c = state.canonical;
  if (!c) {
    host.innerHTML = "";
    return;
  }

  // [I6] Add tooltip to confidence badge so the colour is self-explanatory.
  const badge =
    c.confidence === "green"
      ? '<span title="Title and artist extracted confidently" style="display:inline-flex;align-items:center;gap:4px;font-size:var(--text-xs);color:var(--color-text-success)"><i class="ti ti-circle-check" style="font-size:var(--text-sm)"></i> metadata trusted</span>'
      : '<span title="Title or artist couldn\'t be extracted with certainty — check the fields" style="display:inline-flex;align-items:center;gap:4px;font-size:var(--text-xs);color:var(--color-text-warning)"><i class="ti ti-alert-circle" style="font-size:var(--text-sm)"></i> check fields</span>';

  const inputCss =
    "font-size:var(--text-md);padding:4px 7px;background:var(--color-background-secondary);border:0.5px solid var(--color-border-tertiary);border-radius:var(--border-radius-md);color:var(--color-text-primary);min-width:0";

  // [C1] "Fetch metadata from Discogs" is the primary entry point (gold filled), above the inputs.
  // [C2] title= explains what it does; the kbd hint shows the I shortcut.
  // Vertical order: pick the Discogs release FIRST (badge → Fetch → candidates), then edit the
  // fields it populates (artist/title/version → Genres directly under Version). The Final name
  // preview moved to the rail (next to File). `.sift-cands` sits above the inputs so choosing a
  // release precedes editing.
  host.innerHTML =
    `<div class="col-h" style="margin-bottom:6px">Métadonnées</div>` +
    `<div style="margin-bottom:8px">${badge}</div>` +
    `<button data-fil="identifier" class="sift-id-btn" style="width:100%;margin-bottom:8px" title="Search metadata on Discogs (cover, label, year, genres)"><i class="ti ti-search" style="font-size:var(--text-md);vertical-align:-1px"></i> Fetch metadata from Discogs <span class="kbd" style="font-size:var(--text-2xs);border-color:rgba(0,0,0,.18);color:rgba(0,0,0,.5)">I</span></button>` +
    `<div class="sift-cands" hidden style="margin-bottom:8px"></div>` +
    `<div style="display:grid;grid-template-columns:1fr;gap:5px;margin-bottom:8px">` +
    `<input data-fil="artist" placeholder="Artist" value="${esc(c.artist)}" style="${inputCss}">` +
    `<input data-fil="title" placeholder="Title" value="${esc(c.title)}" style="${inputCss}">` +
    `<input data-fil="version" placeholder="Version" value="${esc(c.version ?? "")}" style="${inputCss}">` +
    `</div>` +
    // Read-only release facts (Label · Année) between the editable identity and Genres. Filled by
    // refreshReleaseLine() below from state; stays empty (no gap) when neither value is known.
    `<div class="sift-release"></div>` +
    `<div class="col-h" style="margin-bottom:4px">Genres</div>` +
    `<div class="sift-genres" style="margin-bottom:10px;min-height:1px"></div>`;

  const upd = () => {
    const a = host.querySelector<HTMLInputElement>('[data-fil="artist"]');
    const t = host.querySelector<HTMLInputElement>('[data-fil="title"]');
    const v = host.querySelector<HTMLInputElement>('[data-fil="version"]');
    if (!state.canonical) return;
    state.canonical.artist = a?.value ?? "";
    state.canonical.title = t?.value ?? "";
    state.canonical.version = v?.value.trim() ? v.value.trim() : null;
    refreshPreview();
    updateHeaderName(mid); // keep the report header's clean name in sync with edits
  };
  host
    .querySelectorAll<HTMLInputElement>('[data-fil="artist"],[data-fil="title"],[data-fil="version"]')
    .forEach((el) => el.addEventListener("input", upd));

  const idBtn = host.querySelector<HTMLButtonElement>('[data-fil="identifier"]');
  const candsHost = host.querySelector<HTMLElement>(".sift-cands");
  if (idBtn && candsHost) {
    idBtn.addEventListener("click", () => void doIdentify(idBtn, candsHost, host, mid));
  }

  refreshReleaseLine(); // read-only Label · Année from state (restored from cache on open); empty when none
}

/** A transient toast at the bottom-right with an optional "Annuler" action. */
function toast(message: string, undo: boolean): void {
  document.getElementById("sift-toast")?.remove();
  const el = document.createElement("div");
  el.id = "sift-toast";
  el.style.cssText =
    "position:fixed;right:18px;bottom:18px;z-index:9998;display:flex;align-items:center;gap:12px;background:var(--color-background-secondary);border:0.5px solid var(--color-border-secondary);border-radius:var(--border-radius-md);padding:9px 13px;font-size:var(--text-md);color:var(--color-text-primary);box-shadow:0 8px 28px rgba(0,0,0,.4)";
  el.innerHTML =
    `<span>${esc(message)}</span>` +
    (undo
      ? '<button data-fil="undo" style="font-size:var(--text-sm);padding:2px 9px">Undo</button>'
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

/** Disable/enable the rail action buttons (visible feedback while an action runs). The buttons
 *  live in #filfoot now, so query the document rather than the #mid pane. */
function setActionsDisabled(disabled: boolean): void {
  document
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
  // "Sur place" checked → destination is the track's own source folder (sentinel), bypassing the
  // tree selection. The sentinel rides the normal binRel channel — no separate flag (single channel).
  const inPlace = fileInPlaceChecked();
  const dest = inPlace ? FILE_IN_PLACE : state.binRel;
  if (dest === null) {
    toast("Choose a destination folder.", false);
    return;
  }
  const ranger = document.querySelector<HTMLElement>('[data-fil="ranger"]');
  const orig = ranger?.innerHTML ?? null;
  acting = true;
  setActionsDisabled(true);
  if (ranger)
    ranger.innerHTML =
      '<i class="ti ti-loader-2 sift-spin" style="font-size:var(--text-md);vertical-align:-2px"></i> Filing…';
  try {
    const res = await fileTrack(state.track.id, dest, state.target, state.canonical);
    // Capture the "after" facts for the rail banner BEFORE we advance (state resets on the next open).
    const filedPath = res.path;
    const batchId = res.batch_id;
    const bin = inPlace ? IN_PLACE_BIN_LABEL : binLabel();
    // Auto-advance: the filed track has left the pending list, so switching away from it here is
    // LEGITIMATE — this is the one place allowed to switch outside syncDetail's player guard, because
    // we KNOW the current track was just filed (never on a passive analysis refresh). Reuse the
    // existing load path openFilingInto; fresh pending list → items[0] is the next track to file.
    let items: QueueItem[] = [];
    try {
      items = await listQueue();
    } catch (err) {
      console.error("listQueue failed after filing", err);
    }
    if (items.length) await openFilingInto(mid, items[0]);
    else clearPane(mid); // no pending left → neutral center; the banner still shows in the rail
    // Filed confirmation as a banner at the BOTTOM of the right rail, under the new track's controls
    // (renderFoot, run by openFilingInto above, already wrote them; the banner is appended below them).
    showFiledConfirm(batchId, bin, filedPath);
  } catch (e) {
    const msg = String(e);
    if (msg.includes("NoLibraryRoot")) toast("No library root configured.", false);
    else if (msg.toLowerCase().includes("upscale")) toast("Refused: no lossy → lossless upscale.", false);
    else toast(`Filing failed: ${msg}`, false);
    console.error("file_track failed", e);
    setActionsDisabled(false);
    if (ranger && orig != null) ranger.innerHTML = orig;
  } finally {
    acting = false;
  }
}

/** Show the "Filed ✓ ↩" confirmation as a BANNER at the TOP of the right rail (#filfoot), above the
 *  next track's controls — the center has already auto-advanced to the next pending track (doRanger).
 *  This is the "after" proof for the file just filed: name + destination path + a targeted Revert.
 *  ONE banner at a time (replaces any prior). Revert is targeted on this file's `batchId`
 *  (revert_batch), available indefinitely via the journal; the ✕ dismisses the banner without
 *  reverting. Does NOT touch #mid or state.track — the advance owns those. */
function showFiledConfirm(batchId: string, bin: string, filedPath: string): void {
  state.filedConfirm = { batchId, bin };
  const foot = document.getElementById("filfoot");
  if (!foot) return; // rail gone (navigated away while the file completed) — nothing to show
  const filename = filedPath.split(/[\\/]/).pop() || filedPath;
  foot.querySelector(".sift-filed-banner")?.remove(); // one at a time — replace any prior banner
  const banner = document.createElement("div");
  banner.className = "sift-filed-banner";
  // CDS single-side accent: success border-left, square corners. Success tint sets it apart from the
  // secondary-coloured rail. renderFoot preserves this node across its re-renders (format clicks).
  // margin-top (not -bottom): the banner sits at the BOTTOM of the rail, under Discard — space it above.
  banner.style.cssText =
    "margin-top:12px;padding:7px 10px;background:var(--color-background-success);border-left:2px solid var(--color-text-success);font-size:var(--text-sm)";
  banner.innerHTML =
    `<div style="display:flex;align-items:center;gap:6px;margin-bottom:5px">` +
    `<i class="ti ti-check" style="font-size:var(--text-md);color:var(--color-text-success)"></i>` +
    `<span style="color:var(--color-text-success);font-weight:500">Filed</span>` +
    `<span style="color:var(--color-text-tertiary);min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">→ ${esc(bin)}</span>` +
    `<button data-fil="filed-close" title="Dismiss" style="margin-left:auto;flex:none;background:none;border:none;color:var(--color-text-tertiary);cursor:pointer;padding:0;line-height:1"><i class="ti ti-x" style="font-size:var(--text-md)"></i></button>` +
    `</div>` +
    `<div style="color:var(--color-text-secondary);font-family:var(--font-mono);font-size:var(--text-xs);word-break:break-all;line-height:1.5;margin-bottom:3px">${esc(filename)}</div>` +
    `<div style="color:var(--color-text-tertiary);font-family:var(--font-mono);font-size:var(--text-2xs);word-break:break-all;line-height:1.4;margin-bottom:7px">${esc(filedPath)}</div>` +
    `<button data-fil="revert" style="display:inline-flex;align-items:center;gap:5px;font-size:var(--text-sm);padding:3px 10px;background:var(--color-background-secondary);border:0.5px solid var(--color-border-secondary);border-radius:var(--border-radius-md);color:var(--color-text-secondary);cursor:pointer"><i class="ti ti-arrow-back-up" style="font-size:var(--text-md);vertical-align:-2px"></i> Revert</button>`;
  foot.append(banner); // at the BOTTOM of the rail — last child, under Format → File → Discard
  banner.querySelector('[data-fil="revert"]')?.addEventListener("click", () => void doRevert(batchId));
  banner.querySelector('[data-fil="filed-close"]')?.addEventListener("click", () => {
    banner.remove();
    state.filedConfirm = null;
  });
}

/** Revert THIS file's filing, targeted on its `batchId` (revert_batch). On success the engine
 *  puts the track back to pending and emits queue:changed → the queue refreshes. On a Blocked
 *  engine error (e.g. the original was purged from the trash) show a clear message rather than
 *  failing mutely. The revert engine itself is untouched here. */
async function doRevert(batchId: string): Promise<void> {
  try {
    await revertBatch(batchId);
    // The filing is undone → drop the banner. The reverted file returns to pending (backend emits
    // queue:changed → the queue list refreshes). We do NOT clearPane: the auto-advanced track in
    // #mid stays put (syncDetail's player guard keeps it), so reverting never yanks the player.
    document.getElementById("filfoot")?.querySelector(".sift-filed-banner")?.remove();
    state.filedConfirm = null;
    toast("Reverted — back in the queue", false);
  } catch (e) {
    const msg = String(e);
    if (msg.includes("source gone")) {
      toast("Revert unavailable: a needed file is gone — the original may have been purged from the trash.", false);
    } else {
      toast(`Revert failed: ${msg}`, false);
    }
    console.error("revert failed", e);
  }
}

/** Re-sourcer (fake) or Écarter (trash) the current track. */
async function doSecondary(mid: HTMLElement, kind: "resource" | "trash"): Promise<void> {
  if (!state.track || acting) return;
  acting = true;
  setActionsDisabled(true);
  try {
    if (kind === "resource") {
      await rejectTrack(state.track.id);
      toast("Marked to re-source", true);
    } else {
      await trashTrack(state.track.id);
      toast("Sent to trash", true);
    }
    clearPane(mid);
  } catch (e) {
    toast(`Failed: ${String(e)}`, false);
    console.error(`${kind} failed`, e);
    setActionsDisabled(false);
  } finally {
    acting = false;
  }
}

/** Empty the detail pane back to a neutral prompt (after an action). */
function clearPane(mid: HTMLElement): void {
  state.track = null;
  state.canonical = null;
  state.target = null;
  state.label = null;
  state.year = null;
  state.filedConfirm = null;
  mid.innerHTML =
    '<div style="flex:1;display:flex;align-items:center;justify-content:center;color:var(--color-text-tertiary);font-size:var(--text-md);padding:20px;text-align:center">Select a track in the queue to listen and file it.</div>';
  // The validation footer lives in the rail (#filfoot); clear it too so no stale controls linger
  // (non-throw: clearPane runs from async revert/undo/secondary callbacks that may fire off Review).
  const ff = document.getElementById("filfoot");
  if (ff) ff.innerHTML = "";
}

/** Banner HTML for a duplicate match (filed = already in library, pending = dupe in queue;
 * `both` = sound-confirmed, `name` = same name only → cautious wording). */
function dupBanner(m: DupMatch): string {
  const where =
    m.status === "filed"
      ? `Already filed: ${esc((m.folder ? m.folder + "/" : "") + (m.filename || ""))}`
      : `Duplicate of a queued file: ${esc(m.filename || "")}`;
  const sure = m.kind === "both";
  const fg = sure ? "var(--color-text-warning)" : "var(--color-text-tertiary)";
  const bg = sure ? "var(--color-background-warning)" : "var(--color-background-secondary)";
  const head = sure ? "Duplicate" : "Possible duplicate (same name — check)";
  return `<div style="display:flex;align-items:flex-start;gap:8px;background:${bg};border-radius:var(--border-radius-md);padding:8px 11px;margin-bottom:10px;font-size:var(--text-sm)"><i class="ti ti-copy" style="font-size:var(--text-lg);flex:none;color:${fg}"></i><div style="min-width:0"><div style="font-weight:500;color:${fg}">${head}</div><div style="color:var(--color-text-tertiary);white-space:nowrap;overflow:hidden;text-overflow:ellipsis">${where}</div></div></div>`;
}

// Bumped on every open; an in-flight open bails at its await points if a newer one started
// (prevents a slow analyze/reconcile from clobbering the pane of a track opened since).
let openSeq = 0;

/** True when the detail-mode "file in place" checkbox is ticked: File targets the track's own
 *  source folder (FILE_IN_PLACE) instead of the bin selected in the #fldz tree. */
function fileInPlaceChecked(): boolean {
  return !!document.querySelector<HTMLInputElement>('[data-fil="inplace"]')?.checked;
}

/** Insert the "file in place" checkbox ONCE, as a sibling between the bin tree (#fldz) and the
 *  action stack (#filfoot) — the least intrusive spot in the .dest column. Its checked state
 *  persists across track opens (so a run of in-place filings needs one tick). */
function ensureInPlaceToggle(): void {
  if (document.getElementById("fil-inplace")) return;
  const foot = document.getElementById("filfoot");
  if (!foot?.parentElement) return;
  const wrap = document.createElement("label");
  wrap.id = "fil-inplace";
  wrap.style.cssText =
    "display:flex;align-items:center;gap:7px;margin-top:12px;font-size:var(--text-sm);color:var(--color-text-secondary);cursor:pointer";
  wrap.innerHTML =
    '<input type="checkbox" data-fil="inplace" style="cursor:pointer;flex:none">' +
    '<span>Sur place <span style="color:var(--color-text-tertiary)">(dossier source)</span></span>';
  foot.parentElement.insertBefore(wrap, foot);
}

/** Render the analysis report + filing footer for `item` into the #mid pane. */
export async function openFilingInto(mid: HTMLElement, item: QueueItem): Promise<void> {
  const myseq = ++openSeq;
  state.track = item;
  state.target = null;
  state.canonical = null;
  // Seed read-only release facts SYNCHRONOUSLY from the session cache (set on a prior identify this
  // session) so a re-open paints label/year with no flash. The persisted `metadata` table is the
  // source of truth and is read below (trackRelease) — it primes the cold-start case (cache empty).
  const cachedRelease = releaseCache.get(item.id);
  state.label = cachedRelease?.label ?? null;
  state.year = cachedRelease?.year ?? null;
  state.filedConfirm = null; // opening a track dismisses any "Filed ↩" confirmation

  mid.innerHTML =
    '<div class="sift-fil" style="display:flex;flex-direction:column;height:100%;min-height:0">' +
    '<div class="sift-fil-dup" style="flex:none"></div>' +
    '<div class="sift-fil-scroll" style="flex:1;min-height:0;overflow:auto">' +
    '<div class="sift-fil-report"></div>' +
    '<div class="sift-fil-editor" style="margin-top:8px;padding-top:8px;border-top:0.5px solid var(--color-border-tertiary)"></div>' +
    '</div>' +
    "</div>";
  const reportEl = requireEl<HTMLElement>(".sift-fil-report", "openFilingInto", mid);
  // The validation footer now lives in the right rail (#filfoot in the .dest column), below the
  // destination tree — so #mid is a pure son-first detail and the rail holds the filing stack.
  const footEl = requireEl("#filfoot", "openFilingInto");
  ensureInPlaceToggle(); // "Sur place" checkbox between the tree and the action stack (once)

  // Duplicate check (by name, sound-confirmed when available) — drives both the banner slot and
  // the verdict-panel UNIQUE/DUPLICATE chip (appended once the panel exists, see end of fn).
  const dupP = findDuplicate(item.id).catch((e): DupMatch | null => {
    console.error("find_duplicate failed", e);
    return null;
  });
  void dupP.then((m) => {
    if (!m || state.track?.id !== item.id) return;
    const slot = mid.querySelector<HTMLElement>(".sift-fil-dup");
    if (slot) slot.innerHTML = dupBanner(m);
  });

  // Analysis report, metadata reconcile, and the persisted release facts are independent DB reads —
  // run them in parallel so the footer renders as soon as they complete rather than sequentially.
  const [, canonical, release] = await Promise.all([
    openReportInto(reportEl, item.path),
    reconcile(item.id).catch((e): Canonical => {
      console.error("reconcile failed", e);
      return { artist: "", title: "", version: null, confidence: "yellow" };
    }),
    trackRelease(item.id).catch((e): TrackRelease => {
      console.error("track_release failed", e);
      return { artist: null, title: null, version: null, label: null, year: null, cover_path: null, identified: false };
    }),
  ]);
  if (myseq !== openSeq) return; // a newer open started while we awaited — don't paint this track

  // When a Discogs identity was applied earlier but not yet filed, the file tags still hold the OLD
  // name, so reconcile (which reads those tags) would wipe the chosen identity on reopen. Trust the
  // persisted metadata instead: artist/title from `metadata`, confidence green (a validated Discogs
  // match), and version kept from reconcile (the filename — metadata has no version column and
  // Discogs has no version field). Not identified → reconcile stays the source, as before.
  state.canonical =
    release.identified && release.artist && release.title
      ? {
          artist: release.artist,
          title: release.title,
          // Prefer the remix/dub stored when the release was chosen; fall back to reconcile's
          // filename-parsed version (metadata has none for that track, e.g. a Discogs title with
          // no parenthetical but a "(Dub)" filename).
          version: release.version ?? canonical.version,
          confidence: "green",
        }
      : canonical;
  // The persisted `metadata` table is the source of truth for label/year (the session cache above
  // was only a flash-avoiding seed). Cold start: this is where an identified-not-filed track gets
  // its identity + label/year back. Keep the cache in sync so later re-opens stay synchronous.
  state.label = release.label;
  state.year = release.year;
  releaseCache.set(item.id, { label: release.label, year: release.year });
  // Tidy the casing of a version parsed from a (often lowercase) filename: "original mix"
  // → "Original Mix". Title/artist are left as reconciled.
  if (state.canonical.version) state.canonical.version = titleCase(state.canonical.version);

  // Default rail by extension (analysis data attribute not available cross-module).
  const ext = (item.path.split(".").pop() || "").toLowerCase();
  let rail = "unknown";
  if (["flac", "wav", "aif", "aiff", "alac"].includes(ext)) rail = "lossless";
  else if (["mp3", "m4a", "aac", "ogg"].includes(ext)) rail = "lossy";
  state.rail = rail; // so previewName/refreshPreview default the extension like the lit chip does

  renderFoot(footEl, mid, rail);
  const editorEl = requireEl<HTMLElement>(".sift-fil-editor", "openFilingInto", mid);
  renderEditor(editorEl, mid, rail);
  // Already-identified track → show the "Identified" line (cover + release) in place of the bare
  // Fetch button, rebuilt from metadata (no network). Runs inside the openSeq-guarded section above,
  // so a superseded open never paints this onto the wrong track.
  if (release.identified && state.canonical) {
    restoreIdentifiedLine(editorEl, mid, state.canonical.artist, state.canonical.title, release.cover_path);
  }
  updateHeaderName(mid); // show the clean proposed name in the report header

  // Verdict-panel chip (board: LOSSLESS · MATCH · UNIQUE): append UNIQUE by default, DUPLICATE
  // when dedup found a match. The MATCH chip is added later by onIdentityApplied.
  void dupP.then((m) => {
    if (myseq !== openSeq) return;
    const chips = mid.querySelector<HTMLElement>(".sift-vchips");
    if (!chips || chips.querySelector('[data-chip="dup"]')) return;
    chips.insertAdjacentHTML(
      "beforeend",
      vchipHtml(m ? "DUPLICATE" : "UNIQUE", m ? "warning" : "neutral").replace("<span ", '<span data-chip="dup" '),
    );
  });
}

/** Keyboard shortcuts for the open track (Revue): ↑/↓ = focus prev/next queue row,
 * Space = play/pause, Enter = File, Backspace (⌫) / X = Discard/Re-source, I = Identify.
 * Matches interaction-model.md §7. Ignored while typing in a field, and only when a track
 * is open. */
export function installFilingKeys(): void {
  document.addEventListener("keydown", (e) => {
    const t = e.target as HTMLElement | null;
    if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA")) return;
    if (!state.track) return; // only with a track open (i.e. on Revue)
    if (e.key === " ") {
      e.preventDefault(); // also stops Space from activating a focused button
      togglePlay();
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      // ↑/↓ moves focus through the live queue: click the prev/next row, which opens it in
      // the detail pane via the #pa delegated handler (reuses the existing open path).
      e.preventDefault();
      const rows = Array.from(document.querySelectorAll<HTMLElement>("#ql .qi"));
      if (!rows.length) return;
      const cur = document.querySelector<HTMLElement>("#ql .qi.cur");
      const i = cur ? rows.indexOf(cur) : -1;
      const next = e.key === "ArrowDown" ? rows[i + 1] : rows[i - 1];
      next?.click();
    } else if (e.key === "Enter") {
      e.preventDefault();
      document.querySelector<HTMLElement>('[data-fil="ranger"]')?.click();
    } else if (e.key === "Backspace" || e.key === "x" || e.key === "X") {
      // ⌫ is the model's Discard key; X kept as an alias (matches the visible button hint).
      e.preventDefault();
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
        if (b) toast("Action undone", false);
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
  // The "Filed ✓ ↩" confirmation now lives as a banner in the right rail (#filfoot), not in #mid, so
  // it no longer blocks auto-advance — after filing, doRanger explicitly advances #mid to the next
  // pending. syncDetail's job here is unchanged: keep the open track stable, else load the first pending.
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
