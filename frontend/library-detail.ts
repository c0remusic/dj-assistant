// Bibliothèque — détail d'une piste rangée, dans la zone D (Tauri only). Monte le rapport partagé
// (report-view : en-tête + audition + Diagnostic) et, dessous, la FICHE MÉTADONNÉES DE REVUE —
// même grammaire, mêmes classes (`.sift-meta-*`, `.sift-attr-*`, `.sift-cands-host`) : « ok pour le
// proposé », point 3 du wireframe « Rangés — inspecteur ouvert » (Antoine, 2026-09-08). L'éditeur
// d'avant (quatre champs bordés sans libellé, pochette « changer », « Voir la release »,
// « Enregistrer », « Supprimer ») est parti : les champs se gravent au BLUR / Entrée comme en Revue
// (décision du 2026-08-25, plus de bouton Enregistrer), la release choisie reste visible en ligne
// inerte (`chosenRowHtml`), et ce qui n'est pas une édition vit au clic droit de la ligne
// (`bibliotheque-view.ts::openBiblioContextMenu` : Fiche Discogs, Changer la pochette…, Envoyer à
// la corbeille).
//
// Ce qui est PARTAGÉ avec Revue : le rendu des candidats et de la ligne choisie (identify-shared.ts),
// le rapport (report-view.ts), les classes CSS de la fiche. Ce qui ne l'est pas, et pourquoi : le
// câblage. La fiche de Revue (filing-identify.ts) lit et écrit `RevueState` — canonical, fileTags,
// diff de tags, prévisualisation du nom — et grave par `write_tags_full` ; ici l'objet est un
// `LibraryTrack` rangé, gravé par `update_metadata` (tags puis base, annulable). Partager le markup
// par des classes et non par une fonction commune est un choix : un helper de markup à deux
// consommateurs qui écrivent des `data-*` différents divergerait à la première option.
import { updateMetadata, identify, applyIdentity, revertBatch, libraryFolders } from "./ipc";
import type { Candidate, AppliedIdentity } from "./ipc";
import type { LibraryTrack, MetadataEdit } from "../shared/contracts";
import { chosenRowHtml, identifyErrorHtml, renderCandidates } from "./identify-shared";
import { openReportInto } from "./report-view";
import { open } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { requireEl, esc } from "./dom";
import { toast } from "./filing-toast";
import { humanizeError } from "./errors";

/** État de la fiche ouverte (une seule à la fois). */
interface EditState {
  track: LibraryTrack;
  saving: boolean;
}

/** La fiche ouverte, pour les actions qui viennent d'AILLEURS que la fiche (le clic droit de la
 *  ligne : « Changer la pochette… »). `null` quand rien n'est ouvert. */
let openEdit: { edit: HTMLElement; host: HTMLElement; st: EditState } | null = null;

/** Une rangée d'attribut dans la grammaire de Revue : libellé tertiaire sur rail fixe, valeur =
 *  un input stylé comme du texte au repos (`.sift-attr-input`), révélé au survol et au focus. */
function attrRow(label: string, inputHtml: string): string {
  return `<div class="sift-attr"><span class="sift-attr-k">${label}</span>${inputHtml}</div>`;
}

/** La release CHOISIE, ligne inerte — même fonction que Revue (`chosenRowHtml`), reconstruite
 *  depuis la piste : label et année depuis la base, pochette locale. Pays et format n'existent pas
 *  sur `LibraryTrack` (pas de colonne backend, migration refusée en Revue : « je me fiche de
 *  l'édition ») — la sous-ligne se contente de ce qui est là. */
function chosenHtml(t: LibraryTrack): string {
  return chosenRowHtml({
    artist: t.artist || "",
    title: t.title || "",
    sub: [t.label, t.year != null ? String(t.year) : null].filter(Boolean).join(" · "),
    coverSrc: t.cover_path ? convertFileSrc(t.cover_path) : null,
  });
}

/** Rend la fiche dans `edit`. Un seul rendu quel que soit l'état (direction B de Revue) :
 *  l'identification et l'édition remplissent les mêmes champs en place, jamais de re-render. */
