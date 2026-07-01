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
  applyTags,
  findDuplicate,
  identify,
  applyIdentity,
  trackRelease,
  trackFileTags,
} from "./ipc";
import type { Candidate, AppliedIdentity } from "./ipc";
import type { DupMatch, TrackRelease, FileTags } from "../shared/contracts";
import { open } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { openReportInto, togglePlay, vchipHtml, row, keyboardHintsHtml } from "./report-view";
import { renderCandidates } from "./identify-shared";
import type { Bin, Canonical, Target, QueueItem, AnalysisReport } from "../shared/contracts";
import { FILE_IN_PLACE } from "../shared/contracts";
import { requireEl } from "./dom";
import { emptyStateHtml } from "./empty-state";

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
  // The would-write sub-genres for the open track (DB track_genres order), shown in .sift-genres and
  // compared (joined) against the file. Set on open from track_release, or from `applied.styles`.
  genres: string[];
  // The file's REAL tags, snapshotted ONCE on open (and re-read after an Apply/File). The marker
  // compares the displayed identity to THIS in-memory snapshot — never a per-keystroke disk read.
  // null until the open-time read resolves.
  fileTags: FileTags | null;
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
  genres: [],
  fileTags: null,
  filedConfirm: null,
};

// Identification card display mode: false = read-only grid (maquette default), true = the
// existing editable artist/title/version inputs. Reset on every track open (Step 3) so a new
// track never inherits the previous track's edit-mode.
let identEditing = false;

// Detail mode's "file in place" state — mirrors sift-live.ts's batchInPlace for batch mode. A
// module variable (not read straight off the checkbox's DOM .checked) because the checkbox now
// renders as part of renderBins's fldz.innerHTML, rebuilt wholesale on every filter keystroke/
// folder click/background refresh — a DOM-only checked flag would reset on each of those.
let detailInPlace = false;

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

// Optional batch pick context: when set, the #fldz tree highlights `selectedRel` and routes a folder
// click to `onPick` (→ batchBin in sift-live) instead of detail's state.binRel. null = detail mode.
let binPick: { selectedRel: string | null; onPick: (rel: string) => void; inert: boolean } | null =
  null;
/** The rel currently highlighted in the tree — batch pick context when active, else detail's. */
function selRel(): string | null {
  return binPick ? binPick.selectedRel : state.binRel;
}

/** Recursive HTML for one tree node + its children when expanded. The root (depth 0,
 * rel "") sits at the top; folders nest under it, each with a caret when it has
 * sub-folders. Selecting a node sets it as the filing destination. */
function binNodeHtml(node: { rel: string; name: string; depth: number }): string {
  const kids = childrenOf(node.rel);
  const isOpen = expanded.has(node.rel);
  const on = node.rel === selRel() ? " on" : "";
  const indent = node.depth * 13;
  const caret = kids.length
    ? `<span data-fil="caret" data-rel="${esc(node.rel)}" title="${isOpen ? "Collapse" : "Expand"}" class="sift-fld-caret" style="${
        isOpen ? "transform:rotate(90deg)" : ""
      }">▸</span>`
    : '<span class="sift-fld-caret-spacer"></span>';
  const icon = node.depth === 0 ? "ti-database" : "ti-folder";
  // Explicit highlight for the selected destination (don't rely on inherited .on CSS): tinted
  // background + an info-coloured folder icon + medium weight so the active bin is unmistakable.
  const sel = on
    ? "background:var(--color-background-info);border-radius:var(--border-radius-sm,4px)"
    : "";
  const iconColor = on ? "var(--color-text-info)" : "var(--color-text-tertiary)";
  const weight = on ? "font-weight:500;" : "";
  let html = `<div class="fld${on} sift-fld-row" data-fil="bin" data-rel="${esc(node.rel)}" title="${esc(
    absPath(node.rel),
  )}" style="${sel};${weight}padding-left:${6 + indent}px">${caret}<i class="ti ${icon} sift-fld-icon" style="font-size:var(--text-base);color:${iconColor}"></i><span class="sift-fld-label">${esc(
    node.name,
  )}</span></div>`;
  if (kids.length && isOpen) html += kids.map(binNodeHtml).join("");
  return html;
}

/** Flat selectable row for the filtered view: shows the full relative path so the location is
 * obvious without the tree context, with the same highlight + absolute-path tooltip as the tree. */
function flatBinHtml(b: Bin): string {
  const on = b.rel === selRel() ? " on" : "";
  const sel = on ? "background:var(--color-background-info);border-radius:var(--border-radius-sm,4px);" : "";
  const color = on ? "var(--color-text-info)" : "var(--color-text-tertiary)";
  return `<div class="fld${on} sift-fld-flat-row" data-fil="bin" data-rel="${esc(b.rel)}" title="${esc(
    absPath(b.rel),
  )}" style="${sel}"><i class="ti ti-folder sift-fld-icon" style="font-size:var(--text-base);color:${color}"></i><span class="sift-fld-label">${esc(
    b.rel,
  )}</span></div>`;
}

/** Render the destination column (#fldz): root picker when unset, else a folder filter + either
 * the collapsible tree (no filter) or a flat list of matching folders (filter active). */
