// Real view router (Tauri only) — étape 1 de DESIGN.md § 17.
//
// Jusqu'ici c'était `frontend/app.js`, la maquette d'origine, qui routait l'app RÉELLE :
// `main.ts` l'importait sans condition, elle tenait l'état de vue, le clic sur `[data-view]`,
// le titre accessible, les deux coquilles (Revue, Accueil) et le redimensionnement de la file —
// et appelait les vues live par huit globales `window.__sift*`. Six de ses sept renderers de
// démo étaient neutralisés par un garde `!('__TAURI_INTERNALS__' in window)` répété six fois,
// et le septième (`renderBatch`) était mort, masqué par une règle CSS de `chrome.ts`.
//
// Ce module reprend EXACTEMENT ce que la maquette fournissait au chemin de production, et rien
// d'autre : mêmes coquilles, mêmes identifiants, mêmes bornes de redimensionnement, même clé de
// stockage. Aucun changement visible — c'est un préalable, pas une refonte. Les étapes 2 et 3
// (barre unifiée, shell à trois zones) construisent dessus.
//
// `app.js` n'est plus chargée que hors Tauri (`main.ts`), où elle redevient ce qu'elle est : une
// démo navigateur. Ses gardes `inTauri` y sont désormais toujours faux, donc elle rend enfin sa
// maquette complète au lieu de coquilles vides, et ses appels `window.__sift*` ne trouvent rien —
// ce qui est correct : le wiring live n'existe pas dans un navigateur.
import { requireEl } from "./dom";
import { closeAside } from "./toolbar";
import { renderQueue } from "./queue-panel";
import { renderEcartes } from "./ecartes-view";
import { renderReglagesLive } from "./reglages-view";
import { renderBiblioLive } from "./bibliotheque-view";
import { paintJournal } from "./journal";
import { renderRekordboxLive } from "./rekordbox-view";
import { renderUsbLive } from "./usb-view";
import { bumpViewEpoch } from "./view-epoch";

export type ViewId = "revue" | "ecarts" | "journal" | "biblio" | "rkb" | "cle" | "reglages";

const VIEWS: readonly ViewId[] = ["revue", "ecarts", "journal", "biblio", "rkb", "cle", "reglages"];

function isViewId(v: string | undefined): v is ViewId {
  return !!v && (VIEWS as readonly string[]).includes(v);
}

/** Revue au démarrage, et non plus Accueil : Accueil a fusionné dans le rail (DESIGN.md § 15,
 *  fusion 1). L'app s'ouvre donc sur son poste de travail plutôt que sur un inventaire. */
let currentView: ViewId = "revue";

/** La vue affichée. Lue par le futur titre de barre unifiée (étape 2) — et par les tests. */
export function activeView(): ViewId {
  return currentView;
}

// ---------------------------------------------------------------------------
// Largeur de la colonne file (#qcol, Revue)
//
// Redimensionnée à la souris, persistée entre rendus ET entre sessions : la coquille de Revue est
// reconstruite à chaque passage sur l'écran, donc la largeur doit être relue depuis le stockage à
// chaque montage, jamais gardée dans une variable vivante. Bornes et clé identiques à celles de
// la maquette — les changer invaliderait la largeur déjà enregistrée chez l'utilisateur.
// ---------------------------------------------------------------------------
const QCOL_MIN = 220;
const QCOL_MAX = 480;
const QCOL_DEFAULT = 272; // miroir de --pane-w (DESIGN.md § 10, D-2)
const QCOL_KEY = "sift-qcol-w";

function qcolWidth(): number {
  try {
    const v = parseInt(localStorage.getItem(QCOL_KEY) ?? "", 10);
    if (v >= QCOL_MIN && v <= QCOL_MAX) return v;
  } catch {
    // localStorage indisponible (mode privé, quota) : le défaut suffit, ce n'est pas une erreur.
  }
  return QCOL_DEFAULT;
}