function renderEdit(edit: HTMLElement, st: EditState): void {
  const t = st.track;
  const identified = !!t.discogs_release_id;
  edit.innerHTML =
    `<div class="sift-meta-header">` +
    `<span class="sift-meta-title">Métadonnées</span>` +
    `</div>` +
    `<div class="sift-meta-body">` +
    // Release choisie au repos (ligne inerte), candidats le temps d'une recherche, vide et masqué
    // sinon — l'identification OUVRE la fiche depuis le 2026-09-07 (Revue, même ordre ici).
    `<div class="sift-cands sift-cands-host"${identified ? "" : " hidden"}>${identified ? chosenHtml(t) : ""}</div>` +
    // Pas de badge « I » : le raccourci est celui de Revue (`shortcuts.ts`), il n'existe pas ici.
    `<div class="sift-meta-actions">` +
    `<button data-lib="identifier" class="sift-meta-ident-btn" title="Rechercher les métadonnées sur Discogs (pochette, label, année, genres)">${t.artist && t.title ? "Ré-identifier" : "Identifier"}</button>` +
    `</div>` +
    `<div class="sift-attr-list">` +
    attrRow("Artiste", `<input data-lib="artist" class="sift-attr-input" placeholder="—" aria-label="Artiste" value="${esc(t.artist ?? "")}">`) +
    attrRow("Titre", `<input data-lib="title" class="sift-attr-input" placeholder="—" aria-label="Titre" value="${esc(t.title ?? "")}">`) +
    attrRow("Label", `<input data-lib="label" class="sift-attr-input" placeholder="—" aria-label="Label" value="${esc(t.label ?? "")}">`) +
    // Année : borne native 1900-2100 (audit B4, 2026-07-24), re-vérifiée dans `doSave` — la borne
    // native ne tient pas une valeur tapée puis quittée.
    attrRow("Année", `<input data-lib="year" type="number" min="1900" max="2100" class="sift-attr-input" placeholder="—" aria-label="Année" value="${t.year ?? ""}">`) +
    // Genres : ÉDITABLES ici (Revue les montre en texte + icône tag, lecture seule — spec Revue
    // § Zone C, décision F). Un input dans la même rangée, autocomplété sur les genres déjà connus.
    attrRow("Genres", `<input data-lib="genres" list="sift-genre-list" class="sift-attr-input" placeholder="—" aria-label="Genres, séparés par une virgule" value="${esc(t.genres.join(", "))}">`) +
    `<datalist id="sift-genre-list"></datalist>` +
    `</div>` +
    `</div>`;
  wireEdit(edit, st);
}

/** Lit les champs de la fiche en `MetadataEdit`. Chaînes vides → null ; genres découpés sur
 *  virgule / point-virgule, épurés, dédoublonnés dans l'ordre. `cover_path` toujours null ici :
 *  la pochette se grave à part (`changeCoverForOpenTrack`), jamais réenvoyée avec un champ. */
function collectEdit(edit: HTMLElement): MetadataEdit {
  const val = (sel: string) => edit.querySelector<HTMLInputElement>(`[data-lib="${sel}"]`)?.value ?? "";
  const trimOrNull = (s: string) => (s.trim() ? s.trim() : null);
  const yearRaw = val("year").trim();
  const year = yearRaw ? Number(yearRaw) : null;
  const seen = new Set<string>();
  const genres = val("genres")
    .split(/[,;]/)
    .map((g) => g.trim())
    .filter((g) => g && !seen.has(g) && seen.add(g));
  return {
    artist: val("artist").trim(),
    title: val("title").trim(),
    label: trimOrNull(val("label")),
    year: year != null && Number.isFinite(year) ? year : null,
    genres,
    cover_path: null,
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

/** Câble la fiche : champs gravés au blur / Entrée, Échap annule, bouton d'identification.
 *
 *  Même contrat que Revue (`filing-identify.ts`, 2026-08-25) : un champ se grave quand on FINIT
 *  de l'éditer, jamais à la frappe ; Échap rend la valeur d'avant le focus et ne grave rien — le
 *  drapeau `cancel` est lu par le handler de blur que `blur()` déclenche synchroniquement, donc
 *  sa durée de vie est exactement celle de l'appel (le bug du 2026-08-25 en Revue : Échap
 *  restaurait à l'écran puis le blur gravait la valeur restaurée… ou pas). */
function wireEdit(edit: HTMLElement, st: EditState): void {
  fillGenreDatalist(edit);
  edit.querySelectorAll<HTMLInputElement>(".sift-attr-input").forEach((inp) => {
    let focusVal = inp.value;
    let cancel = false;
    inp.addEventListener("focus", () => {
      focusVal = inp.value;
      cancel = false;
    });
    inp.addEventListener("keydown", (e: KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        inp.blur();
      } else if (e.key === "Escape") {
        inp.value = focusVal;
        cancel = true;
        inp.blur();
      }
    });
    inp.addEventListener("blur", () => {
      if (cancel) {
        cancel = false;
        return;
      }
      if (inp.value !== focusVal) void doSave(edit, st);
    });
  });

  const idBtn = edit.querySelector<HTMLButtonElement>('[data-lib="identifier"]');
  const candsHost = edit.querySelector<HTMLElement>(".sift-cands-host");
  if (idBtn && candsHost) {
    idBtn.addEventListener("click", () => void doIdentify(idBtn, candsHost, edit, st));
  }
}

/** « Changer la pochette… » — depuis le clic droit de la ligne (bibliotheque-view.ts), sur la
 *  piste dont la fiche est ouverte. Choisit une image, la grave AUSSITÔT (`update_metadata`, les
 *  autres champs tels qu'ils sont), et la pose dans l'en-tête du rapport comme Revue le fait après
 *  une identification (`.sift-report-cover`). Plus de « pochette en attente d'Enregistrer ». */
export async function changeCoverForOpenTrack(): Promise<void> {
  if (!openEdit) return;
  const { edit, st } = openEdit;
  const file = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "Image", extensions: ["jpg", "jpeg", "png"] }],
  });
  if (typeof file !== "string") return;
  const e = { ...collectEdit(edit), cover_path: file };
  if (!e.title) {
    toast("Le titre ne peut pas être vide.");
    return;
  }
  try {
    await updateMetadata(st.track.id, e);
    st.track.cover_path = file;
    st.track.has_cover = true;
    paintHeaderCover(edit, file);
    notifyChanged(st.track);
    toast("Pochette changée");
  } catch (err) {
    toast(humanizeError(err, "Impossible de changer la pochette — réessaie", "update_metadata"));
  }
}

