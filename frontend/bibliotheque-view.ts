// Bibliothèque screen — extracted from sift-live.ts (clean-architecture audit F1, 2026-07-09).
// Click handling for data-bib actions stays in sift-live.ts's delegated #pa handler (same split
// as ecartes-view.ts: render+state live here, dispatch stays centralized). The 3 "doublons
// internes" fields (dupGroups/dupLoading/dupShown) are reassigned from BOTH that handler and
// renderBiblioLive here — bare `let`s can't be reassigned across an import boundary in ES
// modules, so they're consolidated into the single exported `bibDup` object below and mutated by
// property assignment instead (same pattern bibState already used).
import {
  listLibrary,
  libraryFolders,
  scanLibraryDuplicates,
  reanalyzeTracks,
  rejectBatch,
  trashTrack,
  revealTrack,
  openUrl,
} from "./ipc";
import { openContextMenu } from "./context-menu";
import { isStaleViewRender, viewEpoch } from "./view-epoch";
import { anchoredBelowPosition } from "./popover-position";
import { installColumnGestures, resetColumns, columnsAreCustomized } from "./library-columns";
import { confirmAction, BATCH_CONFIRM_THRESHOLD } from "./confirm-modal";
import { toast } from "./filing-toast";
import type { LibraryTrack, LibraryFacets, LibraryFilter, DupGroup } from "../shared/contracts";
import { requireEl, esc, plural } from "./dom";
import { slideSegThumb } from "./seg-thumb";
import { mountBarActions, mountBarSearch, openAside, closeAside } from "./toolbar";
import { humanizeError } from "./errors";
import { libraryUsage, type UsageReport } from "./ipc";
import { renderUsageChart } from "./usage-chart";
import { emptyStateHtml, wireEmptyState } from "./empty-state";
import { createVirtualList, type VirtualList } from "./list-virtual";
import {
  bibName,
  sortTracks,
  libraryTableHeaderHtml,
  libraryTableRowHtml,
  libraryGridRowHtml,
  LIBRARY_GRID_TILES_PER_ROW,
  LIBRARY_TABLE_PROBE_HTML,
  LIBRARY_GRID_PROBE_HTML,
  type LibrarySortState,
} from "./library-views";
import { openLibraryDetailInto, changeCoverForOpenTrack } from "./library-detail";

// Bibliothèque browser state: active filter, which facet column (folder/genre) is shown,
// and the last fetched track list (so a row-click can recover the track's path).
export const bibState: {
  filter: LibraryFilter;
  facet: "folder" | "genre" | "artist";
  tracks: LibraryTrack[];
  viewMode: "table" | "grid";
  sort: LibrarySortState;
} = {
  filter: {},
  facet: "folder",
  tracks: [],
  viewMode: "table",
  sort: { field: "artist", dir: "asc" },
};

// Doublons internes panel state (Bibliothèque). `groups: null` = not run yet this session.
// Reassigned both here and from sift-live.ts's click handler (the "Doublons" stat/chip and its
// resolve action) — kept as one object rather than 3 loose lets precisely so that cross-module
// reassignment is a property write, not a rebinding.
// `error` distingue « le scan a échoué » de « le scan a rendu zéro groupe ». Les deux
// collapsaient en `groups = []` dans les catch de sift-live.ts, donc un scan en ÉCHEC affichait
// « Aucun doublon dans toute la bibliothèque » — une affirmation sur l'état du disque de
// l'utilisateur, produite par une commande qui n'a jamais abouti. Audit 2026-07-28, CC-1.
/** Minuterie du debounce de recherche. Au niveau module et non sur le nœud : le champ vit
 *  désormais dans la barre unifiée et survit aux rendus, mais il disparaît au changement d'écran —
 *  une minuterie accrochée à lui partirait alors avec lui, en laissant un rendu programmé. */
let bibSearchTimer: number | undefined;

/** Sélection courante de la table (étape 5, `docs/ui-specs/bibliotheque.md`).
 *
 *  Un `Set` d'ids et non d'index : la liste est virtualisée ET triable, donc un index ne désigne
 *  pas la même piste d'un rendu à l'autre. L'ancre du ⇧+clic est un id pour la même raison. */
export const bibSelection = new Set<number>();
let bibAnchor: number | null = null;

/** Applique un clic de ligne selon ses modificateurs, à la convention système.
 *  Rend `true` quand la sélection a changé et que l'écran doit se repeindre. */
export function applyRowClick(id: number, mods: { shift: boolean; meta: boolean }, ordered: number[]): void {
  if (mods.shift && bibAnchor != null) {
    const a = ordered.indexOf(bibAnchor);
    const b = ordered.indexOf(id);
    if (a >= 0 && b >= 0) {
      bibSelection.clear();
      for (let i = Math.min(a, b); i <= Math.max(a, b); i++) bibSelection.add(ordered[i]);
      return;
    }
  }
  if (mods.meta) {
    // Bascule : ⌘/Ctrl+clic ajoute ou retire, il ne remplace jamais.
    if (bibSelection.has(id)) bibSelection.delete(id);
    else bibSelection.add(id);
    bibAnchor = id;
    return;
  }
  bibSelection.clear();
  bibSelection.add(id);
  bibAnchor = id;
}

/** Sélectionne tout ce que le filtre courant laisse voir — pas toute la bibliothèque. ⌘A dans une
 *  liste filtrée qui sélectionnerait au-delà du filtre est le raccourci le plus dangereux qui
 *  soit : il porte sur ce qu'on ne voit pas. */
export function selectAllVisible(): void {
  bibSelection.clear();
  for (const t of bibState.tracks) bibSelection.add(t.id);
  bibAnchor = bibState.tracks[0]?.id ?? null;
}

/** Résumé agrégé de la sélection dans la zone D.
 *
 *  Multi-sélection = résumé, JAMAIS un état vide (DESIGN.md § 14). Un inspecteur qui se vide dès
 *  qu'on sélectionne deux lignes punit exactement le geste qu'on vient d'apprendre à l'utilisateur.
 *  Ce qu'il montre est ce qui a un sens agrégé — un compte, des formats, une durée totale — et
 *  rien d'autre : un « artiste » agrégé n'existe pas. */
export function renderSelectionSummary(): void {
  if (bibSelection.size < 2) return;
  const host = openAside();
  if (!host) return;
  const picked = bibState.tracks.filter((t) => bibSelection.has(t.id));
  const fmts = new Map<string, number>();
  let total = 0;
  let unknown = 0;
  for (const t of picked) {
    const f = (t.format || "?").toUpperCase();
    fmts.set(f, (fmts.get(f) ?? 0) + 1);
    if (t.duration != null && Number.isFinite(t.duration)) total += t.duration;
    else unknown++;
  }
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const dur = h > 0 ? `${h} h ${String(m).padStart(2, "0")}` : `${m} min`;
  host.innerHTML =
    `<div class="col-h">Sélection</div>` +
    `<div class="sift-sel-count">${picked.length} pistes</div>` +
    `<dl class="sift-sel-rows">` +
    [...fmts.entries()]
      .sort((a, b) => b[1] - a[1])
      .map(([f, n]) => `<dt>${esc(f)}</dt><dd>${n}</dd>`)
      .join("") +
    `<dt>Durée totale</dt><dd>${dur}${unknown ? ` <span class="sift-sel-partial">+ ${unknown} sans durée</span>` : ""}</dd>` +
    `</dl>`;
}

