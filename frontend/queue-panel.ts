// Revue queue panel — virtualization, keyboard nav, search, row rendering, and Détail/Lot mode
// state. Extracted from sift-live.ts (Phase 1, tranche 1b). currentItems/currentOpenId/reviewMode
// are owned here — all their reassignments already lived in this code before the move. The batch
// controller (tranche 1c) imports these as read values and calls setReviewMode() to mutate mode,
// never reassigns directly (ES module import bindings are read-only from outside this file).
//
// `reviewMode` reste ici, mais son COMMUTATEUR n'est plus dans cette colonne : le segmenté
// Détail / Lot a été retiré le 2026-08-25 (spec `docs/ui-specs/revue.md` §§ Zone A / Zone B′,
// décision du wireframe « Poste de décision »). Le mode Lot s'arme désormais par l'icône de
// sélection de la barre unifiée. Ne pas réintroduire d'onglet de mode dans #qcol.
import { listQueue, reanalyzeTracks, revealTrack } from "./ipc";
import { openFilingInto, syncDetail } from "./filing";
import { refreshBins, clearBinPick } from "./filing-bins";
import { homeProgressZone, taskOf } from "./progress-zone";
import { MAX_ANALYSIS_ATTEMPTS, type QueueItem } from "../shared/contracts";
import { confirmAction } from "./confirm-modal";
import { requireEl, esc } from "./dom";
import { closeAside } from "./toolbar";
import { toast } from "./filing-toast";
import { humanizeError } from "./errors";
import { filingFailure, isFilingInFlight, onFilingOutcome } from "./filing-state";
import { isBatchSheetOpen } from "./batch-sheet";
import { anchoredBelowPosition } from "./popover-position";
import { queueCountLabel } from "./queue-count-label";
import { verdictDot } from "./queue-verdict-dot";

/** A pending track still worth (re)analysing: no current verdict AND not yet terminally broken.
 *  Single source of truth for the "Non analysés" count, filter, and bulk-retry set — a track that
 *  has failed MAX_ANALYSIS_ATTEMPTS times keeps its per-row manual retry (see queueRowHtml) but no
 *  longer inflates the count, so it can actually reach zero on a library with an unrepairable file. */
function isRetryableUnanalyzed(it: QueueItem): boolean {
  return it.needs_analysis && it.analysis_attempts < MAX_ANALYSIS_ATTEMPTS;
}
function unanalyzedItems(): QueueItem[] {
  return currentItems.filter(isRetryableUnanalyzed);
}

// Track ids whose reanalyze IPC is in flight, and whether the bulk retry is running. Rendered FROM
// this state (queueRowHtml / the button labels) rather than by mutating DOM nodes directly: the
// queue rail is rebuilt via innerHTML on every queue:changed, so a spinner written onto a button
// node was landing on a detached element and the visible (fresh) row looked idle mid-retry.
const reanalyzingIds = new Set<number>();
let bulkReanalyzing = false;

/** Re-render just the visible queue window (used to reflect in-flight retry state changes). */
function rerenderQueueWindow(): void {
  const ql = document.getElementById("ql");
  if (ql) renderQueueWindow(ql);
}

// Latest live queue items, kept so a queue-row click can recover the full item (id +
// verdict) the filing pane needs.
export let currentItems: QueueItem[] = [];

/** Source dont la file est filtrée, ou `null` pour tout voir. Au niveau module : le filtre survit
 *  aux re-rendus de la file (progression d'analyse, événement `queue:changed`) et ne doit tomber
 *  que sur une action explicite. */
let queueSourceFilter: number | null = null;

/** Restreint la file à une source, ou lève le filtre avec `null`. Rend la valeur appliquée pour
 *  que l'appelant puisse marquer l'entrée de rail correspondante sans relire l'état d'ici. */
export function setQueueSourceFilter(id: number | null): number | null {
  queueSourceFilter = id;
  queueCacheStale = true; // le cache tient la file NON filtrée : le repeindre ignorerait le filtre
  return queueSourceFilter;
}

/** La source filtrée, pour que le rail sache quelle entrée marquer active. */
export function activeQueueSource(): number | null {
  return queueSourceFilter;
}

// True when `currentItems` is known to no longer reflect the backend, so the "repaint from cache"
// fast path below must NOT be taken. Set when a background conversion fails (P5): that track was
// filtered out of `currentItems` while in flight and has to come back, but nothing else will ever
// invalidate the cache for it — the conversion failed, so the backend emits no `queue:changed`.
// Without this, navigating away and back repaints the stale (filtered) cache and the track stays
// invisible for the rest of the session.
let queueCacheStale = false;

// Single source of truth for which queue row shows `.cur` — NOT read from filing.ts's internal
// state (would risk a race: filing.ts may set its own state before this module's DOM catches up).
// Updated in 3 places: the row click handler, renderQueue's touchDetail branch (via syncDetail's
// return value), and stepQueueSelection (Task 4).
let currentOpenId: number | null = null;

const QUEUE_ROW_BUFFER = 15; // rows rendered above/below the visible window

let queueRowHeightCache: number | null = null;

// Live filter on the queue rail (annotation: "on veut une barre de recherche en bas — filtre
// client sur la file affichée uniquement, titre/artiste, pas de recherche backend"). Filters
// currentItems only — never touches listQueue()/the DB. currentOpenId/stepQueueSelection walk
// this filtered view too, so arrow-key nav only steps through what's actually visible.
let queueSearchTerm = "";

// Le filtre « Non analysés uniquement » vivait ici — bascule de pied de colonne, retirée le
// 2026-08-26 (décision d'Antoine, maquette « Maquette — Revue »). Retrait de l'état AVEC son
// bouton : un drapeau que plus rien ne bascule est du code mort, et le laisser à `false` aurait
// gardé trois branches mortes dans `visibleQueueItems` et l'état vide.
//
// Ce que le retrait avait emporté — la seule façon d'isoler les pistes bloquées en attente
// d'analyse — est RENDU depuis le 2026-08-26 (issue #48) par la facette « Non analysés » du
// pulldown de filtre, et non par le retour d'une bascule séparée : le popover est déjà le lieu où
// l'on compose des critères, une facette de plus n'ajoute aucune surface. Voir `QUEUE_FACETS`.
// `unanalyzedItems()` reste, elle : le bouton « Réanalyser (N) » du pied s'en sert toujours, sur un
// périmètre plus étroit que la facette (voir le commentaire de celle-ci).

// Filtre par facettes (UNION multi-critères) — cases cochées dans le popover « Filtrer ». Set au
// niveau module : survit aux re-rendus (poll 300ms, queue:changed), ne tombe que sur une action
// explicite (case décochée, « Tout afficher »). Vide = tout montrer. Critères dérivés de QueueItem,
// zéro Rust (verdict/rail/dup, shared/contracts.ts). « MP3 » = approximation `rail==="lossy"` :
// QueueItem n'a pas de champ format/extension, un MP3-vs-AAC exact exigerait un champ de contrat.
const queueFacetFilter = new Set<string>();
const QUEUE_FACETS: readonly { id: string; label: string; match: (it: QueueItem) => boolean }[] = [
  { id: "lossless", label: "Lossless", match: (it) => it.rail === "lossless" },
  { id: "mp3", label: "MP3", match: (it) => it.rail === "lossy" },
  { id: "fake", label: "Faux", match: (it) => it.verdict === "fake" },
  { id: "dup", label: "Doublons", match: (it) => it.dup },
  // Ajoutée le 2026-08-26 (issue #48) : elle rend le chemin d'UI vers les pistes sans verdict, que
  // le retrait de la bascule « Non analysés uniquement » avait supprimé. Le critère est
  // `needs_analysis` et JAMAIS `verdict === null` — voir son commentaire dans `shared/contracts.ts`
  // ; le bug du 2026-07-20 (file entière cachée) venait de cette confusion, plus le fait que la
  // bascule était ON par défaut. Ici le Set part vide, donc aucun filtre n'est posé au montage.
  //
  // ⚠️ Périmètre VOLONTAIREMENT plus large qu'`unanalyzedItems()`, qui exclut les pistes ayant
  // épuisé `MAX_ANALYSIS_ATTEMPTS` pour que son compte puisse tomber à zéro. Cette facette-ci les
  // INCLUT : ce sont précisément celles qu'aucune relance ne fera partir, donc celles qu'isoler
  // sert le plus. Deux populations, deux mots — « Non analysés » ici, « Réanalyser (N) » au pied.
  { id: "unanalyzed", label: "Non analysés", match: (it) => it.needs_analysis },
];

/** UNION : un item passe s'il satisfait AU MOINS une facette active. Set vide = aucun filtre. */
function matchesFacet(it: QueueItem): boolean {
  if (queueFacetFilter.size === 0) return true;
  return QUEUE_FACETS.some((f) => queueFacetFilter.has(f.id) && f.match(it));
}

function visibleQueueItems(): QueueItem[] {
  // Plus de filtre d'analyse EN DUR ici depuis le 2026-08-26 : la bascule « Non analysés
  // uniquement » du pied de colonne est retirée (décision d'Antoine, maquette « Maquette — Revue »).
  // La liste part donc toujours de `currentItems`, et seules la recherche et les facettes la
  // restreignent — le critère d'analyse est devenu l'une de ces facettes (issue #48), donc il passe
  // par le même chemin que les autres au lieu d'avoir sa branche.
  const base = currentItems;
  const q = queueSearchTerm.toLowerCase();
  // Recherche (si présente) ET facettes (union) se composent : la recherche cherche dans tout
  // (ci-dessus), les facettes restreignent ; Set de facettes vide → matchesFacet rend true.
  return base.filter(
    (it) =>
      (!queueSearchTerm ||
        (it.filename ?? it.path).toLowerCase().includes(q) ||
        (it.artist ?? "").toLowerCase().includes(q) ||
        (it.title ?? "").toLowerCase().includes(q)) &&
      matchesFacet(it),
  );
}

/** Real rendered height of a queue row, measured once via an offscreen probe (never assumed —
 * same discipline as the spectrogram canvas width / destination-popover positioning elsewhere in
 * this codebase). Cached: the row markup/CSS don't change at runtime. */
function measureQueueRowHeight(ql: HTMLElement): number {
  if (queueRowHeightCache != null) return queueRowHeightCache;
  const probe = document.createElement("div");
  probe.className = "qi";
  probe.style.position = "absolute";
  probe.style.visibility = "hidden";
  probe.style.pointerEvents = "none";
  probe.style.width = "100%";
  probe.innerHTML =
    `<div style="flex:1;min-width:0;display:flex;flex-direction:column;gap:2px">` +
    `<div style="display:flex;align-items:center;gap:6px;min-width:0"><span style="flex:1">probe</span></div>` +
    // MÊME classe que la vraie seconde ligne de `queueRowHtml` — et non une copie de ses styles.
    // Une sonde qui n'hérite pas exactement des mêmes interlignes mesure une autre rangée que
    // celle qui sera peinte, et la virtualisation se décale sans que rien ne le dise (issue #45).
    `<div class="qi-sub">&nbsp;</div></div>`;
  ql.appendChild(probe);
  const h = probe.getBoundingClientRect().height;
  probe.remove();
  queueRowHeightCache = h > 0 ? h : 34; // 34px fallback: never divide by zero if measured off-DOM
  return queueRowHeightCache;
}

