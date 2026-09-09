// App chrome (Tauri only): custom titlebar, the "lean" stylesheet that hides the mockup's
// not-yet-real surfaces, scroll-thumb autohide, and OS drag-drop. Extracted from sift-live.ts
// (audit P-3) — self-contained UI shell, no shared app state; imports only Tauri + ipc + toast
// (the drop handler acknowledges what the backend actually imported, see reportImport below).
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { platform } from "@tauri-apps/plugin-os";
import { importPaths } from "./ipc";
import { toast } from "./filing-toast";

// One-time style: while dragging, an existing zone gets an outline + an overlaid hint
// (::after with the zone's data-dz text). No permanent dashed box — the hint shows only
// during a drag, on the real folder/queue boxes, saving space.
function ensureDropStyle() {
  if (document.getElementById("sift-dz-style")) return;
  const s = document.createElement("style");
  s.id = "sift-dz-style";
  s.textContent =
    ".sift-dz-on{position:relative;outline:1.5px dashed var(--color-text-info);outline-offset:-4px;border-radius:var(--border-radius-md)}" +
    ".sift-dz-on::after{content:attr(data-dz);position:absolute;inset:0;display:flex;align-items:center;justify-content:center;text-align:center;padding:10px;font-size:var(--text-sm);color:var(--color-text-info);background:var(--overlay-drop);border-radius:var(--border-radius-md);pointer-events:none;z-index:50}";
  document.head.appendChild(s);
}

// Existing boxes that double as drop targets, with the hint each shows while dragging.
// Le bouton Destination porte la zone de dépôt « destination » (l'arbre lui-même est un popover,
// masqué par défaut, donc pas une cible fiable) — un dossier lâché là devient la destination.
// Il a DEUX hôtes depuis la décision V2b (2026-08-30) : la rangée réglages de la boîte de lecture
// en mode Détail, le pied de panneau en mode Lot. Le pied est filtré sur `:not([hidden])` parce
// qu'en Détail il existe encore dans le DOM, vidé et masqué (filing.ts `hidePanelFoot`) : sans ce
// filtre il serait balisé comme zone de dépôt tout en étant invisible.
const DROP_ZONES: [string, string][] = [
  ["#filbox-settings", "Dépose un dossier ici — nouvelle destination"],
  ["#filfoot:not([hidden])", "Dépose un dossier ici — nouvelle destination"],
  ["#ql", "Dépose des fichiers audio ici"],
  // `#sift-rail-sources` : le cadran de dépôt du rail (issue #56). Le sélecteur disait
  // `#sift-sources` — un id qui n'existait plus nulle part dans le DOM, donc cette zone n'était
  // JAMAIS balisée pendant un drag (le repli sur #content masquait le trou). Corrigé le 2026-09-03.
  ["#sift-rail-sources", "Dépose un dossier à surveiller"],
];

/** Toggle the drag hint/outline on the relevant existing boxes. Falls back to #content
 * (e.g. Bibliothèque) when none of the named zones are on screen.
 * Idempotent by construction: "over" fires continuously while the pointer moves (dozens of
 * calls per second), so the wanted set is computed first and compared to what is already
 * marked — identical state returns without touching a single class. Only a real change
 * mutates the DOM, and it does so with plain class/attribute writes (the .sift-dz-on CSS has
 * no transition), so the hint still appears and disappears instantly. */
function setDropActive(on: boolean) {
  ensureDropStyle();
  const wanted: [HTMLElement, string][] = [];
  if (on) {
    const present = DROP_ZONES.filter(([sel]) => document.querySelector(sel));
    const targets: [string, string][] = present.length
      ? present
      : [["#content", "Dépose des fichiers (→ file d'attente) ou des dossiers (→ surveillés)"]];
    for (const [sel, label] of targets) {
      const el = document.querySelector<HTMLElement>(sel);
      if (el) wanted.push([el, label]);
    }
  }
  const current = document.querySelectorAll<HTMLElement>(".sift-dz-on");
  if (
    current.length === wanted.length &&
    wanted.every(([el, label]) => el.classList.contains("sift-dz-on") && el.dataset.dz === label)
  )
    return;
  current.forEach((el) => {
    el.classList.remove("sift-dz-on");
    el.removeAttribute("data-dz");
  });
  for (const [el, label] of wanted) {
    el.classList.add("sift-dz-on");
    el.dataset.dz = label;
  }
}

