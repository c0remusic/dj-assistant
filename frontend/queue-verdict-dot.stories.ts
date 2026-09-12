import type { Meta, StoryObj } from "@storybook/html-vite";
import { MAX_ANALYSIS_ATTEMPTS } from "../shared/contracts";
import { verdictDot } from "./queue-verdict-dot";

// La pastille de verdict de la ligne de file (`queue-verdict-dot.ts`), cataloguée dans
// `design-system-states.md` § « Pastille de verdict de file ». Ces stories EXÉCUTENT le vrai rendu
// (`verdictDot`, module pur sans `./ipc`) au lieu d'en recopier le markup — modèle
// `rail-sources.stories.ts` : une copie ne peut que diverger.
//
// Cinq cas pour quatre rendus : les trois verdicts en teinte système PLEINE (décision 2026-08-27 —
// un indicateur d'état est une teinte système vive, jamais une encre de texte ; le point non-lu de
// Mail est systemBlue plein), l'échec terminal en pastille pleine qui PARTAGE sa teinte avec Faux
// (la distinction est le bouton Réanalyser, propre à la ligne non analysée), et l'attente en
// anneau neutre.
//
// LIMITE ASSUMÉE : la LIGNE de file entière (`queueRowHtml`, `.qi`) n'est pas reproduite ici —
// elle vit dans `queue-panel.ts`, qui importe `./ipc`. La rangée témoin ci-dessous ne montre que
// le FAIT documenté par la décision du 2026-08-27 : la pastille est collée à la FIN du titre
// (décision produit, assumée contre le motif Mail des indicateurs au bord droit).

/** Les cinq cas, dans l'ordre de la table de `design-system-states.md`. */
const CAS: readonly { label: string; verdict: "ok" | "fake" | "grey" | null; attempts: number }[] = [
  { label: "Authentique", verdict: "ok", attempts: 1 },
  { label: "Faux / sur-encodé", verdict: "fake", attempts: 1 },
  { label: "Zone grise", verdict: "grey", attempts: 1 },
  { label: "Analyse abandonnée", verdict: null, attempts: MAX_ANALYSIS_ATTEMPTS },
  { label: "En attente d'analyse", verdict: null, attempts: 0 },
];

function host(inner: string): HTMLElement {
  // Fond de la colonne file (`--color-background-queue`) : la teinte se juge sur la surface qui la
  // porte en prod, pas sur le canvas Storybook.
  const el = document.createElement("div");
  el.style.cssText =
    "max-width:320px;padding:var(--space-16);background:var(--color-background-queue);" +
    "color:var(--color-text-secondary);font-size:12px;line-height:15px";
  el.innerHTML = inner;
  return el;
}

const meta: Meta = {
  title: "États de contenu/File — pastille de verdict",
};

export default meta;
type Story = StoryObj;

/** Les cinq cas, pastille en fin de libellé comme dans la ligne réelle (gap 6px, celui de `.qi`). */
export const LesCinqCas: Story = {
  render: () =>
    host(
      CAS.map(
        (c) =>
          `<div style="display:flex;align-items:center;gap:6px;padding:var(--space-4) 0">` +
          `<span>${c.label}</span>` +
          verdictDot({ verdict: c.verdict, analysis_attempts: c.attempts }) +
          `</div>`,
      ).join(""),
    ),
};