/** Zone D, état « aucune sélection » (`docs/ui-specs/bibliotheque.md`) : le résumé de la source
 *  active — compte, répartition par format, occupation disque.
 *
 *  C'est ici que vont les cartes de statistiques et le graphique d'occupation qui vivaient EN TÊTE
 *  DE LA ZONE C. Ils y poussaient la table vers le bas — mesuré le 2026-08-19, ~300px avant la
 *  première ligne dans une fenêtre de 908px de haut — et ils décrivaient la bibliothèque entière
 *  pendant que la table, elle, montrait un sous-ensemble filtré. Deux portées dans la même colonne.
 *
 *  L'inspecteur est leur place : il porte le contexte de ce que la zone C montre, jamais du contenu
 *  à parcourir. C'est le patron Finder — la colonne de droite décrit, elle ne liste pas. */
function renderBibInspectorIdle(): void {
  const host = openAside();
  if (!host) return;
  const f = bibState.filter;
  const source = f.folder ?? f.genre ?? f.artist ?? "Tous";
  const n = bibState.tracks.length;
  // Répartition calculée sur CE QUE LA TABLE MONTRE, jamais sur `library_stats`.
  //
  // Mesuré le 2026-08-19 : la première version reprenait les compteurs globaux, et sous un titre
  // « TECH HOUSE · 2 pistes » on lisait « Lossless 2 · MP3 1 » — trois pistes sous un titre qui en
  // annonce deux. C'est le défaut que sortir ces cartes de la zone C devait supprimer, pas
  // déplacer : un inspecteur décrit ce que la zone C montre, sinon il décrit autre chose sans le
  // dire. `library_stats` n'est plus appelé du tout par cet écran.
  const fmts = new Map<string, number>();
  let fake = 0;
  for (const t of bibState.tracks) {
    const k = (t.format || "?").toUpperCase();
    fmts.set(k, (fmts.get(k) ?? 0) + 1);
    if (t.verdict === "fake") fake++;
  }
  host.innerHTML =
    `<div class="col-h">${esc(source)}</div>` +
    `<div class="sift-sel-count">${n} piste${n > 1 ? "s" : ""}</div>` +
    `<dl class="sift-sel-rows">` +
    [...fmts.entries()]
      .sort((a, b) => b[1] - a[1])
      .map(([k, c]) => `<dt>${esc(k)}</dt><dd>${c}</dd>`)
      .join("") +
    // « À re-sourcer » n'apparaît que s'il y en a. Une ligne à zéro en permanence occupe la place
    // d'une information et n'en porte aucune — et § 4 interdit d'estomper un échec, pas de le taire
    // quand il n'existe pas.
    (fake ? `<dt>À re-sourcer</dt><dd>${fake}</dd>` : "") +
    `</dl>` +
    `<div id="sift-bib-usage" class="sift-bib-usage"></div>`;
  mountBibUsage(host);
}

/** Ordre courant des ids, tel qu'affiché. Recalculé à chaque pas plutôt que mis en cache : le tri
 *  et le filtre changent sous les pieds, et une liste mémorisée ferait sauter la sélection d'une
 *  piste à une autre sans que rien ne bouge à l'écran. */
function orderedIds(): number[] {
  return sortTracks(bibState.tracks, bibState.sort).map((t) => t.id);
}

/** Couche 2 du clavier (DESIGN.md § 9) : ↑ ↓ déplacent, ⇧+↑↓ étendent, Début/Fin vont aux bouts.
 *
 *  Le déplacement se fait par INDEX dans la liste ordonnée, jamais en marchant sur les nœuds du
 *  DOM : la table est virtualisée, donc les lignes hors fenêtre n'existent pas — parcourir le DOM
 *  s'arrêterait silencieusement au bord de ce qui se trouve rendu. Même leçon que
 *  `stepQueueSelection` pour la file de Revue.
 *
 *  Rend `true` si la touche a été consommée. */
export function stepBibSelection(key: string, shift: boolean): boolean {
  const ids = orderedIds();
  if (!ids.length) return false;
  const cursor = bibAnchor != null ? ids.indexOf(bibAnchor) : -1;
  let next: number;
  switch (key) {
    case "ArrowDown":
      next = cursor < 0 ? 0 : Math.min(ids.length - 1, cursor + 1);
      break;
    case "ArrowUp":
      next = cursor < 0 ? ids.length - 1 : Math.max(0, cursor - 1);
      break;
    case "Home":
      next = 0;
      break;
    case "End":
      next = ids.length - 1;
      break;
    default:
      return false;
  }
  const target = ids[next];
  if (shift && bibAnchor != null) {
    // ⇧ étend depuis l'ancre SANS la déplacer — c'est ce qui permet de revenir en arrière dans la
    // même plage. Déplacer l'ancre à chaque pas transformerait l'extension en déplacement.
    const a = ids.indexOf(bibAnchor);
    bibSelection.clear();
    for (let i = Math.min(a, next); i <= Math.max(a, next); i++) bibSelection.add(ids[i]);
  } else {
    bibSelection.clear();
    bibSelection.add(target);
    bibAnchor = target;
  }
  void renderBiblioLive().then(() => {
    renderSelectionSummary();
    // La ligne visée peut être hors de la fenêtre virtualisée, donc absente du DOM : `scrollIntoView`
    // ne trouverait rien. On ne peut pas non plus la faire apparaître sans défiler d'abord. Ce
    // `?.` est donc un no-op assumé au-delà de la fenêtre — la sélection, elle, reste juste, et le
    // prochain rendu la marquera quand la ligne remontera.
    document.querySelector(`.lr[data-id="${target}"]`)?.scrollIntoView({ block: "nearest" });
  });
  return true;
}

export function clearBibSelection(): void {
  bibSelection.clear();
  bibAnchor = null;
}

/** Repeint la marque de sélection sur les lignes MONTÉES, sans reconstruire l'écran.
 *
 *  Un `renderBiblioLive()` complet ferait un aller-retour IPC et remonterait la liste virtuelle,
 *  ce qui émet un `scroll` — or le menu contextuel se ferme au premier défilement (il est ancré à
 *  un point, pas à un élément). Repeindre en place est donc la seule façon d'ouvrir un menu sur
 *  une sélection qu'on vient de changer.
 *
 *  Les lignes hors fenêtre virtualisée ne sont pas dans le DOM ; elles ne sont pas oubliées pour
 *  autant : `libraryTableRowHtml` relit `bibSelection` à chaque montage. */
export function paintBibSelection(): void {
  document.querySelectorAll<HTMLElement>('.lr[data-bib="row"]').forEach((n) => {
    const on = bibSelection.has(Number(n.dataset.id));
    n.classList.toggle("sel", on);
    n.setAttribute("aria-selected", on ? "true" : "false");
  });
}

/** Après une action de masse qui retire des pistes de la vue : la sélection ne désigne plus rien,
 *  l'inspecteur montrait ce qui vient de partir. Les deux se vident, puis l'écran se relit. */
function afterBulkRemoval(): Promise<void> {
  clearBibSelection();
  closeAside();
  return renderBiblioLive();
}

async function bulkReanalyze(ids: number[]): Promise<void> {
  try {
    // `reanalyze_tracks` rend le nombre réellement remis en file — il peut différer de ce qui a été
    // demandé (une piste déjà en analyse n'est pas réempilée). C'est ce nombre-là qui est annoncé.
    const n = await reanalyzeTracks(ids);
    toast(n === 0 ? "Rien à réanalyser — déjà en file" : `${plural(n, "piste")} remise${n > 1 ? "s" : ""} en analyse`);
  } catch (err: unknown) {
    toast(humanizeError(err, "Échec de la réanalyse — réessaie", "reanalyze_tracks"));
  }
}