/** « dest » quand le curseur est au-dessus de l'un des deux hôtes du bouton Destination (la rangée
 * réglages de la boîte en Détail, le pied de panneau en Lot), « source » sinon. La mention de
 * `#fldz` ici décrivait l'arbre persistant d'avant le popover — le code testait déjà `#filfoot`.
 * Tauri 2 emits the
 * drop position already in logical (CSS) pixels — exactly what elementFromPoint expects, so
 * no devicePixelRatio correction (dividing here double-corrected on HiDPI/scaled displays). */
function dropModeAt(pos: { x: number; y: number }): "source" | "dest" {
  const el = document.elementFromPoint(pos.x, pos.y);
  return el && el.closest("#filfoot,#filbox-settings") ? "dest" : "source";
}

/** Acknowledge a drop with what the backend ACTUALLY took in — never p.paths.length, which
 * counts what was dropped, not what was imported: ipc.rs keeps a file only if scanner::is_audio
 * accepts it and add_loose_file really added it, and in "dest" mode a folder counts only when
 * LIBRARY_ROOT is set (otherwise folders_added stays 0 with no error). Five .txt files dropped
 * must read "rien d'importable", not "5 reçus". Wording follows the backend's own split:
 * morceaux (files) vs dossiers (folders), never a merged "éléments". */
function reportImport(res: {
  files_added: number;
  folders_added: number;
  blocked_by: string | null;
}): void {
  const { files_added, folders_added } = res;
  // Impasse A5 (issue #15), et ce commentaire est celui qui nommait déjà le trou juste au-dessus :
  // « en mode dest un dossier ne compte que si LIBRARY_ROOT est réglé (sinon folders_added reste 0
  // sans erreur) ». Le backend le dit maintenant au lieu de le laisser deviner, et sa raison PRIME
  // sur les compteurs — accuser le contenu déposé quand c'est un réglage qui manque envoie
  // l'utilisateur chercher au mauvais endroit.
  if (res.blocked_by) {
    toast(res.blocked_by);
    return;
  }
  if (!files_added && !folders_added) {
    toast("Rien d'importable dans ce dépôt");
    return;
  }
  const parts: string[] = [];
  if (files_added) parts.push(`${files_added} morceau${files_added > 1 ? "x" : ""}`);
  if (folders_added) parts.push(`${folders_added} dossier${folders_added > 1 ? "s" : ""}`);
  const plural = files_added + folders_added > 1 ? "s" : "";
  toast(`${parts.join(" et ")} ajouté${plural}`);
}

/** OS drag-drop: audio files → queue; folders → watched source, or a destination bin when
 * dropped on the "Où on va" column. */
export async function installDragDrop() {
  try {
    await getCurrentWebview().onDragDropEvent((ev) => {
      const p = ev.payload;
      if (p.type === "drop") {
        setDropActive(false);
        if (p.paths.length)
          void importPaths(p.paths, dropModeAt(p.position))
            .then(reportImport)
            .catch((e) => {
              console.error("import_paths failed", e);
              toast("Échec de l'import");
            });
      } else if (p.type === "enter" || p.type === "over") {
        setDropActive(true);
      } else {
        setDropActive(false);
      }
    });
  } catch (e) {
    console.error("drag-drop init failed", e);
  }
}

/** Feuille du shell de bureau : barre de titre custom (decorations:false) et hauteur de #pa.
 *
 * Elle s'appelait "lean" parce qu'elle masquait les surfaces de la maquette que le rendu live
 * n'écrasait pas. Ces deux règles ont été retirées le 2026-08-19 avec l'étape 1
 * (DESIGN.md § 17), pour deux raisons distinctes, chacune vérifiée :
 *
 * - `.pitch, .sub, .frow` ne correspondait à RIEN — aucun de ces trois sélecteurs n'existe dans
 *   `index.html`, `app.js` ni `frontend/*.ts`. Règle morte, sans doute depuis une réécriture
 *   d'`index.html` qui a emporté le bloc marketing sans emporter sa règle ;
 * - `[data-act="revmode"], [data-act="togglequeue"]` masquait les bascules de la maquette, que le
 *   rendu live doublait sous un autre DOM (`data-sift="reviewmode"`, le pied de la colonne de file).
 *   `app.js` ne tourne plus dans Tauri, donc ces attributs n'y existent plus du tout.
 *
 * Injectée une fois ; la démo navigateur ne l'exécute jamais. */