/** Compte de la file — dans la BARRE UNIFIÉE, contre le titre « Revue », depuis le 2026-08-26.
 *
 *  Il vivait à droite de la rangée de filtre et disait alors les pistes VISIBLES après filtre. Ce
 *  compte-là est retiré ; celui-ci est un autre objet : le compte de la FILE, que `revue.md`
 *  § Zone A demandait déjà (« Titre "Revue" + compte de la file, à gauche ») et que le montage de
 *  la barre avait omis.
 *
 *  Le motif vient de Mail (`docs/design-refs/03-mail.png`), qui pose « Inbox » puis « 34 messages »
 *  en tête de colonne — parce que c'est là qu'est le titre de la colonne. Dans Sift le titre de
 *  l'écran vit dans la barre : le couple titre + compte y va donc entier. Même couple, autre
 *  emplacement, parce que le titre a déménagé.
 *
 *  En mode Lot il devient le compte de la SÉLECTION, en teinte d'accent (spec § Zone A) : c'est le
 *  seul état que le mode Lot donne à voir en permanence, et il remplace le total plutôt que de s'y
 *  ajouter — deux nombres côte à côte dans la barre se liraient comme une fraction.
 *
 *  ⚠️ Il dit la file ENTIÈRE au repos, et ce qui est VISIBLE dès qu'un filtre ou une recherche est
 *  posé (issue #49, 2026-08-26) — pas « toujours le total ». Sans ça, cocher une facette sur 3 124
 *  pistes ne rendait aucun retour chiffré : le pulldown nommait la combinaison, rien ne disait ce
 *  qu'elle laissait voir. La règle exacte des trois états vit dans `queue-count-label.ts`, module
 *  sans DOM gelé par Vitest ; cette fonction ne fait plus qu'écrire.
 *
 *  Appelé depuis `renderQueueWindow`, donc sur le CHEMIN CHAUD de la colonne (poll de 300 ms +
 *  scroll rAF-throttlé) : d'où la comparaison avant écriture — une affectation `textContent`
 *  identique reste une écriture DOM. La comparaison interroge le NŒUD et non un cache au niveau
 *  module : la barre est vidée et remontée par la navigation (`clearBarSlots`), et un cache mémoire
 *  répondrait alors « déjà peint » sur une barre qui n'affiche plus rien.
 *
 *  `visible` est PASSÉ par l'appelant et jamais recalculé ici : `renderQueueWindow` vient de faire
 *  ce filtrage pour peindre ses lignes, et le refaire doublerait un parcours de la file sur le
 *  chemin le plus chaud du frontend. */
function paintBarCount(visible: number): void {
  const el = document.getElementById("sift-tb-count");
  if (!el) return;
  const lot = reviewMode === "batch";
  const label = queueCountLabel({
    mode: lot ? "batch" : "detail",
    selected: queueBatchSel.size,
    total: currentItems.length,
    visible,
    // Recherche ET facettes restreignent la même liste (`visibleQueueItems`) : l'une ou l'autre
    // suffit à faire de ce compte un retour de filtre plutôt qu'un total.
    filtered: queueFacetFilter.size > 0 || queueSearchTerm !== "",
  });
  if (el.textContent !== label) el.textContent = label;
  el.classList.toggle("sift-tb-count-lot", lot);
}

/** Ligne portant le CURSEUR CLAVIER de la file — celle d'où `stepQueueSelection` partirait, donc
 *  celle qui doit montrer l'anneau de focus (`.qi.qi-kbd`, peint sous `#ql:focus-visible` seulement).
 *
 *  Ce n'est PAS `.cur` : `.cur` dit « cette piste est ouverte en zone C » (aplat + encre + graisse),
 *  le curseur dit « le clavier est ici » (anneau). Les deux coïncident après une flèche, mais se
 *  séparent dès que la piste ouverte sort de la vue filtrée (recherche, facettes) ou que le mode Lot
 *  supprime tout `.cur` (`highlightId` forcé à `null` plus bas) — d'où deux rendus distincts.
 *
 *  Repli sur la PREMIÈRE ligne visible quand la piste ouverte n'est pas dans la vue : c'est déjà là
 *  qu'un ↓ atterrit (`curIndex` vaut -1, donc `nextIndex` 0), et surtout ça garantit qu'un `#ql`
 *  focalisé montre TOUJOURS un anneau — un conteneur focusable sans focus visible serait un trou
 *  WCAG 2.4.7 créé par le `tabindex` qu'on vient de lui donner.
 *
 *  Coût sur le chemin chaud : un `some()` court-circuité sur la liste DÉJÀ filtrée, sauté
 *  entièrement quand rien n'est ouvert. Un cran sous le balayage de comptes des facettes
 *  (`paintQueueFacetButton`, O(file × facettes)) que ce même rendu paie déjà. */
function keyboardCursorId(items: QueueItem[]): number {
  const open = currentOpenId;
  if (open != null && items.some((it) => it.id === open)) return open;
  return items[0].id;
}

/** Renders only the rows within the visible scroll window (+ QUEUE_ROW_BUFFER above/below) into
 * `ql`, framed by two spacer divs so the scrollbar stays proportional to the full list. Fixes the
 * 7000+-track freeze (memory: sift-large-queue-black-screen) — rebuilding thousands of DOM nodes
 * on every 300ms analysis-progress redraw (see the onAnalysisChanged listener further down) was
 * the actual cost, not just paint. */
function renderQueueWindow(ql: HTMLElement): void {
  const items = visibleQueueItems();
  // Le compte de la barre se peint ici parce que c'est le point qui repasse à CHAQUE changement de
  // la file — ajout, rangement, armement du mode Lot, coche d'une ligne, case de facette. `items`
  // lui est passé : c'est ce que la colonne montre, et c'est ce que le compte dit dès qu'un filtre
  // ou une recherche est posé (issue #49). Sans filtre il retombe sur la file entière.
  paintBarCount(items.length);
  if (!items.length) {
    // "File vide." reads as "nothing was ever here" — misleading when a track is still
    // shown in Detail (currentOpenId set) because it's the last one just treated and the
    // pane hasn't advanced away from it yet. Finding F4, audit-heuristique-visuel.md.
    const emptyLabel =
      currentItems.length && queueSearchTerm
        ? "Aucun morceau ne correspond."
        : currentOpenId != null
          ? "Tous les morceaux ont été traités."
          : "File vide.";
    ql.innerHTML =
      `<div style="font-size:var(--text-md);color:var(--color-text-tertiary);padding:6px 4px">${emptyLabel}</div>`;
    ql.removeAttribute("aria-activedescendant"); // plus aucune option : le curseur n'a plus de cible
    return;
  }
  const rowH = measureQueueRowHeight(ql);
  const viewportH = ql.clientHeight || 400;
  const start = Math.max(0, Math.floor(ql.scrollTop / rowH) - QUEUE_ROW_BUFFER);
  const visibleCount = Math.ceil(viewportH / rowH) + QUEUE_ROW_BUFFER * 2;
  const end = Math.min(items.length, start + visibleCount);
  // Batch mode never shows a "current" row in the queue rail — matches pre-virtualization
  // behavior exactly (a row click always drops back to detail mode first, so a highlighted row
  // while actually IN batch mode never happened before either).
  const highlightId = reviewMode === "batch" ? null : currentOpenId;
  const cursorId = keyboardCursorId(items);
  const topSpacer = start * rowH;
  const bottomSpacer = (items.length - end) * rowH;
  let html = topSpacer > 0 ? `<div style="height:${topSpacer}px"></div>` : "";
  // Le curseur clavier est REPEINT à chaque fenêtre, jamais posé sur un nœud : `#ql` étant le seul
  // élément focusable (les lignes ne le sont pas), aucun re-rendu ne peut faire tomber le focus —
  // seul l'anneau se redessine, sur la ligne que ce passage vient de marquer.
  let cursorRendered = false;
  for (let i = start; i < end; i++) {
    const it = items[i];
    const onCursor = it.id === cursorId;
    if (onCursor) cursorRendered = true;
    html += queueRowHtml(it, it.id === highlightId, onCursor);
  }
  if (bottomSpacer > 0) html += `<div style="height:${bottomSpacer}px"></div>`;
  ql.innerHTML = html;
  // `aria-activedescendant` doit nommer un élément RÉELLEMENT monté : la virtualisation ne monte que
  // la fenêtre visible, donc un curseur resté hors fenêtre (l'utilisateur a scrollé loin) laisserait
  // l'attribut pointer sur un id absent du document. On le retire alors plutôt que de mentir.
  if (cursorRendered) ql.setAttribute("aria-activedescendant", `qi-${cursorId}`);
  else ql.removeAttribute("aria-activedescendant");
  // Même fait, versant visible. Un curseur hors fenêtre n'a AUCUN nœud à cercler, et `#ql` peut
  // pourtant garder le focus : Tab dans la file puis Fin/PagePrec la fait défiler nativement sans
  // le lâcher. Sans ce repli, le focus clavier deviendrait invisible — le trou WCAG 2.4.7 que le
  // `tabindex` de `#ql` a créé. La classe reporte alors l'anneau sur la colonne elle-même
  // (`#ql:focus-visible.ql-cursor-off`), et la première flèche le rend à sa ligne :
  // `stepQueueSelection` ramène la cible dans la fenêtre avant de repeindre.
  // Gratuit sur le chemin chaud — le booléen est déjà calculé par la boucle ci-dessus.
  ql.classList.toggle("ql-cursor-off", !cursorRendered);
}

/** One-time (guarded) scroll listener on #ql, rAF-throttled: re-renders the visible window on
 * scroll without doing so on every fired scroll event (which can be dozens per second).
 *
 * Guarded on the NODE itself, not on a module boolean: app.js's renderRevue() rebuilds the whole
 * screen with `content.innerHTML = …` on every nav to Revue, so #ql is a BRAND NEW element each
 * time. A module flag stayed true across that rebuild, leaving the listener attached to the
 * detached old node — after one nav round-trip the visible list never re-rendered on scroll
 * (virtualized rows past the first window stayed blank). A fresh node carries no marker, so it
 * gets wired; the same node re-passed (every renderQueue call) is skipped as before. */
function ensureQueueScroll(ql: HTMLElement): void {
  if (ql.dataset.siftScrollWired) return;
  ql.dataset.siftScrollWired = "1";
  let ticking = false;
  ql.addEventListener("scroll", () => {
    if (ticking) return;
    ticking = true;
    requestAnimationFrame(() => {
      ticking = false;
      renderQueueWindow(ql);
    });
  });
}

let queueStepTimer: ReturnType<typeof setTimeout> | undefined;
let prefetchTimer: ReturnType<typeof setTimeout> | undefined;

/** Warm the track FOLLOWING the one just opened (analysis report + AIFF pre-transcode via
 *  report-view's prefetchTrack) so the next switch paints instantly. Fires once per
 *  user-initiated open (click / arrow step / batchopen), never from a burst event; the 400ms
 *  delay + debounce keeps it out of the open's own critical path when flicking through rows. */
export function prefetchNextAfter(id: number): void {
  clearTimeout(prefetchTimer);
  prefetchTimer = setTimeout(() => {
    const idx = currentItems.findIndex((t) => t.id === id);
    const next = idx >= 0 ? currentItems[idx + 1] : undefined;
    if (next) void import("./report-view").then((m) => m.prefetchTrack(next.path));
  }, 400);
}

/** ArrowUp/ArrowDown queue navigation. Kept separate from filing.ts's installFilingKeys (Space/
 * Enter/Backspace/I) because stepping through a virtualized queue needs currentItems + the
 * ability to scroll a not-yet-rendered row into view — both owned here, not in filing.ts (which
 * would need a circular import to reach them; sift-live.ts already imports from filing.ts, not
 * the reverse). */
function stepQueueSelection(delta: 1 | -1): void {
  const items = visibleQueueItems();
  if (!items.length) return;
  const curIndex = currentOpenId != null ? items.findIndex((it) => it.id === currentOpenId) : -1;
  const nextIndex = curIndex + delta;
  if (nextIndex < 0 || nextIndex >= items.length) return;
  const next = items[nextIndex];
  currentOpenId = next.id;
  const ql = document.getElementById("ql");
  if (ql) {
    const rowH = measureQueueRowHeight(ql);
    // Keep the target row inside the rendered window: scroll just enough that nextIndex sits
    // within view, never a full jump-to-top/bottom for a one-row step.
    const rowTop = nextIndex * rowH;
    const rowBottom = rowTop + rowH;
    if (rowTop < ql.scrollTop) ql.scrollTop = rowTop;
    else if (rowBottom > ql.scrollTop + ql.clientHeight) ql.scrollTop = rowBottom - ql.clientHeight;
    renderQueueWindow(ql);
  }
  // Debounced like the row-click handler in installLiveWiring — arrow-key repeat shouldn't fire a
  // full decode load per row flicked through.
  //
  // setReviewMode("detail") and openFilingInto(mid, next) MUST fire together, in that order, from
  // this single deferred callback rather than one synchronously here and the other 150ms later.
  // setReviewMode("detail") synchronously calls renderQueue(true), which awaits listQueue() before
  // calling syncDetail on a later tick; syncDetail's guard is `state.track && paneIsOurs` (both set
  // synchronously at the top of openFilingInto, before its own first await). If setReviewMode ran
  // immediately while openFilingInto(mid, next) was still 150ms away, that later syncDetail tick
  // would see state.track/paneIsOurs stale (still pointing at whatever was open before, or null),
  // fall through to "load the first pending track", and clobber currentOpenId back to items[0] —
  // disagreeing with the `next` this function just highlighted. Firing both from the same callback
  // means by the time renderQueue's later tick runs, openFilingInto has already set state.track to
  // `next` synchronously, so syncDetail's guard sees the correct track and returns its id.
  clearTimeout(queueStepTimer);
  queueStepTimer = setTimeout(() => {
    if (reviewMode === "batch") enterDetailMode();
    const mid = document.getElementById("mid");
    if (mid) {
      void openFilingInto(mid, next);
      prefetchNextAfter(next.id);
    }
  }, 150);
}

