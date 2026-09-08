// Écartés — deux destinations du rail, « À re-sourcer » et « Corbeille » (Tauri only).
//
// Refonte du 2026-09-08 (déclinaison #24, « ok pour tout » d'Antoine sur la maquette faite dans la
// vraie fenêtre, artefact « Écartés à la manière d'Apple ») : l'écran unique « Écartés » — deux
// cartes empilées, pastilles de compte et « Purger » dans la zone C, lignes à styles inline avec
// deux boutons icône, « Copier » et six liens boutique au survol — devient ce qu'Apple fait d'une
// corbeille : une DESTINATION de la sidebar par statut (Finder, Photos « Recently Deleted », Mail
// « Indésirables » / « Corbeille », Notes « Suppressions récentes »), chacune une table simple dans
// la grammaire de Rangés, ce qui pilote dans la barre, ce qui décrit dans l'inspecteur, les actions
// au clic droit.
//
// Ce que DESIGN.md § 15 a mesuré le 2026-08-19 tient : `EcarteItem` porte 8 champs (ni pochette, ni
// durée, ni genre), donc PAS la table de Rangés — la même grammaire de ligne (`.lr`, en-tête figé,
// pastille + libellé), quatre colonnes qui existent : Raison · Artiste · Titre · Fichier. Les sept
// affordances restent : requeue, restore, trash, purge (barre), copy-query et store (inspecteur),
// retry (chargement). Les six boutiques sont des RECHERCHES (`search?q=`), pas une disponibilité :
// les API boutiques ont été essayées et écartées (« une galère », Antoine, 2026-09-08). Elles
// vivent dans l'inspecteur, fiche « Racheter », une rangée par boutique — c'est là qu'on voit
// « dans quelles boutiques » aller, le point de l'écran.
import { listEcartes, revealTrack } from "./ipc";
import type { EcarteItem } from "../shared/contracts";
import { requireEl, esc } from "./dom";
import { emptyStateHtml, wireEmptyState } from "./empty-state";
import { createVirtualList, type VirtualList } from "./list-virtual";
import { viewEpoch, isStaleViewRender } from "./view-epoch";
import { openAside, closeAside, mountBarActions } from "./toolbar";
import { openContextMenu } from "./context-menu";
import { copyToClipboard, toast } from "./filing-toast";
import { humanizeError } from "./errors";

export type EcartesKind = "resourcing" | "trash";

// État de l'écran : le statut affiché, le tri, la piste ouverte dans l'inspecteur. Le statut est
// mémorisé pour que les actions (sift-live.ts, `.then(() => renderEcartes())`) repeignent la
// bonne destination sans le connaître.
let kind: EcartesKind = "resourcing";
type SortField = "artist" | "title" | "file";
let sort: { field: SortField; dir: "asc" | "desc" } = { field: "artist", dir: "asc" };
let openId: number | null = null;
let virtual: VirtualList | null = null;
let currentItems: EcarteItem[] = [];

// ---------------------------------------------------------------------------
// Boutiques — des recherches, une par boutique (voir l'en-tête du fichier).
// ---------------------------------------------------------------------------

function ecQuery(it: EcarteItem): string {
  if (it.artist && it.title) return `${it.artist} ${it.title}`;
  return (it.filename || it.path).replace(/\.[^.]+$/, "");
}

const EC_STORES: [string, (q: string) => string][] = [
  ["Beatport", (q) => `https://www.beatport.com/search?q=${q}`],
  ["Traxsource", (q) => `https://www.traxsource.com/search?term=${q}`],
  ["Juno", (q) => `https://www.junodownload.com/search/?q%5Ball%5D%5B%5D=${q}`],
  ["Bandcamp", (q) => `https://bandcamp.com/search?q=${q}`],
  ["Amazon", (q) => `https://www.amazon.fr/s?k=${q}&i=digital-music`],
  ["Apple Music", (q) => `https://music.apple.com/fr/search?term=${q}`],
];