/** Pose une pochette dans l'en-tête du rapport de la colonne — même geste que Revue
 *  (`filing-identify.ts`, `.sift-report-cover`) : `onerror` re-masque l'image si le fichier ne
 *  décode pas (Discogs rend parfois un « no image »), et le repli ::before reprend. */
function paintHeaderCover(edit: HTMLElement, coverPath: string): void {
  const src = convertFileSrc(coverPath);
  const host = edit.closest<HTMLElement>("#sift-aside") ?? document;
  host.querySelectorAll<HTMLImageElement>(".sift-report-cover").forEach((covEl) => {
    covEl.onerror = () => {
      covEl.hidden = true;
    };
    covEl.src = src;
    covEl.hidden = false;
  });
}

/** Recherche Discogs pour la piste ouverte. Liste OUVERTE comme en Revue (fork F) : tous les
 *  candidats visibles, le meilleur pré-sélectionné, sous le bouton qui la lance. */
async function doIdentify(
  btn: HTMLButtonElement,
  host: HTMLElement,
  edit: HTMLElement,
  st: EditState,
): Promise<void> {
  const orig = btn.textContent;
  btn.disabled = true;
  btn.textContent = "Recherche…";
  host.hidden = false;
  host.innerHTML = '<div class="sift-cands-msg">Recherche…</div>';
  try {
    const candidates = await identify(st.track.id);
    renderCandidates(host, candidates, { open: true });
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
    btn.textContent = orig;
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

/** `apply_identity` a déjà persisté le candidat (tags + base, lien de release compris). Le
 *  refléter EN PLACE, comme Revue : les champs se remplissent, la liste se referme SUR la ligne
 *  choisie (retours d'Antoine des 2026-09-06/07), la pochette monte dans l'en-tête — jamais un
 *  re-render de la fiche. */
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
    paintHeaderCover(edit, applied.cover_path);
  }
  const set = (sel: string, v: string) => {
    const inp = edit.querySelector<HTMLInputElement>(`[data-lib="${sel}"]`);
    if (inp) inp.value = v;
  };
  set("artist", st.track.artist ?? "");
  set("title", st.track.title ?? "");
  set("label", st.track.label ?? "");
  set("year", st.track.year != null ? String(st.track.year) : "");
  set("genres", st.track.genres.join(", "));
  const idBtn = edit.querySelector<HTMLButtonElement>('[data-lib="identifier"]');
  if (idBtn) idBtn.textContent = "Ré-identifier";
  host.hidden = false;
  host.innerHTML = chosenHtml(st.track);
  // Same reasoning as doSave(): applied.styles can introduce brand-new genres, so drop the
  // cache here too or the datalist only picks them up after a full app restart.
  genreListCache = null;
  notifyChanged(st.track);
  toast("Identifié — métadonnées appliquées");
}

/** Grave les champs (tags du fichier d'abord, puis base) — au blur / Entrée d'un champ modifié.
 *  Annulable depuis le toast (`revert_batch`), comme avant. */
