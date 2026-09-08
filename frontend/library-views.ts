// Bibliothèque list rendering: table rows/header (sortable) and grid tiles (cover art),
// both consumed by sift-live.ts's virtualized #biblist mount. Kept separate from
// library-detail.ts (the open-track editor) and sift-live.ts (screen orchestration) —
// this file owns only "how one row/tile of the filed-track list looks".
import type { LibraryTrack } from "../shared/contracts";
import { convertFileSrc } from "@tauri-apps/api/core";
import { esc } from "./dom";
import { libraryColumns, columnStyle, type LibraryColumn, type LibraryColumnField } from "./library-columns";

function qualPill(t: LibraryTrack): string {
  const f = (t.format || "?").toUpperCase();
  return `<span class="pill" style="flex:none">${esc(f)}</span>`;
}

// La vue de verdict de la ligne (`VerdictView`, `verdictRank`, `verdictView` — pastille + libellé,
// LOSSLESS / AUTHENTIQUE / FAKE / À VÉRIFIER / —, tranchée le 2026-08-19 contre les littéraux réels
// `ok` / `fake` / `grey` / NULL de `worker.rs::verdict_str`) est partie le 2026-09-08 avec la
// colonne Verdict de Rangés (décision d'Antoine, audit #24 : « pas besoin de mettre le verdict »).
// Le mot de verdict se lit dans l'inspecteur à l'ouverture (`report-view.ts::verdictWordTone`,
// même paire de faits : verdict sain ET rail lossless). Historique : `git log -S verdictView`.

/** Display name for a library row (artist — title, else filename). */
export function bibName(t: LibraryTrack): string {
  return t.artist && t.title ? `${t.artist} — ${t.title}` : t.path.split(/[\\/]/).pop() || t.path;
}

// Le champ de tri EST le champ de colonne : toutes les colonnes sont triables (`DESIGN.md` § 16),
// donc deux listes séparées ne pourraient que diverger. L'alias garde le nom que le reste du code
// emploie déjà.
type LibrarySortField = LibraryColumnField;
export type LibrarySortState = { field: LibrarySortField; dir: "asc" | "desc" };

/** Client-side sort — the filed-track list is small enough (a personal DJ crate, not a
 * streaming catalogue) that a SQL ORDER BY parameter isn't worth the added query surface. */
export function sortTracks(tracks: readonly LibraryTrack[], sort: LibrarySortState): LibraryTrack[] {
  const mul = sort.dir === "asc" ? 1 : -1;
  const sorted = [...tracks];
  sorted.sort((a, b) => {
    // Plus de tri par verdict depuis le 2026-09-08 : la colonne a quitté Rangés (décision d'Antoine,
    // audit #24 — le verdict se lit dans l'inspecteur à l'ouverture). Le tri par RANG catégoriel
    // qu'elle portait (fake, à vérifier, non analysé, sain) est parti avec elle ; le filtre
    // Lossless / MP3 de la barre reste le seul geste sur la qualité.
    // Les deux champs NUMÉRIQUES se comparent en nombres, pas en chaînes : un tri lexical classerait
    // une durée de 7:48 avant 12:03. `-Infinity` place les valeurs manquantes en tête en ascendant,
    // donc en queue en descendant.
    if (sort.field === "year" || sort.field === "duration") {
      const av = a[sort.field] ?? -Infinity,
        bv = b[sort.field] ?? -Infinity;
      return (av - bv) * mul;
    }
    const av = sort.field === "genre" ? (a.genres[0] ?? "") : (a[sort.field] ?? "");
    const bv = sort.field === "genre" ? (b.genres[0] ?? "") : (b[sort.field] ?? "");
    return av.localeCompare(bv) * mul;
  });
  return sorted;
}