// ---------------------------------------------------------------------------
// Raison — le signal catégoriel, une seule forme (pastille + libellé, DESIGN.md § 16)
// ---------------------------------------------------------------------------

interface ReasonView {
  cls: string;
  label: string;
  /** Une phrase pour l'inspecteur. Factuelle : `EcarteItem` ne porte ni mesure ni date, la phrase
   *  ne dit que ce que ses deux champs (`verdict`, `truncated`) attestent. */
  sentence: string;
}

function reasonView(it: EcarteItem): ReasonView {
  if (it.truncated)
    return { cls: "sift-lib-v-check", label: "TRONQUÉ", sentence: "Fin de fichier tronquée : le fichier est incomplet." };
  if (it.verdict === "fake")
    return { cls: "sift-lib-v-fake", label: "FAKE", sentence: "Déclaré lossless, mesuré compressé — un faux lossless, écarté depuis Revue." };
  if (it.verdict === "grey")
    return { cls: "sift-lib-v-check", label: "À VÉRIFIER", sentence: "Douteux à l'analyse — à vérifier avant de le garder." };
  return {
    cls: "sift-lib-v-none",
    label: "—",
    sentence: it.status === "trash" ? "Envoyé à la corbeille depuis Revue." : "Écarté depuis Revue, sans verdict.",
  };
}

const ecFile = (it: EcarteItem) => it.filename || it.path.split(/[\\/]/).pop() || it.path;
const ecExt = (it: EcarteItem) => (ecFile(it).match(/\.([^.]+)$/)?.[1] ?? "?").toUpperCase();

// ---------------------------------------------------------------------------
// Table — la grammaire de Rangés (`.sift-lib-thead`, `.lr`), quatre colonnes qui existent
// ---------------------------------------------------------------------------

function sortItems(items: EcarteItem[]): EcarteItem[] {
  const mul = sort.dir === "asc" ? 1 : -1;
  const key = (it: EcarteItem) => (sort.field === "artist" ? it.artist : sort.field === "title" ? it.title : ecFile(it));
  return [...items].sort((a, b) => key(a).localeCompare(key(b)) * mul);
}

function headHtml(): string {
  const col = (field: SortField, label: string, cls: string) => {
    const active = sort.field === field;
    const arrow = active ? (sort.dir === "asc" ? " ▴" : " ▾") : "";
    const ariaSort = active ? (sort.dir === "asc" ? "ascending" : "descending") : "none";
    return `<span class="${cls} sift-lib-colhead" role="columnheader" aria-sort="${ariaSort}"><button data-ecsort="${field}">${label}${arrow}</button></span>`;
  };
  return (
    `<div class="sift-lib-thead" role="row">` +
    // Raison n'est pas triable : catégorielle, quatre valeurs — le tri d'une liste courte par
    // artiste ou fichier est ce qu'on cherche ici.
    `<span class="sift-lib-col-verdict sift-lib-colhead" role="columnheader">Raison</span>` +
    col("artist", "Artiste", "sift-lib-col-artist") +
    col("title", "Titre", "sift-lib-col-title") +
    col("file", "Fichier", "sift-lib-col-genre") +
    `<span class="sift-lib-thead-tail" aria-hidden="true"></span></div>`
  );
}

function rowHtml(it: EcarteItem): string {
  const r = reasonView(it);
  const cur = it.id === openId ? " cur" : "";
  const label = `${r.label}, ${it.artist || "Artiste inconnu"} — ${it.title || "Titre inconnu"}, ${ecFile(it)}`;
  return (
    `<div class="lr${cur}" data-ecrow="${it.id}" tabindex="0" role="option" aria-label="${esc(label)}">` +
    `<span class="sift-lib-col sift-lib-col-verdict ${r.cls}"><span class="sift-lib-verdict-dot" aria-hidden="true"></span>${r.label}</span>` +
    `<span class="sift-lib-col sift-lib-col-artist">${esc(it.artist || "—")}</span>` +
    `<span class="sift-lib-col sift-lib-col-title">${esc(it.title || "—")}</span>` +
    `<span class="sift-lib-col sift-lib-col-genre sift-ec-file">${esc(ecFile(it))}</span>` +
    `<span class="pill" style="flex:none">${esc(ecExt(it))}</span>` +
    `</div>`
  );
}

