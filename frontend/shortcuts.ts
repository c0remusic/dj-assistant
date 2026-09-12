// Couche 1 du clavier — les raccourcis de FENÊTRE (DESIGN.md § 9, étape 6 de § 17).
//
// Mesure qui a motivé cette couche, faite le 2026-08-19 : l'app avait quatre raccourcis d'action,
// tous derrière `if (!state.track) return` (`filing.ts`), donc Revue seule — plus ↑↓ dans la file.
// Hors Revue il en restait UN dans toute l'application, ⌘Z. Sept écrans sur huit exigeaient la
// souris, pour une cible qui vit au clavier. `installNavKeyboard` (`chrome.ts`) n'ajoutait aucun
// raccourci : il réémet un clic sur Entrée/Espace pour un élément DÉJÀ focalisé.
//
// Trois couches, et chacune a une portée nette :
//   1. fenêtre  — ici, disponible partout ;
//   2. liste    — là où une liste a le focus (file de Revue, table de Bibliothèque) ;
//   3. écran    — les touches propres à un écran (`filing.ts` pour Revue).
//
// La couche 1 ne prend QUE des combinaisons avec modificateur, plus Échap. C'est ce qui lui permet
// de cohabiter avec la couche 3, où Espace, Entrée, I et ⌫ sont des accélérateurs à une touche :
// une couche globale qui prendrait une lettre nue les écraserait sur tous les écrans.
import { goTo, type ViewId } from "./router";
import { focusBarSearch } from "./toolbar";
import {
  focusQueueSearch,
  closeQueueFacet,
  reviewMode,
  selectAllQueueBatch,
  clearQueueBatchSelection,
} from "./queue-panel";
import { toggleRail } from "./chrome";
import {
  selectAllVisible,
  renderBiblioLive,
  renderSelectionSummary,
  stepBibSelection,
  closeFacetPopover,
} from "./bibliotheque-view";

/** Ordre des destinations pour ⌘/Ctrl + 1…8. Lu depuis le rail plutôt que codé ici : le rail EST
 *  l'ordre affiché, et une table parallèle divergerait au premier réarrangement. */
function railViews(): ViewId[] {
  return Array.from(document.querySelectorAll<HTMLElement>("#nav .nv[data-view]"))
    .map((n) => n.dataset.view)
    .filter((v): v is ViewId => !!v);
}

/** Un champ de saisie a le focus : la couche 1 s'efface, sauf pour Échap. Sans cette garde,
 *  ⌘F dans un champ de nommage volerait la frappe, et Échap doit au contraire rester atteignable
 *  pour en sortir. */
function inTextField(t: EventTarget | null): boolean {
  const el = t as HTMLElement | null;
  if (!el) return false;
  return (
    el.tagName === "INPUT" ||
    el.tagName === "TEXTAREA" ||
    el.tagName === "SELECT" ||
    el.isContentEditable
  );
}

/** Ferme la surface temporaire la plus haute, et rend `true` si Échap est déjà pris en charge.
 *
 *  La modale de confirmation possède SON PROPRE gestionnaire d'Échap (`confirm-modal.ts`, qui
 *  piège aussi le focus). On ne le double pas : on constate seulement qu'elle est ouverte pour ne
 *  rien faire d'autre — sinon Échap refermerait le popover DERRIÈRE la modale, laissant à l'écran
 *  ce qui est devant et retirant ce qui est caché. */
function dismissTopmost(): boolean {
  if (document.getElementById("sift-confirm-overlay")) return true;
  const popover = document.querySelector<HTMLElement>(".sift-dest-popover:not([hidden])");
  if (popover) {
    document.querySelector<HTMLElement>('[data-fil="destbtn"]')?.click();
    return true;
  }
  // Filtre de la file de Revue (#sift-qfacet-pop) : même classe `.sift-facet-pop` que le sélecteur
  // de Bibliothèque, mais un id et une fermeture propres — la branche biblio ci-dessous fermerait
  // #sift-facet-pop, pas celui-ci. D'où le test par id, AVANT elle.
  const qfacet = document.getElementById("sift-qfacet-pop");
  if (qfacet && !qfacet.hidden) {
    closeQueueFacet();
    return true;
  }
  // Sélecteur de facette de Bibliothèque (2026-08-19). Fermé directement et non par un clic
  // simulé sur son bouton, contrairement au popover de destination : la bascule passerait par
  // l'état de module et rouvrirait ce qu'on veut fermer.
  if (document.querySelector(".sift-facet-pop:not([hidden])")) {
    closeFacetPopover();
    return true;
  }
  return false;
}