export function renderBins(fldz: HTMLElement): void {
  if (!state.rootSet) {
    fldz.innerHTML =
      '<div class="sift-fldz-hint">Choisis ta racine de bibliothèque pour commencer à ranger.</div>' +
      '<button data-fil="pickroot"><i class="ti ti-folder sift-icon-inline-base"></i> Choisir…</button>';
    fldz
      .querySelector('[data-fil="pickroot"]')
      ?.addEventListener("click", () => void pickRoot(fldz));
    return;
  }

  const filtering = state.binFilter.trim().length > 0;

  // Folder filter (only worth showing once there are sub-folders to sift through).
  const filterRow = state.bins.length
    ? `<input data-fil="binfilter" placeholder="Filtrer les dossiers…" value="${esc(
        state.binFilter,
      )}" class="sift-binfilter">`
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
      : '<div class="sift-fldz-no-match">Aucun dossier correspondant.</div>';
  } else {
    const tree = binNodeHtml({ rel: "", name: rootName(), depth: 0 });
    const emptyNote =
      state.bins.length === 0 && expanded.has("")
        ? '<div class="sift-fldz-empty-note">vide — crée un dossier</div>'
        : "";
    body = tree + emptyNote;
  }

  // "+ nouveau" creates under the selected folder (nested). Hidden while filtering.
  const nestLabel = state.binRel ? ` dans ${binLabel()}` : "";
  const newRow = filtering
    ? ""
    : state.creating
      ? `<input data-fil="newin" placeholder="${esc(
          state.binRel ? `dossier dans ${binLabel()}…` : "nom du dossier…",
        )}" class="sift-newin">`
      : `<div class="fld sift-newbin-row" data-fil="newbin"><i class="ti ti-plus sift-icon-inline-lg"></i> nouveau${esc(
          nestLabel,
        )}</div>`;

  // "Sur place" lives INSIDE the popover now (maquette: filter → in-place row → tree), instead of
  // a separate persistent element outside #fldz — same attribute per mode so the existing wiring
  // (detail: change listener below; batch: sift-live.ts's delegated #pa "change" listener, which
  // catches it regardless of where inside #pa it renders) needs no other changes. The tree itself
  // (not the checkbox) is wrapped so batch's "in place greys the tree" behavior can target just that
  // wrapper — checking the box must never make itself un-clickable.
  const inPlaceChecked = binPick ? binPick.inert : detailInPlace;
  const inPlaceAttr = binPick ? 'data-sift="inplace"' : 'data-fil="inplace"';
  const inPlaceRow = `<label class="sift-inplace-toggle"><input type="checkbox" ${inPlaceAttr}${
    inPlaceChecked ? " checked" : ""
  }><span>Sur place <span class="sift-inplace-note">(dossier source)</span></span></label>`;
  // Real disk path caption (maquette: "📁 {rootPath}\"), title= carries the full path for a
  // narrow popover where the text itself gets ellipsis-truncated.
  const rootCaption = state.rootPath
    ? `<div class="sift-fldz-rootpath" title="${esc(state.rootPath)}"><i class="ti ti-folder sift-icon-inline-sm"></i> ${esc(state.rootPath)}\\</div>`
    : "";

  fldz.innerHTML =
    filterRow + inPlaceRow + rootCaption + `<div class="sift-fldz-tree">${body}${newRow}</div>`;

  if (!binPick) {
    fldz.querySelector<HTMLInputElement>('[data-fil="inplace"]')?.addEventListener("change", (e) => {
      detailInPlace = (e.target as HTMLInputElement).checked;
    });
  }

  // Batch in-place greys the TREE ONLY (never the checkbox that controls it). Re-assert on every
  // render, unconditionally (not just when binPick.inert is true) — this makes renderBins
  // self-consistent across mode switches with no external reset needed (previously an explicit
  // cleanup in setReviewMode's "leave batch" branch was required because this only ever SET
  // opacity, never cleared it, when binPick was null).
  const treeWrap = fldz.querySelector<HTMLElement>(".sift-fldz-tree");
  if (treeWrap) {
    const inert = !!binPick?.inert;
    treeWrap.style.opacity = inert ? ".4" : "1";
    treeWrap.style.pointerEvents = inert ? "none" : "auto";
  }

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
      const rel = el.dataset.rel ?? "";
      if (binPick) {
        binPick.onPick(rel); // batch: caller updates batchBin + re-renders tree/rail/preview
      } else {
        state.binRel = rel;
        renderBins(fldz);
        refreshFootButton();
        fldz.hidden = true; // picking a destination closes the popover (like the mockup's pickBin)
      }
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
  repositionDestPopoverIfOpen();
}

/** Re-anchor the destination popover to the Destination button's CURRENT position, but only if
 *  it's actually open. `positionDestPopover` was previously called once, at open time — but
 *  `#fldz`'s content (this file's `renderBins`) is rebuilt on background events (queue/analysis
 *  changes trigger `refreshBins`) independent of any user click, and the rail itself
 *  (`renderFoot`/`renderBatchRail`) can reflow too (e.g. a filename wrapping differently) — either
 *  can silently move the Destination button while the popover, once positioned, never re-anchored.
 *  That produced the "position aléatoire" bug: correct if you'd JUST clicked Destination, stale
 *  and drifted otherwise. Calling this from every content/layout path that could move the button
 *  keeps the popover glued to it regardless of what triggered the change. */
export function repositionDestPopoverIfOpen(): void {
  const pop = document.getElementById("fldz");
  if (pop && !pop.hidden) positionDestPopover(pop);
}

/** Default target from the analysed rail (lossless → AIFF, else MP3 320). */
export function defaultTarget(rail: string): Target {
  return rail === "lossless" ? "aiff_16_44" : "mp3_320";
}

export const TARGET_LABEL: Record<Target, string> = {
  mp3_320: "MP3 320",
  aiff_16_44: "AIFF",
  wav_16_44: "WAV",
};