// ---------------------------------------------------------------------------
// Inspecteur — au repos, le résumé ; une piste ouverte, l'en-tête de Revue + trois fiches
// ---------------------------------------------------------------------------

function renderIdle(items: EcarteItem[]): void {
  const host = openAside();
  if (!host) return;
  const n = items.length;
  const counts = new Map<string, number>();
  for (const it of items) {
    const l = reasonView(it).label;
    counts.set(l, (counts.get(l) ?? 0) + 1);
  }
  host.innerHTML =
    `<div class="col-h">${kind === "trash" ? "Corbeille" : "À re-sourcer"}</div>` +
    `<div class="sift-sel-count">${n} piste${n > 1 ? "s" : ""}</div>` +
    `<dl class="sift-sel-rows">` +
    [...counts.entries()]
      .sort((a, b) => b[1] - a[1])
      .map(([k, c]) => `<dt>${esc(k === "—" ? "Sans verdict" : k)}</dt><dd>${c}</dd>`)
      .join("") +
    `</dl>` +
    `<div class="sift-ec-rule">${
      kind === "trash"
        ? "Les fichiers restent sur le disque jusqu'au vidage de la corbeille."
        : "Des pistes à racheter : le fichier est faux, tronqué ou douteux."
    }</div>`;
}

function renderDetail(it: EcarteItem): void {
  const host = openAside();
  if (!host) return;
  const r = reasonView(it);
  const q = encodeURIComponent(ecQuery(it));
  host.innerHTML =
    // En-tête de Revue, sans pochette ni lecteur : un fichier écarté ne s'écoute pas ici, et
    // `EcarteItem` n'a pas de pochette.
    `<div class="sift-player-header"><div class="sift-player-header-body">` +
    `<div class="sift-player-title-row">` +
    `<div class="sift-report-name sift-player-name">${esc(it.title || ecFile(it))}</div>` +
    `<div class="sift-player-verdict ${r.cls}"><span class="sift-player-verdict-dot" aria-hidden="true"></span><span class="sift-player-verdict-word">${r.label}</span><span class="sift-player-verdict-fmt">· ${esc(ecExt(it))}</span></div>` +
    `</div>` +
    `<div class="sift-report-sub sift-player-sub">${esc(it.artist || "")}</div>` +
    `<div class="sift-ec-file sift-ec-detail-file">${esc(ecFile(it))}</div>` +
    `</div></div>` +
    // Trois fiches au gabarit de « Métadonnées » (`.sift-meta-title`, direction T).
    `<div class="sift-meta-header sift-ec-fiche"><span class="sift-meta-title">Raison</span></div>` +
    `<div class="sift-ec-sentence">${esc(r.sentence)}</div>` +
    `<div class="sift-meta-header sift-ec-fiche"><span class="sift-meta-title">Racheter</span></div>` +
    // Une rangée par boutique — grammaire des rangées du Diagnostic en colonne (libellé, puis la
    // valeur) : le libellé est la boutique, la valeur est le geste. « Dans quelles boutiques »
    // se lit en une colonne, c'est le point de l'écran.
    `<div class="sift-ec-stores">` +
    EC_STORES.map(
      ([label, fn]) =>
        `<div class="sift-row"><span class="sift-row-label">${esc(label)}</span>` +
        `<button class="sift-meta-ident-btn" data-ec="store" data-url="${encodeURIComponent(fn(q))}">Ouvrir la recherche</button></div>`,
    ).join("") +
    `</div>` +
    `<div class="sift-meta-actions"><button class="sift-meta-ident-btn" data-ec="copy-query" data-q="${esc(ecQuery(it))}">Copier le nom</button></div>` +
    `<div class="sift-meta-header sift-ec-fiche"><span class="sift-meta-title">Actions</span></div>` +
    `<div class="sift-ec-actions">` +
    (kind === "trash"
      ? `<button class="sift-meta-ident-btn" data-ec="restore" data-id="${it.id}">Restaurer</button>`
      : `<button class="sift-meta-ident-btn" data-ec="requeue" data-id="${it.id}">Remettre en file</button>` +
        `<button class="sift-meta-ident-btn" data-ec="trash" data-id="${it.id}">Envoyer à la corbeille</button>`) +
    `</div>`;
}

