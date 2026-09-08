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
  revertBatch,
  libraryFolders,
} from "./ipc";
import type { Candidate, AppliedIdentity } from "./ipc";
import type { LibraryTrack, MetadataEdit } from "../shared/contracts";
import { identifyErrorHtml, renderCandidates } from "./identify-shared";
import { confirmAction } from "./confirm-modal";
import { openReportInto } from "./report-view";
import { open } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { requireEl, esc } from "./dom";
import { toast } from "./filing-toast";
import { humanizeError } from "./errors";

/** Per-open editor state (one detail panel open at a time). `pendingCover` is set only when
 * the user picks a new image — left null otherwise so a save never re-embeds the same art. */
interface EditState {
  track: LibraryTrack;
  pendingCover: string | null;
  saving: boolean;
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
    : `<i class="ti ti-vinyl" style="font-size:var(--text-2xl);color:var(--color-text-tertiary)"></i>`;
  return (
    `<button data-lib="cover" title="Changer la pochette" aria-label="Changer la pochette" style="position:relative;width:72px;height:72px;flex:none;border-radius:var(--border-radius-md);overflow:hidden;background:var(--color-background-secondary);border:1px solid var(--color-border-tertiary);display:flex;align-items:center;justify-content:center;padding:0;cursor:pointer">` +
    inner +
    // Encre + scrim theme-INVARIANTS : ce bandeau est posé sur la pochette de l'utilisateur, pas
    // sur une surface de l'app, donc son ratio dépend de l'image et non des tokens. Utilisait
    // --color-text-on-accent, qui bascule sur une encre sombre en thème sombre : encre sombre sur
    // scrim noir, mesuré à 1,42:1 sur pochette noire et 1,29:1 sur gris moyen. --color-text-on-scrim
    // ne bascule jamais, et --overlay-scrim-caption est assez dense pour garantir le pire cas
    // (pochette blanche) à 7,26:1 dans les deux thèmes. Voir leurs commentaires dans styles.css.
    `<span style="position:absolute;inset:auto 0 0 0;background:var(--overlay-scrim-caption);color:var(--color-text-on-scrim);font-size:var(--text-xs);padding:2px 0;text-align:center">changer</span>` +
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
    `<div class="lib-edit-pair">` +
    // Audit-ref B4 (Bibliothèque, 2026-07-09, réf. shadcn Field) : placeholder seul n'est pas une
    // vraie étiquette accessible (disparaît une fois rempli) — aria-label ajouté, valeur = placeholder.
    `<input data-lib="artist" placeholder="Artiste" aria-label="Artiste" value="${esc(t.artist ?? "")}" class="sift-editor-input" style="width:100%">` +
    `<input data-lib="title" placeholder="Titre" aria-label="Titre" value="${esc(t.title ?? "")}" class="sift-editor-input" style="width:100%">` +
    `</div>` +
    `<input data-lib="genres" list="sift-genre-list" placeholder="Genres (séparés par une virgule)" aria-label="Genres" value="${esc(t.genres.join(", "))}" class="sift-editor-input" style="width:100%">` +
    `<datalist id="sift-genre-list"></datalist>` +
    `<div class="lib-edit-labelled">` +
    `<input data-lib="year" type="number" min="1900" max="2100" placeholder="Année" aria-label="Année" value="${t.year ?? ""}" class="sift-editor-input" style="width:100%">` +
    `<input data-lib="label" placeholder="Label" aria-label="Label" value="${esc(t.label ?? "")}" class="sift-editor-input" style="width:100%">` +
    `</div>` +
    `</div></div>` +
    `<div class="lib-edit-meta">${releaseRowHtml(st)}</div>` +
    `<div class="sift-cands" hidden></div>` +
    `<div class="lib-edit-actions">` +
    `<button data-lib="save" style="flex:1;background:var(--color-accent-fill);color:var(--color-accent-ink);border:none;font-weight:500">Enregistrer</button>` +
    `<button data-lib="trash" class="sift-secondary-trash" title="Envoyer à la corbeille" aria-label="Envoyer à la corbeille">Supprimer</button>` +
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
    // Not clamped here — the browser only enforces the input's min/max (1900-2100) via the
    // stepper UI / native validation bubble, not on a value typed directly then blurred, so an
    // out-of-range value can reach this point untouched. doSave() rejects it explicitly instead
    // of silently clamping (the input would otherwise keep showing the raw typed value while a
    // different, clamped value got written to the file — see doSave()'s year bounds check).
    year: year != null && Number.isFinite(year) ? year : null,
    genres,
    // Only send a cover when the user picked a new one — null preserves the embedded art.
    cover_path: st.pendingCover,
  };
}