/** Guarded so installLiveWiring can call this once even if it ever re-runs. */
let queueNavKeysWired = false;
export function installQueueNavKeys(): void {
  if (queueNavKeysWired) return;
  queueNavKeysWired = true;
  const blurShortcutFocus = () => {
    const active = document.activeElement;
    if (!(active instanceof HTMLElement) || active === document.body) return;
    // `#ql` EXEMPTÉ. Ce blur existe pour décrocher le focus d'un BOUTON avant qu'un raccourci agisse
    // (spec § Clavier : sinon Espace active le bouton focalisé EN PLUS de la lecture). La file, elle,
    // est la cible même de ↑/↓ : la flouter ici éteindrait l'anneau du curseur au premier appui,
    // c'est-à-dire exactement au moment où le clavier prend la main. Un `#ql` focalisé n'a d'ailleurs
    // aucune action par défaut à voler — son défilement natif est déjà coupé par le `preventDefault`
    // ci-dessous.
    if (active.id === "ql") return;
    active.blur();
  };
  document.addEventListener("keydown", (e) => {
    const t = e.target as HTMLElement | null;
    if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA")) return;
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    // Liste de candidats Discogs ouverte (fork F) : là, ↑/↓ navigue la liste, pas la file. Gate sans
    // importer filing-state (règle d'import unidirectionnelle) ; la listbox gère ses propres flèches.
    if (t && t.closest(".sift-cands")) return;
    // Same gate as filing.ts's installFilingKeys (`if (!state.track) return`), but reached without
    // importing filing.ts's internal `state` (would violate the one-directional import rule —
    // filing.ts must never import from sift-live.ts, and sift-live.ts already imports FROM
    // filing.ts, so a reverse re-export isn't an option either). currentOpenId alone is NOT
    // equivalent: it's set once a track opens in Revue and is never cleared on nav-away (no
    // `currentOpenId = null` reset exists outside syncDetail returning null for an empty queue) —
    // so checking it alone let ↑/↓ still fire from Bibliothèque/Réglages after visiting Revue once.
    // Mirror filing.ts's syncDetail `paneIsOurs` check instead: only act if #mid currently shows
    // the real filing pane (not app.js's mock redrawn over it on nav-away).
    const mid = document.getElementById("mid");
    if (currentOpenId == null || !mid || !mid.querySelector(".sift-fil")) return;
    e.preventDefault();
    blurShortcutFocus();
    stepQueueSelection(e.key === "ArrowDown" ? 1 : -1);
  });

  // Clic sur case de lot — intercepte avant le handler .qi générique (case dans une .qi).
  // Sur `document` : #qcol est reconstruit à chaque navigation, un listener posé dessus
  // disparaîtrait. Même motif que le keydown ci-dessus.
  document.addEventListener("click", (e) => {
    if ((e.target as HTMLElement).dataset.sift !== "queuepick") return;
    e.stopPropagation();
    const id = Number((e.target as HTMLElement).dataset.id);
    if (queueBatchSel.has(id)) queueBatchSel.delete(id);
    else queueBatchSel.add(id);
    const ql = document.getElementById("ql");
    if (ql) renderQueueWindow(ql);
  });

  // Clic droit sur une ligne de file — menu contextuel, dans LES DEUX modes depuis le 2026-08-26.
  // Il ne servait que le mode Lot (`if (reviewMode !== "batch") return`), alors que `revue.md`
  // § Interactions le veut sur la ligne : « Ouvrir l'emplacement · Réanalyser · Écarter · Changer
  // la destination. En mode Batch, agit sur la sélection. »
  //
  // Sur `document` (pas sur #qcol) : #qcol est reconstruit à chaque navigation, un listener
  // posé dessus disparaîtrait ; `document` est la racine stable de tous les gestionnaires
  // délégataires de cette colonne (même motif que le keydown ci-dessus).
  document.addEventListener("contextmenu", (e) => {
    const qi = (e.target as HTMLElement).closest<HTMLElement>(".qi");
    if (!qi) return;
    e.preventDefault();
    const id = Number(qi.dataset.id);

    // « Changer la destination » OUVRE le vrai bouton de la zone C au lieu de refaire son travail.
    // Même geste que le raccourci ⌫ de `filing.ts`, qui clique `[data-fil="trash"]` plutôt que de
    // rappeler `doSecondary` : le popover de destination porte de l'état (bac courant, sur-place,
    // arbre chargé), et un second chemin d'ouverture serait un second endroit où le désaccorder.
    // Le sélecteur vaut dans les deux modes — `filing.ts` et `batch-panel.ts` rendent tous deux
    // leur bouton avec `data-fil="destbtn"`.
    const changerDestination = {
      label: "Changer la destination",
      onPick: () => document.querySelector<HTMLElement>('[data-fil="destbtn"]')?.click(),
    };

    if (reviewMode === "batch") {
      // Assure que la piste cliquée est dans la sélection
      if (!queueBatchSel.has(id)) {
        queueBatchSel.add(id);
      }
      const n = queueBatchSel.size;
      void import("./context-menu").then(({ openContextMenu }) => {
        void import("./batch-panel").then(({ handleBatchQueueAction }) => {
          openContextMenu(e.clientX, e.clientY, [
            { label: `Ranger ${n} piste${n > 1 ? "s" : ""}`, onPick: () => handleBatchQueueAction("file") },
            changerDestination,
            {
              label: `Écarter ${n} piste${n > 1 ? "s" : ""}`,
              danger: true,
              separated: true,
              onPick: () => handleBatchQueueAction("discard"),
            },
          ]);
        });
      });
      const ql = document.getElementById("ql");
      if (ql) renderQueueWindow(ql);
      return;
    }

    // Mode Détail. La ligne devient d'abord la piste OUVERTE — c'est ce que fait tout clic droit
    // dans Finder ou Mail, et c'est ce qui rend les deux dernières entrées adressables : « Changer
    // la destination » et « Écarter » agissent sur la zone C, donc sur la piste ouverte, pas sur un
    // identifiant. Le menu s'affiche par-dessus pendant que l'ouverture (débouncée à 150 ms) se
    // fait ; le temps de lire quatre entrées et d'en choisir une couvre largement ce délai, et
    // `openTrackGuard` refuse quand même d'agir si la piste ouverte n'est pas celle du menu.
    handleQueueItemClick(qi, e);
    const surLaPisteDuMenu = () => currentOpenId === id;
    void import("./context-menu").then(({ openContextMenu }) => {
      openContextMenu(e.clientX, e.clientY, [
        {
          label: "Ouvrir l'emplacement",
          onPick: () => {
            void revealTrack(id).catch((err) => {
              toast(humanizeError(err, "Impossible d'ouvrir l'emplacement", "reveal_track"));
            });
          },
        },
        { label: "Réanalyser", onPick: () => void reanalyzeTrack(id) },
        changerDestination,
        {
          label: "Écarter",
          danger: true,
          separated: true,
          onPick: () => {
            // Garde plutôt que fallback silencieux : si l'ouverture n'a pas abouti, écarter
            // cliquerait le bouton d'une AUTRE piste — exactement le genre d'action qu'on ne
            // rattrape pas. On le dit au lieu de le faire au hasard.
            if (!surLaPisteDuMenu()) {
              toast("Piste pas encore ouverte — réessaie");
              return;
            }
            document.querySelector<HTMLElement>('[data-fil="trash"]')?.click();
          },
        },
      ]);
    });
  });
}

/** Relance l'analyse d'UNE piste bloquée. Vivait en closure dans `installLiveWiring`
 *  (sift-live.ts) jusqu'au 2026-08-26 ; remontée ici avec le menu contextuel, qui en avait besoin —
 *  et c'est de toute façon sa place : ses deux moitiés d'état, `beginReanalyze` / `endReanalyze`,
 *  sont déclarées dans ce module et n'en sortaient que pour elle.
 *
 *  Le spinner en vol passe par cet état partagé, JAMAIS par une mutation du nœud du bouton : le
 *  rail de file est reconstruit en `innerHTML` au `queue:changed` du backend, donc un spinner écrit
 *  sur le nœud resterait échoué sur un élément détaché pendant que la ligne fraîche aurait l'air au
 *  repos (attrapé en revue). Rendre depuis l'état survit à la reconstruction. */
export async function reanalyzeTrack(id: number): Promise<void> {
  beginReanalyze([id]);
  try {
    await reanalyzeTracks([id]);
    toast("Réanalyse relancée");
  } catch (e) {
    toast(humanizeError(e, "Échec de la réanalyse — réessaie", "reanalyze_tracks"));
  } finally {
    endReanalyze([id]);
  }
}

// Review mode: "detail" = one track at a time (filing pane), "batch" = triage many at once
// (board's Detail|Batch segmented control). `batchSel` holds the ticked track ids; it is
// pruned to the currently-ready set on every batch render so a filed/removed id can't linger.
export let reviewMode: "detail" | "batch" = "detail";

/** Tracks sélectionnés dans la colonne de file pour le mode Lot.
 *  Distinct de batchSel (batch-panel.ts) — source de vérité pour la sélection de file.
 *  Exporté pour être lu par batch-panel et selection-summary. */
export const queueBatchSel = new Set<number>();

/** Pré-sélectionne tous les items avec verdict != null au passage en mode Lot.
 *  Appelé par `enterBatchMode` (ce module) — plus exporté depuis le 2026-08-26, même raison que
 *  `setReviewModeRaw` : armer le mode est une séquence, pas une suite d'appels à composer dehors. */
function initQueueBatchSel(items: QueueItem[]): void {
  queueBatchSel.clear();
  for (const it of items) {
    if (it.verdict !== null) queueBatchSel.add(it.id);
  }
}

/** Pistes qu'une sélection de lot peut contenir : celles qui ont un verdict — le même prédicat
 *  qu'`initQueueBatchSel`, une piste sans verdict ne se range pas. */
function batchEligible(): QueueItem[] {
  return currentItems.filter((it) => it.verdict !== null);
}

/** Remplace la sélection de lot par `ids`, puis repeint ce qui la montre : les cases de la file,
 *  le compte de la barre (`renderQueueWindow`) et le résumé de la zone C (`renderBatch`, par import
 *  dynamique : `batch-panel` importe ce module, jamais l'inverse en statique). */
function setQueueBatchSelection(ids: Iterable<number>): void {
  queueBatchSel.clear();
  for (const id of ids) queueBatchSel.add(id);
  const ql = document.getElementById("ql");
  if (ql) renderQueueWindow(ql);
  if (reviewMode === "batch") void import("./batch-panel").then((m) => m.renderBatch());
}

/** Tout sélectionner (mode Lot) — Finder : Édition › Tout sélectionner, `Ctrl+A`. Exportée pour
 *  `shortcuts.ts`. Sans effet hors mode Lot. */
export function selectAllQueueBatch(): void {
  if (reviewMode !== "batch") return;
  setQueueBatchSelection(batchEligible().map((it) => it.id));
}

/** Tout désélectionner (mode Lot) — Photos / Mail : Édition › Tout désélectionner, `Ctrl+Maj+A`.
 *  Exportée pour `shortcuts.ts`. Sans effet hors mode Lot. */
export function clearQueueBatchSelection(): void {
  if (reviewMode !== "batch") return;
  setQueueBatchSelection([]);
}

/** Le menu du pulldown « Sélection » (issue #60, 2026-09-10) : « Tout », « Aucune », puis
 *  « Seulement <catégorie> » par facette de la file — la sélection devient EXACTEMENT cette
 *  catégorie, filtre ou pas. Sift n'a pas de barre de menus (#58) : ce que macOS met dans Édition
 *  vit ici, dans un pulldown du kit (§ 02-08), à côté de « Terminé ». « Non analysés » n'y est pas :
 *  une piste sans verdict n'entre pas dans un lot. */