function openDetail(id: number | null): void {
  openId = id;
  document.querySelectorAll<HTMLElement>(".lr[data-ecrow]").forEach((r) => r.classList.toggle("cur", Number(r.dataset.ecrow) === id));
  const it = id != null ? currentItems.find((x) => x.id === id) : undefined;
  if (it) renderDetail(it);
  else renderIdle(currentItems);
}

function openMenu(x: number, y: number, it: EcarteItem): void {
  const detailOpen = openId === it.id;
  openContextMenu(x, y, [
    {
      label: "Ouvrir l'emplacement",
      onPick: () =>
        void revealTrack(it.id).catch((err: unknown) => toast(humanizeError(err, "Impossible d'ouvrir l'emplacement", "reveal_track"))),
    },
    { label: detailOpen ? "Masquer le détail" : "Ouvrir le détail", onPick: () => openDetail(detailOpen ? null : it.id) },
    { label: "Copier le nom", separated: true, onPick: () => copyToClipboard(ecQuery(it), "Recherche copiée") },
    // Les six boutiques vivent dans l'inspecteur (une rangée chacune) : « Racheter… » y mène.
    // Pas de sous-menu — HIG Context menus : « aim for a small number of menu items ».
    { label: "Racheter…", onPick: () => openDetail(it.id) },
    ...(kind === "trash"
      ? [{ label: "Restaurer", separated: true, onPick: () => clickAction("restore", it.id) }]
      : [
          { label: "Remettre en file", separated: true, onPick: () => clickAction("requeue", it.id) },
          { label: "Envoyer à la corbeille", onPick: () => clickAction("trash", it.id) },
        ]),
  ]);
}

/** Les actions passent par le délégué `[data-ec]` de sift-live.ts (qui porte les IPC et leurs
 *  erreurs, une seule fois) : on lui envoie un clic sur un bouton fantôme plutôt que dupliquer
 *  ses trois `catch`. */
function clickAction(act: "requeue" | "trash" | "restore", id: number): void {
  const pa = document.getElementById("pa");
  if (!pa) return;
  const b = document.createElement("button");
  b.hidden = true;
  b.dataset.ec = act;
  b.dataset.id = String(id);
  pa.appendChild(b);
  b.click();
  b.remove();
}

// ---------------------------------------------------------------------------
// Rendu de l'écran
// ---------------------------------------------------------------------------

/** Peint la destination `k` (ou la dernière peinte). Rappelée par sift-live.ts après chaque
 *  action (requeue / trash / restore / purge) : `#content` se réécrit, l'inspecteur suit. */
