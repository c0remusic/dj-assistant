// Bibliothèque detail/edit panel (Tauri only). Mounts the shared read-only analysis report
// (report-view: player + verdict + spectrogram) and, beneath it, an inline metadata editor
// for a filed track: artist / title / genres / year / label / cover → update_metadata, plus
// Identifier-or-Voir-la-release (Discogs) and Supprimer. The Revue equivalent is filing.ts;
// candidate rendering is shared via identify-shared.ts (spec M6b Lot 2).
import {
  updateMetadata,
  identify,
  applyIdentity,
  openUrl,
  trashTrack,
} from "./ipc";
import type { Candidate, AppliedIdentity } from "./ipc";
import type { LibraryTrack, MetadataEdit } from "../shared/contracts";
import { renderCandidates } from "./identify-shared";
import { openReportInto } from "./report-view";
import { open } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { requireEl } from "./dom";

const esc = (s: string) =>
  s.replace(/[&<>"']/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : c === '"' ? "&quot;" : "&#39;",
  );

/** Per-open editor state (one detail panel open at a time). `pendingCover` is set only when
 * the user picks a new image — left null otherwise so a save never re-embeds the same art. */
interface EditState {
  track: LibraryTrack;
  pendingCover: string | null;
  saving: boolean;
}

/** A transient bottom-right toast (mirrors filing.ts, no undo affordance here). */
function toast(message: string): void {
  document.getElementById("sift-toast")?.remove();
  const el = document.createElement("div");
  el.id = "sift-toast";
  el.style.cssText =
    "position:fixed;right:18px;bottom:18px;z-index:9998;display:flex;align-items:center;gap:12px;background:var(--color-background-secondary);border:0.5px solid var(--color-border-secondary);border-radius:var(--border-radius-md);padding:9px 13px;font-size:var(--text-md);color:var(--color-text-primary);box-shadow:0 8px 28px rgba(0,0,0,.4)";
  el.textContent = message;
  document.body.appendChild(el);
  setTimeout(() => el.remove(), 4000);
}

/** Current cover source for the thumbnail (pending pick > stored path > none). */
function coverSrc(st: EditState): string | null {
  const p = st.pendingCover ?? st.track.cover_path;
  return p ? convertFileSrc(p) : null;
}

/** Cover thumbnail with a "changer" overlay button. */
function coverHtml(st: EditState): string {
  const src = coverSrc(st);
  const inner = src
    ? `<img src="${esc(src)}" alt="" style="width:100%;height:100%;object-fit:cover">`
    : `<i class="ti ti-vinyl" style="font-size:var(--text-hero);color:var(--color-text-tertiary)"></i>`;
  return (
    `<button data-lib="cover" title="Changer la pochette" aria-label="Changer la pochette" style="position:relative;width:72px;height:72px;flex:none;border-radius:var(--border-radius-md);overflow:hidden;background:var(--color-background-secondary);border:0.5px solid var(--color-border-tertiary);display:flex;align-items:center;justify-content:center;padding:0;cursor:pointer">` +
    inner +
    `<span style="position:absolute;inset:auto 0 0 0;background:rgba(0,0,0,.55);color:#fff;font-size:var(--text-2xs);padding:2px 0;text-align:center">changer</span>` +
    `</button>`
  );
}

/** The release link (when Discogs-identified) or the Identifier entry button. */
function releaseRowHtml(st: EditState): string {
  if (st.track.discogs_release_id) {
    return (
      `<button data-lib="release" title="Ouvrir la page Discogs"><i class="ti ti-external-link" style="font-size:var(--text-md);vertical-align:-1px"></i> Voir la release</button>` +
      `<button data-lib="identifier" class="sift-id-btn" title="Rechercher à nouveau sur Discogs"><i class="ti ti-refresh" style="font-size:var(--text-sm);vertical-align:-1px"></i> Ré-identifier</button>`
    );
  }
  return `<button data-lib="identifier" class="sift-id-btn" title="Rechercher les métadonnées sur Discogs"><i class="ti ti-search" style="font-size:var(--text-md);vertical-align:-1px"></i> Identifier</button>`;
}

/** Render the editor footer into `edit`. Re-rendered after identify (release link appears). */
function renderEdit(edit: HTMLElement, st: EditState): void {
  const t = st.track;
  edit.innerHTML =
    `<div style="display:flex;gap:12px;align-items:flex-start">` +
    coverHtml(st) +
    `<div style="flex:1;min-width:0;display:flex;flex-direction:column;gap:6px">` +
    `<div style="display:grid;grid-template-columns:1fr 1fr;gap:6px">` +
    `<input data-lib="artist" placeholder="Artiste" value="${esc(t.artist ?? "")}" class="sift-editor-input" style="width:100%">` +
    `<input data-lib="title" placeholder="Titre" value="${esc(t.title ?? "")}" class="sift-editor-input" style="width:100%">` +
    `</div>` +
    `<input data-lib="genres" placeholder="Genres (séparés par une virgule)" value="${esc(t.genres.join(", "))}" class="sift-editor-input" style="width:100%">` +
    `<div style="display:grid;grid-template-columns:90px 1fr;gap:6px">` +
    `<input data-lib="year" type="number" placeholder="Année" value="${t.year ?? ""}" class="sift-editor-input" style="width:100%">` +
    `<input data-lib="label" placeholder="Label" value="${esc(t.label ?? "")}" class="sift-editor-input" style="width:100%">` +
    `</div>` +
    `</div></div>` +
    `<div style="display:flex;align-items:center;gap:6px;margin-top:9px;flex-wrap:wrap">${releaseRowHtml(st)}</div>` +
    `<div class="sift-cands" style="margin-top:7px" hidden></div>` +
    `<div style="display:flex;gap:8px;margin-top:10px">` +
    `<button data-lib="save" style="flex:1;background:var(--color-background-info);color:var(--color-text-info);border:none;font-weight:500"><i class="ti ti-device-floppy" style="font-size:var(--text-md);vertical-align:-2px"></i> Enregistrer</button>` +
    `<button data-lib="trash" class="sift-secondary-trash" title="Envoyer à la corbeille" aria-label="Envoyer à la corbeille"><i class="ti ti-trash" style="font-size:var(--text-md);vertical-align:-2px"></i> Supprimer</button>` +
    `</div>`;

  wireEdit(edit, st);
}

/** Collect the editor's current field values into a MetadataEdit. Empty strings → null;
 * genres split on commas/semicolons, trimmed, de-duplicated by order. */
function collectEdit(edit: HTMLElement, st: EditState): MetadataEdit {
  const val = (sel: string) => edit.querySelector<HTMLInputElement>(`[data-lib="${sel}"]`)?.value ?? "";
  const trimOrNull = (s: string) => (s.trim() ? s.trim() : null);
  const yearRaw = val("year").trim();
  const year = yearRaw ? Number(yearRaw) : null;
  const genres = val("genres")
    .split(/[,;]/)
    .map((g) => g.trim())
    .filter(Boolean);
  return {
    artist: val("artist").trim(),
    title: val("title").trim(),
    label: trimOrNull(val("label")),
    year: year != null && Number.isFinite(year) ? year : null,
    genres,
    // Only send a cover when the user picked a new one — null preserves the embedded art.
    cover_path: st.pendingCover,
  };
}

/** Wire the editor's buttons + identify flow. */
function wireEdit(edit: HTMLElement, st: EditState): void {
  edit.querySelector('[data-lib="cover"]')?.addEventListener("click", () => void pickCover(edit, st));
  edit.querySelector('[data-lib="release"]')?.addEventListener("click", () => {
    if (st.track.discogs_release_id)
      void openUrl(`https://www.discogs.com/release/${st.track.discogs_release_id}`);
  });
  edit.querySelector('[data-lib="save"]')?.addEventListener("click", () => void doSave(edit, st));
  edit.querySelector('[data-lib="trash"]')?.addEventListener("click", () => void doTrash(st));

  const idBtn = edit.querySelector<HTMLButtonElement>('[data-lib="identifier"]');
  const candsHost = edit.querySelector<HTMLElement>(".sift-cands");
  if (idBtn && candsHost) {
    idBtn.addEventListener("click", () => void doIdentify(idBtn, candsHost, edit, st));
  }
}

/** Pick a new cover image and preview it (saved only when the user clicks Enregistrer). */
async function pickCover(edit: HTMLElement, st: EditState): Promise<void> {
  const file = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "Image", extensions: ["jpg", "jpeg", "png"] }],
  });
  if (typeof file !== "string") return;
  st.pendingCover = file;
  renderEdit(edit, st); // re-render so the thumbnail updates
}