export function targetExt(t: Target): string {
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

/** Re-render the bin label wherever it's shown in the rail — the File button AND the Destination
 *  popover trigger both carry it (bin can change while a track is open). */
function refreshFootButton(): void {
  document
    .querySelectorAll<HTMLElement>('[data-fil="ranger"] .sift-fil-bin, [data-fil="destbtn"] .sift-fil-bin')
    .forEach((el) => (el.textContent = binLabel()));
}

/** Re-sync the filename preview from the current canonical + target. The preview lives in the rail
 *  (#filfoot) right below the format chips, so a format change or a field edit must refresh this
 *  node (the extension follows state.target). Probe non-throw: the rail may be gone. */
function refreshPreview(): void {
  const name = previewName();
  const prev = document.querySelector<HTMLElement>(".sift-fil-prev");
  if (prev) prev.textContent = `→ ${name}`;
  const verdictName = document.querySelector<HTMLElement>(".sift-verdict-finalname");
  if (verdictName) verdictName.textContent = `→ ${name}`;
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
    `<div class="sift-release-line">` +
    `<i class="ti ti-tag" title="Release (Discogs)"></i>` +
    `<span>${value}</span></div>`;
}

/** Render the genre chips into `.sift-genres` from `state.genres` (single source — set on open from
 *  track_release, or from `applied.styles` on identify). Empty list → empty box (no chips). */
function renderGenres(): void {
  const el = document.querySelector<HTMLElement>(".sift-genres");
  if (!el) return; // editor not mounted
  el.innerHTML = state.genres
    .map((s) => `<span class="sift-genre-chip" title="Sous-genres Discogs">${esc(s)}</span>`)
    .join("");
}

/** Join genres EXACTLY like write_tags_full (trim, drop empties, "A; B"), so the comparison against
 *  the file's single Genre field is like-for-like. */
const joinGenres = (g: string[]): string => g.map((s) => s.trim()).filter(Boolean).join("; ");

/** Which displayed tag fields would CHANGE the file if written — i.e. diverge from `state.fileTags`.
 *  Mirrors write_tags_full's semantics: artist/title are ALWAYS written (compare directly), while
 *  label/year/genres are only written when non-empty (an empty would-write never clears the file, so
 *  it is NOT a discrepancy). All comparison is in memory against the on-open snapshot — no disk read. */
function tagFieldDiffs(): { artist: boolean; title: boolean; label: boolean; year: boolean; genres: boolean; any: boolean } {
  const f = state.fileTags;
  const c = state.canonical;
  const none = { artist: false, title: false, label: false, year: false, genres: false, any: false };
  if (!f || !c) return none; // snapshot not loaded yet → show nothing rather than a false alarm
  const norm = (s: string | null | undefined): string => (s ?? "").trim();
  const artist = norm(c.artist) !== norm(f.artist);
  const title = norm(c.title) !== norm(f.title);
  const labelW = norm(state.label);
  const label = labelW !== "" && labelW !== norm(f.label);
  const yearW = state.year ?? 0;
  const year = yearW > 0 && yearW !== (f.year ?? 0);
  const genresW = joinGenres(state.genres);
  const genres = genresW !== "" && genresW !== norm(f.genre_joined);
  return { artist, title, label, year, genres, any: artist || title || label || year || genres };
}

/** Show/hide the "tags not written" banner and mark the diverging fields. Cheap (a few
 *  querySelectors + class toggles) — safe to call on open, on each field edit, and after Apply/File.
 *  Reads `state.fileTags` (the cached snapshot), never the disk. */
function refreshDiscrepancy(): void {
  const editor = document.querySelector<HTMLElement>(".sift-fil-editor");
  if (!editor) return;
  const d = tagFieldDiffs();
  const banner = editor.querySelector<HTMLElement>(".sift-tag-warn");
  // Visibility via display ONLY (the banner has no `hidden` attribute — that conflicted with an
  // inline display and kept it stuck on). flex when there's a discrepancy, none otherwise.
  if (banner) banner.style.display = d.any ? "flex" : "none";
  const mark = (sel: string, on: boolean) =>
    editor.querySelector<HTMLElement>(sel)?.classList.toggle("sift-tag-stale", on);
  mark('[data-fil="artist"]', d.artist);
  mark('[data-fil="title"]', d.title);
  mark(".sift-release", d.label || d.year);
  mark(".sift-genres", d.genres);
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

  // Verdict-panel MATCH chip — qualitative (the backend has no % score). Only shown when there's
  // real doubt (CHECK MATCH, amber): a confident green match shows nothing, per the maquette rule
  // that the chip exists only to flag something worth checking, not to confirm the obvious.
  const vchips = mid.querySelector<HTMLElement>(".sift-vchips");
  if (vchips) {
    vchips.querySelector('[data-chip="match"]')?.remove();
    const green = applied.canonical.confidence === "green";
    if (!green) {
      vchips.insertAdjacentHTML(
        "beforeend",
        vchipHtml("CHECK MATCH", "warning").replace("<span ", '<span data-chip="match" '),
      );
    }
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

  // [m11] Genres: store the would-write list (single source) and render the chips. The list also
  // feeds the file-vs-display discrepancy check (joined form), so it must live in state, not only DOM.
  state.genres = applied.styles;
  renderGenres();

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
  idBtn.innerHTML = '<i class="ti ti-refresh sift-icon-inline-sm"></i> Ré-identifier';

  // The displayed identity just changed while the FILE keeps its old tags → surface the gap, and
  // reset the Apply button (a prior "Appliqué ✓" no longer reflects this new identity).
  resetApplyButton(editor);
  refreshDiscrepancy();
}

/** Markup for the "Identified: artist — title" confirmation line (cover thumb + "change" button).
 *  Single source of truth, reused by a fresh fetch (onIdentityApplied) and by the reopen of an
 *  already-identified track (restoreIdentifiedLine) so both render identically. */
function identifiedLineHtml(artist: string, title: string, coverPath: string | null): string {
  const coverThumb = coverPath
    ? `<img src="${esc(convertFileSrc(coverPath))}" alt="" class="sift-identified-cover">`
    : `<span class="sift-identified-noart"><i class="ti ti-vinyl"></i></span>`;
  return (
    `<div class="sift-identified-line">` +
    coverThumb +
    `<span class="sift-identified-text">` +
    `<span class="sift-identified-label">Identifié :</span> ${esc(artist)} — ${esc(title)}` +
    `</span>` +
    `<button class="sift-cand-jump sift-cand-change-btn" data-fil="cand-changer">modifier</button>` +
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
  idBtn.innerHTML = '<i class="ti ti-refresh sift-icon-inline-sm"></i> Ré-identifier';
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
          host.innerHTML = `<div class="sift-cands-msg sift-cands-error"><i class="ti ti-alert-triangle sift-cand-error-icon"></i>${esc(String(e))}</div>`;
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
  btn.innerHTML = '<i class="ti ti-loader-2 sift-spin sift-searching-icon"></i> Recherche…';
  host.hidden = false;
  host.innerHTML = '<div class="sift-cands-msg">Recherche…</div>';

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
        `<div class="sift-cands-msg">Discogs limite les recherches anonymes — ajoute ton jeton (gratuit) dans Réglages.</div>` +
        `<button class="sift-cand-jump sift-goto-reglages" data-fil="goto-reglages">Ouvrir Réglages →</button>`;
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
        host.innerHTML = `<div class="sift-cands-msg">Discogs limite le débit — réessaie dans ${rl[1]}s.</div>`;
      } else {
        // [m10] network/server errors get a warning icon to distinguish from "no results"
        host.innerHTML = `<div class="sift-cands-msg sift-cands-error"><i class="ti ti-alert-triangle sift-cand-error-icon"></i>Discogs injoignable.</div>`;
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
        return `<span class="chip sift-chip-disabled" title="Pas de surqualité depuis un fichier lossy">${TARGET_LABEL[t]}</span>`;
      const on = (state.target ?? defaultTarget(rail)) === t ? " on" : "";
      return `<span class="chip${on}" data-fil="fmt" data-t="${t}">${TARGET_LABEL[t]}</span>`;
    })
    .join(" ");

  const fake = state.track?.verdict === "fake";
  const secondary = fake
    ? '<button data-fil="resource" class="sift-secondary-resource" title="Fichier faux — va dans Écartés (⌫)"><span class="kbd">⌫</span> <i class="ti ti-alert-triangle sift-icon-inline-md"></i> Re-source</button>'
    : '<button data-fil="trash" class="sift-secondary-trash" title="Envoyer à la corbeille (⌫)"><span class="kbd">⌫</span> <i class="ti ti-trash sift-icon-inline-md"></i> Jeter</button>';

  // Destination button opens the tree as a popover (#fldz, a sibling of #filfoot — see styles.css)
  // instead of the old persistent .dest column. The rail keeps the system.md stack tail: FORMAT →
  // Final name → CTA (File) → secondary. Rebuilt inside this innerHTML so a format-chip re-render
  // keeps it; the popover's own hidden state lives on #fldz itself, untouched by this rewrite.
  foot.innerHTML =
    `<button data-fil="destbtn" class="sift-dest-btn"><span class="sift-dest-btn-label">Destination</span><span class="sift-fil-bin">${esc(binLabel())}</span><i class="ti ti-chevron-down sift-dest-btn-caret"></i></button>` +
    `<div class="sift-rail-fmt-group"><span class="col-h">Format</span><div class="sift-fmt-chips">${chips}</div></div>` +
    `<div class="sift-rail-spacer"></div>` +
    // Keyboard hints anchored to the bottom rail (maquette's keyHints), not the scrollable
    // detail content — moved here from report-view.ts, which used to inject them under the hero.
    keyboardHintsHtml() +
    secondary +
    `<button data-fil="ranger" class="sift-ranger-btn"><i class="ti ti-corner-down-left sift-icon-inline-md"></i> Ranger → <span class="sift-fil-bin">${esc(binLabel())}</span> <span class="kbd">⏎</span></button>`;
  if (filedBanner) foot.append(filedBanner); // restore the banner below the freshly-rendered controls

  foot.querySelector('[data-fil="destbtn"]')?.addEventListener("click", (e) => {
    e.stopPropagation();
    toggleDestPopover();
  });
  ensureDestPopoverAutoClose();

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
  repositionDestPopoverIfOpen(); // the destbtn above was just rebuilt — keep an open popover glued to it
}

/** Anchors the popover to the Destination button's real on-screen position (position:fixed,
 *  recalculated here) instead of a hardcoded left/bottom — keeps it aligned if the rail's height
 *  changes (e.g. a longer secondary-button label wrapping).
 *
 *  Uses `top` derived from the popover's OWN measured height, not `bottom` derived from
 *  `window.innerHeight` — the previous `bottom:${window.innerHeight - r.top + 8}px` formula
 *  placed the popover near the top of the window instead of just above the button in the real
 *  Tauri webview (window.innerHeight apparently diverges from the coordinate space
 *  getBoundingClientRect reports here, a HiDPI/webview scaling quirk — confirmed by comparing a
 *  real screenshot's button position against the popover's actual rendered position). Deriving
 *  the position purely from two getBoundingClientRect() calls (button + popover), both in the
 *  same coordinate space by construction, sidesteps that mismatch entirely. */
function positionDestPopover(pop: HTMLElement): void {
  const btn = document.querySelector<HTMLElement>('[data-fil="destbtn"]');
  if (!btn) return;
  const r = btn.getBoundingClientRect();
  const popH = pop.getBoundingClientRect().height;
  pop.style.left = `${r.left}px`;
  pop.style.bottom = "auto";
  pop.style.top = `${r.top - popH - 8}px`;
}

/** Open/close the destination popover (#fldz). Its own hidden state persists across renderFoot's
 *  innerHTML rewrites since #fldz is a sibling of #filfoot, never touched by them. Exported: Batch
 *  mode has its own Destination button (sift-live.ts) and must go through this same function —
 *  the popover is position:fixed with no CSS fallback, so any toggle that bypasses this and
 *  flips `fldz.hidden` directly leaves it unpositioned (rendered wherever it falls in the layout). */
export function toggleDestPopover(force?: boolean): void {
  const pop = document.getElementById("fldz");
  if (!pop) return;
  const opening = force !== undefined ? force : pop.hidden;
  pop.hidden = !opening;
  if (opening) positionDestPopover(pop);
}

// One-time (guarded) document listener: closes the destination popover on an outside click or
// Escape, like every other popover in the app (candidate lists, palettes). Also repositions it
// on resize while open, since position:fixed coordinates are frozen at open time.
let destPopoverAutoCloseWired = false;
export function ensureDestPopoverAutoClose(): void {
  if (destPopoverAutoCloseWired) return;
  destPopoverAutoCloseWired = true;
  window.addEventListener("resize", () => {
    const pop = document.getElementById("fldz");
    if (pop && !pop.hidden) positionDestPopover(pop);
  });
  // Capture phase: the #pa delegated handler (queue rows, etc.) calls stopPropagation() on most
  // clicks, which would otherwise stop this listener ever seeing them in the bubble phase.
  document.addEventListener(
    "click",
    (e) => {
      const pop = document.getElementById("fldz");
      if (!pop || pop.hidden) return;
      const target = e.target as Node;
      if (pop.contains(target) || (target as HTMLElement).closest?.('[data-fil="destbtn"]')) return;
      pop.hidden = true;
    },
    { capture: true },
  );
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") toggleDestPopover(false);
  });
}