function installQueueResize(qcol: HTMLElement, handle: HTMLElement): void {
  handle.addEventListener("mousedown", (e: MouseEvent) => {
    e.preventDefault();
    const startX = e.clientX;
    const startW = qcol.getBoundingClientRect().width;
    handle.classList.add("sift-qresize--active");
    const onMove = (ev: MouseEvent) => {
      const w = Math.max(QCOL_MIN, Math.min(QCOL_MAX, startW + (ev.clientX - startX)));
      qcol.style.width = `${w}px`;
    };
    const onUp = () => {
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
      handle.classList.remove("sift-qresize--active");
      try {
        localStorage.setItem(QCOL_KEY, String(parseInt(qcol.style.width, 10)));
      } catch {
        // Même raison que ci-dessus : ne pas casser un redimensionnement pour un stockage refusé.
      }
    };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
  });
}

// ---------------------------------------------------------------------------
// Coquilles
//
// Chacune ne pose que la structure que la vue live exige, puis lui rend la main. Les identifiants
// sont contractuels : `queue-panel.ts` exige `#qcol`, `filing*.ts` exigent `#mid`, `#filfoot` et
// `#fldz`. Les changer casse un
// `requireEl` au montage — fail-fast, mais au montage seulement.
//
// `.h1` est également contractuel : `reglages-view.ts:36` et `usb-view.ts:35` parcourent les
// enfants de `#content`, gardent le premier `.h1` et masquent le reste.
// ---------------------------------------------------------------------------

/** Zone C vide, pour un écran qui n'a qu'elle.
 *
 *  Ne pose plus ni `display` ni `overflow` depuis l'étape 3 : `#content` EST la zone C, et son
 *  comportement vit dans la feuille de style, posé une fois. Un écran qui le redécidait à chaque
 *  rendu est exactement ce qui produisait deux grammaires de layout pour huit écrans.
 *
 *  Ne pose plus de `.h1` depuis l'étape 2 : le titre a quitté le contenu pour la barre unifiée.
 *  `reglages-view.ts` et `usb-view.ts` parcourent encore les enfants de `#content` pour garder un
 *  `.h1` et masquer le reste — avec zéro enfant leur boucle ne fait rien, ce qui est voulu. */
function blockShell(content: HTMLElement): void {
  content.style.display = "";
  content.style.flexDirection = "";
  content.style.overflowY = "";
  content.innerHTML = "";
}


/** Revue pose ses propres colonnes DANS la zone C, donc elle lui prend son défilement :
 *  `overflow:hidden` ici, et chaque colonne défile chez elle. C'est la règle générale — la page
 *  ne défile jamais — appliquée un cran plus bas.
 *
 *  #qcol ne porte ICI que ses deux ancres : l'en-tête de filtre `.sift-qhead` et la liste `#ql`.
 *  Le champ de recherche (au-DESSUS de l'en-tête) et les bascules de pied (sous `#ql`) sont
 *  injectés par `queue-panel.ts`, qui les range d'après `QCOL_ORDER` — c'est là, et nulle part
 *  ailleurs, que l'ordre de la colonne est déclaré. Cette fonction reconstruit #qcol EN ENTIER à
 *  chaque navigation vers Revue, donc les blocs injectés disparaissent avec lui et sont reposés
 *  au rendu suivant : ne pas supposer qu'un nœud de la colonne survit à un aller-retour.
 *
 *  Le segmenté Détail / Lot a été retiré de cette colonne le 2026-08-25 (spec
 *  `docs/ui-specs/revue.md` §§ Zone A / Zone B′) : le mode Lot s'arme par l'icône de sélection de
 *  la barre unifiée, pas par un onglet de la file.
 *
 *  `.sift-qhead` n'est plus un TITRE mais la RANGÉE DE FILTRE (wireframe « Poste de décision »
 *  §§ 09-10, 2026-08-25) : le libellé « File » est retiré — la barre unifiée nomme déjà l'écran.
 *  Elle est posée VIDE : `queue-panel.ts` y injecte le pulldown de filtre à gauche et le bouton de
 *  sélection à droite, tous deux avec leur état. Le compte de pistes visibles qui occupait la
 *  droite est retiré le 2026-08-26, et le compte de la FILE vit désormais dans la barre unifiée,
 *  contre le titre (`paintBarCount`, queue-panel.ts) — voir son commentaire pour le motif Mail.
 *
 *  `#ql` est une LISTBOX (`role` + `tabindex="0"`), et c'est le SEUL point d'entrée du clavier dans
 *  la file : les lignes restent non focusables, la ligne courante se nomme par
 *  `aria-activedescendant` (patron listbox de l'APG, posé par `renderQueueWindow`). Raison
 *  structurelle et non stylistique — la liste est virtualisée, un `tabindex` par ligne ferait
 *  tomber le focus au premier repeint qui démonte la ligne focalisée (toutes les 300 ms pendant une
 *  analyse) ; `#ql`, lui, n'est jamais remplacé, seul son `innerHTML` l'est. Les trois attributs
 *  vivent ICI et pas dans `renderQueueWindow` : ils sont STATIQUES, et ce rendu-là est le chemin
 *  chaud de la colonne. ⚠️ Cette fonction recrée `#ql` à chaque navigation vers Revue — le focus est
 *  donc réellement perdu en quittant l'écran, ce qui est le comportement voulu. */