/** Run Discogs identify for the open track. Mirrors filing.ts error handling. */
async function doIdentify(
  btn: HTMLButtonElement,
  host: HTMLElement,
  edit: HTMLElement,
  st: EditState,
): Promise<void> {
  const orig = btn.innerHTML;
  btn.disabled = true;
  btn.innerHTML = '<i class="ti ti-loader-2 sift-spin" style="font-size:var(--text-sm);vertical-align:-1px"></i> Recherche…';
  host.hidden = false;
  host.innerHTML = '<div class="sift-cands-msg">Recherche…</div>';
  try {
    const candidates = await identify(st.track.id);
    renderCandidates(host, candidates);
    wireCandidateClicks(host, candidates, edit, st);
  } catch (err) {
    const msg = String(err);
    if (msg.includes("NO_TOKEN")) {
      host.innerHTML =
        `<div class="sift-cands-msg">Discogs limite les recherches anonymes — ajoute ton jeton (gratuit) dans Réglages.</div>` +
        `<button class="sift-cand-jump" data-lib="goto-reglages" style="margin-top:5px;font-size:var(--text-sm);padding:3px 9px">Ouvrir Réglages →</button>`;
      host.querySelector('[data-lib="goto-reglages"]')?.addEventListener("click", () => {
        requireEl('[data-view="reglages"]', "library-detail goto-reglages").dispatchEvent(
          new MouseEvent("click", { bubbles: true }),
        );
      });
    } else {
      const rl = msg.match(/RATE_LIMITED:(\d+)/);
      host.innerHTML = rl
        ? `<div class="sift-cands-msg">Discogs limite le débit — réessaie dans ${rl[1]}s.</div>`
        : `<div class="sift-cands-msg sift-cands-error"><i class="ti ti-alert-triangle" style="font-size:var(--text-md);vertical-align:-2px;margin-right:4px"></i>Discogs injoignable.</div>`;
    }
  } finally {
    btn.disabled = false;
    btn.innerHTML = orig;
  }
}