// Les colonnes, leur ordre et leurs largeurs vivent dans `library-columns.ts` depuis le 2026-08-19 :
// elles sont devenues un ÉTAT (réordonnable, redimensionnable, mémorisé), et un état ne se déclare
// pas dans le module qui le peint.
//
// Durée est un AJOUT du 2026-08-19 : le champ existait dans le contrat (`shared/contracts.ts`) et
// n'atteignait pas l'écran. BPM était entré le même jour et ressort le 2026-09-08 : `metadata.bpm`
// n'a aucun écrivain côté Rust (vérifié par grep, audit #24) — une colonne de tirets sur toute
// bibliothèque. Elle reviendra avec l'analyse qui l'écrira, pas avant.
//
// Ni tonalité ni énergie : vérifié le 2026-08-19, aucun des deux n'existe dans `contracts.ts` ni
// dans `db.rs`. Aucune colonne fantôme n'est déclarée pour du vide — les ajouter est un chantier
// d'analyse Rust, pas une décision de design.

/** `mm:ss` — jamais `Intl.NumberFormat`, qui rendrait une durée comme un nombre. Une valeur
 *  absente rend un tiret cadratin, pas « 0:00 » : zéro seconde est un fait, l'absence en est un
 *  autre, et les confondre ferait croire à un fichier vide. */
function fmtDuration(sec: number | null): string {
  if (sec == null || !Number.isFinite(sec)) return "—";
  const m = Math.floor(sec / 60);
  const r = Math.round(sec % 60);
  return `${m}:${String(r).padStart(2, "0")}`;
}

/** Contenu texte d'une cellule, par champ. Les cinq branches sont exhaustives sur
 *  `LibraryColumnField` : ajouter une colonne sans son rendu casse la compilation, ce qui est le
 *  seul moment où l'oubli est rattrapable — une cellule vide, elle, se lit comme une donnée absente.
 *  `verdict` (pastille + libellé) et `bpm` (entier, jamais écrit côté Rust) sont parties le
 *  2026-09-08 avec leurs colonnes — voir `library-columns.ts`. */
function cellText(field: LibraryColumnField, t: LibraryTrack): string {
  switch (field) {
    case "artist":
      return esc(t.artist || "—");
    case "title":
      return esc(t.title || "—");
    case "duration":
      return fmtDuration(t.duration);
    case "genre":
      return esc(t.genres[0] || "—");
    case "year":
      return esc(t.year ? String(t.year) : "—");
  }
}

/** Une cellule. `data-col` est le crochet que `paintColumnWidth` mute pendant un redimensionnement —
 *  c'est lui qui permet d'écrire une largeur sur les lignes déjà montées sans re-rendre la liste.
 *
 *  UN SEUL gabarit d'enrobage, quelle que soit la colonne : la classe, le `data-col` et la largeur
 *  sont ce qui fait qu'une cellule s'aligne sur son en-tête. Seul le contenu varie. */
function cellHtml(col: LibraryColumn, t: LibraryTrack): string {
  return `<span class="sift-lib-col ${col.cls}" data-col="${col.field}"${columnStyle(col)}>${cellText(col.field, t)}</span>`;
}

/** Sortable column header row — each header is a real <button> (native keyboard support),
 * aria-sort on the active column announces direction to screen readers.
 *
 * Chaque en-tête porte aussi, depuis le 2026-08-19, les deux gestes de `library-columns.ts` : il est
 * la POIGNÉE de déplacement de sa colonne, et il se termine par un séparateur de redimensionnement.
 * Les deux cohabitent avec le clic de tri par un seuil de déplacement — voir `DRAG_THRESHOLD`. */