function openQueueSelectionMenu(anchor: HTMLElement): void {
  const eligible = batchEligible();
  const r = anchor.getBoundingClientRect();
  void import("./context-menu").then(({ openContextMenu }) => {
    const perFacet = QUEUE_FACETS.filter((f) => f.id !== "unanalyzed").map((f) => {
      const ids = eligible.filter(f.match).map((it) => it.id);
      return {
        label: `Seulement ${f.label} (${ids.length.toLocaleString("fr-FR")})`,
        onPick: ids.length ? () => setQueueBatchSelection(ids) : undefined,
      };
    });
    // « Sans Faux » (demande d'Antoine, 2026-09-10) : la sélection COURANTE moins ses pistes à
    // verdict `fake` — le geste « tout ranger sauf les faux » en un clic, là où « Seulement »
    // remplace. Le compte est celui des faux actuellement cochés ; zéro = entrée désactivée.
    const selectedFake = eligible.filter((it) => queueBatchSel.has(it.id) && it.verdict === "fake");
    const sansFaux = {
      label: `Sans Faux (${selectedFake.length.toLocaleString("fr-FR")})`,
      onPick: selectedFake.length
        ? () => setQueueBatchSelection([...queueBatchSel].filter((id) => !selectedFake.some((it) => it.id === id)))
        : undefined,
      separated: true,
    };
    openContextMenu(r.left, r.bottom + 4, [
      { label: `Tout (${eligible.length.toLocaleString("fr-FR")})`, onPick: selectAllQueueBatch },
      { label: "Aucune", onPick: queueBatchSel.size ? clearQueueBatchSelection : undefined },
      ...perFacet.map((m, i) => (i === 0 ? { ...m, separated: true } : m)),
      sansFaux,
    ]);
  });
}

// Verdict = sens seul, et la teinte vient de la table verdict de `DESIGN.md` § 16 — la même que la
// colonne Verdict de la Bibliothèque (`library-views.ts`), pour que le même fait n'ait pas deux
// couleurs selon l'écran où on le lit.
//
// `verdictDot` (et sa table `VERDICT_DOT`) vit dans `queue-verdict-dot.ts` depuis le 2026-08-27 —
// module PUR extrait pour que la story et un test env Node exécutent le vrai rendu sans charger
// `./ipc` (même motif que `popover-position.ts`). `queueRowHtml` ci-dessous reste son seul
// appelant de prod.

// `verdictWord` vivait ici — retiré le 2026-08-26 avec le mot de verdict de la ligne. Ses deux
// derniers rendus qui n'étaient PAS des verdicts (« échec », « analyse… ») sont passés dans
// `verdictDot`, qui distingue désormais l'abandon de l'attente. Retrait, pas mise en commentaire :
// même règle que `ipc_filing::create_bin` (`9451577`), un code sans appelant se supprime.

/** One queue row's markup. `active` stamps the `.cur` highlight at creation time — required so
 * the highlight survives virtualization (Task 2): once #ql only mounts the visible window, a
 * row for the open track may not exist in the DOM to be found and classed after the fact.
 *
 * `onCursor` marque de même le CURSEUR CLAVIER (`.qi-kbd`, cf. `keyboardCursorId`) : même contrainte,
 * même remède — une classe posée à la construction, jamais cherchée après coup sur un nœud qui peut
 * ne pas exister. La ligne reste NON focusable (`role="option"` + `aria-activedescendant` sur `#ql`,
 * patron listbox de l'APG) : donner un `tabindex` aux lignes ferait perdre le focus au premier
 * repeint qui démonte la ligne focalisée — soit toutes les 300 ms pendant une analyse.
 * ⚠️ Le bouton Réanalyser plus bas est un descendant interactif d'un `role="option"`, ce que l'APG
 * interdit ; conflit connu, signalé au rapport de Q-5, non résolu ici (le sortir de la ligne est un
 * changement de markup qui ne tient pas dans cette tâche). */
function queueRowHtml(it: QueueItem, active: boolean, onCursor: boolean): string {
  // A conversion that failed in the background (P5/D5) outranks the analysis verdict on the row:
  // it is the one thing about this track the user must see, and it has to survive navigation — it
  // is re-read from filing-state on every paint, so leaving Revue and coming back keeps it.
  const failure = filingFailure(it.id);
  const title = esc(it.filename || it.path);
  const artist = it.artist ? esc(it.artist) : "";
  return (
    `<div class="qi${active ? " cur" : ""}${onCursor ? " qi-kbd" : ""}" id="qi-${it.id}" role="option" aria-selected="${active}" data-id="${it.id}" data-path="${esc(it.path)}" title="Écouter et convertir" style="cursor:pointer">` +
    (reviewMode === "batch"
      ? `<input type="checkbox" class="qi-ck" data-sift="queuepick" data-id="${it.id}" tabindex="-1"${queueBatchSel.has(it.id) ? " checked" : ""}>`
      : "") +
    `<div style="flex:1;min-width:0;display:flex;flex-direction:column;gap:2px">` +
    // Pastille COLLÉE À LA FIN DU TITRE, pas en tête de ligne (décision produit d'Antoine,
    // 2026-08-27, assumée CONTRE le motif Mail des indicateurs au bord droit — maquette Figma,
    // composant Ligne de file). Le titre ne pousse plus (`flex:0 1 auto`) : il s'ellipse si long
    // et la pastille reste visible, accolée à son dernier caractère.
    `<div style="display:flex;align-items:center;gap:6px;min-width:0">` +
    `<span style="flex:0 1 auto;min-width:0;overflow:hidden;text-overflow:ellipsis;font-weight:500">${title}</span>` +
    verdictDot(it) +
    `</div>` +
    // Always render the second line (never conditionally omit it) — otherwise a
    // not-yet-identified track (no artist) renders one line shorter than an identified
    // one, making queue rows visibly uneven heights next to each other.
    `<div class="qi-sub">${artist || "&nbsp;"}</div>` +
    `</div>` +
    // L'échec de CONVERSION garde son libellé, et il est le seul : ce n'est pas un verdict, la
    // pastille ne le porte pas, et c'est un fait de fond que l'utilisateur doit voir sans survoler.
    // Le mot de VERDICT, lui, est parti le 2026-08-26 — il doublait la pastille (voir `verdictDot`).
    (failure
      ? `<span title="${esc(failure)}" style="flex:none;display:inline-flex;align-items:center;gap:4px;font-size:var(--text-xs);color:var(--color-text-warning)"><i class="ti ti-alert-triangle"></i>conversion échouée</span>`
      : "") +
    // Pastille DUPLICATE, au BORD DROIT de la ligne (wireframe « Poste de décision » §§ 09-10 ;
    // spec `docs/ui-specs/revue.md` § Zone B′ : « rendu hors colonne verdict »). Elle a quitté la
    // première ligne — où elle était collée au nom de fichier sous forme de glyphe nu ⧉ teinté
    // warning — pour le niveau `.qi`, seul endroit qui a un bord droit ; c'est la place que la durée
    // occupait avant son retrait du 2026-08-21, retirée précisément parce qu'elle « mangeait la
    // place du signal doublon ». Elle reste NEUTRE (gris de sélection, encre secondaire) et non
    // warning : un doublon n'est pas un verdict — il ne sort pas de `tracks.verdict` mais du scan de
    // dédoublonnage, et `design-system-states.md` (« il n'y a pas de sixième rendu ») en fait le
    // point explicite. Lui donner l'ambre de « à vérifier » ferait deux faits d'une seule couleur.
    // Style en RÈGLE CSS (`.qi-dup`) et non en attributs inline : cette ligne est concaténée dans la
    // boucle de `renderQueueWindow`, donc chaque attribut se paie en octets de chaîne à CHAQUE
    // fenêtre rendue — poll de 300 ms + scroll rAF-throttlé.
    (it.dup ? '<span class="qi-dup" title="Doublon possible (même nom)">DUPLICATE</span>' : "") +
    // Only for a not-yet-analysed row — retry a track stuck without a verdict (e.g. a
    // transient decode error on first pass) instead of leaving it silently unreachable.
    // data-reanalyze is checked BEFORE the .qi row-open branch in the delegated click handler
    // (sift-live.ts) so clicking it never also opens the track. Shown for every needs_analysis row
    // (incl. terminally-broken ones dropped from the count) so a manual retry stays reachable.
    // The in-flight spinner is rendered FROM reanalyzingIds, never by mutating the button node —
    // the rail is rebuilt on every queue:changed, which would otherwise strand the spinner on a
    // detached element.
    (it.needs_analysis
      ? reanalyzingIds.has(it.id)
        ? `<button data-reanalyze="${it.id}" class="lk-icon" title="Réanalyse en cours" disabled aria-label="Réanalyse en cours"><i class="ti ti-loader-2 sift-spin"></i></button>`
        : `<button data-reanalyze="${it.id}" class="lk-icon" title="Réanalyser ce morceau" aria-label="Réanalyser ce morceau"><i class="ti ti-refresh"></i></button>`
      : "") +
    `</div>`
  );
}