export function injectLeanStyle() {
  if (document.getElementById("sift-lean-style")) return;
  const st = document.createElement("style");
  st.id = "sift-lean-style";
  st.textContent =
    // Barre UNIFIÉE (decorations:false — Tauri seulement) : barre de titre et toolbar fusionnées,
    // une seule barre, jamais deux empilées. Deux VRAIES zones DOM (pas un dégradé de fond, qui
    // laisse une couture sous-pixel) : la gauche prend la largeur du rail (--rail-w) pour que la
    // barre se lise comme sa continuation — et son TON aussi, qui est depuis le 2026-08-27 le fond
    // de fenêtre (le rail est passé au plan le plus en retrait, motif sidebar Mail — voir .sb).
    // La bordure verticale entre les deux est le même border-right que le rail, donc la ligne
    // court sans interruption de la barre jusque dans .sb.
    // FILET SOUS LA BARRE, bord à bord (décision maquette 2026-08-26, portage #50) : c'est le
    // séparateur qui BORNE la zone barre — motif Mail, filet pleine largeur sous l'en-tête de
    // colonne. 1px (le kit dessine ses séparateurs à 1), inclus dans les 48 (border-box global).
    // Hauteur : var(--toolbar-h), dérivée du kit App Window (voir le bloc de géométrie de
    // styles.css). Elle était codée en dur à 30px ici ET dans le calc de #pa plus bas, pendant
    // que --titlebar-h vivait dans styles.css avec un seul autre consommateur.
    "#sift-titlebar{height:var(--toolbar-h);flex:none;display:flex;align-items:stretch;" +
    "background:var(--color-background-primary);border-bottom:1px solid var(--color-border-tertiary);" +
    "-webkit-user-select:none;user-select:none}" +
    "#sift-tb-left{width:var(--rail-w);flex:none;display:flex;align-items:center;" +
    "background:var(--color-background-primary);border-right:1px solid var(--color-border-tertiary)}" +
    // Marque de l'app, déplacée du rail vers ce segment le 2026-09-03 (issue #56) : convention
    // Windows de l'icône d'app en titlebar (écart macOS assumé — la toolbar Apple ne porte pas de
    // logo), et le rail y gagne une rangée. Mêmes valeurs que l'ex-`.nav-brand` de styles.css.
    "#sift-tb-brand{display:flex;align-items:center;gap:var(--space-8);padding-left:var(--space-12);" +
    "font-size:var(--text-lg);font-weight:600;color:var(--color-text-primary);letter-spacing:var(--tracking-normal)}" +
    "#sift-tb-brand i{font-size:16px;color:var(--color-text-info)}" +
    "#sift-tb-lspacer{flex:1}" +
    "#sift-rail-toggle{margin-right:var(--space-8)}" +
    // Rail replié : la zone gauche fait 56px (--rail-w redéfinie sur <body>) — seul le bouton de
    // repli survit, centré ; même règle de survie que l'ex-`.nav-brand` replié.
    "body.sift-rail-collapsed #sift-tb-brand{display:none}" +
    "body.sift-rail-collapsed #sift-tb-left{justify-content:center}" +
    "body.sift-rail-collapsed #sift-rail-toggle{margin-right:0}" +
    "#sift-tb-right{flex:1;min-width:0;display:flex;align-items:center;gap:var(--space-12);" +
    "padding:0 var(--space-8) 0 var(--space-16)}" +
    // Titre de la VUE courante, plus le littéral « Sift » : le nom de l'app est déjà dans le
    // segment gauche de la barre (#sift-tb-brand), et une barre qui répète le nom du logiciel à
    // la place du nom de l'écran ne renseigne rien.
    "#sift-tb-title{font-size:var(--text-base);font-weight:500;color:var(--color-text-primary);" +
    "overflow:hidden;text-overflow:ellipsis;white-space:nowrap;flex:none}" +
    // Compte de la file, contre le titre. Subordonné au titre par la TAILLE et l'ENCRE, jamais par
    // un séparateur : c'est le traitement de « 34 messages » sous « Inbox » dans Mail, transposé à
    // l'horizontale. `margin-left` et non un `gap` du parent — la barre n'aligne pas ses enfants
    // par un gap unique, et le titre doit rester collé à son compte quand les autres bougent.
    "#sift-tb-count{font-size:var(--text-sm);color:var(--color-text-tertiary);white-space:nowrap;" +
    "flex:none;margin-left:var(--space-8)}" +
    "#sift-tb-count:empty{display:none}" +
    // Mode Lot : le compte devient celui de la sélection et prend l'encre d'accent — le seul état
    // permanent du mode, et la couleur est ce qui le distingue au premier coup d'œil du compte de
    // file qu'il remplace au même endroit.
    "#sift-tb-count.sift-tb-count-lot{color:var(--color-text-info)}" +
    // Deux emplacements que les vues remplissent : actions contextuelles (2 à 3 au maximum) et
    // recherche. Vides, ils n'occupent rien. L'espaceur entre eux pousse la recherche à droite,
    // où elle est sur toutes les apps système — et l'y garde quelle que soit la vue.
    "#sift-tb-actions{display:flex;align-items:center;gap:var(--space-8);min-width:0}" +
    "#sift-tb-spacer{flex:1;min-width:var(--space-8)}" +
    "#sift-tb-search{display:flex;align-items:center;gap:var(--space-8);flex:none}" +
    "#sift-tb-controls{display:flex;height:100%}" +
    ".sift-win{width:44px;height:100%;display:flex;align-items:center;justify-content:center;border:none;" +
    "background:transparent;color:var(--color-text-tertiary);cursor:pointer;border-radius:0;padding:0}" +
    ".sift-win:hover{background:var(--color-background-secondary);color:var(--color-text-primary)}" +
    ".sift-win-close:hover{background:#e81123;color:#fff}.sift-win i{font-size:15px}" +
    // macOS: 3 small round traffic lights on the left instead of square right-aligned buttons.
    // Reuses the same buttons/click-wiring; only placement (markup order) and this styling differ.
    // The controls move into the left (nav-tone) zone, the title stays alone in the right zone.
    ".sift-tb-mac #sift-tb-left{justify-content:flex-start;gap:8px;padding-left:12px}" +
    // Ni `justify-content` ni `padding-left` à re-déclarer ici depuis la barre unifiée : la zone
    // droite porte son propre padding et son propre espaceur, identiques sur les deux cibles.
    // Seul le PLACEMENT des boutons de fenêtre diffère entre macOS et le reste, et il est déjà
    // porté par le markup (zone gauche contre zone droite), pas par ces deux règles.
    ".sift-tb-mac .sift-win{width:12px;height:12px;border-radius:50%;color:transparent;font-size:0}" +
    ".sift-tb-mac .sift-win:hover{color:inherit;font-size:8px}" +
    '.sift-tb-mac .sift-win[data-win="close"]{background:var(--color-text-danger)}' +
    '.sift-tb-mac .sift-win[data-win="min"]{background:var(--color-text-warning)}' +
    '.sift-tb-mac .sift-win[data-win="max"]{background:var(--color-text-success)}' +
    ".sift-tb-mac .sift-win-close:hover{background:var(--color-text-danger)}" +
    // `<body>` devient une COLONNE flex et `#pa` prend le reste, plutôt qu'un
    // `height:calc(100vh - var(--toolbar-h))`. Le calc marchait tant que la barre était la seule
    // chose au-dessus du shell ; il devient faux dès qu'une deuxième surface s'y ajoute — et c'est
    // le cas depuis que la porte de racine manquante est remontée au niveau fenêtre (fusion 1).
    // Une hauteur qui doit connaître le nombre de ses voisins est la même erreur que le
    // `calc(100dvh - 210px)` retiré de la colonne de facettes à l'étape 3.
    "body{display:flex;flex-direction:column}" +
    "#pa{flex:1;min-height:0;height:auto!important}";
  document.head.appendChild(st);
}