export function installWindowShortcuts(): void {
  document.addEventListener("keydown", (e) => {
    // Échap est le seul à traverser un champ de saisie : c'est la sortie de secours.
    if (e.key === "Escape") {
      if (dismissTopmost()) return; // la surface concernée gère elle-même sa fermeture
      if (inTextField(e.target)) (e.target as HTMLElement).blur();
      return;
    }

    // COUCHE 2 — la liste. Avant le test de modificateur : ⇧+↑↓ étend une plage, et ⇧ n'est pas un
    // modificateur de commande. Bornée à un écran qui porte réellement une table, sinon ↑↓ y
    // volerait le défilement de la page.
    if (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === "Home" || e.key === "End") {
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      if (inTextField(e.target)) return;
      if (!document.querySelector('.lr[data-bib="row"]')) return;
      if (stepBibSelection(e.key, e.shiftKey)) e.preventDefault();
      return;
    }

    // Ctrl sur Windows, ⌘ sur macOS. Accepter les deux plutôt que brancher sur `platform()` : ce
    // lookup a un chemin d'échec (chrome.ts se rabat sur la disposition Windows quand il jette),
    // et un raccourci est le mauvais endroit où en hériter — c'est exactement la raison pour
    // laquelle macOS n'a eu AUCUN Cmd+Z jusqu'au 2026-08-05.
    const mod = e.ctrlKey || e.metaKey;
    if (!mod || e.altKey) return;
    if (inTextField(e.target) && e.key !== "f" && e.key !== "F") return;

    // ⌘/Ctrl + 1…8 — n-ième destination du rail, dans l'ordre affiché.
    if (e.key >= "1" && e.key <= "9") {
      const views = railViews();
      const i = Number(e.key) - 1;
      if (i < views.length) {
        e.preventDefault();
        goTo(views[i]);
      }
      return;
    }

    switch (e.key) {
      case "f":
      case "F":
        // Ne prend la frappe que si la vue courante monte une recherche. Sinon on laisse passer :
        // avaler ⌘F sur un écran sans recherche donnerait un raccourci qui « ne marche pas », ce
        // qui se retient plus mal qu'un raccourci qui n'existe pas.
        //
        // View-aware : en Revue, la recherche vit dans la colonne file (queue-panel), pas dans la
        // barre unifiée. focusQueueSearch teste la présence de #sift-qsearch-input (aucun import
        // routeur) ; focusBarSearch reste le fallback des autres écrans.
        if (focusQueueSearch() || focusBarSearch()) e.preventDefault();
        return;
      case ",":
        e.preventDefault();
        goTo("reglages");
        return;
      case "b":
      case "B":
        e.preventDefault();
        toggleRail();
        return;
      case "a":
      case "A": {
        // Mode Lot de Revue (issue #60, 2026-09-10) : Ctrl+A = tout, Ctrl+Maj+A = aucune — les
        // deux entrées du menu Édition de Photos et Mail, que Sift n'a pas (#58).
        if (reviewMode === "batch" && document.getElementById("sift-qselmenu")) {
          e.preventDefault();
          if (e.shiftKey) clearQueueBatchSelection();
          else selectAllQueueBatch();
          return;
        }
        // ⌘/Ctrl+A n'agit QUE sur une table présente à l'écran. Ailleurs on laisse passer : la
        // sélection de texte du navigateur reste le comportement attendu partout ailleurs.
        if (!document.querySelector('.lr[data-bib="row"]')) return;
        e.preventDefault();
        selectAllVisible();
        void renderBiblioLive().then(renderSelectionSummary);
        return;
      }
      default:
        return;
    }
  });
}