async function bulkReject(ids: number[]): Promise<void> {
  if (
    ids.length > BATCH_CONFIRM_THRESHOLD &&
    !(await confirmAction(`Écarter ${plural(ids.length, "piste")} de la bibliothèque ?`, "Écarter"))
  )
    return;
  try {
    const r = await rejectBatch(ids);
    // `failed` n'est pas décoratif : un compte seul se lirait comme un succès plus petit, et
    // l'utilisateur croirait avoir écarté ce qui est resté. Voir `docs/ui-specs/bibliotheque.md`.
    toast(
      r.failed.length
        ? `${plural(r.rejected, "piste")} écartée${r.rejected > 1 ? "s" : ""}, ${r.failed.length} en échec`
        : `${plural(r.rejected, "piste")} écartée${r.rejected > 1 ? "s" : ""}`,
    );
  } catch (err: unknown) {
    toast(humanizeError(err, "Impossible d'écarter", "reject_batch"));
    return;
  }
  await afterBulkRemoval();
}

async function bulkTrash(ids: number[]): Promise<void> {
  if (
    ids.length > BATCH_CONFIRM_THRESHOLD &&
    !(await confirmAction(`Envoyer ${plural(ids.length, "piste")} à la corbeille ?`, "Envoyer à la corbeille"))
  )
    return;
  // SÉQUENTIEL, et ce n'est pas un oubli : `trash_track` est unitaire côté IPC, et le backend est
  // synchrone derrière un Mutex unique (`db::lock_conn`). Un `Promise.all` sur une grande sélection
  // lancerait autant d'invokes concurrents sur une frontière qui les sérialise de toute façon, en
  // bloquant tout le reste de l'IPC pendant ce temps.
  let done = 0;
  const failed: number[] = [];
  for (const id of ids) {
    try {
      await trashTrack(id);
      done++;
    } catch (err: unknown) {
      console.error("trash_track failed", id, err);
      failed.push(id);
    }
  }
  toast(
    failed.length
      ? `${plural(done, "piste")} à la corbeille, ${failed.length} en échec`
      : `${plural(done, "piste")} à la corbeille`,
  );
  await afterBulkRemoval();
}

/** Menu contextuel de la LIGNE D'EN-TÊTE. Patron Finder : le clic droit sur un en-tête de colonne
 *  ouvre les réglages de colonnes, pas les actions de piste.
 *
 *  Une seule entrée pour l'instant, et c'est la porte de sortie du redimensionnement : une colonne
 *  réduite à son plancher ou déplacée par mégarde pendant un clic de tri doit pouvoir être défaite
 *  sans aller vider un stockage navigateur. Désactivée quand la disposition est déjà d'origine —
 *  désactivée et non masquée, comme toutes les entrées de `context-menu.ts`. */
export function openColumnHeaderMenu(x: number, y: number): void {
  openContextMenu(x, y, [
    {
      label: "Réinitialiser les colonnes",
      onPick: columnsAreCustomized()
        ? () => {
            resetColumns();
            void renderBiblioLive();
          }
        : undefined,
    },
  ]);
}

/** Menu contextuel de la table (`docs/ui-specs/bibliotheque.md`, décisions du 2026-08-19).
 *
 *  La liste des entrées et leur ordre ne changent JAMAIS avec la taille de la sélection : ce qui
 *  ne s'applique pas à N pistes est désactivé, pas retiré (règle de `context-menu.ts` — un menu
 *  dont les entrées vont et viennent doit se relire à chaque ouverture). Seuls les libellés
 *  portent le compte.
 *
 *  Trois actions passent à N, et les trois ont été mesurées contre le contrat, pas choisies :
 *  `reanalyze_tracks` prend déjà un tableau, `reject_batch` est l'IPC du mode Lot, `trash_track`
 *  est unitaire et se boucle. Identifier n'y est pas : chaque identification demande de CHOISIR un
 *  candidat, donc l'appliquer en masse serait décider à la place de l'utilisateur. */
export function openBiblioContextMenu(x: number, y: number, id: number): void {
  // Convention système : un clic droit HORS de la sélection la remplace par la ligne visée ; un
  // clic droit DEDANS la garde entière. Sans cette règle, un clic droit sur la troisième ligne
  // d'une sélection de cent agirait soit sur une piste, soit sur cent, selon ce que le code a
  // décidé — et l'utilisateur ne pourrait pas le prédire.
  if (!bibSelection.has(id)) {
    applyRowClick(id, { shift: false, meta: false }, orderedIds());
    paintBibSelection();
    renderSelectionSummary();
  }
  const ids = [...bibSelection];
  if (ids.length === 0) return;
  const one = ids.length === 1;
  const suffix = one ? "" : ` (${ids.length})`;
  const track = one ? bibState.tracks.find((t) => t.id === ids[0]) : undefined;
  const rid = track?.discogs_release_id;
  // `openBiblioDetail` BASCULE. Le libellé suit donc l'état réel : sur une piste déjà ouverte,
  // l'entrée referme le panneau, et annoncer « Ouvrir » y était faux.
  const detailOpen = one && bibOpenId === ids[0];
  openContextMenu(x, y, [
    {
      label: "Ouvrir l'emplacement",
      // Une piste à la fois : révéler N fichiers ouvrirait N fenêtres d'explorateur.
      onPick: one
        ? () =>
            void revealTrack(ids[0]).catch((err: unknown) =>
              toast(humanizeError(err, "Impossible d'ouvrir l'emplacement", "reveal_track")),
            )
        : undefined,
    },
    {
      label: detailOpen ? "Masquer le détail" : "Ouvrir le détail",
      onPick: one ? () => openBiblioDetail(ids[0]) : undefined,
    },
    {
      label: "Fiche Discogs",
      onPick: one && rid ? () => void openUrl(`https://www.discogs.com/release/${rid}`) : undefined,
    },
    // Depuis le 2026-09-08 la fiche de la zone D est celle de Revue et n'a plus de bouton « changer »
    // sur la pochette : le geste vit ici, sur la piste dont la fiche est OUVERTE (la seule dont on
    // sait dans quel en-tête poser l'image) — sinon l'entrée est présente et désactivée, comme les
    // autres entrées à une piste.
    {
      label: "Changer la pochette…",
      onPick: detailOpen ? () => void changeCoverForOpenTrack() : undefined,
    },
    { label: `Réanalyser${suffix}`, separated: true, onPick: () => void bulkReanalyze(ids) },
    // « Écarter » n'est PAS `danger`, et ce n'est pas un oubli : `DESIGN.md` § 4 réserve le rouge au
    // « risque réel, destructif », pas à un simple avertissement. Écarter range la piste dans
    // Écartés, d'où elle se restaure — le geste est réversible et ne touche pas au fichier. Le
    // rouge y perdait son sens pour l'entrée d'en dessous, la seule des deux qui sorte quelque
    // chose de la bibliothèque. Deux rouges côte à côte n'en font qu'un.
    { label: `Écarter${suffix}`, separated: true, onPick: () => void bulkReject(ids) },
    { label: `Envoyer à la corbeille${suffix}`, danger: true, onPick: () => void bulkTrash(ids) },
  ]);
}

export const bibDup: {
  groups: DupGroup[] | null;
  loading: boolean;
  shown: boolean;
  error: string | null;
} = {
  groups: null,
  loading: false,
  shown: false,
  error: null,
};