/** Bascule le title/aria-label du bouton "Agrandir" selon l'état maximisé courant — l'icône
 * reste volontairement identique (ti-square) dans les deux états. */
function syncMaxButton(btn: HTMLElement, maximized: boolean): void {
  const label = maximized ? "Restaurer" : "Agrandir";
  btn.title = label;
  btn.setAttribute("aria-label", label);
}

/** Inject the custom window titlebar (the native one is off via decorations:false) and wire
 * its minimise / maximise / close buttons. The bar + its title are drag regions. macOS gets
 * traffic lights on the left (sift-tb-mac class); everyone else keeps today's right-aligned
 * layout — same markup order, so the non-mac branch renders byte-identical to before. */
export async function injectTitlebar(): Promise<void> {
  if (document.getElementById("sift-titlebar")) return;

  let isMac = false;
  try {
    isMac = platform() === "macos";
  } catch (e) {
    console.error("platform() failed, defaulting to the Windows titlebar layout", e);
  }

  const bar = document.createElement("div");
  bar.id = "sift-titlebar";
  if (isMac) bar.classList.add("sift-tb-mac");
  bar.setAttribute("data-tauri-drag-region", "");
  // Le titre est rempli par router.ts à chaque rendu de vue (`setBarTitle`) ; « Sift » n'est que
  // la valeur avant le premier rendu. Les deux emplacements qui l'accompagnent sont vides et le
  // restent tant qu'une vue n'y monte rien.
  const title =
    '<span id="sift-tb-title" data-tauri-drag-region>Sift</span>' +
    // Compte de la file, contre le titre (2026-08-26). Vide hors Revue — c'est `paintBarCount`
    // (queue-panel.ts) qui l'écrit, et `clearBarSlots` qui le vide en quittant l'écran. Motif de
    // Mail : le compte suit le TITRE de ce qu'il compte, et dans Sift ce titre vit dans la barre.
    // `data-tauri-drag-region` comme le titre : c'est du texte inerte, il ne doit pas créer un trou
    // dans la zone de déplacement de la fenêtre.
    '<span id="sift-tb-count" data-tauri-drag-region></span>' +
    '<div id="sift-tb-actions"></div>' +
    '<div id="sift-tb-spacer" data-tauri-drag-region></div>' +
    // `#sift-tb-actions-right` occupait cette place — emplacement de bord droit créé pour la seule
    // icône de sélection de Revue, retiré le 2026-08-26 quand ce commutateur est descendu dans la
    // tête de la colonne de file (`syncQueueSelectButton`, queue-panel.ts). Aucun autre écran n'y
    // montait quoi que ce soit.
    '<div id="sift-tb-search"></div>';
  const controls =
    '<div id="sift-tb-controls">' +
    '<button class="sift-win" data-win="min" title="Réduire" aria-label="Réduire"><i class="ti ti-minus"></i></button>' +
    '<button class="sift-win" data-win="max" title="Agrandir" aria-label="Agrandir"><i class="ti ti-square"></i></button>' +
    '<button class="sift-win sift-win-close" data-win="close" title="Fermer" aria-label="Fermer"><i class="ti ti-x"></i></button>' +
    "</div>";
  // Two real zones (left = nav width/tone, right = content tone — see injectLeanStyle's CSS
  // comment for why this is DOM, not a gradient). Windows: title + controls both live in the
  // right zone (space-between keeps today's layout, just shifted right by 152px). macOS: the
  // traffic-light controls move into the left zone, the title stays alone in the right zone.
  // Marque + bouton de repli du rail (ex-`.nav-brand` d'index.html, déplacés ici le 2026-09-03,
  // issue #56). Le bouton garde son id : `installRailToggle` le retrouve tel quel — cette barre
  // est injectée AVANT lui dans `installLiveWiring`, et de façon synchrone (le premier `await`
  // de cette fonction vient après l'insertBefore).
  const brand =
    '<span id="sift-tb-brand" data-tauri-drag-region><i class="ti ti-filter" aria-hidden="true"></i>Sift</span>' +
    '<span id="sift-tb-lspacer" data-tauri-drag-region></span>' +
    '<button id="sift-rail-toggle" class="lk-icon" title="Replier le rail" aria-label="Replier le rail" aria-expanded="true"><i class="ti ti-layout-sidebar-left-collapse" aria-hidden="true"></i></button>';
  const left = `<div id="sift-tb-left" data-tauri-drag-region>${isMac ? controls : ""}${brand}</div>`;
  const right = `<div id="sift-tb-right" data-tauri-drag-region>${title}${isMac ? "" : controls}</div>`;
  bar.innerHTML = left + right;
  document.body.insertBefore(bar, document.body.firstChild);

  const w = getCurrentWindow();
  // On Windows 11, an undecorated window gets its shadow enabled by default, which draws as a
  // 1px light border with rounded corners around the whole window (annotation: "il semble y
  // avoir un cadre ou une bordure le long de la fenetre") — this app already draws its own
  // rounded/transparent frame via body:has(#sift-titlebar), so the native shadow is redundant.
  // `void` sur cette promesse a masqué le défaut pendant tout ce temps : la permission
  // `core:window:allow-set-shadow` manquait de `capabilities/default.json`, l'appel était rejeté
  // à CHAQUE démarrage, et le rejet non géré ne se lisait que dans le log de `tauri dev`. Le
  // correctif ci-dessus n'avait donc jamais pris effet — la bordure était toujours là. Permission
  // ajoutée le 2026-08-05 ; l'échec est désormais bruyant, conformément au « pas de fallback
  // silencieux » du projet.
  w.setShadow(false).catch((e: unknown) => {
    console.error("setShadow(false) refusé — la bordure native restera visible :", e);
  });
  const maxBtn = bar.querySelector<HTMLElement>('[data-win="max"]');

  bar.querySelectorAll<HTMLElement>(".sift-win").forEach((b) =>
    b.addEventListener("click", () => {
      const act = b.dataset.win;
      if (act === "min") void w.minimize();
      else if (act === "max") void w.toggleMaximize();
      else if (act === "close") void w.close();
    }),
  );

  if (maxBtn) {
    try {
      syncMaxButton(maxBtn, await w.isMaximized());
      await w.onResized(() => {
        void w.isMaximized().then((m) => syncMaxButton(maxBtn, m));
      });
    } catch (e) {
      console.error("maximize-state sync failed, keeping the default Agrandir icon", e);
    }
  }
}