/** Genre names already known to the library (facet counts dropped), fetched once and reused
 * across every editor open — reuses the same `library_folders` IPC call bibliotheque-view.ts
 * already makes to populate its Genres facet, no new backend command added. */
let genreListCache: string[] | null = null;

/** Rebuild the `#sift-genre-list` datalist options from the Genres input's CURRENT value, so
 * autocomplete keeps working past the first comma. A native `<datalist>` filters its options
 * against the input's whole value, not the last comma-separated token — so for a multi-value
 * field like this one it silently stops suggesting anything after the first comma is typed. Fix:
 * on every keystroke, split off the last segment (after the last comma/semicolon), match it
 * against `names`, and emit each option as "already-typed-prefix + candidate" — the option's
 * full text still starts with what the user typed, so the browser's own whole-value prefix
 * filter keeps working, while what's actually offered/inserted is genre-name completion for just
 * the segment being typed. */
function renderGenreDatalist(datalist: HTMLDataListElement, value: string, names: string[]): void {
  const lastSep = Math.max(value.lastIndexOf(","), value.lastIndexOf(";"));
  const prefix = lastSep >= 0 ? value.slice(0, lastSep + 1) + " " : "";
  const segment = (lastSep >= 0 ? value.slice(lastSep + 1) : value).trim().toLowerCase();
  const matches = segment ? names.filter((n) => n.toLowerCase().startsWith(segment)) : names;
  datalist.innerHTML = matches.map((n) => `<option value="${esc(prefix + n)}"></option>`).join("");
}

/** Fill the `#sift-genre-list` datalist so typing in Genres offers autocomplete against genres
 * already used elsewhere in the library (avoids "House" vs "house" duplicates), and keep it
 * re-filtered per comma-separated segment as the user types (see renderGenreDatalist). Best-effort:
 * a fetch failure just leaves the datalist empty, the free-text input keeps working either way. */
function fillGenreDatalist(edit: HTMLElement): void {
  const datalist = edit.querySelector<HTMLDataListElement>("#sift-genre-list");
  const input = edit.querySelector<HTMLInputElement>('[data-lib="genres"]');
  if (!datalist || !input) return;
  const rerender = () => {
    if (genreListCache) renderGenreDatalist(datalist, input.value, genreListCache);
  };
  input.addEventListener("input", rerender);
  if (genreListCache) {
    rerender();
    return;
  }
  void libraryFolders()
    .then((facets) => {
      genreListCache = facets.genres.map((g) => g.name);
      rerender();
    })
    .catch((e) => console.error("genre datalist load failed", e));
}

/** Wire the editor's buttons + identify flow. */
function wireEdit(edit: HTMLElement, st: EditState): void {
  fillGenreDatalist(edit);
  edit.querySelector('[data-lib="cover"]')?.addEventListener("click", () => void pickCover(edit, st));
  edit.querySelector('[data-lib="release"]')?.addEventListener("click", () => {
    if (st.track.discogs_release_id)
      void openUrl(`https://www.discogs.com/release/${st.track.discogs_release_id}`);
  });
  edit.querySelector('[data-lib="save"]')?.addEventListener("click", () => void doSave(edit, st));
  edit.querySelector('[data-lib="trash"]')?.addEventListener("click", () => void doTrash(edit, st));

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
    // Même cascade que `filing-identify.ts`, et c'est le problème qu'on retire : elle était
    // recopiée ici, donc les impasses A9 et A10 (issue #15) y vivaient en double. Une seule
    // source désormais — `identifyErrorHtml`.
    const { html, gotoReglages } = identifyErrorHtml(err);
    host.innerHTML =
      html +
      (gotoReglages
        ? `<button class="sift-cand-jump sift-goto-reglages" data-lib="goto-reglages"><i class="ti ti-arrow-right"></i> Ouvrir Réglages</button>`
        : "");
    host.querySelector('[data-lib="goto-reglages"]')?.addEventListener("click", () => {
      requireEl('[data-view="reglages"]', "library-detail goto-reglages").dispatchEvent(
        new MouseEvent("click", { bubbles: true }),
      );
    });
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
          host.innerHTML = `<div class="sift-cands-msg sift-cands-error"><i class="ti ti-alert-triangle" style="font-size:var(--text-md);vertical-align:-2px;margin-right:4px"></i>${esc(humanizeError(e, "Impossible d'appliquer cette release — réessaie", "apply_identity"))}</div>`;
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
  // Same reasoning as doSave(): applied.styles can introduce brand-new genres, so drop the
  // cache here too or the datalist only picks them up after a full app restart.
  genreListCache = null;
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
  if (e.year != null && (e.year < 1900 || e.year > 2100)) {
    toast("Année hors limites (1900-2100).");
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
    const batchId = await updateMetadata(st.track.id, e);
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
    // A save can introduce a brand-new genre — drop the cache so the next datalist fill (any
    // editor opened afterward) refetches and offers it, instead of only picking it up after a
    // full app restart (defeats the point of the datalist: avoiding "House"/"house" duplicates
    // within the same session).
    genreListCache = null;
    notifyChanged(st.track);
    toast("Enregistré", true, () => {
      void revertBatch(batchId).catch((err: unknown) => {
        console.error("revert_batch failed", err);
        toast("Annulation impossible — réessaie");
      });
    });
  } catch (err) {
    toast(humanizeError(err, "Échec de l'enregistrement — réessaie", "update_metadata"));
  } finally {
    st.saving = false;
    if (btn && orig != null) {
      btn.disabled = false;
      btn.innerHTML = orig;
    }
  }
}