/// Lance le scan de doublons et repeint, quel que soit l'issue.
///
/// Existe pour supprimer une duplication qui avait déjà divergé en pratique : `sift-live.ts`
/// portait DEUX copies de cette séquence (chip « doublons » et bouton dupscan), chacune avec son
/// `.catch` posant `groups = []` — donc chacune capable d'annoncer « aucun doublon » sur un scan
/// échoué. Un seul endroit, un seul comportement. Audit 2026-07-28, CC-1.
export function loadDuplicates(): void {
  bibDup.loading = true;
  bibDup.error = null;
  void renderBiblioLive();
  void scanLibraryDuplicates()
    .then((groups) => {
      bibDup.groups = groups;
      bibDup.error = null;
    })
    .catch((e: unknown) => {
      console.error("scan_library_duplicates failed", e);
      // `groups` reste à null : sans résultat, on ne prétend RIEN sur la bibliothèque.
      bibDup.groups = null;
      bibDup.error = humanizeScanError(e);
    })
    .finally(() => {
      bibDup.loading = false;
      void renderBiblioLive();
    });
}

/// Message court et actionnable à partir d'une erreur IPC brute. La chaîne brute reste en
/// console.error ; l'écran n'affiche jamais un `Error: ...` non traduit.
function humanizeScanError(e: unknown): string {
  const raw = String(e);
  if (raw.includes("db lock") || raw.includes("poisoned")) {
    return "La base est occupée. Réessaie dans un instant.";
  }
  return "Vérifie que la bibliothèque est accessible, puis réessaie.";
}

// Virtualized library list controller. Torn down and recreated on each full renderBiblioLive
// (which replaces #content.innerHTML, orphaning the old #biblist host — its scroll listener sits
// on the PERMANENT #content, so it must be explicitly destroyed or it leaks + double-renders).
// Private to this module — nothing outside renderBiblioLive touches it.
let bibVirtual: VirtualList | null = null;
// Which library row is open in the detail panel — stamped as `.cur` at row-creation time so the
// highlight survives virtualization. Private — only openBiblioDetail/renderBiblioLive touch it.
let bibOpenId: number | null = null;

function dupMemberHtml(m: DupGroup["members"][number]): string {
  const name = esc(m.filename || m.path.split(/[\\/]/).pop() || m.path);
  const fmt = (m.format || "?").toUpperCase();
  const br = m.bitrate ? `${m.bitrate} kbps` : "";
  return (
    `<div style="display:flex;align-items:center;gap:8px;padding:4px 0${m.recommend_keep ? "" : ";opacity:.6"}">` +
    `<span style="flex:1;min-width:0;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">${name}</span>` +
    `<span class="pill" style="flex:none">${esc(fmt)}</span>` +
    `<span style="flex:none;width:80px;text-align:right;font-size:var(--text-sm);color:var(--color-text-tertiary)">${esc(br)}</span>` +
    (m.recommend_keep
      ? `<span class="pill" style="flex:none;background:var(--color-background-success);color:var(--color-text-success)" title="${esc(m.reason || "")}">Recommandé</span>`
      : "") +
    `</div>`
  );
}

function dupGroupHtml(g: DupGroup, idx: number): string {
  const loserCount = g.members.filter((m) => !m.recommend_keep).length;
  return (
    `<div class="sift-dup-group" style="border:1px solid var(--color-border-tertiary);border-radius:var(--border-radius-md);padding:10px 12px;margin-bottom:8px">` +
    g.members.map((m) => dupMemberHtml(m)).join("") +
    `<div style="margin-top:6px"><button data-bib="dupresolve" data-idx="${idx}">Envoyer ${loserCount} doublon${loserCount > 1 ? "s" : ""} à la corbeille</button></div>` +
    `</div>`
  );
}

/** Ouvre ou ferme le sélecteur de facette, ancré sous son bouton.
 *
 *  Le popover est peint `hidden` par le rendu : tant qu'il l'est, ses nœuds mesurent 0 — d'où
 *  l'ordre ici, montrer PUIS mesurer PUIS placer, et le pouce du contrôle segmenté positionné
 *  seulement après. Le placer avant l'affichage le collerait à l'origine, sans erreur visible. */
/** Le menu survit-il au PROCHAIN rendu ? Un seul, et seulement celui qu'il a lui-même déclenché.
 *
 *  Changer de type (Dossiers / Genres / Artistes) relance `renderBiblioLive`, qui réécrit
 *  `#content` : sans mémoire, le menu se refermerait sous le doigt au premier type cliqué. Mais la
 *  première version gardait un simple booléen « ouvert », que CHAQUE rendu relisait — et les rendus
 *  arrivent de partout, y compris d'un tick de scan. Mesuré le 2026-08-19 : le menu se rouvrait
 *  indéfiniment et finissait ancré à 884 px du haut, seul en bas de la fenêtre, sur un écran où
 *  personne ne l'avait demandé.
 *
 *  Un jeton à usage unique plutôt qu'un état durable : c'est le geste qui prolonge le menu, pas le
 *  temps qui passe. */
let facetPopSurviveNextRender = false;
/** Ce que la barre porte hors segmenté (pilote + chips) au dernier montage : tant que c'est égal,
 *  `renderBiblioLive` ne remonte pas la barre et le pouce Tableau/Grille garde son nœud. */
let lastBarKey = "";

/** Appelée par le dispatch avant de relancer un rendu depuis le menu lui-même. */
export function keepFacetPopoverOpen(): void {
  facetPopSurviveNextRender = true;
}

export function toggleFacetPopover(): void {
  const pop = document.getElementById("sift-facet-pop");
  if (pop && !pop.hidden) closeFacetPopover();
  else showFacetPopover();
}

/** Affiche et place le popover. Séparée de la bascule pour être rappelable après un rendu, quand
 *  l'état dit ouvert mais que le nœud vient d'être recréé fermé. */
function showFacetPopover(): void {
  const pop = document.getElementById("sift-facet-pop");
  const btn = document.querySelector<HTMLElement>('[data-bib="facetpop"]');
  if (!pop || !btn) return;
  // Un bouton de largeur nulle n'est pas encore peint : l'ancrer donnerait une position calculée
  // sur une géométrie vide, et le menu partirait dans un coin de la fenêtre — vu le 2026-08-19,
  // ouvert en bas à gauche après un rendu. Mieux vaut refermer que placer au hasard.
  if (!btn.getBoundingClientRect().width) {
    closeFacetPopover();
    return;
  }
  pop.hidden = false;
  btn.setAttribute("aria-expanded", "true");
  // Placement au SECOND frame, pas dans la foulée. Un rAF s'exécute AVANT le recalcul de style
  // dans Chromium/WebView2 — mesurer là donne les dimensions d'avant l'affichage, et l'ancrage
  // part d'une géométrie qui n'existe déjà plus. Même leçon que `playFadeIn` (confirm-modal.ts),
  // qui attend le second frame pour la même raison.
  //
  // Ce qui a rendu la leçon nécessaire ici : ce panneau est rouvert par `renderBiblioLive` après
  // un rebuild complet de `#content`, donc au milieu d'un cycle de mise en page où la barre
  // unifiée et la liste virtualisée ne sont pas encore montées. Un ancrage calculé à cet instant
  // se retrouve décalé de tout ce qui s'est monté ensuite.
  requestAnimationFrame(() => requestAnimationFrame(() => placeFacetPopover()));
  pop.querySelector<HTMLElement>(".fld, .sift-seg-opt")?.focus();
}

/** Ancre le panneau sous son bouton, depuis la géométrie du moment. Séparée de l'affichage pour
 *  être rejouable — le placement d'un popover n'est valable que pour la mise en page qui l'a vu
 *  naître. */