/** Render the center metadata editor (Identify + editable fields + genres) into `host`, below the
 *  analysis report. The final-name preview lives in the rail (`renderFoot`) next to the File button;
 *  this pane ends with genres. One-shot innerHTML — called once per track open, not on a
 *  burst event, so create-once/update-in-place is not required here. `rail` is accepted for symmetry
 *  with renderFoot; the editor itself is format-agnostic (the extension comes from state.target). */
function renderEditor(host: HTMLElement, mid: HTMLElement, rail: string, report: AnalysisReport | null): void {
  void rail;
  const c = state.canonical;
  if (!c) {
    host.innerHTML = "";
    return;
  }
  const yn = (b: boolean) => (b ? "oui" : "non");

  // [I6] Add tooltip to confidence badge so the colour is self-explanatory.
  const badge =
    c.confidence === "green"
      ? '<span title="Titre et artiste extraits avec confiance" class="sift-badge-ok"><i class="ti ti-circle-check"></i> métadonnées fiables</span>'
      : '<span title="Titre ou artiste incertain — vérifie les champs" class="sift-badge-warn"><i class="ti ti-alert-circle"></i> à vérifier</span>';

  const inputCss = "sift-editor-input";

  // [C1] "Fetch metadata from Discogs" is the primary entry point (gold filled), above the inputs.
  // [C2] title= explains what it does; the kbd hint shows the I shortcut.
  // Vertical order: pick the Discogs release FIRST (badge → Fetch → candidates), then edit the
  // fields it populates (artist/title/version → Genres directly under Version). The Final name
  // preview moved to the rail (next to File). `.sift-cands` sits above the inputs so choosing a
  // release precedes editing.
  const displayName =
    c.artist && c.title ? `${c.artist} — ${c.title}${c.version ? ` (${c.version})` : ""}` : "Non identifié";
  host.innerHTML =
    `<div class="sift-ident-head">` +
    `<span class="col-h sift-editor-title">Identification · Discogs</span>` +
    `<button data-fil="ident-edit" class="sift-ident-edit-btn" title="Modifier manuellement"><i class="ti ti-pencil"></i></button>` +
    `</div>` +
    (identEditing
      ? `<div class="sift-editor-badge-row">${badge}</div>` +
        `<button data-fil="identifier" class="sift-id-btn sift-id-btn-full" title="Rechercher les métadonnées sur Discogs (pochette, label, année, genres)"><i class="ti ti-search sift-icon-inline-sm"></i> Récupérer les métadonnées Discogs <span class="kbd sift-kbd-hint-id">I</span></button>` +
        `<div class="sift-cands sift-cands-host" hidden></div>` +
        `<div class="sift-editor-fields">` +
        `<input data-fil="artist" placeholder="Artist" value="${esc(c.artist)}" class="${inputCss}">` +
        `<input data-fil="title" placeholder="Title" value="${esc(c.title)}" class="${inputCss}">` +
        `<input data-fil="version" placeholder="Version" value="${esc(c.version ?? "")}" class="${inputCss}">` +
        `</div>` +
        `<button data-fil="ident-done" class="sift-ident-done-btn">Terminé</button>`
      : `<div class="sift-ident-display">${esc(displayName)}</div>`) +
    // Read-only release facts (Label · Année) between the editable identity and Genres. Filled by
    // refreshReleaseLine() below from state; stays empty (no gap) when neither value is known.
    `<div class="sift-release"></div>` +
    `<div class="col-h sift-col-h-tight">Genres</div>` +
    `<div class="sift-genres sift-genres-box"></div>` +
    // Compatibilité CDJ / Version ID3: moved here from the spectral-proof box (report-view.ts) —
    // the maquette groups them with Label/Année/Genre in Identification, not with the spectrum
    // evidence. `report` is null only if analysis failed to load; nothing renders in that case.
    (report
      ? `<div class="sift-spectro-rows">` +
        row("Compatibilité CDJ", yn(report.tags_cdj_ok)) +
        row("Version ID3", report.id3_version || "—") +
        `</div>`
      : "") +
    // Apply ID3 tags: write these fields onto the file in place (no move, no encode, no 'filed'
    // change), revertable. Distinct from File (rail) — a neutral secondary button in the editor.
    `<button data-fil="applytags" class="sift-applytags-btn" title="Écrire ces tags dans le fichier en place — pas de déplacement, pas d'encodage, réversible"><i class="ti ti-tag sift-icon-inline-md"></i> Appliquer les tags ID3</button>` +
    // Discrepancy banner — sits JUST BELOW Apply. Hidden by default via inline display:none; the LONE
    // visibility mechanism is refreshDiscrepancy toggling style.display (no `hidden`+display conflict).
    // Look lives in .sift-tag-warn (styles.css). Shown only when the display diverges from the file.
    `<div class="sift-tag-warn" style="display:none"><i class="ti ti-alert-triangle sift-icon-inline-md sift-icon-flex-none"></i><span>Tags non écrits dans le fichier — <strong>Ranger</strong> ou <strong>Appliquer</strong> pour les graver</span></div>`;

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
    refreshDiscrepancy(); // editing a field may make the display diverge from the file (or re-converge)
  };
  host
    .querySelectorAll<HTMLInputElement>('[data-fil="artist"],[data-fil="title"],[data-fil="version"]')
    .forEach((el) => el.addEventListener("input", upd));

  const idBtn = host.querySelector<HTMLButtonElement>('[data-fil="identifier"]');
  const candsHost = host.querySelector<HTMLElement>(".sift-cands");
  if (idBtn && candsHost) {
    idBtn.addEventListener("click", () => void doIdentify(idBtn, candsHost, host, mid));
  }

  const applyBtn = host.querySelector<HTMLButtonElement>('[data-fil="applytags"]');
  if (applyBtn) setApplyIdle(applyBtn); // idle on every fresh render; doApplyTags flips it to "applied"

  host.querySelector<HTMLButtonElement>('[data-fil="ident-edit"]')?.addEventListener("click", () => {
    identEditing = true;
    renderEditor(host, mid, rail, report);
  });
  host.querySelector<HTMLButtonElement>('[data-fil="ident-done"]')?.addEventListener("click", () => {
    identEditing = false;
    renderEditor(host, mid, rail, report);
  });

  refreshReleaseLine(); // read-only Label · Année from state (restored from cache on open); empty when none
}