async function doSave(edit: HTMLElement, st: EditState): Promise<void> {
  if (st.saving) return;
  const e = collectEdit(edit);
  if (!e.title) {
    toast("Le titre ne peut pas être vide.");
    return;
  }
  if (e.year != null && (e.year < 1900 || e.year > 2100)) {
    toast("Année hors limites (1900-2100).");
    return;
  }
  st.saving = true;
  try {
    const batchId = await updateMetadata(st.track.id, e);
    st.track.artist = e.artist;
    st.track.title = e.title;
    st.track.label = e.label;
    st.track.year = e.year;
    st.track.genres = e.genres;
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
  }
}

// Callbacks set per open: keep the Bibliothèque list in sync without owning its markup.
let savedCb: ((t: LibraryTrack) => void) | null = null;
function notifyChanged(t: LibraryTrack): void {
  savedCb?.(t);
}

/** Ouvre le détail d'une piste rangée dans `host` (la zone D).
 * `onSaved` permet à l'appelant de rafraîchir la ligne de la table en place (le lecteur survit).
 * `onDeleted` et `onClose` restent dans la signature : la corbeille et la fermeture ont quitté la
 * fiche (clic droit de la ligne, re-clic de la ligne), mais `bibliotheque-view.ts` les passe
 * encore et pourrait les rebrancher — ils sont ignorés ici, pas supprimés du contrat. */
export function openLibraryDetailInto(
  host: HTMLElement,
  track: LibraryTrack,
  onSaved: (t: LibraryTrack) => void,
  onDeleted: () => void,
  onClose: () => void,
): void {
  savedCb = onSaved;
  void onDeleted;
  void onClose;
  const st: EditState = { track: { ...track, genres: [...track.genres] }, saving: false };

  // « Ok pour le proposé » (Antoine, 2026-09-08, wireframe « Rangés — inspecteur ouvert ») : la
  // colonne parle Revue. La carte « Piste ouverte » (titre + chevron) est partie — elle ne disait
  // rien que la ligne surlignée et l'en-tête du lecteur ne disent déjà ; fermer = re-cliquer la ligne
  // (`openBiblioDetail`, bibliotheque-view.ts). Le rapport et la fiche reposent sur le sol de la
  // colonne, sans carte (patron Finder « Lire les informations »).
  // Ordre de Revue : en-tête + audition, Métadonnées, Diagnostic. Le Diagnostic sort du rapport
  // pour un slot à lui (`.lib-diag`, 5ᵉ argument d'`openReportInto`, le même mécanisme que Revue
  // avec `#sift-aside`) — sinon il se peindrait AVANT la fiche, dans le scroll du rapport.
  host.innerHTML =
    '<div class="lib-detail-stack">' +
    '<div class="lib-report"></div>' +
    '<div class="lib-edit"></div>' +
    '<div class="lib-diag"></div>' +
    '<div class="lib-verdict"></div>' +
    "</div>";
  const reportEl = requireEl<HTMLElement>(".lib-report", "openLibraryDetailInto", host);
  const editEl = requireEl<HTMLElement>(".lib-edit", "openLibraryDetailInto", host);
  const diagEl = requireEl<HTMLElement>(".lib-diag", "openLibraryDetailInto", host);
  // Le slot verdict ne porte plus que les états transitoires de l'analyse (squelette), comme en
  // Revue — le mot de verdict vit dans l'en-tête du lecteur.
  const verdictEl = requireEl<HTMLElement>(".lib-verdict", "openLibraryDetailInto", host);
  // L'en-tête du lecteur porte le TITRE et l'ARTISTE (`.sift-report-name` / `.sift-report-sub`),
  // comme Revue après reconcile (`filing-preview.ts::updateHeaderName`) — plus le nom de fichier
  // en 15/600 sur trois lignes. Sans artiste ni titre, le nom de fichier reste (défaut du rapport).
  void openReportInto(
    reportEl,
    track.path,
    verdictEl,
    {
      showAnalysisFailure: false,
      title: track.title || undefined,
      subtitle: track.artist || undefined,
    },
    diagEl,
  );
  // La pochette de l'en-tête à l'OUVERTURE, comme Revue au réopen (`restoreHeroCover`) : la coque
  // du rapport est posée synchroniquement par `openReportInto`, l'image y est déjà, masquée.
  if (track.cover_path) paintHeaderCover(editEl, track.cover_path);
  renderEdit(editEl, st);
  openEdit = { edit: editEl, host: editEl, st };
}
