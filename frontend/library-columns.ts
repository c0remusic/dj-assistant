// Colonnes de la table Bibliothèque : ordre, largeurs, et les deux gestes qui les changent.
//
// `DESIGN.md` § 16 demande les deux depuis le début — « largeurs redimensionnables au glisser sur
// le séparateur d'en-tête, mémorisées » et « colonnes réordonnables au glisser, ordre mémorisé » —
// et aucun des deux n'existait : `SORT_COLUMNS` était une liste fixe de six entrées et les largeurs
// vivaient dans des règles CSS (`.sift-lib-col-*`). Vérifié le 2026-08-19, `col-resize` n'apparaissait
// que sur la poignée de la file de Revue.
//
// Source Apple pour le redimensionnement, § macOS de « Lists and tables » (HIG, lue le 2026-08-19) :
// « Let people resize columns. Data displayed in a table view often varies in width. » Le
// réordonnancement, lui, n'a PAS de source dans cette page — elle ne parle que de réordonner des
// LIGNES. Il vient donc de `DESIGN.md` § 16, précédence 1, et se marque comme tel.
//
// ⚠️ La même page conseille aussi « Consider using alternating row colors in a multicolumn table ».
// `DESIGN.md` § 16 l'interdit — « la séparation vient de l'espace, pas d'un trait ni d'un zébrage ».
// Le conflit est tranché par la précédence (1 avant 3) et l'argument d'Apple ne mord pas ici : il
// vise une table LARGE, et celle de Sift vit dans une zone centrale bornée par un rail et un
// inspecteur. Aucun zébrage n'est introduit.
//
// Persistance en `localStorage`, décision tranchée dans `docs/ui-specs/bibliotheque.md` : une largeur
// de colonne est un état d'affichage de la fenêtre, que le backend ne lit jamais — même catégorie que
// le repli du rail. L'argument « `settings` survit à un changement de machine » qui avait ouvert la
// question était faux : la base vit dans `app_data_dir()` (`src-tauri/src/lib.rs:222`).

// `verdict` et `bpm` sont SORTIS de la liste le 2026-09-08 (audit Rangés, #24, décision d'Antoine :
// « pas besoin de mettre le verdict ») — le verdict se lit dans l'inspecteur à l'ouverture, et BPM
// n'a aucun écrivain côté Rust (colonne `metadata.bpm` lue, jamais remplie : « aucune colonne
// fantôme pour du vide », DESIGN.md § 16). Un stockage qui les porte encore est filtré au chargement.
export type LibraryColumnField = "artist" | "title" | "duration" | "genre" | "year";

export interface LibraryColumn {
  field: LibraryColumnField;
  label: string;
  /** Classe de largeur par défaut, celle que `styles.css` porte encore quand rien n'est mémorisé. */
  cls: string;
  /** Largeur mémorisée, en px. `undefined` = la colonne suit encore sa règle CSS. */
  width?: number;
}

/** Ordre et libellés d'origine — `DESIGN.md` § 16. Le tableau sert aussi de validation : une entrée
 *  mémorisée qui ne s'y trouve pas est jetée au chargement. */
const DEFAULT_COLUMNS: readonly LibraryColumn[] = [
  // La colonne Verdict (colonne 1 de `DESIGN.md` § 16, entrée le 2026-08-19 dans le système de
  // colonnes pour être triable et déplaçable comme les autres) est retirée le 2026-09-08 — voir
  // le commentaire de `LibraryColumnField`. BPM part le même jour, pour n'avoir jamais été écrit.
  { field: "artist", label: "Artiste", cls: "sift-lib-col-artist" },
  { field: "title", label: "Titre", cls: "sift-lib-col-title" },
  { field: "duration", label: "Durée", cls: "sift-lib-col-num" },
  { field: "genre", label: "Genre", cls: "sift-lib-col-genre" },
  { field: "year", label: "Année", cls: "sift-lib-col-year" },
];

/** Bornes du redimensionnement. Le plancher n'est pas cosmétique : sous 48px un en-tête de colonne
 *  n'affiche plus son libellé ni sa flèche de tri, et la colonne devient impossible à réélargir
 *  puisque son propre séparateur n'a plus de prise.
 *
 *  ⚠️ `MIN_COL_W` **mire** le token `--col-min-w` de `styles.css`, il ne le remplace pas — même
 *  discipline que `QCOL_DEFAULT` face à `--pane-w`. Les deux sont nécessaires et ne font pas le
 *  même travail : le CSS empêche une colonne VOISINE de s'écraser quand une autre s'élargit (c'est
 *  le navigateur qui tient l'invariant), le JS borne la colonne qu'on DRAGUE. Éditer l'un sans
 *  l'autre laisse les deux planchers désaccordés. */