export async function renderEcartes(k?: EcartesKind): Promise<void> {
  if (k) {
    if (k !== kind) openId = null;
    kind = k;
  }
  const content = requireEl("#content", "renderEcartes");
  const token = viewEpoch();
  virtual?.destroy();
  virtual = null;

  const alreadyRendered = !!content.querySelector(".sift-library-main, .sift-empty-state");
  if (!alreadyRendered) {
    // Squelette statique (DESIGN.md § 6) plutôt qu'un spinner nu.
    content.innerHTML = `<div class="sift-library-main"><span class="sift-skel sift-skel-line"></span></div>`;
  }

  let all: EcarteItem[] = [];
  try {
    all = await listEcartes();
    if (isStaleViewRender(token)) return;
  } catch (e) {
    console.error("listEcartes failed", e);
    if (isStaleViewRender(token)) return;
    content.innerHTML =
      '<div class="sift-library-main"><div class="sift-ec-fail">Impossible de charger cette liste. Vérifie la connexion à la base et réessaie. ' +
      '<button data-ec="retry" class="sift-meta-ident-btn">Réessayer</button></div></div>';
    content.querySelector<HTMLButtonElement>('[data-ec="retry"]')?.addEventListener("click", () => void renderEcartes());
    mountBarActions("");
    return;
  }
  currentItems = sortItems(all.filter((i) => i.status === kind));
  if (openId != null && !currentItems.some((i) => i.id === openId)) openId = null;

  // Barre : compte à côté du titre (le titre vient du rail, `router.ts::syncNav`) ; sur la
  // Corbeille, « Vider la corbeille » — la seule action de l'écran, en secondaire danger, avec la
  // confirmation in-app que le délégué de sift-live.ts porte déjà.
  const countEl = document.getElementById("sift-tb-count");
  if (countEl) countEl.textContent = `${currentItems.length} piste${currentItems.length > 1 ? "s" : ""}`;
  mountBarActions(
    kind === "trash" && currentItems.length
      ? `<button data-ec="purge" class="sift-secondary-trash sift-bar-btn">Vider la corbeille</button>`
      : "",
  );

  if (currentItems.length === 0) {
    content.innerHTML = emptyStateHtml(
      kind === "trash"
        ? { title: "La corbeille est vide", note: "Les pistes envoyées à la corbeille depuis Revue ou À re-sourcer attendent ici avant le vidage.", backToRevue: true }
        : { title: "Rien à re-sourcer", note: "Les pistes écartées depuis Revue — fausses, tronquées, douteuses — apparaissent ici, à racheter ou à remettre en file.", backToRevue: true },
    );
    wireEmptyState(content);
    closeAside();
    return;
  }

  content.innerHTML = `<div class="sift-library-main">${headHtml()}<div id="sift-ec-list" role="listbox" aria-label="${kind === "trash" ? "Corbeille" : "À re-sourcer"}"></div></div>`;
  const listHost = requireEl<HTMLElement>("#sift-ec-list", "renderEcartes", content);
  virtual = createVirtualList<EcarteItem>({
    host: listHost,
    scrollContainer: content,
    items: currentItems,
    rowHtml,
    probeHtml: rowHtml(currentItems[0]),
    fallbackRowH: 32,
  });
  wireTable(content);
  openDetail(openId);
}

function wireTable(content: HTMLElement): void {
  content.addEventListener("click", (e) => {
    const t = e.target as HTMLElement;
    const sortBtn = t.closest<HTMLElement>("[data-ecsort]");
    if (sortBtn) {
      const field = sortBtn.dataset.ecsort as SortField;
      sort = { field, dir: sort.field === field && sort.dir === "asc" ? "desc" : "asc" };
      void renderEcartes();
      return;
    }
    const row = t.closest<HTMLElement>("[data-ecrow]");
    if (row) {
      const id = Number(row.dataset.ecrow);
      openDetail(openId === id ? null : id);
    }
  });
  content.addEventListener("contextmenu", (e) => {
    const row = (e.target as HTMLElement).closest<HTMLElement>("[data-ecrow]");
    if (!row) return;
    e.preventDefault();
    const it = currentItems.find((x) => x.id === Number(row.dataset.ecrow));
    if (it) openMenu(e.clientX, e.clientY, it);
  });
  content.addEventListener("keydown", (e) => {
    const row = (e.target as HTMLElement).closest<HTMLElement>("[data-ecrow]");
    if (!row) return;
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      const id = Number(row.dataset.ecrow);
      openDetail(openId === id ? null : id);
    }
  });
}