/** Audit-ref C3 (Accueil, 2026-07-08, réf. shadcn Sidebar) : `.nv`/`[data-view]` sont des `<div>`
 * cliquables sans équivalent clavier — `app.js` (importé sans garde `inTauri`, main.ts:6) gère déjà
 * le clic réel (`e.target.closest('[data-view]')`) mais n'écoute que "click". Complète en Enter/
 * Espace sans toucher app.js (figé) : redispatche un clic synthétique sur l'élément focus. Couvre
 * aussi les lignes `.qi[data-sift="homerow"]` (rail de sources, supprime le 2026-08-19), l'arbre de destination
 * `[data-fil="bin"]` (audit-ref R4, Revue, filing.ts) et les facettes/lignes Bibliothèque
 * `[data-bib="pick"]`/`[data-bib="row"]` (audit-ref B1) et la ligne de sélection réparations
 * master.db `[data-sift="mdbpick"]` (audit-ref G3, Rekordbox) — mêmes `tabindex`+`role` posés côté
 * markup, un seul point de câblage clavier générique pour toute nouvelle ligne de ce type. Étendu le
 * 2026-07-24 (audit UX/accessibilité) aux lignes de sélection Lot `[data-sift="batchpick"]`/
 * `[data-sift="batchpickfake"]` (batch-panel.ts), même défaut que mdbpick avant son fix. */