function placeFacetPopover(): void {
  const pop = document.getElementById("sift-facet-pop");
  const btn = document.querySelector<HTMLElement>('[data-bib="facetpop"]');
  if (!pop || pop.hidden || !btn) return;
  const r = btn.getBoundingClientRect();
  const { top, left } = anchoredBelowPosition(
    { top: r.top, bottom: r.bottom, left: r.left },
    pop.offsetWidth,
    pop.offsetHeight,
    document.documentElement.clientWidth,
    document.documentElement.clientHeight,
  );
  pop.style.top = `${top}px`;
  pop.style.left = `${left}px`;
}

/** Le panneau est ancré à un POINT, pas à un élément : dès que la mise en page bouge sous lui, il
 *  désigne autre chose que ce qu'il montre. Défiler ou redimensionner le FERME plutôt que de
 *  courir après la géométrie — c'est déjà la règle du menu contextuel (`context-menu.ts`), et une
 *  seule règle pour deux surfaces flottantes vaut mieux que deux comportements à retenir. */
export function installFacetPopoverDismiss(): void {
  const close = () => closeFacetPopover();
  document.addEventListener("scroll", close, { capture: true });
  window.addEventListener("resize", close);
}

export function closeFacetPopover(): void {
  facetPopSurviveNextRender = false;
  const pop = document.getElementById("sift-facet-pop");
  if (!pop || pop.hidden) return;
  pop.hidden = true;
  document.querySelector('[data-bib="facetpop"]')?.setAttribute("aria-expanded", "false");
}


/** Le pouce du segmenté Tableau/Grille — placement partagé (`seg-thumb.ts`). Le renvoi qui vivait
 *  ici, « même motif que positionFacetThumb() », était MORT : cette fonction-là n'existe plus. */
export function positionViewModeThumb(): void {
  const seg = document.getElementById("sift-bib-viewmode-seg");
  if (seg) slideSegThumb(seg, "[data-bib='viewmode'].on");
}

/** Dernière ventilation par format de la bibliothèque, gardée entre deux rendus.
 *
 * `renderBiblioLive` se rejoue à chaque frappe de recherche, chaque facette et chaque tri, et il
 * écrase `#content` à chaque fois. Or ce graphique décrit la bibliothèque ENTIÈRE, pas le
 * sous-ensemble filtré : le refaire à chaque touche coûterait un aller-retour IPC pour un résultat
 * identique, et le ferait clignoter. On le relit donc au premier affichage de l'écran seulement,
 * et on redessine depuis cette valeur ensuite. */
let bibUsage: UsageReport | null = null;

/** Dessine le graphique d'occupation dans le slot que `renderBibInspectorIdle` vient de poser.
 *
 *  Le premier affichage lit l'IPC, les suivants redessinent depuis `bibUsage` : ce graphique décrit
 *  la bibliothèque ENTIÈRE, pas le sous-ensemble filtré, donc le refaire à chaque frappe de
 *  recherche coûterait un aller-retour pour un résultat identique — et le ferait clignoter. */
function mountBibUsage(host: HTMLElement): void {
  const slot = host.querySelector<HTMLElement>("#sift-bib-usage");
  if (!slot) return;

  const draw = (report: UsageReport) => {
    slot.innerHTML = '<div class="col-h">Occupation</div>';
    slot.appendChild(renderUsageChart({ report }));
  };

  if (bibUsage) {
    draw(bibUsage);
    return;
  }
  slot.innerHTML =
    '<div class="col-h">Occupation</div>' +
    '<div class="sift-usb-empty">Lecture…</div>';
  void libraryUsage()
    .then((r) => {
      bibUsage = r;
      draw(r);
    })
    .catch((e: unknown) => {
      // Impasse A19 (issue #15) : `slot.remove()` faisait disparaître la section, si bien que du
      // point de vue de l'utilisateur elle n'avait jamais existé — rien à réessayer, rien à
      // signaler. Le cas jumeau de Clé USB (`usb-view.ts::mountUsage`) garde son slot et affiche
      // la chaîne brute ; c'est ce modèle qu'on porte ici, plus une porte de sortie.
      slot.innerHTML =
        '<div class="col-h">Occupation</div>' +
        '<div class="sift-usb-empty">' +
        esc(humanizeError(e, "Occupation indisponible.", "libraryUsage")) +
        "<br>" +
        esc(String(e)) +
        "</div>" +
        '<div class="sift-settings-subactions"><button data-bib="retryusage" class="sift-settings-btn sift-settings-btn-quiet">Réessayer</button></div>';
      slot
        .querySelector<HTMLButtonElement>('[data-bib="retryusage"]')
        ?.addEventListener("click", () => {
          // `bibUsage` est resté nul (l'appel a échoué), donc ce remontage relit vraiment l'IPC.
          mountBibUsage(host);
        });
    });
}

/** Live Bibliothèque view: lists filed tracks with search + quality chips + folder/genre
 * facets, wired to real data. Actions go through the #pa delegated handler (data-bib). */