/** Replaces the mockup queue list with real pending items (Revue screen). */
export async function renderQueue(touchDetail = true) {
  const ql = document.getElementById("ql");
  if (!ql) return;

  // Returning to Revue after visiting another screen: app.js's renderRevue rebuilds
  // #qcol/#ql/#mid from scratch on every nav click (content.innerHTML, unconditional), even
  // though currentItems already holds a freshly-loaded queue in memory — only the DOM was wiped,
  // not the state. Detect that exact case (freshly (re)created empty #ql + state already loaded)
  // and paint synchronously from the cached currentItems instead of flashing "Chargement…" and
  // re-issuing a full listQueue() IPC round-trip on every tab switch. Real data changes keep
  // arriving through the backend's "queue:changed" event (onQueueChanged → refresh(),
  // sift-live.ts) independently of navigation — a genuine change while #ql already has rows still
  // falls through to the full reload below, same as before.
  if (!ql.childElementCount && currentItems.length && !queueCacheStale) {
    const qcol = document.getElementById("qcol");
    if (qcol) ensureQueueColumnChrome(qcol);
    // Live destination bins. Cet appel vit DANS le bloc de cache et non en tête de fonction :
    // ce chemin `return` plus bas, donc il n'atteint jamais l'appel gardé par `touchDetail`, et
    // les bacs resteraient ceux d'avant la navigation. En tête de fonction en revanche, il
    // s'exécuterait AUSSI sur le redraw 300ms de la progression d'analyse — or `list_bins` marche
    // toute la racine bibliothèque récursivement côté backend, ce que le garde de l'autre appel
    // existe précisément pour éviter. Ici le poll ne passe pas : ce bloc exige `#ql` vide, et un
    // poll d'analyse repeint une liste qui a déjà ses lignes.
    const fldz = requireEl("#fldz", "renderQueue");
    void refreshBins(fldz);
    if (touchDetail) {
      if (reviewMode === "batch") {
        batchRenderer?.();
      } else {
        const mid = requireEl("#mid", "renderQueue");
        if (mid) {
          currentOpenId = syncDetail(mid, currentItems);
        }
      }
    }
    renderQueueWindow(ql);
    ensureQueueScroll(ql);
    // Le badge de rail que ce point tenait d'accord avec la file est retiré (2026-08-26, voir
    // l'emplacement d'`updateRevueBadge` plus bas). Le compte vit maintenant dans la barre, et
    // `renderQueueWindow` ci-dessus le repeint par `paintBarCount` — même repaint, même instant,
    // donc l'accord qui demandait un appel séparé est devenu structurel.
    return;
  }

  // First paint has nothing to show yet (the mockup skeleton leaves #ql empty) — on a large
  // library listQueue() can take a couple of seconds, otherwise that's a blank screen the whole
  // time (audit UI/UX 2026-07-03, fix 4). Gated on "no rows yet" so later polls (queue:changed,
  // the 300ms debounce) never flash this over the still-valid existing rows.
  if (!ql.childElementCount) {
    // `data-sift="qloading"` n'est pas décoratif : c'est ce qui permet au `catch` de `listQueue`
    // plus bas de savoir s'il regarde CE placeholder (à remplacer par une erreur) ou de vraies
    // lignes déjà peintes (à ne pas détruire). Sans marque, il faudrait deviner.
    ql.innerHTML =
      '<div data-sift="qloading" style="display:flex;align-items:center;gap:8px;padding:8px 7px;color:var(--color-text-tertiary);font-size:var(--text-md)">' +
      '<i class="ti ti-loader sift-spin" style="font-size:var(--text-md)"></i> Chargement…</div>';
  }
  let items: QueueItem[] = [];
  try {
    items = await listQueue();
  } catch (e) {
    // Impasse A7 (issue #15) : ce `catch` faisait `console.error` + `return` sec, donc le
    // « Chargement… » peint juste au-dessus tournait pour toujours. Un spinner permanent est un
    // échec silencieux — le rail de sources (`rail-sources.ts`) le dit et le
    // corrige déjà, la correction n'avait pas été portée ici.
    const display = humanizeError(
      e,
      "Impossible de charger la file. Vérifie la connexion à la base et réessaie.",
      "listQueue",
    );
    // Deux cas, pas un. Rail encore vide (premier chargement) : c'est LE spinner qu'il faut
    // remplacer, par une carte d'erreur avec sa porte de sortie. Rail déjà peuplé (un poll de
    // `queue:changed` qui échoue) : les lignes affichées restent valides, les écraser perdrait
    // de l'information juste — un toast dit l'échec sans détruire l'état.
    if (ql.querySelector('[data-sift="qloading"]')) {
      ql.innerHTML =
        '<div class="sift-ui-card-soft sift-ui-card-soft-pad" style="color:var(--color-text-danger)">' +
        esc(display) +
        '<div style="margin-top:var(--space-8)"><button data-sift="retryqueue" style="font-size:var(--text-xs);color:var(--color-text-info)">Réessayer</button></div>' +
        "</div>";
      ql.querySelector<HTMLButtonElement>('[data-sift="retryqueue"]')?.addEventListener(
        "click",
        () => {
          ql.innerHTML = "";
          void renderQueue(touchDetail);
        },
      );
    } else {
      toast(display);
    }
    return;
  }
  // P5 (PRD 2026-07-27, D3): a track whose conversion is still running in the background has left
  // the user's loop, but it is still `pending` backend-side (it only becomes `filed` once the encode
  // commits), so list_queue keeps returning it. Dropping it HERE — the single point where the front
  // takes delivery of the queue — is what stops the auto-advance from re-opening it and the user
  // from converting it a second time, in the rail as well as in Lot mode (both read currentItems).
  // It comes back on its own if the conversion fails: see the onFilingOutcome subscription below.
  items = items.filter((it) => !isFilingInFlight(it.id));
  // Filtre de SOURCE (fusion 1, DESIGN.md § 15) : cliquer un dossier surveillé dans le rail
  // restreint la file à ses fichiers, comme cliquer une source dans la sidebar de Finder restreint
  // sa liste. Appliqué ICI, au point unique où le front prend livraison de la file — donc le mode
  // Lot, le badge et l'auto-avance voient tous la même liste, sans qu'aucun ait à connaître le
  // filtre. `source_id` existe déjà dans le contrat : aucun aller-retour backend n'est ajouté.
  if (queueSourceFilter != null) items = items.filter((it) => it.source_id === queueSourceFilter);
  currentItems = items;
  // This IS the fresh delivery the stale flag was waiting for, and the single point where the
  // front takes the queue in — so it is also where the badge is brought back in step with the rail.
  queueCacheStale = false;
  const qcol = document.getElementById("qcol");
  if (qcol) ensureQueueColumnChrome(qcol);
  // Background-analysis progress moved to the global progress zone (bottom of #nav, persistent
  // across views) — see pushAnalyzeProgress, fed by the analysis:changed event below.

  // Only sync the detail pane on structural changes (nav, queue add/remove/file). A background
  // ANALYSIS finishing must NOT re-open / switch the open track — that thrashes and aborts the
  // player's audio load (waveform shows from peaks, but no sound). See touchDetail=false caller.
  if (touchDetail) {
    // Live destination bins + neutral detail prompt (replace the mockup's hardcoded ones). Gated
    // on touchDetail like the pane sync: the bins are the library root + its folders on disk
    // (loadBins → getSetting + list_bins), which a background analysis finishing cannot change.
    // list_bins walks the WHOLE library root recursively backend-side, so leaving this ungated
    // ran a full recursive directory scan on every 300ms analysis-progress redraw.
    const fldz = requireEl("#fldz", "renderQueue");
    void refreshBins(fldz);
    if (reviewMode === "batch") {
      batchRenderer?.();
    } else {
      const mid = requireEl("#mid", "renderQueue");
      if (mid) {
        currentOpenId = syncDetail(mid, items);
      }
    }
  }
  renderQueueWindow(ql);
  ensureQueueScroll(ql);
}

// ---------------------------------------------------------------------------
// Ordre de la colonne file — DÉCLARÉ ici, plus déduit de l'ordre d'appel
// ---------------------------------------------------------------------------

/** Ordre vertical de #qcol, de haut en bas (spec `docs/ui-specs/revue.md` § Zone B′) :
 *  recherche → rangée de filtre → liste virtualisée → bascules de pied.
 *
 *  Jusqu'au 2026-08-25 cet ordre était un EFFET DE BORD de l'ordre d'appel dans `renderQueue` :
 *  la recherche s'insérait en `firstChild`, les deux autres blocs s'ajoutaient à la fin, et rien
 *  n'écrivait nulle part que la colonne devait se lire ainsi — permuter deux appels la
 *  réordonnait en silence. Les blocs se posent désormais PAR RAPPORT À CETTE LISTE
 *  (`placeInQueueColumn`), donc l'ordre d'appel de `ensureQueueColumnChrome` n'a plus d'effet.
 *
 *  Tient à la RECONSTRUCTION : un aller-retour de navigation refait #qcol par `content.innerHTML`
 *  (`revueShell`, router.ts — et app.js pour la maquette), donc tous ces nœuds disparaissent
 *  ensemble et sont reposés ensemble au rendu suivant, chacun à son rang.
 *  Tient au POLL de 300ms : chaque `ensure*` sort en tête sur son nœud déjà monté, aucun ne
 *  redéplace un nœud existant — déplacer un `<input>` monté lui ferait perdre le focus en pleine
 *  frappe, et le déplacement lui-même est une écriture DOM que ce poll ne doit pas payer.
 *
 *  `.sift-qhead` et `#ql` ne sont pas injectés ici (ils viennent du markup de `revueShell`) : ils
 *  figurent dans la liste comme ANCRES, c'est-à-dire comme les repères devant lesquels les blocs
 *  injectés doivent se ranger. */
const QCOL_ORDER = [
  // La rangée de filtre coiffe la colonne, la recherche se pose CONTRE la liste qu'elle
  // interroge. C'est le retour au wireframe § 15 (décision d'Antoine, 2026-08-26) : la décision E
  // du 2026-08-24 avait remonté la recherche en tête, elle s'y lisait détachée de sa liste.
  // Le motif est celui du volet de lecture de Mail (`docs/design-refs/03-mail.png`) : ce qui
  // qualifie la liste est en tête, ce qui la filtre reste collé à elle.
  ".sift-qhead", // 1. ancre : rangée de filtre — pulldown (revueShell)
  "#sift-qsearch", // 2. recherche — contre la liste
  "#ql", // 3. ancre : liste virtualisée (revueShell)
  // La bascule « Non analysés uniquement » occupait la position 4 — retirée le 2026-08-26, le pied
  // ne porte plus qu'un contrôle.
  "#sift-qreanalyze-all", // 4. pied : « Réanalyser (N) »
  "#sift-qfacet-pop", // hors flux (`position:fixed`) : rangé en dernier, sa place ne se voit pas
] as const;

/** Une des places déclarées ci-dessus. Union de littéraux : un sélecteur absent de `QCOL_ORDER`
 *  ne compile pas, plutôt que d'atterrir silencieusement en tête de colonne à l'exécution. */
type QcolSlot = (typeof QCOL_ORDER)[number];

/** Insère `el` dans #qcol à la place que `QCOL_ORDER` lui donne : devant le premier successeur
 *  déjà monté, ou à la fin s'il n'y en a aucun (`insertBefore(…, null)` = `appendChild`). Appelé
 *  au seul MONTAGE d'un bloc — les `ensure*` sortent en tête quand leur nœud existe déjà. */
function placeInQueueColumn(qcol: HTMLElement, el: HTMLElement, slot: QcolSlot): void {
  const kids = Array.from(qcol.children);
  let ref: Element | null = null;
  for (let i = QCOL_ORDER.indexOf(slot) + 1; i < QCOL_ORDER.length; i++) {
    const next = kids.find((c) => c.matches(QCOL_ORDER[i]));
    if (next) {
      ref = next;
      break;
    }
  }
  qcol.insertBefore(el, ref);
}

/** Les trois blocs injectés de la colonne file, montés depuis UN seul point d'appel. L'ordre des
 *  trois lignes ci-dessous suit `QCOL_ORDER` pour se lire comme la colonne, mais il ne la décide
 *  plus : c'est `placeInQueueColumn` qui range. */
function ensureQueueColumnChrome(qcol: HTMLElement): void {
  ensureQueueSearch(qcol);
  ensureQueueFacet(qcol);
  syncQueueSelectButton();
  syncQueueFoot(qcol);
}

/** Pied de colonne. Il ne porte plus qu'un contrôle depuis le 2026-08-26 — « Réanalyser (N) ».
 *
 *  La bascule « Non analysés uniquement » qui l'accompagnait est retirée (décision d'Antoine). Ce
 *  point d'entrée reste parce que le compte de pistes non analysées se relit à chaque fois, et
 *  qu'il était déjà le passage obligé de ses trois rappels — après un clic, après une relance, et
 *  après que la relance s'est réglée. */
function syncQueueFoot(qcol: HTMLElement): void {
  ensureQueueReanalyzeAllButton(qcol, unanalyzedItems().length);
}

/** Commutateur du mode Lot — à DROITE de la rangée de filtre, en TEXTE, depuis le 2026-08-26.
 *
 *  Il vivait au bord droit de la barre unifiée (patron toolbar de Photos, `2d2c6d4`). Antoine : « le
 *  bouton batch devrait être dans le panneau de la file ». Ce qui tranche : le patron Photos place
 *  « Sélectionner » dans la toolbar parce que la grille de photos EST l'écran — la toolbar y
 *  commande la seule chose visible. Dans Revue la file n'est qu'une colonne sur trois, et la barre
 *  commande déjà l'écran entier ; le même geste n'a plus la même portée. Ici, l'action rejoint le
 *  pulldown de filtre, l'autre contrôle qui ne pilote que cette liste. L'ancien emplacement était à
 *  ~1 150 px de la file qu'il commande, et traversait la frontière barre/contenu.
 *
 *  TEXTE et non icône : `CLAUDE.md` § Front, un CTA à libellé descriptif se dit en texte seul. Le
 *  libellé dit l'ACTION du clic et non l'état affiché — « Sélectionner » puis « Terminé », comme
 *  Photos ; `aria-pressed` dit l'état, et les deux ne se recopient pas.
 *
 *  Le clic n'est PAS câblé ici : `data-sift="reviewmode"` est l'action que le dispatch de
 *  `sift-live.ts` sert déjà, et `data-m` porte le mode CIBLE — l'inverse de l'état courant. C'est ce
 *  qui fait d'un commutateur à deux positions un simple bouton d'action pour le dispatch, qui n'a
 *  alors rien à savoir de l'état. Même contrat que l'icône de barre qu'il remplace.
 *
 *  Créé une fois puis muté : `ensureQueueColumnChrome` repasse ici au rendu de la file, poll de
 *  300 ms compris. Les deux portes de mode l'appellent aussi — c'est ce qui repeint le libellé au
 *  moment du basculement, sans attendre le prochain rendu. */