export function libraryTableHeaderHtml(sort: LibrarySortState): string {
  const cells = libraryColumns().map((col) => {
    const { field, label, cls } = col;
    const active = sort.field === field;
    const ariaSort = active ? (sort.dir === "asc" ? "ascending" : "descending") : "none";
    const arrow = active ? (sort.dir === "asc" ? " ▴" : " ▾") : "";
    // `<span role="columnheader">` et NON `<th>`. Mesure du 2026-08-19 dans la vraie fenêtre : la
    // ligne d'en-tête est un `<div>`, pas un `<table>`, et le parseur HTML SUPPRIME un `<th>` hors
    // contexte de table — il ne gardait que le `<button>` à l'intérieur. La classe de largeur et
    // l'`aria-sort` partaient donc avec la balise : les colonnes ne s'alignaient pas sur celles de
    // la ligne, et la direction de tri n'était annoncée à personne. Le défaut était antérieur à
    // l'ajout de BPM et Durée ; il ne se voyait pas tant que l'en-tête n'avait aucune largeur à
    // porter.
    return (
      `<span class="${cls} sift-lib-colhead" role="columnheader" aria-sort="${ariaSort}"` +
      ` data-colhead="${field}" data-col="${field}"${columnStyle(col)}>` +
      `<button data-bib="sort" data-field="${field}">${esc(label)}${arrow}</button>` +
      // Le séparateur est un enfant de l'en-tête, pas un frère : il doit rester collé au bord droit
      // de SA colonne quand celle-ci change de largeur ou de place, et un frère se serait décalé.
      `<span class="sift-lib-colsep" data-for="${field}" aria-hidden="true"></span>` +
      `</span>`
    );
  }).join("");
  // No wrapping role="table"/"grid" exists (this is a flex layout, not a real <table>) — role="row"
  // here without that ancestor was a half-applied ARIA table pattern a screen reader can't make
  // sense of. Dropped; each data row instead carries a composite aria-label (see libraryTableRowHtml).
  // Deux espaceurs, pas un. La ligne porte des affordances AVANT ses colonnes (bouton lecture,
  // pochette) et APRÈS (pastille de qualité, lien Discogs) ; sans un espaceur de chaque côté,
  // l'en-tête flotte décalé au-dessus des colonnes qu'il nomme. Mesuré : 62px devant, 69 derrière.
  return (
    `<div class="sift-lib-thead" role="row"><span class="sift-lib-thead-cov"></span>${cells}` +
    `<span class="sift-lib-thead-tail" aria-hidden="true"></span></div>`
  );
}

/** One table row — play button + cover thumbnail + the sortable columns of `libraryColumns()` +
 * the quality pill and the Discogs affordance.
 *
 * Anatomie depuis le 2026-09-08 (audit Rangés, #24) : pochette-bouton de lecture · colonnes ·
 * pastille de format. Sont partis ce jour-là le bouton lecture séparé (le triangle vit au survol
 * de la pochette), la colonne Verdict (lue dans l'inspecteur) et l'icône Discogs / loupe de fin de
 * ligne (clic droit, inspecteur). Les espaceurs d'en-tête suivent : `.sift-lib-thead-cov` mesure
 * la pochette seule, `.sift-lib-thead-tail` la pastille seule. */
export function libraryTableRowHtml(t: LibraryTrack, curId: number | null, selected = false): string {
  const cur = (t.id === curId ? " cur" : "") + (selected ? " sel" : "");
  const cov = t.cover_path
    ? `<img src="${esc(convertFileSrc(t.cover_path))}" alt="" class="sift-lib-cov">`
    : `<i class="ti ti-vinyl sift-lib-cov-fallback"></i>`;
  // Plus d'icône Discogs / loupe en fin de ligne depuis le 2026-09-08 (audit #24) : DESIGN.md § 16,
  // « chaque action secondaire est un bouton dans la ligne… mange de la largeur sur 15 k lignes pour
  // servir sur une ». Les deux actions vivent au clic droit et dans l'inspecteur.
  // Composite name so a screen reader announces the sortable columns for this row instead of
  // just "button" — role="button" alone loses the artist/title/genre/year association a table
  // reading mode would otherwise give. Le verdict n'ouvre plus la phrase depuis le 2026-09-08 : il
  // n'est plus à l'écran, et annoncer ce qu'on ne montre pas serait un second écran pour l'oreille.
  const rowLabel = `${t.artist || "Artiste inconnu"} — ${t.title || "Titre inconnu"}, ${fmtDuration(t.duration)}, ${t.genres[0] || "genre inconnu"}, ${t.year != null ? t.year : "année inconnue"}`;
  return (
    `<div class="lr${cur}" data-bib="row" data-id="${t.id}" tabindex="0" role="option" aria-selected="${selected}" aria-label="${esc(rowLabel)}">` +
    // La POCHETTE est le bouton de lecture (patron Musique, 2026-09-08) : au repos la ligne ne montre
    // que la donnée, le triangle apparaît au survol de la ligne, par-dessus la vignette. Un objet de
    // moins en tête de ligne (22 px + gap), et le geste reste là où l'œil cherche « écouter ».
    `<button class="pb" data-bib="play" data-id="${t.id}" aria-label="Écouter">${cov}<i class="ti ti-player-play sift-lib-play" aria-hidden="true"></i></button>` +
    libraryColumns().map((col) => cellHtml(col, t)).join("") +
    qualPill(t) +
    `</div>`
  );
}