/** Wire clicks on rendered candidate rows → apply the chosen identity. */
function wireCandidateClicks(
  host: HTMLElement,
  candidates: Candidate[],
  edit: HTMLElement,
  st: EditState,
): void {
  host.querySelectorAll<HTMLElement>("[data-cand]").forEach((el) => {
    const idx = Number(el.dataset.cand);
    el.addEventListener("click", () => {
      const c = candidates[idx];
      if (!c) return;
      el.style.opacity = "0.5";
      el.style.pointerEvents = "none";
      void applyIdentity(st.track.id, c)
        .then((applied) => onIdentityApplied(applied, c, edit, st, host))
        .catch((e) => {
          el.style.opacity = "";
          el.style.pointerEvents = "";
          host.innerHTML = `<div class="sift-cands-msg sift-cands-error"><i class="ti ti-alert-triangle" style="font-size:var(--text-md);vertical-align:-2px;margin-right:4px"></i>${esc(String(e))}</div>`;
        });
    });
  });
}

/** apply_identity already persisted the chosen candidate (tags + DB, including the release
 * link). Reflect it in the open panel: update the track + editor fields, then re-render so the
 * "Voir la release" link appears and the cover refreshes. */
function onIdentityApplied(
  applied: AppliedIdentity,
  c: Candidate,
  edit: HTMLElement,
  st: EditState,
  host: HTMLElement,
): void {
  st.track.artist = applied.canonical.artist;
  st.track.title = applied.canonical.title;
  st.track.label = applied.label;
  st.track.year = applied.year;
  st.track.genres = applied.styles;
  st.track.discogs_release_id = c.release_id;
  if (applied.cover_path) {
    st.track.cover_path = applied.cover_path;
    st.track.has_cover = true;
  }
  st.pendingCover = null; // the applied cover is already saved; don't re-send on next save
  notifyChanged(st.track);
  renderEdit(edit, st);
  host.hidden = true;
  toast("Identifié — métadonnées appliquées");
}