// Apply-button state machine. ONE button toggles between "Apply ID3 tags" (writes the file) and
// "Appliqué ✓ — Annuler" (reverts the batch just written). `onclick` is reassigned (not
// addEventListener) so a toggle never stacks handlers.
const APPLY_IDLE_HTML =
  '<i class="ti ti-tag sift-icon-inline-md"></i> Appliquer les tags ID3';

/** Put the Apply button in its idle "write" state. */
function setApplyIdle(btn: HTMLButtonElement): void {
  btn.disabled = false;
  btn.style.color = "var(--color-text-secondary)";
  btn.innerHTML = APPLY_IDLE_HTML;
  btn.onclick = () => void doApplyTags(btn);
}

/** Put the Apply button in its "applied — click to undo" state (the whole button reverts `batchId`). */
function setApplyApplied(btn: HTMLButtonElement, batchId: string): void {
  btn.disabled = false;
  btn.style.color = "var(--color-text-success)";
  btn.innerHTML =
    '<i class="ti ti-circle-check sift-icon-inline-md"></i> Appliqué ✓ — <span class="sift-underline">Annuler</span>';
  btn.onclick = () => void doUndoApply(btn, batchId);
}

/** Reset a possibly-"applied" Apply button back to idle (e.g. when the identity changes under it). */
function resetApplyButton(scope: HTMLElement): void {
  const btn = scope.querySelector<HTMLButtonElement>('[data-fil="applytags"]');
  if (btn) setApplyIdle(btn);
}