function syncQueueSelectButton(): void {
  const head = document.querySelector<HTMLElement>(".sift-qhead");
  if (!head) return;
  let btn = document.getElementById("sift-qselect") as HTMLButtonElement | null;
  if (!btn) {
    btn = document.createElement("button");
    btn.id = "sift-qselect";
    btn.className = "sift-qselect-btn";
    btn.type = "button";
    btn.dataset.sift = "reviewmode";
    // `appendChild` : le pulldown de filtre se `prepend` à gauche, ce bouton se pose à droite, et
    // c'est le `justify-content:space-between` de la rangée qui les écarte.
    head.appendChild(btn);
  }
  const armed = reviewMode === "batch";
  const label = armed ? "Terminé" : "Sélectionner";
  if (btn.textContent !== label) btn.textContent = label;
  btn.dataset.m = armed ? "detail" : "batch";
  btn.setAttribute("aria-pressed", armed ? "true" : "false");
  btn.title = armed ? "Quitter le mode Lot" : "Sélectionner plusieurs pistes";

  // Pulldown « Sélection » (issue #60) : créé une fois, montré en mode Lot seulement, posé juste
  // avant « Terminé ». Même gabarit de kit que le pulldown de filtre à gauche.
  let menu = document.getElementById("sift-qselmenu") as HTMLButtonElement | null;
  if (!menu) {
    menu = document.createElement("button");
    menu.id = "sift-qselmenu";
    menu.className = "sift-qfacet-btn sift-qselmenu-btn";
    menu.type = "button";
    menu.setAttribute("aria-haspopup", "menu");
    menu.title = "Tout, aucune, ou seulement une catégorie";
    menu.innerHTML =
      '<span class="sift-qfacet-label">Sélection</span><i class="ti ti-chevron-down" aria-hidden="true"></i>';
    menu.addEventListener("click", (e) => {
      e.stopPropagation();
      openQueueSelectionMenu(menu as HTMLElement);
    });
    head.insertBefore(menu, btn);
  }
  menu.hidden = !armed;
}

/** Confirmation threshold for the bulk retry — a few stuck tracks retry on one click, but a mass
 *  reset (e.g. a whole fresh import still showing as unanalysed) requires an in-app confirmation
 *  (CLAUDE.md: destructive/costly actions never fire unguarded). */
const BULK_REANALYZE_CONFIRM_THRESHOLD = 10;

/** Add/remove ids from the in-flight retry set and re-render so the row spinner reflects it.
 *  Exported for the per-row retry handler in sift-live.ts (the bulk button uses them internally). */
export function beginReanalyze(ids: number[]): void {
  for (const id of ids) reanalyzingIds.add(id);
  rerenderQueueWindow();
}
export function endReanalyze(ids: number[]): void {
  for (const id of ids) reanalyzingIds.delete(id);
  rerenderQueueWindow();
}

/** Rangée de statut « N non analysées · [Réanalyser] » — retries every currently-unanalysed
 * track in one click, regardless of whether the "Non analysés uniquement" filter is on. Sibling
 * of the filter toggle, same hidden-when-nothing-to-act-on rule.
 *
 * Le lien souligné « Réanalyser (N) » est refait en RANGÉE le 2026-09-06 (audit œil-Apple,
 * wireframe validé par Antoine) : un lien souligné est un idiome web sans équivalent système —
 * Apple dit un compte en pied (barre de statut du Finder, « 17 éléments ») et une action en
 * bouton. Compte à gauche (lisible sans cliquer, formaté fr-FR), bouton à droite (grammaire du
 * bouton secondaire, même famille que Ré-identifier). Create-once, muté ensuite — ce chemin est
 * appelé par les re-rendus de file (poll 300 ms, queue:changed). */
function ensureQueueReanalyzeAllButton(qcol: HTMLElement, unanalyzedCount: number): void {
  let el = document.getElementById("sift-qreanalyze-all");
  if (!el) {
    el = document.createElement("div");
    el.id = "sift-qreanalyze-all";
    el.className = "sift-qfoot-stat";
    // La piste de progression est un enfant PERMANENT, masquée au repos : pendant une analyse
    // (`taskOf("analyze")`, demande d'Antoine 2026-09-06 — « N non analysées » mentait alors,
    // ces pistes partent toutes seules), la rangée devient « Analyse — x/y » + barre, bouton
    // masqué. Mode Utilitaire de disque : progression in-place, là où le compte vivait.
    el.innerHTML =
      `<span class="sift-qfoot-count"></span>` +
      `<div class="sift-qfoot-track" hidden><div class="sift-qfoot-fill"></div></div>` +
      `<button class="sift-qfoot-go" type="button"></button>`;
    el.querySelector("button")?.addEventListener("click", async () => {
      if (bulkReanalyzing) return; // already running — ignore re-clicks
      const ids = unanalyzedItems().map((it) => it.id);
      if (!ids.length) return;
      if (
        ids.length >= BULK_REANALYZE_CONFIRM_THRESHOLD &&
        !(await confirmAction(
          `Réanalyser ${ids.length} morceaux ? Leur analyse en cache est effacée et recalculée.`,
          "Réanalyser",
        ))
      ) {
        return;
      }
      bulkReanalyzing = true;
      beginReanalyze(ids);
      syncQueueFoot(qcol); // reflète « Relance… » + désactivé
      try {
        await reanalyzeTracks(ids);
        // queue:changed (emitted unconditionally by the backend) drives the queue re-render.
        toast(`${ids.length} morceau${ids.length > 1 ? "x" : ""} réanalysé${ids.length > 1 ? "s" : ""}`);
      } catch (e) {
        console.error("reanalyze_tracks failed", e);
        // Humanized fallback (audit UX/accessibilité 2026-07-24) — raw error kept in console.error
        // above only; same pattern as filing-identify.ts's doApplyTags/doUndoApply.
        toast("Échec de la réanalyse — réessaie");
      } finally {
        bulkReanalyzing = false;
        endReanalyze(ids);
        syncQueueFoot(qcol); // re-libelle + réactive depuis l état réglé
      }
    });
    placeInQueueColumn(qcol, el, "#sift-qreanalyze-all");
  }
  const count = el.querySelector<HTMLElement>(".sift-qfoot-count");
  const track = el.querySelector<HTMLElement>(".sift-qfoot-track");
  const fill = el.querySelector<HTMLElement>(".sift-qfoot-fill");
  const go = el.querySelector<HTMLButtonElement>(".sift-qfoot-go");
  if (!count || !track || !fill || !go) return;
  const an = taskOf("analyze");
  const analyzing = an?.state === "running" && an.total > 0;
  el.hidden = unanalyzedCount === 0 && !analyzing;
  // `textContent`/`transform` sur des nœuds persistants — jamais d'innerHTML ici, ce chemin
  // tourne en rafale (poll 300 ms, queue:changed, task:progress relayé par sift-live).
  if (analyzing) {
    count.textContent = `Analyse — ${an.done.toLocaleString("fr-FR")}/${an.total.toLocaleString("fr-FR")}`;
    fill.style.transform = `scaleX(${an.done / an.total})`;
    track.hidden = false;
    go.hidden = true;
    return;
  }
  // « N pistes non analysées », jamais « N non analysées » — l'accord portait sur un nom absent
  // (retour d'Antoine, 2026-09-06). « pistes » = le mot de la barre (« 3372 pistes ») et de la
  // spec ; la confirmation du lot dit encore « morceaux », écart préexistant non traité ici.
  const s = unanalyzedCount > 1 ? "s" : "";
  count.textContent = `${unanalyzedCount.toLocaleString("fr-FR")} piste${s} non analysée${s}`;
  track.hidden = true;
  go.hidden = false;
  // State-driven, not a mid-flight eager re-enable: the button is disabled iff a bulk retry is
  // actually running (bulkReanalyzing), so a queue:changed re-render during the retry can't flip it
  // back to enabled under the in-flight handler (review-caught double-submit race).
  go.disabled = bulkReanalyzing;
  go.textContent = bulkReanalyzing ? "Relance…" : "Réanalyser";
}

/** Resynchronise la rangée de statut du pied depuis l'état de tâche COURANT — appelée par
 *  sift-live à chaque `task:progress` d'analyse, pour que la barre avance à la cadence des
 *  événements et non à celle des re-rendus de file. No-op tant que la rangée n'est pas montée. */
export function refreshQueueFootStat(): void {
  const el = document.getElementById("sift-qreanalyze-all");
  const qcol = el?.closest<HTMLElement>("#qcol");
  if (!el || !qcol) return;
  ensureQueueReanalyzeAllButton(qcol, unanalyzedItems().length);
}

/** Live filter bar for the queue rail (annotation: "on veut une barre de recherche en bas" —
 * remontée en TÊTE de la colonne le 2026-08-25, décision Revue #47 : la recherche coiffe la file,
 * comme le champ de filtre d'un inspecteur macOS). Injected once, à la PREMIÈRE place de
 * `QCOL_ORDER` : `.sift-qhead` puis `#ql` la suivent, donc elle coiffe la liste au lieu de flotter
 * dessous. Sa place ne dépend plus de l'ordre d'appel — voir `placeInQueueColumn`.
 * Filters currentItems client-side only (title/artist) — see visibleQueueItems().
 *
 * GABARIT DU KIT depuis le 2026-08-25 (Big Sur, « Search fields » : 45:812 au repos, 45:825 au
 * focus, 45:899 avec clear), aligné sur le champ JUMEAU de la barre unifiée
 * (`toolbar.ts::mountBarSearch`, `.sift-bar-search`) plutôt qu'en variante inventée : loupe à
 * GAUCHE dans le flux, placeholder « Rechercher », bouton clear dès qu'il y a du texte.
 * Ce que ce geste RETIRE, et qui venait de l'annotation « met juste une icone de loupe sur la
 * droite qui disparait quand on tape » :
 *   · la loupe en overlay `position:absolute` à droite, masquée à la première frappe ;
 *   · les 30px de gouttière droite qu'elle réservait — la gouttière, elle, RESTAIT une fois
 *     l'icône partie, donc un champ rempli montrait 30px de vide inerte exactement là où le kit
 *     pose son clear.
 * L'apparence (hauteur, rayon, trait, focus) vit maintenant dans `.sift-search-wrap`
 * (`styles.css`, région de la colonne file) : plus aucune valeur en dur ici. */
function ensureQueueSearch(qcol: HTMLElement): void {
  if (document.getElementById("sift-qsearch")) return;
  const wrap = document.createElement("div");
  wrap.id = "sift-qsearch";
  // `sift-search-wrap` porte le trait, le rayon, la hauteur ET le traitement de focus : l'input
  // lui-même reste sans bordure, donc le traitement de focus des champs texte (`styles.css`,
  // border-color) ne peut toujours rien montrer sur lui.
  wrap.className = "sift-search-wrap";
  // Aucune donnée d'exécution ici — que des littéraux, rien à passer par `esc()`.
  wrap.innerHTML =
    '<i class="ti ti-search" aria-hidden="true"></i>' +
    '<input id="sift-qsearch-input" type="search" placeholder="Rechercher" ' +
    'aria-label="Filtrer la file">' +
    '<button type="button" id="sift-qsearch-clear" class="sift-search-clear" hidden ' +
    'aria-label="Effacer la recherche" title="Effacer la recherche">' +
    '<i class="ti ti-circle-x" aria-hidden="true"></i></button>';
  // En TÊTE de #qcol : devant `.sift-qhead` puis `#ql`. Le segmenté Détail / Lot, qui vivait au-
  // dessus d'elle, a été retiré le 2026-08-25 — plus rien ne la précède. Guard en tête de fonction
  // → insertion unique : le poll de 300ms ne redéplace pas le champ (il perdrait le focus).
  placeInQueueColumn(qcol, wrap, "#sift-qsearch");
  const input = wrap.querySelector<HTMLInputElement>("#sift-qsearch-input")!;
  const clear = wrap.querySelector<HTMLButtonElement>("#sift-qsearch-clear")!;
  // REMISE EN PHASE AU MONTAGE. Le terme survit à la navigation (`queueSearchTerm`, niveau module)
  // alors que le champ, lui, est recréé VIDE par `revueShell` (`content.innerHTML`). Sans cette
  // ligne, un retour sur Revue rendait une file filtrée par un terme que plus rien n'affichait —
  // et depuis que le clear se cache sur un champ vide, sans plus aucune commande pour en sortir.
  // Même piège que « Non analysés uniquement », qui porte son propre garde-fou daté plus haut, et
  // même choix que `queueFacetFilter` : l'état gouverne, le contrôle se relit dessus.
  input.value = queueSearchTerm;
  clear.hidden = queueSearchTerm === "";
  // UN SEUL point qui applique le terme, partagé par la frappe et par le clear : les deux chemins
  // ne peuvent donc pas diverger. Un clear qui aurait oublié `ql.scrollTop = 0` rendrait la file
  // ENTIÈRE à la position de défilement d'une liste filtrée bien plus courte.
  // Coût par frappe : une propriété booléenne sur le bouton, plus le rendu de la fenêtre
  // virtualisée qui existait déjà. Aucun nœud créé, aucun `innerHTML` — la frappe est l'événement
  // en rafale par excellence (CLAUDE.md § Front).
  const applySearch = (): void => {
    queueSearchTerm = input.value.trim();
    clear.hidden = input.value === "";
    const ql = document.getElementById("ql");
    if (ql) {
      ql.scrollTop = 0; // a shorter filtered list can leave scrollTop referring to nothing
      renderQueueWindow(ql);
    }
  };
  input.addEventListener("input", applySearch);
  clear.addEventListener("click", () => {
    input.value = "";
    applySearch(); // terme vide → la file ENTIÈRE revient, et le bouton repasse en `hidden`
    // La main revient au champ : on efface pour retaper, pas pour partir. Sans ce focus, le clear
    // garderait le focus clavier sur un bouton qui vient de disparaître.
    input.focus();
  });
}