export const LIBRARY_TABLE_PROBE_HTML =
  `<div class="lr"><button class="pb"><i class="ti ti-vinyl sift-lib-cov-fallback"></i><i class="ti ti-player-play sift-lib-play" aria-hidden="true"></i></button>` +
  `<span class="sift-lib-col">probe</span></div>`;

/** How many tiles sit in one virtualized "row" — the grid is chunked into rows of this many
 * tiles so createVirtualList (one fixed-height row at a time) can still window a cover grid
 * without rendering thousands of DOM nodes at once (see docs/design-system-states.md, the
 * 7000+-track freeze this codebase already hit once with an unvirtualized queue). */
export const LIBRARY_GRID_TILES_PER_ROW = 4;

function libraryGridTileHtml(t: LibraryTrack, curId: number | null): string {
  const cur = t.id === curId ? " cur" : "";
  const cov = t.cover_path
    ? `<img src="${esc(convertFileSrc(t.cover_path))}" alt="" class="sift-lib-tile-cov">`
    : `<i class="ti ti-vinyl sift-lib-tile-cov-fallback"></i>`;
  // Composite name, same reason as libraryTableRowHtml's rowLabel: role="button" alone announces
  // just "button" and the two text lines the tile paints are lost. One rule throughout: the name
  // says what the tile actually paints, nothing more. Hence NOT the row's 4-column label (a tile
  // shows no genre/year, naming them would announce what the sighted user can't see there); hence
  // also no "Artiste inconnu" segment — when t.artist is empty the .sift-lib-tile-sub line below
  // is painted EMPTY, unlike libraryTableRowHtml which paints an explicit "—" that "Artiste
  // inconnu" honestly stands for. The title slot reuses the tile's own bibName() fallback so an
  // untitled track is named by what's actually painted rather than by "Titre inconnu".
  const tileLabel = t.artist ? `${t.artist} — ${t.title || bibName(t)}` : t.title || bibName(t);
  return (
    `<div class="sift-lib-tile${cur}" data-bib="tile" data-id="${t.id}" tabindex="0" role="button" aria-label="${esc(tileLabel)}">` +
    cov +
    `<div class="sift-lib-tile-title">${esc(t.title || bibName(t))}</div>` +
    `<div class="sift-lib-tile-sub">${esc(t.artist || "")}</div>` +
    `</div>`
  );
}

/** One virtualized grid "row" = up to LIBRARY_GRID_TILES_PER_ROW tiles side by side. */
export function libraryGridRowHtml(rowTracks: readonly LibraryTrack[]): string {
  return `<div class="sift-lib-grid-row">${rowTracks.map((t) => libraryGridTileHtml(t, null)).join("")}</div>`;
}

export const LIBRARY_GRID_PROBE_HTML =
  `<div class="sift-lib-grid-row"><div class="sift-lib-tile"><i class="ti ti-vinyl sift-lib-tile-cov-fallback"></i><div class="sift-lib-tile-title">probe</div><div class="sift-lib-tile-sub">probe</div></div></div>`;