/** Write the current edited tags onto the file in place (apply_tags). On success the file matches
 *  the display, so re-snapshot to clear the marker and flip the button to "Appliqué ✓ — Annuler".
 *  No move/encode/status change — works on any file. openSeq-guarded: a later open never repaints
 *  this track's state/UI. */
async function doApplyTags(btn: HTMLButtonElement): Promise<void> {
  if (!state.track || !state.canonical) return;
  const trackId = state.track.id;
  const edited = state.canonical;
  const myseq = openSeq;
  btn.disabled = true;
  btn.innerHTML =
    '<i class="ti ti-loader-2 sift-spin sift-icon-inline-md"></i> Applying…';
  try {
    const batchId = await applyTags(trackId, edited);
    const snap = await trackFileTags(trackId); // file changed → refresh the in-memory snapshot
    if (myseq !== openSeq) return; // another track opened meanwhile — leave its state/UI alone
    state.fileTags = snap;
    refreshDiscrepancy(); // file == display now → marker clears
    setApplyApplied(btn, batchId);
  } catch (e) {
    console.error("apply_tags failed", e);
    toast("Échec de l'écriture des tags", false);
    if (myseq === openSeq) setApplyIdle(btn);
  }
}

/** Undo the just-applied tag write (targeted revert of its batch). The file returns to its old tags
 *  → re-snapshot → the marker reappears and the button returns to idle. openSeq-guarded. */