function revueShell(content: HTMLElement): void {
  content.style.display = "flex";
  content.style.flexDirection = "";
  content.style.overflowY = "hidden";
  content.innerHTML =
    `<div class="sift-revue-row">` +
    `<div class="queue" id="qcol" style="width:${qcolWidth()}px">` +
    `<div class="sift-qhead"></div>` +
    `<div id="ql" role="listbox" tabindex="0" aria-label="File de revue"></div>` +
    `</div>` +
    `<div class="sift-qresize" title="Redimensionner la file"></div>` +
    `<div class="sift-inspector" id="rvinspector">` +
    `<div class="mid" id="mid"></div>` +
    `<div class="sift-action-rail" id="filfoot"></div>` +
    `<div class="sift-dest-popover" id="fldz" hidden></div>` +
    `</div></div>`;
  const qcol = requireEl<HTMLElement>("#qcol", "revueShell");
  const handle = requireEl<HTMLElement>(".sift-qresize", "revueShell");
  installQueueResize(qcol, handle);
}

// ---------------------------------------------------------------------------
// Rendu
// ---------------------------------------------------------------------------

/** Vide les deux emplacements de la barre unifiée. Appelé avant chaque rendu : une vue qui monte
 *  une action ou une recherche la laisserait sinon sur l'écran suivant. */
function clearBarSlots(): void {
  const actions = document.getElementById("sift-tb-actions");
  const search = document.getElementById("sift-tb-search");
  const count = document.getElementById("sift-tb-count");
  if (actions) actions.textContent = "";
  // Le compte de file se vide comme les emplacements : il est propre à Revue, et le laisser
  // afficherait « 3 124 pistes » à côté du titre « Réglages ». `:empty{display:none}` fait
  // disparaître le nœud avec son texte. La classe de mode Lot part avec — sinon un retour sur
  // Revue en mode Détail repeindrait un compte de file à l'encre d'accent.
  if (count) {
    count.textContent = "";
    count.classList.remove("sift-tb-count-lot");
  }
  if (search) search.textContent = "";
}

/** Le libellé du rail EST le nom humain de la vue : le relire plutôt que maintenir une table
 *  vue → titre, qui divergerait au premier renommage. Le premier `<span>` porte le libellé ; le
 *  second, sur Revue, est le badge de compte — d'où `querySelector("span")` et non `textContent`,
 *  qui rendrait « Revue2710 ».
 *
 *  Le libellé alimente DEUX destinations, et c'est le point de l'étape 2 : le `<h1>` accessible
 *  invisible d'`index.html`, et le titre de la barre unifiée. Les écrans n'émettent donc plus de
 *  `.h1` dans leur contenu — le titre a quitté le contenu pour la fenêtre. */