/** Focus + sélectionne le champ de recherche de la file (Revue). Rend false si le champ n'est pas
 *  monté (autre vue, ou colonne file absente) — l'appelant (⌘F, shortcuts.ts) se rabat alors sur la
 *  recherche de la barre unifiée. Testé par présence DOM, pas par un état de vue exporté : aucun
 *  import routeur ici. */
export function focusQueueSearch(): boolean {
  const input = document.getElementById("sift-qsearch-input") as HTMLInputElement | null;
  if (!input) return false;
  input.focus();
  input.select();
  return true;
}

// ---------------------------------------------------------------------------
// Filtre par facettes — pulldown de la rangée de filtre + popover cochable
// ---------------------------------------------------------------------------

/** Re-applique le filtre à la file affichée (après une case cochée / « Tout afficher »). */
function applyFacetFilter(): void {
  const ql = document.getElementById("ql");
  if (ql) {
    ql.scrollTop = 0; // une liste plus courte peut laisser scrollTop pointer dans le vide
    renderQueueWindow(ql);
  }
}

/** Ferme le popover de facettes. Exporté pour dismissTopmost (shortcuts.ts) + l'auto-fermeture. */
export function closeQueueFacet(): void {
  const pop = document.getElementById("sift-qfacet-pop");
  if (!pop || pop.hidden) return;
  pop.hidden = true;
  document.querySelector('[data-fil="qfacet"]')?.setAttribute("aria-expanded", "false");
}