export async function renderBiblioLive() {
  const content = requireEl("#content", "renderBiblioLive");
  // Jeton capturé avec `#content` (issue #42) : sous scan, le `Promise.all` ci-dessous attend le
  // `Mutex<Connection>` que le scanner tient, et l'utilisateur a le temps de changer d'écran.
  const token = viewEpoch();
  // Tear down any previous virtual list first: its scroll listener sits on the permanent #content,
  // which this render is about to overwrite — leaving it attached would leak the listener and fire
  // renders against a detached host.
  bibVirtual?.destroy();
  bibVirtual = null;
  // The IPC round-trip below (Promise.all) can take a beat on a large library — without a signal
  // the FIRST paint would sit frozen (blank #content, nothing rendered yet). Same "Chargement…"
  // pattern as queue-panel.ts's renderQueue(), gated the same way: only when there's no prior
  // Bibliothèque render already on screen. Without this gate, every subsequent renderBiblioLive()
  // call — search keystroke, facet/quality-chip click, table/grid toggle, sort change, dup scan —
  // would blank the whole panel (stats/facets/toolbar/list) before repainting, even though valid
  // data is already showing.
  // Le repère du « déjà rendu » suit la ligne d'en-tête depuis que le contrôle segmenté de
  // facette a disparu (2026-08-19) : viser un nœud supprimé aurait blanchi l'écran à chaque frappe.
  const alreadyRendered = !!content.querySelector(".sift-bib-headline");
  if (!alreadyRendered) {
    content.innerHTML =
      '<div style="display:flex;align-items:center;gap:8px;padding:8px 7px;color:var(--color-text-tertiary);font-size:var(--text-md)">' +
      '<i class="ti ti-loader sift-spin" style="font-size:var(--text-md)"></i> Chargement…</div>';
  }
  let facets: LibraryFacets = { folders: [], genres: [], artists: [] };
  try {
    // `library_stats` a quitté ce Promise.all le 2026-08-19 : il n'alimentait que les cartes de
    // statistiques, retirées de la zone C, et l'inspecteur compte désormais ce que la table montre
    // plutôt que la bibliothèque entière. Un aller-retour IPC de moins à chaque frappe de recherche.
    [bibState.tracks, facets] = await Promise.all([listLibrary(bibState.filter), libraryFolders()]);
    if (isStaleViewRender(token)) return;
  } catch (e) {
    // Impasse A20 (issue #15) : cette carte était la seule des trois sans porte de sortie — Écartés
    // (`ecartes-view.ts`) et le bloc doublons plus bas en ont une depuis leur audit respectif.
    // « réessaie » sans bouton demande à l'utilisateur de deviner comment.
    if (isStaleViewRender(token)) return;
    content.innerHTML =
      '<div class="sift-ui-card-soft sift-ui-card-soft-pad" style="color:var(--color-text-danger)">' +
      esc(
        humanizeError(
          e,
          "Impossible de charger la Bibliothèque. Vérifie la connexion à la base et réessaie.",
          "library load",
        ),
      ) +
      '<div style="margin-top:var(--space-8)"><button data-bib="retryload" style="font-size:var(--text-xs);color:var(--color-text-info)">Réessayer</button></div>' +
      "</div>";
    content
      .querySelector<HTMLButtonElement>('[data-bib="retryload"]')
      ?.addEventListener("click", () => void renderBiblioLive());
    return;
  }

  // Audit-ref B2 (Bibliothèque, 2026-07-09) : <span> converti en <button> pour un clavier natif
  // (pas besoin d'étendre installNavKeyboard — un vrai bouton gère déjà Enter/Espace lui-même).
  const chips =
    (["all", "lossless", "mp3"] as const)
      .map((q) => {
        const on = (bibState.filter.quality ?? "all") === q;
        const label = q === "all" ? "Tous" : q === "lossless" ? "Lossless" : "MP3";
        return `<button class="chip${on ? " on" : ""}" data-bib="qual" data-q="${q}">${label}</button>`;
      })
      .join("") +
    `<button class="chip${bibDup.shown ? " on" : ""}" data-bib="dupscan">Doublons</button>`;

  const facetList =
    bibState.facet === "folder" ? facets.folders : bibState.facet === "genre" ? facets.genres : facets.artists;
  const sideKey = bibState.facet;
  const activeFacetVal =
    bibState.facet === "folder"
      ? bibState.filter.folder
      : bibState.facet === "genre"
        ? bibState.filter.genre
        : bibState.filter.artist;
  const facetLabel =
    bibState.facet === "folder" ? "Dossiers" : bibState.facet === "genre" ? "Genres" : "Artistes";
  // MENU, pas carte flottante — refonte du 2026-08-19 sur la remarque d'Antoine (« le panneau est
  // placé bizarrement, regarde comment fait Apple Music »).
  //
  // Source : HIG « Pop-up buttons », lue le 2026-08-19. « A pop-up button displays a menu of
  // mutually exclusive options » et « the button can update its content to indicate the current
  // selection » — c'est exactement ce couple bouton/valeur. La même page envoie vers le
  // pull-down button dès qu'il y a un SOUS-MENU, ce que la version précédente avait de fait : un
  // contrôle segmenté à trois onglets empilé au-dessus d'une liste, dans une carte de 272px.
  //
  // Ce qui change : plus de contrôle segmenté (le type devient une première SECTION du menu,
  // séparée par un filet), largeur prise sur le contenu au lieu d'un `--pane-w` hérité de la
  // colonne qu'on a supprimée, items compacts, et la valeur active portée par une COCHE — la
  // marque d'un item choisi dans un menu macOS, pas un fond plein comme dans une liste.
  const facetTypes: [typeof bibState.facet, string][] = [
    ["folder", "Dossiers"],
    ["genre", "Genres"],
    ["artist", "Artistes"],
  ];
  const check = (on: boolean) =>
    `<span class="sift-menu-check" aria-hidden="true">${on ? "✓" : ""}</span>`;
  const side =
    `<div class="sift-menu-section">` +
    facetTypes
      .map(
        ([f, label]) =>
          `<button type="button" class="sift-menu-item" data-bib="facet" data-f="${f}" role="menuitemradio" aria-checked="${bibState.facet === f}">${check(bibState.facet === f)}<span class="sift-menu-label">${label}</span></button>`,
      )
      .join("") +
    `</div>` +
    // Audit-ref B1 : rôle et clavier natifs — ce sont de vrais `<button>`, donc Entrée/Espace
    // marchent sans passer par `installNavKeyboard`.
    `<div class="sift-menu-section">` +
    (facetList.length
      ? `<button type="button" class="sift-menu-item" data-bib="pick" data-key="${sideKey}" data-val="" role="menuitemradio" aria-checked="${!activeFacetVal}">${check(!activeFacetVal)}<span class="sift-menu-label">Tous</span></button>` +
        facetList
          .map(
            (b) =>
              `<button type="button" class="sift-menu-item" data-bib="pick" data-key="${sideKey}" data-val="${esc(b.name)}" role="menuitemradio" aria-checked="${activeFacetVal === b.name}">${check(activeFacetVal === b.name)}<span class="sift-menu-label">${esc(b.name)}</span><span class="sift-menu-count">${b.count}</span></button>`,
          )
          .join("")
      : // Facette sans valeur : le dire. Mesuré le 2026-08-19 sur une vraie bibliothèque — la
        // facette Dossiers ne rend rien tant qu'aucune piste n'est rangée dans un sous-dossier, et
        // le menu s'ouvrait alors sur ses trois types et du vide, ce qui se lit comme un défaut de
        // chargement plutôt que comme une absence.
        `<div class="sift-facet-empty">Aucun ${bibState.facet === "folder" ? "dossier" : bibState.facet === "genre" ? "genre" : "artiste"} pour l'instant.</div>`) +
    `</div>`;

  // The list is virtualized (createVirtualList below) — this placeholder is the mount host, filled
  // with only the visible window of rows after content.innerHTML. Rendering all bibState.tracks
  // here would reintroduce the 15k-track freeze (audit 2026-07-05 P2). `rows` non-empty iff there's
  // at least one track, used only to pick the "no result" fallback below.
  const rows = bibState.tracks.length ? '<div id="biblist"></div>' : "";
  // bibState.sort is shared across both view modes on purpose: the sortable column headers only
  // exist in Tableau, but Grille inheriting whatever order was last chosen there (instead of
  // reverting to an unrelated default the moment you switch) matches how table/grid toggles behave
  // elsewhere (e.g. Finder/Explorer icon view keeps the list view's sort). Accepted as-is — no
  // Grille-side sort control added; revisit only if that inherited-but-unchangeable order is
  // reported as confusing in practice.
  const sortedTracks = sortTracks(bibState.tracks, bibState.sort);
  const tableHead = bibState.viewMode === "table" ? libraryTableHeaderHtml(bibState.sort) : "";
  // Truly empty (no filed track at all, no filter narrowing it) vs. a filter that just matches
  // nothing right now — only the former is DESIGN.md's "État vide" dead-end with a back-to-Revue
  // link; the latter keeps the search/chips/facets on screen so the filter can be cleared.
  const noFilter =
    !bibState.filter.q &&
    !bibState.filter.quality &&
    !bibState.filter.folder &&
    !bibState.filter.genre &&
    !bibState.filter.artist;
  const trulyEmpty = bibState.tracks.length === 0 && noFilter;

  // L'état d'ERREUR passe avant tout le reste : tant qu'il est posé, on ne dit rien sur le
  // contenu de la bibliothèque. Dire « aucun doublon » après un scan qui a échoué serait
  // affirmer un fait qu'on n'a pas mesuré.
  // Le scan est un MODE de la zone C depuis le 2026-08-19, plus un appendice sous la table
  // (`docs/ui-specs/bibliotheque.md`) : « un scan est un résultat, pas un appendice de liste ».
  // Rendu sous la table, il obligeait à faire défiler tout un inventaire pour lire la réponse à
  // une question qu'on venait de poser, et laissait deux listes concurrentes à l'écran.
  const dupSection = !bibDup.shown
    ? ""
    : bibDup.loading
      ? `<div style="margin-top:10px;font-size:var(--text-md);color:var(--color-text-tertiary)">Scan en cours (toute la bibliothèque)…</div>`
      : bibDup.error
        ? // Même forme que l'état d'erreur d'Écartés (ecartes-view.ts) : carte douce, texte
          // danger, bouton Réessayer discret. Réutilisé plutôt que réinventé, pour que les deux
          // écrans échouent de la même façon.
          `<div class="sift-ui-card-soft sift-ui-card-soft-pad" style="margin-top:10px;color:var(--color-text-danger)">` +
          `Le scan de doublons n'a pas abouti. ${esc(bibDup.error)}` +
          `<div style="margin-top:8px"><button data-bib="dupretry" style="font-size:var(--text-xs);padding:4px 10px;color:var(--color-text-info)">Réessayer</button></div>` +
          `</div>`
        : bibDup.groups === null
          ? ""
          : bibDup.groups.length === 0
            ? `<div style="margin-top:10px;font-size:var(--text-md);color:var(--color-text-tertiary)">Aucun doublon dans toute la bibliothèque.</div>`
            : `<div style="margin-top:10px"><div style="font-size:var(--text-xs);color:var(--color-text-tertiary);margin-bottom:4px">Doublons détectés dans toute la bibliothèque (pas seulement la vue filtrée actuelle)</div>${bibDup.groups.map((g, i) => dupGroupHtml(g, i)).join("")}</div>`;

  // Export (Rekordbox/Clé USB) lives in the nav rail now, not here — matches the maquette's
  // persistent Export section (index.html nav-export items, wired in installLiveWiring below).
  // Retour Antoine 2026-07-09 : le toolbar (recherche+chips) était sa PROPRE boîte flottante
  // au-dessus du panneau — une seule information (le filtre courant), grouper ça seul ajoute du
  // chrome pour rien (même règle HIG Boxes que la consolidation Réglages, 2026-07-08). Intégré
  // maintenant comme bandeau supérieur du panneau .sift-library-main, séparé par un filet.
  // Étape 2 (DESIGN.md § 17) : filtres, mode de vue et recherche ont quitté le panneau pour la
  // barre unifiée. Ils n'appartenaient pas au contenu — ce sont des contrôles de la fenêtre, et
  // leur place change aujourd'hui d'un écran à l'autre. La recherche y gagne en plus de survivre
  // au rendu : dans `#content` elle était détruite par le rebuild `innerHTML` que sa PROPRE frappe
  // déclenchait, donc le focus tombait à chaque recherche (voir `toolbar.ts`).
  // Le bouton de facette (pop-up « Dossiers · Tous ⌄ ») monte dans la barre le 2026-09-08 — « F
  // avec la barre de M » (Antoine, audit #24, wireframe sur la grille de Revue). Précédent Apple :
  // Photos met son pop-up « Days / Months / Years » au centre de la toolbar (docs/design-refs/
  // 02-photos.png) ; HIG Toolbars, « center area : common, useful controls ». Le popover
  // `#sift-facet-pop` reste dans `#content` et s'ancre sur ce bouton par `[data-bib="facetpop"]`,
  // donc son placement (`showFacetPopover`) n'a pas bougé. En mode doublons, la porte de sortie
  // nommée prend SA place : ce qui pilote la zone C reste au même endroit d'un mode à l'autre.
  const pilot = bibDup.shown
    ? `<button data-bib="dupscan" class="sift-bib-back"><i class="ti ti-chevron-left" aria-hidden="true"></i> Retour à la table</button>`
    : `<button data-bib="facetpop" class="sift-bib-facet-btn" aria-haspopup="true" aria-expanded="false">` +
      `<span class="sift-bib-facet-kind">${esc(facetLabel)}</span>` +
      `<span class="sift-bib-facet-val">${esc(activeFacetVal || "Tous")}</span>` +
      `<i class="ti ti-chevron-down" aria-hidden="true"></i></button>`;
  const barActionsHtml =
    pilot +
    chips +
    `<div class="sift-seg sift-seg-thumbed" id="sift-bib-viewmode-seg">` +
    `<div class="sift-seg-thumb"></div>` +
    `<button class="sift-seg-opt${bibState.viewMode === "table" ? " on" : ""}" data-bib="viewmode" data-mode="table" aria-label="Vue tableau"><i class="ti ti-list"></i></button>` +
    `<button class="sift-seg-opt${bibState.viewMode === "grid" ? " on" : ""}" data-bib="viewmode" data-mode="grid" aria-label="Vue grille"><i class="ti ti-layout-grid"></i></button></div>`;

  // Zone C = la table, et rien d'autre (`docs/ui-specs/bibliotheque.md`). Trois blocs l'ont quittée
  // le 2026-08-19 : les cartes de statistiques et le graphique d'occupation sont montés dans la
  // zone D (ils décrivent, ils ne se parcourent pas), et le sélecteur de facette est replié en un
  // BOUTON portant la valeur active — il filtrait, et un filtre n'appartient pas au contenu.
  // Ce qui reste au-dessus de la table est une seule ligne : facette · valeur · compte.
  content.innerHTML = trulyEmpty
    ? emptyStateHtml({
        title: "Bibliothèque vide",
        note: "Les pistes que tu convertis depuis Revue apparaissent ici, prêtes à exporter vers Rekordbox ou une clé USB.",
        backToRevue: true,
      })
    : // Plus de carte ni de rangée de tête depuis le 2026-09-08 (audit Rangés, #24, « F avec la barre
      // de M » — Antoine) : la table est AU SOL, même fond que la zone C de Revue (patterns.md, « une
      // surface de contenu ne peint rien », appliqué à Revue le 08-14 et ici ce jour-là), et tout ce
      // qui la pilote — facette, compte, retour du mode doublons — vit dans la barre unifiée
      // (`barActionsHtml`, `#sift-tb-count`). Mesuré avant : zone C de Rangés peinte de la couleur de
      // la file de Revue (0.2939) sur le sol (0.2273), plus un cran pour l'en-tête — trois plans là
      // où Revue en a deux.
      `<div class="sift-library-main">` +
      (bibDup.shown
        ? // MODE SCAN. La table cède la place ; la porte de sortie nommée (« Retour à la table ») est
          // dans la barre, à l'emplacement du bouton de facette — ce qui pilote la zone C reste au
          // même endroit d'un mode à l'autre.
          dupSection
        : tableHead +
          // « Aucun résultat » reste SOUS l'en-tête de colonnes, il ne le remplace pas.
          // Référence : shadcn `data-table-demo`, dont l'état vide est une ligne du corps
          // (`colSpan`, texte centré) et non un bloc à la place de la table. Le motif tient
          // au-delà du style : remplacer la table emporte les en-têtes, donc les contrôles de
          // tri — on retire à l'utilisateur les commandes qui pourraient défaire son filtre.
          (rows ||
            `<div class="sift-bib-noresult">Aucun résultat pour ce filtre. <button data-bib="stat" data-stat="all">Réinitialiser les filtres</button></div>`)) +
      `</div>` +
      // Le popover de facette vit DANS `#content` mais en `position:fixed` : il est peint par le
      // même rendu que son bouton, donc il ne peut pas survivre à un changement d'écran — un
      // popover orphelin resterait à l'écran en pointant une facette qui n'existe plus.
      `<div class="sift-facet-pop" id="sift-facet-pop" hidden>${side}</div>`;

  // Zone D. Une sélection multiple montre son résumé agrégé, une piste ouverte son détail, et
  // sinon l'inspecteur porte le contexte de la source active — jamais rien.
  if (!trulyEmpty && bibSelection.size < 2 && bibOpenId == null) renderBibInspectorIdle();
  wireEmptyState(content);
  // Redimensionnement et réordonnancement des colonnes (`DESIGN.md` § 16, livrés le 2026-08-19).
  // Réinstallés à chaque rendu parce que la ligne d'en-tête est un nœud NEUF à chaque fois : rien
  // ne fuit, les écouteurs partent avec le nœud que `content.innerHTML` vient de remplacer.
  const thead = content.querySelector<HTMLElement>(".sift-lib-thead");
  if (thead) installColumnGestures(thead, () => void renderBiblioLive());
  // Le menu ne revient que s'il a demandé CE rendu (changement de type). Le jeton se consomme :
  // tout autre rendu — frappe de recherche, tick de scan, changement de tri — le laisse fermé.
  if (facetPopSurviveNextRender) {
    facetPopSurviveNextRender = false;
    showFacetPopover();
  }
  positionViewModeThumb();

  if (trulyEmpty) {
    // Bibliothèque vide : pas de filtre à offrir, donc rien dans la barre. C'est une impasse
    // assumée (DESIGN.md § 8) — le rail et le titre restent, les contrôles non.
    mountBarActions("");
    const emptyCount = document.getElementById("sift-tb-count");
    if (emptyCount) emptyCount.textContent = "";
    return;
  }

  // Le segmenté Tableau/Grille SURVIT au re-rendu de la barre : le clic sur une vue glisse le pouce
  // (sift-live.ts, `positionViewModeThumb`) puis appelle ce rendu, qui reconstruisait la barre par
  // `innerHTML` — le pouce neuf repartait de `left:0` et reglissait (mesuré le 2026-09-10 : 31 → 0
  // → 1 au retour vers Tableau, un saut visible). Une transition n'anime rien sur un nœud recréé
  // (CLAUDE.md § Front) : on remonte la barre, puis on remet l'ANCIEN segmenté à la place du neuf,
  // classes `.on` recalées — le pouce garde sa position de départ et la transition joue.
  // Remettre l'ancien nœud à la place du neuf ne suffit pas non plus : un nœud détaché puis
  // rattaché perd sa transition en vol (mesuré : même nœud, mais 31 px dès 100 ms). La barre ne se
  // remonte donc QUE si ce qu'elle porte hors segmenté (pilote, chips) a changé.
  const barKey = pilot + chips;
  const seg = document.getElementById("sift-bib-viewmode-seg");
  if (seg?.isConnected && barKey === lastBarKey) {
    seg
      .querySelectorAll<HTMLElement>("[data-bib='viewmode']")
      .forEach((b) => b.classList.toggle("on", b.dataset.mode === bibState.viewMode));
  } else {
    mountBarActions(barActionsHtml);
    lastBarKey = barKey;
  }
  positionViewModeThumb(); // au premier montage le nœud est neuf — le placer après montage
  // Le compte vit dans la barre, à côté du titre, depuis le 2026-09-08 — comme la file de Revue
  // (`queue-panel.ts`, même slot `#sift-tb-count`, spec revue.md § Zone A). Il dit ce que la table
  // MONTRE (filtre compris), jamais un total global ; en mode doublons il nomme la portée du scan.
  const countEl = document.getElementById("sift-tb-count");
  if (countEl) {
    countEl.textContent = bibDup.shown
      ? "Doublons — toute la bibliothèque"
      : `${bibState.tracks.length} piste${bibState.tracks.length > 1 ? "s" : ""}`;
  }

  // La recherche est le seul contrôle frappé pendant que son écran se re-rend : `mountBarSearch`
  // réutilise le champ existant et ne pousse la valeur que si elle diffère, sinon le curseur
  // sauterait en fin de champ à chaque re-rendu. Le voile d'attente se pose sur l'enveloppe plutôt
  // que par un nœud ajouté-retiré : un nœud créé à chaque frappe est exactement ce que la règle
  // « créer une fois, muter ensuite » interdit.
  const searchInput = mountBarSearch({
    placeholder: "Rechercher…",
    ariaLabel: "Rechercher dans la bibliothèque",
    value: bibState.filter.q ?? "",
    onInput: (value) => {
      bibState.filter.q = value || undefined;
      clearTimeout(bibSearchTimer);
      document.querySelector(".sift-bar-search")?.classList.add("sift-bar-search--pending");
      bibSearchTimer = window.setTimeout(() => void renderBiblioLive(), 250);
    },
  });
  if (searchInput) document.querySelector(".sift-bar-search")?.classList.remove("sift-bar-search--pending");

  // Virtualize the filed-track list: #content is the scroll container (app.js's block() set it to
  // overflow-y:auto), but the list is only ONE section of it (stats/rekordbox/header/facets sit
  // above) — createVirtualList handles that offset. #biblist exists iff bibState.tracks non-empty.
  const biblist = document.getElementById("biblist");
  if (biblist) {
    if (bibState.viewMode === "table") {
      bibVirtual = createVirtualList<LibraryTrack>({
        host: biblist,
        scrollContainer: content,
        items: sortedTracks,
        rowHtml: (t, i) => libraryTableRowHtml(t, bibOpenId, bibSelection.has(t.id), i % 2 === 1),
        probeHtml: LIBRARY_TABLE_PROBE_HTML,
        fallbackRowH: 32, // --row-h : repli seulement, la sonde mesure la vraie ligne
      });
    } else {
      const gridRows: LibraryTrack[][] = [];
      for (let i = 0; i < sortedTracks.length; i += LIBRARY_GRID_TILES_PER_ROW) {
        gridRows.push(sortedTracks.slice(i, i + LIBRARY_GRID_TILES_PER_ROW));
      }
      bibVirtual = createVirtualList<LibraryTrack[]>({
        host: biblist,
        scrollContainer: content,
        items: gridRows,
        rowHtml: (row) => libraryGridRowHtml(row),
        probeHtml: LIBRARY_GRID_PROBE_HTML,
        fallbackRowH: 150,
      });
    }
  }
}