async function doUndoApply(btn: HTMLButtonElement, batchId: string): Promise<void> {
  const trackId = state.track?.id;
  const myseq = openSeq;
  btn.disabled = true;
  btn.innerHTML =
    '<i class="ti ti-loader-2 sift-spin sift-icon-inline-md"></i> Annulation…';
  try {
    await revertBatch(batchId);
    if (trackId != null) {
      const snap = await trackFileTags(trackId);
      if (myseq !== openSeq) return;
      state.fileTags = snap;
    }
    if (myseq !== openSeq) return;
    refreshDiscrepancy(); // file back to old tags → display diverges again → marker reappears
    setApplyIdle(btn);
  } catch (e) {
    console.error("revert tag_edit failed", e);
    toast("Annulation impossible", false);
    if (myseq === openSeq) setApplyApplied(btn, batchId); // stay applied so the user can retry
  }
}

/** A transient toast at the bottom-right with an optional "Undo" action. With `onUndo` the Undo
 *  button runs that callback (e.g. a targeted revert of a specific batch); without it, Undo falls
 *  back to `undoLast` (the LIFO most-recent action) and clears the detail pane. */
function toast(message: string, undo: boolean, onUndo?: () => void): void {
  document.getElementById("sift-toast")?.remove();
  const el = document.createElement("div");
  el.id = "sift-toast";
  el.className = "sift-toast";
  el.innerHTML =
    `<span>${esc(message)}</span>` +
    (undo
      ? '<button data-fil="undo" class="sift-toast-undo">Annuler</button>'
      : "");
  document.body.appendChild(el);
  el.querySelector('[data-fil="undo"]')?.addEventListener("click", () => {
    el.remove();
    if (onUndo) {
      onUndo(); // targeted revert (e.g. revertBatch of THIS tag_edit) — pane stays as-is
      return;
    }
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
    toast("Choisis un dossier de destination.", false);
    return;
  }
  const ranger = document.querySelector<HTMLElement>('[data-fil="ranger"]');
  const orig = ranger?.innerHTML ?? null;
  acting = true;
  setActionsDisabled(true);
  if (ranger)
    ranger.innerHTML =
      '<i class="ti ti-loader-2 sift-spin sift-icon-inline-md"></i> Rangement en cours…';
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
    else clearPane(mid, true); // no pending left → the formal empty state; the banner still shows in the rail
    // Filed confirmation as a banner at the BOTTOM of the right rail, under the new track's controls
    // (renderFoot, run by openFilingInto above, already wrote them; the banner is appended below them).
    showFiledConfirm(batchId, bin, filedPath);
  } catch (e) {
    const msg = String(e);
    if (msg.includes("NoLibraryRoot")) toast("Aucune racine de bibliothèque configurée.", false);
    else if (msg.toLowerCase().includes("upscale")) toast("Refusé : pas de surqualité lossy → lossless.", false);
    else toast(`Échec du rangement : ${msg}`, false);
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
  banner.innerHTML =
    `<div class="sift-filed-banner-head">` +
    `<i class="ti ti-check"></i>` +
    `<span class="sift-filed-banner-label">Rangé</span>` +
    `<span class="sift-filed-banner-bin">→ ${esc(bin)}</span>` +
    `<button data-fil="filed-close" title="Fermer" class="sift-filed-banner-close"><i class="ti ti-x"></i></button>` +
    `</div>` +
    `<div class="sift-filed-banner-name">${esc(filename)}</div>` +
    `<div class="sift-filed-banner-path">${esc(filedPath)}</div>` +
    `<button data-fil="revert" class="sift-filed-banner-revert"><i class="ti ti-arrow-back-up"></i> Annuler</button>`;
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
    toast("Annulé — retour dans la file", false);
  } catch (e) {
    const msg = String(e);
    if (msg.includes("source gone")) {
      toast("Annulation impossible : un fichier nécessaire a disparu — l'original a peut-être été purgé de la corbeille.", false);
    } else {
      toast(`Échec de l'annulation : ${msg}`, false);
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
      toast("Marqué à re-sourcer", true);
    } else {
      await trashTrack(state.track.id);
      toast("Envoyé à la corbeille", true);
    }
    clearPane(mid);
  } catch (e) {
    toast(`Échec : ${String(e)}`, false);
    console.error(`${kind} failed`, e);
    setActionsDisabled(false);
  } finally {
    acting = false;
  }
}

/** Empty the detail pane back to a neutral prompt (after an action), or — when `emptyQueue` is
 *  true — to the formal empty state (DESIGN.md "État vide"): the caller already knows the queue
 *  has nothing left, a real dead-end rather than a mid-session deselect. Revue is the entry point
 *  so it never gets a "back to X" link; the rail is already cleared below in both cases. */
function clearPane(mid: HTMLElement, emptyQueue = false): void {
  state.track = null;
  state.canonical = null;
  state.target = null;
  state.label = null;
  state.year = null;
  state.genres = [];
  state.fileTags = null;
  state.filedConfirm = null;
  mid.innerHTML = emptyQueue
    ? emptyStateHtml({
        title: "Rien à revoir",
        note: "Les morceaux à traiter apparaissent ici une fois ajoutés depuis Accueil ou déposés dans la file.",
      })
    : '<div class="sift-clear-pane">Sélectionne un morceau dans la file pour l\'écouter et le ranger.</div>';
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
      ? `Déjà rangé : ${esc((m.folder ? m.folder + "/" : "") + (m.filename || ""))}`
      : `Doublon d'un fichier en file : ${esc(m.filename || "")}`;
  const sure = m.kind === "both";
  const fg = sure ? "var(--color-text-warning)" : "var(--color-text-tertiary)";
  const bg = sure ? "var(--color-background-warning)" : "var(--color-background-secondary)";
  const head = sure ? "Doublon" : "Doublon possible (même nom — à vérifier)";
  return `<div class="sift-dup-banner" style="background:${bg}"><i class="ti ti-copy" style="color:${fg}"></i><div class="sift-dup-banner-body"><div class="sift-dup-banner-head" style="color:${fg}">${head}</div><div class="sift-dup-banner-where">${where}</div></div></div>`;
}