/** Save the manual edits via update_metadata (file tags first, then DB). */
async function doSave(edit: HTMLElement, st: EditState): Promise<void> {
  if (st.saving) return;
  const e = collectEdit(edit, st);
  if (!e.title) {
    toast("Le titre ne peut pas être vide.");
    return;
  }
  const btn = edit.querySelector<HTMLButtonElement>('[data-lib="save"]');
  const orig = btn?.innerHTML ?? null;
  st.saving = true;
  if (btn) {
    btn.disabled = true;
    btn.innerHTML = '<i class="ti ti-loader-2 sift-spin" style="font-size:var(--text-md);vertical-align:-2px"></i> Enregistrement…';
  }
  try {
    await updateMetadata(st.track.id, e);
    // Reflect saved values back into the open track + notify the list.
    st.track.artist = e.artist;
    st.track.title = e.title;
    st.track.label = e.label;
    st.track.year = e.year;
    st.track.genres = e.genres;
    if (st.pendingCover) {
      st.track.cover_path = st.pendingCover;
      st.track.has_cover = true;
      st.pendingCover = null;
    }
    notifyChanged(st.track);
    toast("Enregistré");
  } catch (err) {
    toast(`Échec de l'enregistrement : ${String(err)}`);
    console.error("update_metadata failed", err);
  } finally {
    st.saving = false;
    if (btn && orig != null) {
      btn.disabled = false;
      btn.innerHTML = orig;
    }
  }
}

/** Move the track's file to the bin (reversible via the global Ctrl+Z undo). */
async function doTrash(st: EditState): Promise<void> {
  try {
    await trashTrack(st.track.id);
    toast("Envoyé à la corbeille");
    deletedCb?.();
  } catch (err) {
    toast(`Échec : ${String(err)}`);
    console.error("trash_track failed", err);
  }
}

// Callbacks set per open: keep the Bibliothèque list in sync without owning its markup.
let savedCb: ((t: LibraryTrack) => void) | null = null;
let deletedCb: (() => void) | null = null;
function notifyChanged(t: LibraryTrack): void {
  savedCb?.(t);
}

/** Open the unified detail/edit panel for a filed track into `host`.
 * `onSaved` lets the caller refresh the list row in place (player stays alive);
 * `onDeleted` fires after a successful Supprimer (the caller re-renders the list). */
export function openLibraryDetailInto(
  host: HTMLElement,
  track: LibraryTrack,
  onSaved: (t: LibraryTrack) => void,
  onDeleted: () => void,
): void {
  savedCb = onSaved;
  deletedCb = onDeleted;
  const st: EditState = { track: { ...track, genres: [...track.genres] }, pendingCover: null, saving: false };

  host.innerHTML =
    '<div style="display:flex;flex-direction:column;gap:10px;margin-top:10px;border-top:0.5px solid var(--color-border-tertiary);padding-top:10px">' +
    '<div class="lib-report"></div>' +
    '<div class="lib-edit"></div>' +
    '<div class="lib-verdict"></div>' +
    "</div>";
  const reportEl = requireEl<HTMLElement>(".lib-report", "openLibraryDetailInto", host);
  const editEl = requireEl<HTMLElement>(".lib-edit", "openLibraryDetailInto", host);
  // Verdict is the CONCLUSION — rendered last, after Identification, matching the maquette
  // (see docs/refonte-ui-plan.md, décision du 2026-07-02).
  const verdictEl = requireEl<HTMLElement>(".lib-verdict", "openLibraryDetailInto", host);

  void openReportInto(reportEl, track.path, verdictEl);
  renderEdit(editEl, st);
}
