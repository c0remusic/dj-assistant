// Pastille de verdict de la ligne de file — module PUR (aucun import `./ipc`, aucun DOM),
// extrait de `queue-panel.ts` le 2026-08-27 pour que la story et un test env Node puissent
// exécuter le VRAI rendu au lieu d'en recopier le markup — même motif que `rail-source-entry.ts`
// et `popover-position.ts` (l'env Node de Vitest ne peut pas charger un module important `./ipc`).
// `queue-panel.ts` (queueRowHtml) reste l'unique appelant de prod.
import { MAX_ANALYSIS_ATTEMPTS, type QueueItem } from "../shared/contracts";

// Le « vert/ambre uniquement » du brief de refonte 2026-07 est PÉRIMÉ depuis la révision du
// 2026-08-19 : `fake` passe à `danger`. C'était le seul écran où un faux lossless se disait en
// ambre, c'est-à-dire du même ton que « à vérifier » — or « l'échec est l'information qu'on n'a pas
// le droit d'estomper » (§ 4), et c'est la raison d'être de l'app.
//
// La règle qui, elle, ne bouge pas : JAMAIS un hex en dur ici (l'ancien `#e2685e` rouge la
// cassait) — lire les tokens CSS, pas une 3ᵉ teinte inventée à côté.
// Teintes SYSTÈME vives, pas des encres de texte (décision 2026-08-27, maquette Figma « Maquette —
// Revue », composant Pastille de verdict) : un indicateur d'état est systemGreen/Red/Yellow plein —
// le point non-lu de Mail est systemBlue plein, jamais une couleur de label. Les encres text-*
// restent aux MOTS (badge LOSSLESS de zone C, libellés) ; la pastille seule porte le vif.
const VERDICT_DOT: Record<string, [string, string]> = {
  ok: ["var(--color-hue-green-solid)", "authentique"],
  fake: ["var(--color-hue-red-solid)", "faux / sur-encodé"],
  grey: ["var(--color-hue-yellow-solid)", "zone grise"],
};
/** La pastille porte le verdict À ELLE SEULE depuis le 2026-08-26 : le mot qui la doublait dans la
 *  ligne est retiré (« la pastille est là pour ça », Antoine), ce qui rend la file à `revue.md`
 *  § Zone B′, qui n'a jamais listé ce mot.
 *
 *  Conséquence directe, et c'est elle qui impose la signature élargie : `verdictWord` ne rendait pas
 *  que des verdicts, il rendait aussi les états de PIPELINE (« échec », « analyse… ») que la
 *  pastille ne portait pas — un `verdict` nul sortait en cercle vide, identique pour une analyse en
 *  cours et pour une analyse abandonnée. Couper le mot sans élargir la pastille aurait rendu les
 *  deux indistinguables, contre `revue.md` § États (« la ligne se voit MIEUX que les autres »).
 *
 *  Quatre rendus, donc, et non plus deux : les trois verdicts, l'échec terminal en pastille pleine
 *  danger, et l'attente en anneau. L'échec partage sa teinte avec `fake` — la distinction est portée
 *  par le bouton Réanalyser, qui n'existe que sur une ligne non analysée (voir `queueRowHtml`). */
export function verdictDot(it: Pick<QueueItem, "verdict" | "analysis_attempts">): string {
  const base = "flex:none;width:9px;height:9px;border-radius:50%";
  const v = it.verdict;
  if (v && VERDICT_DOT[v]) {
    const [c, title] = VERDICT_DOT[v];
    return `<span title="${title}" style="${base};background:${c}"></span>`;
  }
  if (it.analysis_attempts >= MAX_ANALYSIS_ATTEMPTS) {
    return `<span title="analyse abandonnée" style="${base};background:var(--color-hue-red-solid)"></span>`;
  }
  // not analysed yet
  return `<span title="en attente d'analyse" style="${base};border:1.5px solid var(--color-text-tertiary);box-sizing:border-box"></span>`;
}