function syncNav(view: ViewId): void {
  const nav = requireEl<HTMLElement>("#nav", "syncNav");
  // Marqueur de vue sur <body> — consommateur : la règle « l'analyse s'affiche à UN endroit »
  // (retour d'Antoine, 2026-09-06). Sur Revue, le pied de la colonne de file porte « Analyse —
  // x/y » (queue-panel.ts) et la ligne analyze du rail se masque (styles.css, .sift-pz-analyze) ;
  // partout ailleurs le rail reste le seul porteur du signal.
  document.body.dataset.view = view;
  const viewTitle = document.getElementById("sift-view-title");
  const barTitle = document.getElementById("sift-tb-title");
  // `[data-view]` EST le filtre, pas un ornement (issue #42). Le rail porte deux familles de `.nv` :
  // les sept destinations, et les entrées de source de la section Sources (`rail-source-entry.ts`,
  // même grammaire `.nv`, même marqueur `on`, mais AUCUN `data-view`). Sans ce filtre, la boucle
  // ci-dessous évaluait `undefined === view` sur chaque source — donc faux — et son
  // `classList.toggle("on", false)` ARRACHAIT le marqueur d'une source active, que
  // `renderRailSources()` venait de poser et reposera au prochain repeint.
  //
  // Le résultat n'était pas « une source jamais marquée » mais pire : un marqueur qui clignotait
  // entre les deux moitiés du système, à la fréquence des repeints du rail — plusieurs fois par
  // seconde pendant un scan. `#sift-rail-sources` précédant les destinations dans `index.html`,
  // « la première entrée active du rail » désignait tantôt la source, tantôt l'écran courant. Un
  // état qui dépend de qui a repeint en dernier n'est pas un état.
  //
  // Le marquage d'une source appartient à `rail-sources.ts`, qui seul connaît le filtre de file.
  // Le routeur n'y touche plus.
  nav.querySelectorAll<HTMLElement>(".nv[data-view]").forEach((n) => {
    const on = n.dataset.view === view;
    n.classList.toggle("on", on);
    if (!on) return;
    const label = n.querySelector("span")?.textContent ?? "";
    if (viewTitle) viewTitle.textContent = label;
    if (barTitle) barTitle.textContent = label;
  });
}

export function render(): void {
  const content = requireEl<HTMLElement>("#content", "render");
  // Ouvre une génération de rendu AVANT de déléguer (issue #42) : les renderers ci-dessous sont
  // asynchrones et lancés en `void`, donc celui de l'écran qu'on QUITTE peut encore être en vol.
  // Le jeton qu'il a capturé devient périmé ici, et son écriture tardive sera refusée au lieu de
  // repeindre le `#content` de l'écran qu'on vient d'ouvrir.
  bumpViewEpoch();
  syncNav(currentView);
  clearBarSlots();
  closeAside();

  switch (currentView) {
    case "revue":
      revueShell(content);
      void renderQueue();
      return;
    case "biblio":
      blockShell(content);
      void renderBiblioLive();
      return;
    case "ecarts":
      blockShell(content);
      void renderEcartes();
      return;
    case "journal":
      blockShell(content);
      paintJournal(true);
      return;
    case "rkb":
      blockShell(content);
      void renderRekordboxLive();
      return;
    case "cle":
      blockShell(content);
      renderUsbLive();
      return;
    case "reglages":
      blockShell(content);
      void renderReglagesLive();
      return;
  }
}

/** Navigue vers `view` et rend. Exportée pour les liens inter-écrans (l'état vide de Bibliothèque
 *  renvoie vers Revue, la porte de racine manquante vers Réglages) — jusqu'ici ces liens
 *  passaient par un `[data-view]` synthétique cliqué à travers le routeur de la maquette. */
export function goTo(view: ViewId): void {
  currentView = view;
  render();
}

/** Câble le routage et pose la première vue. Un seul écouteur, délégué sur `#pa` : chaque élément
 *  porteur d'un `data-view` navigue, où qu'il soit dans l'arbre (rail, état vide, bandeau). */
export function installRouter(): void {
  requireEl<HTMLElement>("#pa", "installRouter").addEventListener("click", (e) => {
    const el = (e.target as HTMLElement | null)?.closest<HTMLElement>("[data-view]");
    if (!el) return;
    const v = el.dataset.view;
    if (!isViewId(v) || v === currentView) return;
    goTo(v);
  });
  render();
}
