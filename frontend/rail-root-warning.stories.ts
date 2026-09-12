import type { Meta, StoryObj } from "@storybook/html-vite";
import { rootWarningHtml } from "./rail-warn-card";

// Rappel « racine de bibliothèque non définie », logé sous la section Sources du rail (issue #54,
// 2026-09-02, direction A2). Il remplace le bandeau pleine largeur `#sift-gate`, supprimé le même
// jour : la racine a cessé d'être un prérequis de la conversion, donc le rappel n'a plus à occuper
// toute la fenêtre sur tous les écrans. Catalogué dans `design-system-states.md` § « Pattern
// d'erreur/échec » (ligne `.sift-railwarn`) et dans `docs/ui-specs/rail.md` § États.
//
// La story EXÉCUTE le vrai rendu (`rootWarningHtml`) — modèle `rail-sources.stories.ts` : recopier
// le markup garantirait la divergence.
//
// Le conteneur reprend la vraie charpente du rail (`.sb`, `index.html`) : la carte se juge sur la
// teinte qui la porte en prod (rail EN RETRAIT, plan le plus bas des trois) et sur la mesure que
// `--rail-w` donne à ses deux lignes — pas sur le fond du canvas.
//
// ÉTAT NON REPRÉSENTABLE ICI : le rail replié (`body.sift-rail-collapsed`), où la carte disparaît
// entière — la classe vit sur `<body>`, hors de portée d'une story statique.

function railHost(inner: string): HTMLElement {
  const host = document.createElement("div");
  host.className = "sb";
  host.innerHTML = inner;
  return host;
}

const meta: Meta = {
  title: "Rail/Rappel de racine",
};
export default meta;

/** Le seul état visible : la racine manque. Une racine posée ne rend rien du tout — la carte est
 *  retirée du DOM, pas masquée, donc il n'y a pas de second rendu à cataloguer. */
export const RacineNonDefinie: StoryObj = {
  render: () =>
    railHost(
      `<div class="nv-grp">Sources</div>` +
        `<button class="nv sift-rail-src-add" type="button"><span>Ajouter un dossier</span></button>` +
        rootWarningHtml(),
    ),
};