/** Ancre le popover sous le bouton filtre, depuis la géométrie du moment (rejouable). */
function placeQueueFacet(): void {
  const pop = document.getElementById("sift-qfacet-pop");
  const btn = document.querySelector<HTMLElement>('[data-fil="qfacet"]');
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

/** Compte par facette (wireframe « Poste de décision » § 10 : « Lossless 5 · MP3 3 · Faux 3 ·
 *  Doublons 2 »), plus le TOTAL de la file sur la rangée « Tout afficher » — ce que cette rangée
 *  ramènerait à l'écran si on la cliquait.
 *
 *  Lus sur `currentItems`, la file SOURCE, et JAMAIS sur `visibleQueueItems()` : sur la liste déjà
 *  filtrée, cocher « Faux » ferait tomber « Lossless », « MP3 » et « Doublons » à 0, et le menu
 *  cesserait de dire ce qu'il y a à filtrer pour ne plus dire que ce qui reste après filtrage.
 *
 *  ⚠️ C'est l'exacte symétrique du compte de la BARRE, qui lui se lit sur la liste filtrée
 *  (`paintBarCount`, issue #49) : celui-ci dit ce qu'il y a À filtrer, celui-là ce que le filtre
 *  LAISSE VOIR. Les deux nombres divergent dès qu'une case est cochée, et c'est voulu — les
 *  intervertir rendrait chacun muet sur sa propre question.
 *
 *  Appelé à l'OUVERTURE du menu seulement (`toggleQueueFacet`), jamais depuis le rendu de la file :
 *  ce balayage est en O(file × facettes) et la colonne se repeint sur un poll de 300ms — c'est le
 *  point chaud du frontend. Une file fermée n'a pas de compte à montrer. */
function paintFacetCounts(pop: HTMLElement): void {
  const counts = new Map<string, number>(QUEUE_FACETS.map((f) => [f.id, 0]));
  // Un seul parcours de la file pour toutes les facettes (elles ne s'excluent pas : un même item
  // peut être compté dans plusieurs, l'union du filtre le dédoublonne ensuite).
  for (const it of currentItems) {
    for (const f of QUEUE_FACETS) if (f.match(it)) counts.set(f.id, (counts.get(f.id) ?? 0) + 1);
  }
  for (const f of QUEUE_FACETS) {
    // `f.id` est un littéral de QUEUE_FACETS, pas une donnée : sûr dans un sélecteur.
    const el = pop.querySelector<HTMLElement>(`[data-facet-count="${f.id}"]`);
    const s = String(counts.get(f.id) ?? 0);
    if (el && el.textContent !== s) el.textContent = s;
  }
  const totalEl = pop.querySelector<HTMLElement>("[data-facet-total]");
  const total = String(currentItems.length);
  if (totalEl && totalEl.textContent !== total) totalEl.textContent = total;
}

/** Ouvre/ferme le popover. Les cases ET les comptes reflètent l'état à chaque ouverture (muter, pas
 *  rebuild). Placement au SECOND frame — le style n'est pas recalculé au premier dans WebView2
 *  (même leçon que le popover de facettes de Bibliothèque et le placement du popover Destination). */
function toggleQueueFacet(): void {
  const pop = document.getElementById("sift-qfacet-pop");
  const btn = document.querySelector<HTMLElement>('[data-fil="qfacet"]');
  if (!pop || !btn) return;
  if (!pop.hidden) {
    closeQueueFacet();
    return;
  }
  pop.querySelectorAll<HTMLInputElement>("input[data-facet]").forEach((cb) => {
    cb.checked = queueFacetFilter.has(cb.dataset.facet || "");
  });
  // AVANT `hidden = false`, comme les cases juste au-dessus : le menu ne doit jamais être peint,
  // même une frame, avec des cellules de compte vides. Elles le sont au montage (`ensureQueueFacet`
  // pose le markup sans valeur) et le resteraient jusqu'ici à la première ouverture.
  paintFacetCounts(pop);
  pop.hidden = false;
  btn.setAttribute("aria-expanded", "true");
  requestAnimationFrame(() => requestAnimationFrame(() => placeQueueFacet()));
}

/** Résumé EN TOUTES LETTRES de la combinaison cochée — c'est le libellé du pulldown (wireframe
 *  « Poste de décision » §§ 09-10 ; spec § Zone B′ : « le bouton résume la combinaison »). Il
 *  remplace le compte numérique de critères, qui disait « 2 » là où la rangée dit maintenant
 *  « Faux + Doublons » : le filtre actif se lit sans ouvrir le menu.
 *
 *  Ordre de `QUEUE_FACETS`, PAS l'ordre d'insertion du Set — sinon la même combinaison se nommerait
 *  « Faux + Doublons » ou « Doublons + Faux » selon l'ordre des clics.
 *
 *  Rien de coché → « Tout afficher », mot pour mot la commande de remise à zéro du menu : l'état
 *  sans filtre et la sortie du filtre se disent pareil, donc le bouton ne peut pas laisser croire
 *  qu'un filtre est posé alors qu'il n'y en a aucun. */
function facetSummary(): string {
  const on = QUEUE_FACETS.filter((f) => queueFacetFilter.has(f.id)).map((f) => f.label);
  return on.length ? on.join(" + ") : "Tout afficher";
}

/** Repeint le pulldown depuis `queueFacetFilter` : libellé résumé + marque `on` (qui ne pilote plus
 *  qu'une ENCRE, `styles.css` — filtre posé en encre primaire, « Tout afficher » en tertiaire).
 *  Plus d'`aria-pressed` : ce bouton n'est pas une bascule mais un *pop-up button* (`aria-haspopup`
 *  + `aria-expanded`), et son libellé visible dit déjà l'état — le doubler d'un état pressé faisait
 *  lire deux choses contradictoires à un lecteur d'écran. */
function paintFacetButton(btn: HTMLElement): void {
  btn.classList.toggle("on", queueFacetFilter.size > 0);
  const label = btn.querySelector<HTMLElement>(".sift-qfacet-label");
  // `textContent`, pas `innerHTML` : les libellés viennent de `QUEUE_FACETS` (littéraux), et cette
  // fonction est rappelée à chaque rendu de la file — poll de 300ms compris.
  if (label) label.textContent = facetSummary();
}

// Auto-fermeture (scroll / resize / clic extérieur), câblée une fois. Le popover est ancré à un
// POINT : défiler ou redimensionner le ferme plutôt que courir après la géométrie — même règle que
// le menu contextuel et le popover de facettes de Bibliothèque.
let queueFacetDismissWired = false;
function ensureQueueFacetDismiss(): void {
  if (queueFacetDismissWired) return;
  queueFacetDismissWired = true;
  const close = () => closeQueueFacet();
  document.addEventListener("scroll", close, { capture: true });
  window.addEventListener("resize", close);
  document.addEventListener(
    "click",
    (e) => {
      const pop = document.getElementById("sift-qfacet-pop");
      if (!pop || pop.hidden) return;
      const t = e.target as Node;
      if (pop.contains(t) || (t as HTMLElement).closest?.('[data-fil="qfacet"]')) return;
      closeQueueFacet();
    },
    { capture: true },
  );
}

/** Pulldown de filtre (à GAUCHE de la rangée `.sift-qhead`) + son popover cochable, créés UNE fois
 *  puis mutés — appelé au rendu de la file (poll compris), donc JAMAIS innerHTML= en rafale : le
 *  markup est posé au premier montage, les appels suivants ne re-peignent que le libellé. Réutilise
 *  la classe .sift-facet-pop (styles.css) et la géométrie anchoredBelowPosition (popover-position.ts).
 *
 *  GABARIT DU KIT depuis le 2026-08-25 (Big Sur, *pulldown* : 45:152 au repos, 45:169 au survol,
 *  45:186 pressé) — bouton à LIBELLÉ + chevron, plus le bouton icône-seule qu'il était. Ce que ce
 *  geste retire, et pourquoi :
 *    · le glyphe `ti-filter` et le compte numérique de critères (« 2 ») — le libellé les remplace
 *      en toutes lettres, et un CTA à libellé descriptif se dit en TEXTE SEUL (CLAUDE.md § Front) ;
 *    · l'`aria-label` « Filtrer la file » — sur un bouton qui a désormais du texte visible, il le
 *      MASQUERAIT au lecteur d'écran au lieu de l'aider. Le `title` reste : il nomme l'action là où
 *      le libellé ne nomme que l'état.
 *  L'apparence (hauteur, rayon, padding, survol, pressé) vit dans `.sift-qfacet-btn`
 *  (`styles.css`, région de la colonne file) : aucune valeur en dur ici. */
function ensureQueueFacet(qcol: HTMLElement): void {
  ensureQueueFacetDismiss();
  const head = qcol.querySelector<HTMLElement>(".sift-qhead");
  let btn = document.getElementById("sift-qfacet-btn");
  if (head && !btn) {
    btn = document.createElement("button");
    btn.id = "sift-qfacet-btn";
    btn.className = "sift-qfacet-btn";
    btn.setAttribute("type", "button");
    btn.setAttribute("data-fil", "qfacet");
    btn.setAttribute("aria-haspopup", "true");
    btn.setAttribute("aria-expanded", "false");
    btn.setAttribute("title", "Filtrer la file");
    // Aucune donnée d'exécution ici — le libellé est posé par `paintFacetButton` en `textContent`.
    btn.innerHTML =
      '<span class="sift-qfacet-label"></span><i class="ti ti-chevron-down" aria-hidden="true"></i>';
    btn.addEventListener("click", (e) => {
      e.stopPropagation();
      toggleQueueFacet();
    });
    // `prepend`, pas `appendChild` : le pulldown tient la GAUCHE de la rangée, le bouton de
    // sélection la droite (`syncQueueSelectButton`, qui `appendChild`). Insertion unique (gardée
    // par `!btn`), donc le poll de 300 ms ne redéplace rien.
    head.prepend(btn);
  }
  if (!document.getElementById("sift-qfacet-pop")) {
    const pop = document.createElement("div");
    pop.id = "sift-qfacet-pop";
    pop.className = "sift-facet-pop";
    pop.hidden = true;
    // Libellés STATIQUES (QUEUE_FACETS), aucune donnée non fiable — esc() par prudence de site neuf.
    //
    // GABARIT DU KIT (Menu/Menu-items 58:49) depuis le 2026-08-25 : chaque option est une rangée à
    // trois cellules — case à cocher, libellé, compte — et « Tout afficher » en est une quatrième,
    // derrière un séparateur. Deux ajouts que le wireframe § 10 dessine et que ce menu n'avait pas :
    //   · la CASE au gabarit du kit — la case native reste le contrôle (clavier, `change`, nom
    //     accessible via le <label>), `.sift-qfacet-box` en est l'habillage peint (`styles.css`) ;
    //   · le COMPTE par facette, à droite, écrit par `paintFacetCounts` à l'ouverture du menu.
    // `.sift-menu-label` / `.sift-menu-count` sont RÉUTILISÉS et non redéclarés : ce sont les deux
    // cellules de la grammaire de menu du dépôt (une seule dans l'app), et `.sift-menu-count` porte
    // déjà les chiffres tabulaires que le wireframe demande.
    pop.innerHTML =
      QUEUE_FACETS.map(
        (f) =>
          `<label class="sift-qfacet-opt">` +
          `<input type="checkbox" class="sift-qfacet-ck" data-facet="${esc(f.id)}">` +
          `<span class="sift-qfacet-box" aria-hidden="true"><i class="ti ti-check"></i></span>` +
          `<span class="sift-menu-label">${esc(f.label)}</span>` +
          `<span class="sift-menu-count" data-facet-count="${esc(f.id)}"></span>` +
          `</label>`,
      ).join("") +
      '<div class="sift-qfacet-sep" role="separator"></div>' +
      '<button type="button" class="sift-qfacet-reset" data-facet-reset>' +
      '<span class="sift-menu-label">Tout afficher</span>' +
      '<span class="sift-menu-count" data-facet-total></span>' +
      "</button>";
    pop.querySelectorAll<HTMLInputElement>("input[data-facet]").forEach((cb) =>
      cb.addEventListener("change", () => {
        const id = cb.dataset.facet || "";
        if (cb.checked) queueFacetFilter.add(id);
        else queueFacetFilter.delete(id);
        const b = document.getElementById("sift-qfacet-btn");
        if (b) paintFacetButton(b);
        applyFacetFilter();
      }),
    );
    pop.querySelector("[data-facet-reset]")?.addEventListener("click", () => {
      queueFacetFilter.clear();
      pop.querySelectorAll<HTMLInputElement>("input[data-facet]").forEach((cb) => (cb.checked = false));
      const b = document.getElementById("sift-qfacet-btn");
      if (b) paintFacetButton(b);
      applyFacetFilter();
      closeQueueFacet();
    });
    // Rangé en dernier dans #qcol : il est `position:fixed` (`.sift-facet-pop`), donc hors flux —
    // sa place dans le DOM ne se voit pas, mais elle est déclarée comme les autres pour que
    // l'ordre de la colonne se lise en un seul endroit. Enfant de #qcol pour disparaître avec lui
    // quand la navigation reconstruit la colonne.
    placeInQueueColumn(qcol, pop, "#sift-qfacet-pop");
  }
  if (btn) paintFacetButton(btn);
}

// `updateRevueBadge` vivait ici — retirée le 2026-08-26 avec le badge de rail de Revue.
// Motif Mail, relevé sur `docs/design-refs/03-mail.png` : « Inbox » est écrit AUX DEUX endroits,
// sidebar et tête de colonne, mais son compte n'est qu'à UN seul — celui de la colonne, jamais la
// sidebar. Notes fait l'inverse pour la raison inverse : sa colonne n'a AUCUN en-tête, donc le
// compte remonte sur l'entrée de sidebar qui nomme la liste. La règle commune est la même — le
// compte va contre le nom de la liste, à un seul endroit — et Sift est le cas de Mail : « Revue »
// est écrit dans le rail ET dans la barre, donc le compte reste dans la barre (`paintBarCount`).
// Les badges des SOURCES restent, eux : leur compte n'est écrit nulle part ailleurs, c'est le cas
// de Notes.

// Debounces the heavy report/audio-decode load triggered by a queue-row selection (click or
// ↑/↓, which dispatches a real .click() — see installFilingKeys). Flicking through several
// rows fast would otherwise fire a full fetch+decodeAudioData per row, most immediately
// discarded. Row highlighting itself stays instant — only this load is deferred.
let queueSelectTimer: ReturnType<typeof setTimeout> | undefined;

/** Queue row click (Revue): opens the filing pane after a 150ms debounce (flicking through rows
 *  fast must not fire a decode+fetch per row). Extracted from installLiveWiring's #pa click
 *  listener (Phase 1, tranche 1b) — same split as handleRekordboxAction (tranche 1a): the state
 *  this reads/writes (currentItems, currentOpenId, reviewMode) already lives in this module. */
export function handleQueueItemClick(qi: HTMLElement, e: MouseEvent): void {
  e.stopPropagation();
  // In batch mode a row-click means "inspect this one" → drop back to the detail pane.
  // But NOT while a batch filing is running (sheet open) — the queue stays interactive for
  // selecting the next batch (wireframe § 17: "non-modale, la file reste utilisable").
  if (reviewMode === "batch" && !isBatchSheetOpen()) enterDetailMode();
  const id = Number(qi.dataset.id);
  const item = currentItems.find((it) => it.id === id);
  const mid = requireEl("#mid", "qi-click");
  currentOpenId = id;
  const ql = document.getElementById("ql");
  if (ql) renderQueueWindow(ql);
  clearTimeout(queueSelectTimer);
  queueSelectTimer = setTimeout(() => {
    if (item && mid) {
      void openFilingInto(mid, item);
      prefetchNextAfter(item.id);
    } else if (qi.dataset.path)
      void import("./report-view").then((m) => m.openReportModal(qi.dataset.path!));
  }, 150);
}

/** Raw reviewMode mutator, no side effects. Plus exporté depuis le 2026-08-26 : ses deux seuls
 *  appelants sont désormais les deux portes de ce module, `enterDetailMode` et `enterBatchMode`.
 *  Le garder hors de la surface publique EST la garde — muter le mode sans repeindre la file était
 *  le bug que la porte batch a corrigé, et un appelant extérieur pourrait le refaire. */
function setReviewModeRaw(m: "detail" | "batch"): void {
  reviewMode = m;
  queueRowHeightCache = null; // hauteur change entre détail et lot (case ajoutée)
  if (m === "detail") queueBatchSel.clear();
}

/** The "detail" branch of the old setReviewMode, extracted verbatim — it never touched batch
 *  state, so unlike the "batch" branch it can live here. Called directly by queue code
 *  (handleQueueItemClick, stepQueueSelection) and by sift-live.ts's setReviewMode when switching
 *  away from batch mode. */
export function enterDetailMode(): void {
  // Fail-fast assertion restored from the original setReviewMode, which ran this
  // unconditionally before branching on mode (result unused here — batch branch
  // was the only one that needed #fldz's value).
  requireEl("#fldz", "enterDetailMode");
  setReviewModeRaw("detail");
  // Plus de segmenté à repeindre ici depuis le 2026-08-25 : c'est l'icône de sélection de la barre
  // unifiée qui porte l'état armé/désarmé du mode Lot (spec `docs/ui-specs/revue.md` § Zone A).
  // Leave batch pick mode: tree reverts to detail's state.binRel. No manual opacity/checkbox
  // cleanup needed — renderBins (filing.ts) always re-derives .sift-fldz-tree's opacity from
  // the current binPick (null in detail) and renders the one shared in-place checkbox itself.
  clearBinPick();
  // Return the progress zone to its left-sidebar home (it was relocated into the batch rail).
  homeProgressZone();
  syncQueueSelectButton();
  void renderQueue(true);
}

/** La porte du mode Lot, symétrique d'`enterDetailMode` — et la raison de son existence est le bug
 *  que sa dissymétrie avait laissé passer (mesuré dans la vraie fenêtre le 2026-08-26).
 *
 *  La branche « batch » de `setReviewMode` (sift-live.ts) vivait éparpillée chez son appelant, et
 *  elle repeignait `#mid` (`renderBatch`) et `#fldz` (`refreshBinsForBatch`) sans jamais repeindre
 *  LA FILE. Tant que le mode Lot n'avait pas de markup par ligne c'était sans conséquence ; depuis
 *  que la ligne porte une case (`0edbff2`, voir `queueRowHtml`), armer le mode ne changeait plus
 *  rien de visible dans la colonne : zéro case, et la surbrillance `.cur` que le mode Lot interdit
 *  (voir `highlightId`) restait peinte jusqu'au prochain repaint fortuit. Aucune erreur, aucune
 *  gate rouge — `queue-panel.ts` ne tourne que sous Tauri, et Vitest est en env Node.
 *
 *  Les deux modes ont donc désormais UNE porte chacun, et chaque porte finit par sa repeinture.
 *
 *  Repeinture des LIGNES seules (`renderQueueWindow`) et non `renderQueue()` : le mode a changé,
 *  les données non. `renderQueue` re-tomberait dans son chemin de rechargement complet (son bloc de
 *  cache exige un `#ql` VIDE, or il est plein ici), donc un aller-retour `listQueue()` sur toute la
 *  file — et il REMPLACE `currentItems`, ce qui courrait après l'`initQueueBatchSel` ci-dessus.
 *
 *  `refreshBinsForBatch` reste chez l'appelant : il a besoin de `batchBin`/`batchInPlace`, qui
 *  vivent dans `batch-panel.ts` — un import statique retour d'ici serait le cycle que
 *  `registerBatchRenderer` existe précisément pour casser. */
export function enterBatchMode(): void {
  requireEl("#fldz", "enterBatchMode");
  setReviewModeRaw("batch");
  // Le Diagnostic de la piste ouverte vit en zone D depuis le 2026-09-07 (filing.ts
  // `openFilingInto`). Le mode Lot n'a pas de piste ouverte — sa zone C est le résumé de
  // sélection — donc la colonne se referme ici, à la porte. Le retour en Détail la rouvre par
  // `syncDetail` → `openFilingInto` (le #mid réécrit par le Lot n'est plus « le nôtre »).
  closeAside();
  initQueueBatchSel(currentItems);
  // Pas d'appel optionnel `batchRenderer?.()` ici : à ce point du programme un renderer absent
  // n'est pas un état de repos possible, c'est `installLiveWiring` qui n'a pas tourné. Le taire
  // rendrait la zone C muette en silence — le fallback silencieux que la méthode du dépôt refuse.
  if (!batchRenderer) throw new Error("enterBatchMode: batchRenderer non enregistré");
  batchRenderer();
  syncQueueSelectButton();
  const ql = document.getElementById("ql");
  if (ql) renderQueueWindow(ql);
}

// A background conversion just settled (P5). On FAILURE the track is still `pending` and must
// reappear in the file with its marker — it was filtered out of currentItems while in flight, so a
// window re-render alone would not bring it back: re-fetch. `touchDetail: false` on purpose, the
// user is somewhere else by now and the pane they are looking at must not be hijacked (same reason
// the analysis-tick refresh passes false). On SUCCESS nothing is done here: the backend emits
// `queue:changed`, whose existing handler already refreshes the whole view.
onFilingOutcome((o) => {
  if (!o.error) return;
  // Mark the cache stale BEFORE re-rendering: off the Revue screen `#ql` does not exist and
  // renderQueue returns at once, so the re-fetch below may not happen at all. The flag is what
  // makes the NEXT paint (on returning to Revue) take the full reload path instead of repainting
  // the cache this track was filtered out of.
  queueCacheStale = true;
  void renderQueue(false);
});

let batchRenderer: (() => void) | null = null;

/** Registers the batch panel's render function so renderQueue's "reviewMode === batch" branch can
 *  trigger it without a static import — queue-panel.ts must never import from sift-live.ts (see
 *  the plan's Architecture section, 2nd occurrence of the same coupling). Call once from
 *  installLiveWiring, before any queue interaction is possible. */
export function registerBatchRenderer(fn: () => void): void {
  batchRenderer = fn;
}