export function installNavKeyboard() {
  document.addEventListener("keydown", (e) => {
    if (e.key !== "Enter" && e.key !== " ") return;
    // Entrée/Espace NUS seulement — c'est le geste d'activation (couche 3, shortcuts.ts).
    // ⌘/Ctrl et Alt appartiennent à d'autres couches : les combinaisons à modificateur sont les
    // raccourcis de FENÊTRE de la couche 1 (`installWindowShortcuts`, dont le contrat est de ne
    // prendre QUE des combinaisons avec modificateur pour cohabiter avec ces accélérateurs à une
    // touche), et Alt est le geste de l'inspecteur dev (`dev-inspector.ts`, Alt+Clic). Sans cette
    // garde, ⌘Entrée/Ctrl+Espace/Alt+Espace sur une destination du rail focalisée redéclenchaient
    // une navigation à travers le routeur `#pa` — bug voisin repéré en diagnostiquant #42, hors de
    // son correctif. ⇧ est épargné : la couche 1 ne le réclame pas, c'est le modificateur de plage,
    // et le clic synthétique ci-dessous ne porte de toute façon aucun modificateur — ⇧Entrée reste
    // donc une activation simple, sans jamais devenir un geste de plage par ce chemin.
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    const target = e.target as HTMLElement;
    // .lr (audit-ref B1) nests a real <button> (lecture) — if THAT has focus, its own native Enter/
    // Space handling must fire alone; closest() would otherwise also match the ancestor row and
    // double-fire (play the track AND toggle the detail panel from one keypress).
    if (/^(BUTTON|A|INPUT|SELECT|TEXTAREA)$/.test(target?.tagName ?? "")) return;
    const el = target?.closest<HTMLElement>(
      '[data-view][tabindex],[data-sift="homerow"][tabindex],[data-fil="bin"][tabindex],[data-bib="pick"][tabindex],[data-bib="row"][tabindex],[data-sift="mdbpick"][tabindex],[data-sift="mdspick"][tabindex],[data-sift="maspick"][tabindex],[data-sift="dedpick"][tabindex],[data-bib="tile"][tabindex],[data-sift="batchpick"][tabindex],[data-sift="batchpickfake"][tabindex],[data-reglages="cat"][tabindex],[data-rkb="section"][tabindex]',
    );
    if (!el) return;
    e.preventDefault();
    el.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  });
}