/** Move the track's file to the bin (reversible via the global Ctrl+Z undo). */
async function doTrash(edit: HTMLElement, st: EditState): Promise<void> {
  if (
    !(await confirmAction(
      "Envoyer ce morceau à la corbeille ? Annulable via Ctrl+Z.",
      "Envoyer à la corbeille",
    ))
  )
    return;
  const btn = edit.querySelector<HTMLButtonElement>('[data-lib="trash"]');
  if (btn) btn.disabled = true;
  try {
    await trashTrack(st.track.id);
    toast("Envoyé à la corbeille");
    deletedCb?.();
  } catch (err) {
    toast(humanizeError(err, "Impossible d'envoyer à la corbeille — réessaie", "trash_track"));
  } finally {
    if (btn) btn.disabled = false;
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
  onClose: () => void,
): void {
  savedCb = onSaved;
  deletedCb = onDeleted;
  const st: EditState = { track: { ...track, genres: [...track.genres] }, pendingCover: null, saving: false };

  // « Ok pour le proposé » (Antoine, 2026-09-08, wireframe « Rangés — inspecteur ouvert ») : la
  // colonne parle Revue. La carte « Piste ouverte » (titre + chevron) est partie — elle ne disait
  // rien que la ligne surlignée et l'en-tête du lecteur ne disent déjà ; fermer = re-cliquer la ligne
  // (`openBiblioDetail`, bibliotheque-view.ts). Le rapport et l'éditeur reposent sur le sol de la
  // colonne, sans carte (patron Finder « Lire les informations »). L'éditeur garde son ancienne
  // grammaire (champs bordés, Enregistrer, Supprimer) jusqu'au chantier 3 de ce wireframe — la
  // fiche Métadonnées de Revue vit dans filing-identify.ts, couplée à RevueState, et se partage par
  // extraction, pas par copie.
  host.innerHTML =
    '<div class="lib-detail-stack">' +
    '<div class="lib-report"></div>' +
    '<div class="lib-edit"></div>' +
    '<div class="lib-verdict"></div>' +
    "</div>";
  const reportEl = requireEl<HTMLElement>(".lib-report", "openLibraryDetailInto", host);
  const editEl = requireEl<HTMLElement>(".lib-edit", "openLibraryDetailInto", host);
  // Verdict is the CONCLUSION — rendered last, after Identification, matching the maquette
  // (see docs/superpowers/plans/2026-07-02-refonte-ui-plan.md, décision du 2026-07-02).
  const verdictEl = requireEl<HTMLElement>(".lib-verdict", "openLibraryDetailInto", host);
  void onClose; // la porte de fermeture est la ligne elle-même ; le paramètre reste pour l'appelant
  // L'en-tête du lecteur porte le TITRE et l'ARTISTE (`.sift-report-name` / `.sift-report-sub`),
  // comme Revue après reconcile (`filing-preview.ts::updateHeaderName`) — plus le nom de fichier
  // en 15/600 sur trois lignes. Sans artiste ni titre, le nom de fichier reste (défaut du rapport).
  void openReportInto(reportEl, track.path, verdictEl, {
    showAnalysisFailure: false,
    title: track.title || undefined,
    subtitle: track.artist || undefined,
  });
  renderEdit(editEl, st);
}