const MIN_COL_W = 48;
const MAX_COL_W = 600;

const STORE_KEY = "sift-libcols-v1";

interface StoredLayout {
  order?: string[];
  width?: Record<string, number>;
}

/** État vivant, reconstruit une fois au chargement du module puis muté par les deux gestes.
 *  Mémorisé plutôt que recalculé à chaque appel : `libraryTableRowHtml` le lit une fois par ligne
 *  visible, à chaque tick de la liste virtualisée. */
let columns: LibraryColumn[] = load();

function load(): LibraryColumn[] {
  let stored: StoredLayout = {};
  try {
    stored = JSON.parse(localStorage.getItem(STORE_KEY) ?? "{}") as StoredLayout;
  } catch {
    // Stockage refusé ou JSON abîmé : les défauts suffisent. Une disposition de colonnes perdue
    // n'est pas une erreur à remonter — c'est la disposition d'origine.
    stored = {};
  }
  const byField = new Map(DEFAULT_COLUMNS.map((c) => [c.field, c]));
  const out: LibraryColumn[] = [];
  // L'ordre mémorisé est FILTRÉ contre les défauts, jamais adopté tel quel : une entrée inconnue
  // (colonne supprimée d'une version à l'autre) peindrait une cellule vide dans chaque ligne, et une
  // entrée manquante ferait disparaître une donnée sans que rien ne le dise.
  for (const f of stored.order ?? []) {
    const c = byField.get(f as LibraryColumnField);
    if (c && !out.some((o) => o.field === c.field)) out.push({ ...c });
  }
  // Une colonne absente du stockage est une colonne NOUVELLE — un ordre partiel n'arrive jamais
  // par geste (moveColumn persiste l'ordre complet), seulement par un stockage écrit avant qu'elle
  // existe. L'utilisateur n'a donc jamais exprimé de préférence à son sujet : elle prend son index
  // PAR DÉFAUT (plafonné), pas la fin de liste. Sans ça, Verdict — colonne 1 de DESIGN.md § 16 —
  // se peignait en dernière position sur toute disposition mémorisée d'avant le 2026-08-19, et la
  // seule sortie était « Réinitialiser les colonnes », au prix des largeurs.
  // `i` est passé nu : `splice` borne déjà son `start` à la longueur du tableau, donc le
  // `Math.min(i, out.length)` qui était écrit ici ne pouvait rien corriger — une garde inerte, qui
  // laissait croire qu'un dépassement était possible et méritait d'être surveillé.
  DEFAULT_COLUMNS.forEach((c, i) => {
    if (!out.some((o) => o.field === c.field)) out.splice(i, 0, { ...c });
  });
  for (const c of out) {
    const w = stored.width?.[c.field];
    if (typeof w === "number" && Number.isFinite(w)) c.width = clampWidth(w);
  }
  return out;
}

function persist(): void {
  const width: Record<string, number> = {};
  for (const c of columns) if (c.width != null) width[c.field] = c.width;
  try {
    localStorage.setItem(STORE_KEY, JSON.stringify({ order: columns.map((c) => c.field), width }));
  } catch {
    // Même raison que `installQueueResize` (router.ts) : un stockage refusé ne casse pas le geste,
    // il l'empêche seulement de survivre à la session.
  }
}

function clampWidth(px: number): number {
  return Math.max(MIN_COL_W, Math.min(MAX_COL_W, Math.round(px)));
}

/** Les colonnes dans leur ordre courant. Le tableau rendu est le tableau vivant — ne pas le muter
 *  depuis l'extérieur ; passer par `setColumnWidth` / `moveColumn`, qui persistent. */
export function libraryColumns(): readonly LibraryColumn[] {
  return columns;
}

/** Style inline d'une cellule ou d'un en-tête. Vide tant que la colonne n'a pas été redimensionnée :
 *  sans largeur mémorisée, la table garde les proportions de `styles.css` et continue de s'adapter à
 *  la largeur de la zone. Dès qu'une colonne est draguée, elle se FIGE en px — c'est le sens du
 *  geste, et c'est ce que fait une table macOS : on élargit une colonne pour qu'elle reste large. */
export function columnStyle(col: LibraryColumn): string {
  return col.width == null ? "" : ` style="flex:none;width:${col.width}px"`;
}