// Bumped on every open; an in-flight open bails at its await points if a newer one started
// (prevents a slow analyze/reconcile from clobbering the pane of a track opened since).
let openSeq = 0;

/** True when the detail-mode "file in place" checkbox is ticked: File targets the track's own
 *  source folder (FILE_IN_PLACE) instead of the bin selected in the #fldz tree. */
function fileInPlaceChecked(): boolean {
  return detailInPlace;
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
  identEditing = false; // Identification card always opens in read-only display mode

  mid.innerHTML =
    '<div class="sift-fil sift-fil-root">' +
    '<div class="sift-fil-dup"></div>' +
    '<div class="sift-fil-scroll">' +
    '<div class="sift-fil-report"></div>' +
    '<div class="sift-fil-editor sift-fil-editor-margin"></div>' +
    '<div class="sift-fil-verdict sift-fil-editor-margin"></div>' +
    '</div>' +
    "</div>";
  const reportEl = requireEl<HTMLElement>(".sift-fil-report", "openFilingInto", mid);
  // Verdict is the CONCLUSION — rendered last, after Identification, matching the maquette
  // (see docs/refonte-ui-plan.md, décision du 2026-07-02). Passed to openReportInto below.
  const verdictEl = requireEl<HTMLElement>(".sift-fil-verdict", "openFilingInto", mid);
  // The validation footer now lives in the right rail (#filfoot in the .dest column), below the
  // destination tree — so #mid is a pure son-first detail and the rail holds the filing stack.
  const footEl = requireEl("#filfoot", "openFilingInto");

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

  // Analysis report, metadata reconcile, the persisted release facts, and the file's REAL tags are
  // independent reads — run them in parallel so the footer renders as soon as they complete. The
  // file-tags read is the ONE disk read for the discrepancy marker (cached after; never per-keystroke).
  const [report, canonical, release, fileTags] = await Promise.all([
    openReportInto(reportEl, item.path, verdictEl),
    reconcile(item.id).catch((e): Canonical => {
      console.error("reconcile failed", e);
      return { artist: "", title: "", version: null, confidence: "yellow" };
    }),
    trackRelease(item.id).catch((e): TrackRelease => {
      console.error("track_release failed", e);
      return { artist: null, title: null, version: null, label: null, year: null, cover_path: null, genres: [], identified: false };
    }),
    // On failure: leave fileTags null (no marker) and log it — never assert a discrepancy we could
    // not measure (no silent false alarm).
    trackFileTags(item.id).catch((e): FileTags | null => {
      console.error("track_file_tags failed", e);
      return null;
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
  // Would-write genres (shown in .sift-genres, compared joined) + the file's real-tags snapshot,
  // both cached here for the in-memory discrepancy check. fileTags may be null (read failed → no marker).
  state.genres = release.genres;
  state.fileTags = fileTags;
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
  renderEditor(editorEl, mid, rail, report);
  // Already-identified track → show the "Identified" line (cover + release) in place of the bare
  // Fetch button, rebuilt from metadata (no network). Runs inside the openSeq-guarded section above,
  // so a superseded open never paints this onto the wrong track.
  if (release.identified && state.canonical) {
    restoreIdentifiedLine(editorEl, mid, state.canonical.artist, state.canonical.title, release.cover_path);
  }
  renderGenres(); // fill .sift-genres from state.genres (also shows genres on reopen, not just fresh fetch)
  refreshDiscrepancy(); // flag the marker if the file's tags differ from the displayed identity
  updateHeaderName(mid); // show the clean proposed name in the report header

  // Verdict-panel chip (board: LOSSLESS · DUPLICATE): only appended when dedup found a real match —
  // no "UNIQUE" chip for the common case, per the maquette rule that a chip exists to flag
  // something worth checking, not to confirm the absence of a problem. The MATCH/CHECK MATCH chip
  // is added later by onIdentityApplied.
  void dupP.then((m) => {
    if (myseq !== openSeq) return;
    const chips = mid.querySelector<HTMLElement>(".sift-vchips");
    if (!chips || chips.querySelector('[data-chip="dup"]') || !m) return;
    chips.insertAdjacentHTML(
      "beforeend",
      vchipHtml("DUPLICATE", "warning").replace("<span ", '<span data-chip="dup" '),
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

/** Render the tree in batch pick mode (no reload — state.bins already loaded). */
export function renderBinsForBatch(
  fldz: HTMLElement,
  selectedRel: string | null,
  onPick: (rel: string) => void,
  inert: boolean,
): void {
  binPick = { selectedRel, onPick, inert };
  renderBins(fldz);
}

/** Load bins then render the tree in batch pick mode (entry when switching into batch). */
export async function refreshBinsForBatch(
  fldz: HTMLElement,
  selectedRel: string | null,
  onPick: (rel: string) => void,
  inert: boolean,
): Promise<void> {
  binPick = { selectedRel, onPick, inert };
  await loadBins();
  renderBins(fldz);
}

/** Leave batch pick mode → tree reverts to detail's state.binRel. */
export function clearBinPick(): void {
  binPick = null;
}

/** Update the batch tree's inert (greyed) flag WITHOUT rebuilding the tree — so binPick.inert stays
 *  the single source of truth that renderBins re-asserts on every render (incl. queue refreshes during
 *  a run). Called by the rail's ensureBatchDestUI on each rebuild. No-op outside batch pick mode. */
export function setBinPickInert(inert: boolean): void {
  if (binPick) binPick.inert = inert;
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
  clearPane(mid, true); // truly nothing to review — the formal empty state
  return null;
}