/** Open the unified detail/edit panel for a filed track into #bibplayer, highlighting its row.
 * On save, patch the row label in place (player stays alive); on delete, re-render the list. */
export function openBiblioDetail(id: number): void {
  const t = bibState.tracks.find((x) => x.id === id);
  // Zone D du shell depuis l'étape 3, plus `#bibplayer` en fin de liste. Le détail y était rendu
  // APRÈS la table et après la section doublons : ouvrir une piste au rang 300 poussait son propre
  // détail hors de l'écran, et il fallait faire défiler pour voir ce qu'on venait d'ouvrir.
  const host = openAside();
  if (!host) return;
  if (!t) {
    closeAside();
    return;
  }
  if (bibOpenId === id) {
    bibOpenId = null;
    document.querySelectorAll(".lr.cur").forEach((n) => n.classList.remove("cur"));
    closeAside(); // vider sans refermer laisserait une colonne vide occuper --pane-w
    return;
  }
  // Track the open id so the `.cur` highlight is re-stamped by biblioRowHtml when a scrolled-away
  // row re-enters the virtualized window. Clear the class on currently-mounted rows immediately for
  // instant feedback (rows outside the window aren't in the DOM — bibOpenId covers them on mount).
  bibOpenId = id;
  document.querySelectorAll(".lr.cur").forEach((n) => n.classList.remove("cur"));
  document.querySelector(`.lr[data-id="${id}"]`)?.classList.add("cur");
  openLibraryDetailInto(
    host,
    t,
    (updated) => {
      // Keep the in-memory list + the visible row label in sync without a full re-render.
      const i = bibState.tracks.findIndex((x) => x.id === updated.id);
      if (i >= 0) bibState.tracks[i] = updated;
      const span = document.querySelector(`.lr[data-id="${updated.id}"] .bib-name`);
      if (span) span.textContent = bibName(updated);
    },
    () => void renderBiblioLive(),
    () => {
      bibOpenId = null;
      document.querySelectorAll(".lr.cur").forEach((n) => n.classList.remove("cur"));
      closeAside();
    },
  );
}
