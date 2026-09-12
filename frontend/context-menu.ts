// Menu contextuel — étape 5 de DESIGN.md § 17, spec `docs/ui-specs/bibliotheque.md`.
//
// Mesuré le 2026-08-19 : ZÉRO gestionnaire `contextmenu` dans tout `frontend/`. C'est le manque le
// plus coûteux de la table, et le coût est structurel, pas esthétique — sans lui, chaque action
// secondaire doit être un bouton DANS la ligne, donc manger de la largeur sur 15 000 lignes pour
// servir sur une. Le patron macOS l'énonce à l'envers : les actions secondaires vivent au clic
// droit, la ligne ne garde que son geste primaire.
//
// Le menu est construit une fois par ouverture puis jeté — ce n'est pas un rendu en rafale, c'est
// un geste ponctuel, donc la règle « créer une fois, muter ensuite » ne s'applique pas ici.
import { esc } from "./dom";

export interface MenuItem {
  /** Libellé affiché. Jamais une icône seule : ce menu n'a pas de place pour une infobulle. */
  label: string;
  /** Action. Une entrée sans `onPick` est rendue désactivée plutôt que masquée — voir plus bas. */
  onPick?: () => void;
  /** Rendu en encre `danger`. Réservé à ce qui retire quelque chose de la vue de l'utilisateur. */
  danger?: boolean;
  /** Séparateur au-dessus de cette entrée. */
  separated?: boolean;
  /** Rangée de pastilles de teinte à la place d'un bouton pleine largeur (patron Finder Tags :
   *  les couleurs vivent EN RANGÉE dans le menu contextuel même, jamais dans un sous-menu — les
   *  HIG § Menus réservent le sous-menu au terme répété sur 3+ entrées, et il « cache les
   *  éléments qu'il contient »). `label` devient l'étiquette de la rangée ; `onPick` et `danger`
   *  sont ignorés sur cette entrée. */
  swatches?: SwatchRow;
}

export interface SwatchRow {
  /** Teintes dans l'ordre d'affichage. `key` est une clé de `.sift-rail-src-dot-<key>` —
   *  la classe qui ne déclare QUE le fond (styles.css, section Sources du rail) ; `label`
   *  est l'infobulle humaine. */
  hues: readonly { key: string; label: string }[];
  /** Teinte marquée d'un anneau : la couleur RÉSOLUE (override posé, sinon cycle auto). */
  active: string;
  onPick: (key: string) => void;
}

const MENU_ID = "sift-context-menu";

/** Ferme le menu s'il est ouvert. Idempotent. */
export function closeContextMenu(): void {
  document.getElementById(MENU_ID)?.remove();
}

/** Ouvre un menu contextuel au point donné.
 *
 *  Une entrée sans action est **désactivée, pas retirée** : un menu dont les entrées apparaissent
 *  et disparaissent selon la ligne oblige à relire la liste à chaque clic droit, et fait rater
 *  l'entrée qu'on visait par mémoire de position. Un menu stable se pointe sans lire.
 *
 *  Le positionnement est ramené dans la fenêtre APRÈS montage, en lisant la taille réelle : la
 *  hauteur d'un menu dépend du nombre d'entrées ET des séparateurs, donc l'estimer avant montage
 *  se trompe précisément là où ça compte, en bas de l'écran. */
export function openContextMenu(x: number, y: number, items: MenuItem[]): void {
  closeContextMenu();
  if (items.length === 0) return;

  const menu = document.createElement("div");
  menu.id = MENU_ID;
  menu.className = "sift-ctx-menu";
  menu.setAttribute("role", "menu");
  menu.innerHTML = items
    .map((it, i) => {
      const sep = it.separated && i > 0 ? " sift-ctx-item--sep" : "";
      if (it.swatches) {
        const dots = it.swatches.hues
          .map(
            (h) =>
              `<button type="button" class="sift-ctx-swatch${h.key === it.swatches!.active ? " on" : ""}"` +
              ` role="menuitem" data-ctx="${i}" data-hue="${esc(h.key)}" title="${esc(h.label)}">` +
              `<span class="sift-ctx-swatch-fill sift-rail-src-dot-${esc(h.key)}" aria-hidden="true"></span></button>`,
          )
          .join("");
        return `<div class="sift-ctx-swatchrow${sep}"><span class="sift-ctx-swatchlabel">${esc(it.label)}</span>${dots}</div>`;
      }
      const cls =
        "sift-ctx-item" +
        (it.danger ? " sift-ctx-item--danger" : "") +
        (it.onPick ? "" : " sift-ctx-item--disabled") +
        sep;
      const dis = it.onPick ? "" : ' aria-disabled="true"';
      return `<button type="button" class="${cls}" role="menuitem" data-ctx="${i}"${dis}>${esc(it.label)}</button>`;
    })
    .join("");
  document.body.appendChild(menu);

  const r = menu.getBoundingClientRect();
  const left = Math.max(4, Math.min(x, window.innerWidth - r.width - 4));
  const top = Math.max(4, Math.min(y, window.innerHeight - r.height - 4));
  menu.style.left = `${left}px`;
  menu.style.top = `${top}px`;

  menu.addEventListener("click", (e) => {
    const btn = (e.target as HTMLElement).closest<HTMLElement>("[data-ctx]");
    if (!btn) return;
    e.stopPropagation();
    const item = items[Number(btn.dataset.ctx)];
    const hue = btn.dataset.hue;
    closeContextMenu();
    if (item?.swatches && hue != null) item.swatches.onPick(hue);
    else item?.onPick?.();
  });

  // Fermeture. `capture` sur le clic pour partir AVANT que le clic n'atteigne ce qu'il vise —
  // sinon un clic hors menu déclencherait l'action de la ligne survolée en plus de fermer.
  // `once` sur chacun : le menu est jeté à la première fermeture, ses écouteurs avec lui.
  const dismiss = () => closeContextMenu();
  document.addEventListener("click", dismiss, { capture: true, once: true });
  document.addEventListener("contextmenu", dismiss, { capture: true, once: true });
  window.addEventListener("resize", dismiss, { once: true });
  // Le défilement ferme aussi : le menu est ancré à un POINT, pas à un élément, donc il resterait
  // sur place pendant que sa ligne s'en va — et pointerait alors une autre piste.
  document.addEventListener("scroll", dismiss, { capture: true, once: true });
  document.addEventListener(
    "keydown",
    (e) => {
      if (e.key === "Escape") {
        e.stopPropagation(); // ne pas laisser la couche 1 traiter le même Échap
        closeContextMenu();
      }
    },
    { capture: true, once: true },
  );

  menu.querySelector<HTMLElement>(".sift-ctx-item:not(.sift-ctx-item--disabled)")?.focus();
}