/** Reveal a scroll area's thumb while it scrolls, then hide it ~700ms after it stops (the
 * CSS keeps it hidden at rest). Capture-phase so it catches scrolling on any inner element. */
export function installScrollAutohide() {
  const timers = new WeakMap<Element, ReturnType<typeof setTimeout>>();
  document.addEventListener(
    "scroll",
    (e) => {
      const el = e.target;
      if (!(el instanceof Element)) return;
      el.classList.add("sift-scrolling");
      const prev = timers.get(el);
      if (prev) clearTimeout(prev);
      timers.set(
        el,
        setTimeout(() => el.classList.remove("sift-scrolling"), 700),
      );
    },
    true,
  );
}

/** Repli du rail (étape 4, DESIGN.md § 14).
 *
 *  Le rail replié garde ses icônes et perd ses libellés — il ne disparaît pas : une navigation qui
 *  s'évapore oblige à la rouvrir pour savoir où l'on est. L'état est persisté, comme la largeur de
 *  la file de Revue : c'est une préférence de poste de travail, pas un état de session.
 *
 *  Le geste est instantané. `--rail-w` est redéfinie sur `<body>`, donc le rail ET la zone gauche
 *  de la barre unifiée bougent ensemble et la bordure verticale reste continue — l'invariant pour
 *  lequel le token existe. Animer serait animer une largeur, ce que DESIGN.md § 6 interdit. */
const RAIL_KEY = "sift-rail-collapsed";

function applyRailCollapsed(collapsed: boolean): void {
  document.body.classList.toggle("sift-rail-collapsed", collapsed);
  const btn = document.getElementById("sift-rail-toggle");
  if (!btn) return;
  const label = collapsed ? "Déplier le rail" : "Replier le rail";
  btn.title = label;
  btn.setAttribute("aria-label", label);
  btn.setAttribute("aria-expanded", collapsed ? "false" : "true");
  const icon = btn.querySelector("i");
  if (icon)
    icon.className = collapsed
      ? "ti ti-layout-sidebar-left-expand"
      : "ti ti-layout-sidebar-left-collapse";
}

/** Bascule le rail et persiste le nouvel état. Exportée pour le raccourci clavier. */
export function toggleRail(): void {
  const next = !document.body.classList.contains("sift-rail-collapsed");
  applyRailCollapsed(next);
  try {
    localStorage.setItem(RAIL_KEY, next ? "1" : "0");
  } catch {
    // Stockage refusé (mode privé, quota) : la bascule marche quand même, elle ne survit pas.
  }
}

export function installRailToggle(): void {
  let collapsed = false;
  try {
    collapsed = localStorage.getItem(RAIL_KEY) === "1";
  } catch {
    // Idem : l'absence de stockage vaut « déplié », l'état par défaut.
  }
  applyRailCollapsed(collapsed);
  document.getElementById("sift-rail-toggle")?.addEventListener("click", (e) => {
    // Le bouton vit dans la barre unifiée depuis le 2026-09-03 (issue #56) ; stopPropagation
    // reste par prudence — un clic qui bulle jusqu'à un délégué ne doit rien déclencher d'autre.
    e.stopPropagation();
    toggleRail();
  });
}