export function setColumnWidth(field: LibraryColumnField, px: number): void {
  const c = columns.find((x) => x.field === field);
  if (!c) return;
  c.width = clampWidth(px);
  persist();
}

/** Déplace la colonne `field` devant `beforeField` (ou en fin si `null`). */
export function moveColumn(field: LibraryColumnField, beforeField: LibraryColumnField | null): void {
  const from = columns.findIndex((c) => c.field === field);
  if (from < 0) return;
  const [moved] = columns.splice(from, 1);
  const to = beforeField == null ? columns.length : columns.findIndex((c) => c.field === beforeField);
  columns.splice(to < 0 ? columns.length : to, 0, moved);
  persist();
}

/** Rétablit ordre et largeurs d'origine. Porte de sortie obligatoire : une colonne réduite à son
 *  plancher, ou déplacée par erreur pendant un clic de tri, doit pouvoir être défaite sans aller
 *  chercher un stockage navigateur. */
export function resetColumns(): void {
  columns = DEFAULT_COLUMNS.map((c) => ({ ...c }));
  try {
    localStorage.removeItem(STORE_KEY);
  } catch {
    // Idem : rien à signaler, l'état en mémoire est déjà revenu au défaut.
  }
}

/** Vrai si l'utilisateur a touché à la disposition — sert à n'offrir « Réinitialiser » que quand il
 *  y a quelque chose à réinitialiser. */
export function columnsAreCustomized(): boolean {
  return (
    columns.some((c) => c.width != null) ||
    columns.some((c, i) => c.field !== DEFAULT_COLUMNS[i].field)
  );
}

/** Marqueur posé sur le dernier drag d'en-tête, lu par le dispatch de clic pour savoir qu'un
 *  `click` qui suit un déplacement ne doit PAS trier. Un en-tête est à la fois un bouton de tri et
 *  une poignée de déplacement ; sans ce garde, tout réordonnancement trierait aussi la table. */
let suppressNextSort = false;

export function consumeSortSuppression(): boolean {
  const v = suppressNextSort;
  suppressNextSort = false;
  return v;
}

/** Distance en px au-delà de laquelle un mousedown sur un en-tête cesse d'être un clic de tri et
 *  devient un déplacement de colonne. En dessous, le geste reste un clic — c'est ce qui permet de
 *  garder les deux sur le même élément. */
const DRAG_THRESHOLD = 5;

/** Câble les deux gestes sur une ligne d'en-tête fraîchement rendue.
 *
 *  `onChange` est appelé une fois le geste FINI, jamais pendant : le redimensionnement écrit
 *  directement dans le style des nœuds montés (pas de rendu par frame — la liste est virtualisée et
 *  un rebuild par mouvement de souris repeindrait des centaines de lignes), et seul le relâchement
 *  demande à l'écran de se refaire pour que les lignes hors fenêtre adoptent la nouvelle largeur. */
export function installColumnGestures(thead: HTMLElement, onChange: () => void): void {
  thead.addEventListener("mousedown", (e: MouseEvent) => {
    const sep = (e.target as HTMLElement).closest<HTMLElement>(".sift-lib-colsep");
    if (sep) {
      startResize(e, sep, onChange);
      return;
    }
    const head = (e.target as HTMLElement).closest<HTMLElement>("[data-colhead]");
    if (head) startReorder(e, thead, head, onChange);
  });
}

function startResize(e: MouseEvent, sep: HTMLElement, onChange: () => void): void {
  const field = sep.dataset.for as LibraryColumnField | undefined;
  if (!field) return;
  e.preventDefault();
  e.stopPropagation();
  const headCell = sep.closest<HTMLElement>("[data-colhead]");
  if (!headCell) return;
  const startX = e.clientX;
  const startW = headCell.getBoundingClientRect().width;
  // Plafond DYNAMIQUE, et il compte plus que `MAX_COL_W` : le plancher de 48px protège la colonne
  // qu'on drague, pas ses voisines. Mesuré le 2026-08-19 dans la vraie fenêtre — Artiste figée à
  // 316px dans une zone qui n'en offrait pas tant écrasait Titre à un caractère, sans que rien ne
  // s'y oppose. Ce que la colonne peut prendre est exactement ce que les AUTRES peuvent céder,
  // chacune jusqu'à son propre plancher. Aucune colonne ne s'écrase, et rien ne déborde de la zone.
  let slack = 0;
  for (const other of document.querySelectorAll<HTMLElement>("[data-colhead]")) {
    if (other === headCell) continue;
    // Une colonne qui ne rétrécit pas ne cède RIEN, et la compter donnerait un plafond trop haut :
    // BPM, Durée et Année sont `flex:none` en CSS, une colonne déjà figée par un drag l'est aussi.
    // Sans ce filtre le plafond dépassait de 12px — trois colonnes fixes comptées pour 4px chacune.
    if (getComputedStyle(other).flexShrink === "0") continue;
    slack += Math.max(0, other.getBoundingClientRect().width - MIN_COL_W);
  }
  const maxHere = Math.min(MAX_COL_W, startW + slack);
  sep.classList.add("sift-lib-colsep--active");
  document.body.classList.add("sift-col-resizing");
  let last = startW;
  const onMove = (ev: MouseEvent) => {
    last = Math.min(maxHere, clampWidth(startW + (ev.clientX - startX)));
    // Écriture directe sur les nœuds montés — en-tête ET cellules de la colonne — plutôt qu'un
    // re-rendu : c'est la règle « créer une fois, muter ensuite » appliquée à un geste continu.
    paintColumnWidth(field, last);
  };
  const onUp = () => {
    document.removeEventListener("mousemove", onMove);
    document.removeEventListener("mouseup", onUp);
    sep.classList.remove("sift-lib-colsep--active");
    document.body.classList.remove("sift-col-resizing");
    setColumnWidth(field, last);
    onChange();
  };
  document.addEventListener("mousemove", onMove);
  document.addEventListener("mouseup", onUp);
}

/** Applique une largeur à l'en-tête et à toutes les cellules montées de la colonne. Les lignes hors
 *  fenêtre virtualisée ne sont pas dans le DOM ; elles prennent la largeur à leur prochain montage,
 *  depuis `columnStyle`. */
function paintColumnWidth(field: LibraryColumnField, px: number): void {
  for (const el of document.querySelectorAll<HTMLElement>(`[data-col="${field}"]`)) {
    el.style.flex = "none";
    el.style.width = `${px}px`;
  }
}

function startReorder(e: MouseEvent, thead: HTMLElement, head: HTMLElement, onChange: () => void): void {
  const field = head.dataset.colhead as LibraryColumnField | undefined;
  if (!field) return;
  const startX = e.clientX;
  let dragging = false;
  let target: LibraryColumnField | null = null;

  const clearMarks = () => {
    for (const el of thead.querySelectorAll(".sift-lib-colhead--dropbefore, .sift-lib-colhead--dropafter"))
      el.classList.remove("sift-lib-colhead--dropbefore", "sift-lib-colhead--dropafter");
  };

  const onMove = (ev: MouseEvent) => {
    if (!dragging) {
      if (Math.abs(ev.clientX - startX) < DRAG_THRESHOLD) return;
      dragging = true;
      head.classList.add("sift-lib-colhead--dragging");
      document.body.classList.add("sift-col-resizing");
    }
    clearMarks();
    // Cible = l'en-tête sous le pointeur. La moitié survolée décide du côté : gauche → insertion
    // avant, droite → après. Sans ce partage, déposer sur la dernière colonne serait impossible.
    const over = document
      .elementFromPoint(ev.clientX, ev.clientY)
      ?.closest<HTMLElement>("[data-colhead]");
    if (!over || over === head) {
      target = null;
      return;
    }
    const r = over.getBoundingClientRect();
    const before = ev.clientX < r.left + r.width / 2;
    over.classList.add(before ? "sift-lib-colhead--dropbefore" : "sift-lib-colhead--dropafter");
    const overField = over.dataset.colhead as LibraryColumnField;
    if (before) {
      target = overField;
    } else {
      const idx = columns.findIndex((c) => c.field === overField);
      target = columns[idx + 1]?.field ?? null;
    }
  };

  const onUp = () => {
    document.removeEventListener("mousemove", onMove);
    document.removeEventListener("mouseup", onUp);
    head.classList.remove("sift-lib-colhead--dragging");
    document.body.classList.remove("sift-col-resizing");
    clearMarks();
    if (!dragging) return; // simple clic : c'est un tri, on ne touche à rien
    // Le `click` qui suit ce `mouseup` part vers le bouton de tri. Il est neutralisé par le drapeau
    // plutôt que par un `preventDefault` : le clic n'est pas encore émis ici, et le seul endroit qui
    // peut le refuser est celui qui le reçoit (`sift-live.ts`).
    suppressNextSort = true;
    if (target !== field) moveColumn(field, target);
    onChange();
  };

  document.addEventListener("mousemove", onMove);
  document.addEventListener("mouseup", onUp);
}
