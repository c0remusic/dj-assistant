// Géométrie d'ancrage d'un popover `position:fixed`, sans DOM.
//
// Module séparé de `filing-bins.ts` pour une raison de test, pas d'esthétique : `filing-bins.ts`
// importe `./ipc` et `@tauri-apps/plugin-dialog` au niveau module, et la suite Vitest tourne en
// environnement Node sans Tauri (voir `vitest.config.ts`). Ici il n'y a que des nombres — donc le
// calcul qui décide si le popover sort de la fenêtre est couvert par un test, alors que la branche
// `flip` est inatteignable dans la vraie fenêtre (le bouton Destination vit dans la barre d'action
// ancrée en bas : il y a toujours plus de place au-dessus qu'en dessous).
//
// Le rendu, lui, reste vérifié dans la vraie fenêtre — issue #27, mesures du 2026-08-13.

export const POPOVER_GAP = 8; // entre le bouton déclencheur et le popover
export const POPOVER_MARGIN = 8; // entre le popover et le bord de la fenêtre

/** Rectangle du déclencheur, dans le repère de `getBoundingClientRect`. */
export interface AnchorRect {
  top: number;
  bottom: number;
  left: number;
}

/** Résultat d'un placement : coordonnées `position:fixed` + le côté réellement retenu
 *  (`above`/`below`). Le côté est décidé UNE fois, ici, avec la position — un consommateur qui doit
 *  s'orienter selon le placement (bec, ombre directionnelle, origine d'animation) le lit plutôt que
 *  de recalculer le flip à côté, ce qui divergerait au moindre écart.
 *
 *  ⚠️ Aucun consommateur de production à ce jour : le bec du popover Destination, qui devait le lire,
 *  a été retiré le 2026-08-25 (un pulldown macOS n'a pas de bec — cf. revue.md). Le champ reste :
 *  extension non-cassante (les appelants ne déstructurent que `{ top, left }`), gelée par
 *  `test/popover-position.test.ts`, prête pour un futur consommateur directionnel. */
export interface PopoverPosition {
  top: number;
  left: number;
  placement: "above" | "below";
}

/** Position d'un popover ancré au-dessus de son déclencheur, ramené dans la fenêtre.
 *
 *  `vw`/`vh` sont le viewport de MISE EN PAGE (`documentElement.clientWidth/clientHeight`), pas
 *  `window.innerWidth/innerHeight` : le second a déjà divergé du repère de `getBoundingClientRect`
 *  dans le webview Tauri (voir le commentaire de `positionDestPopover`), le premier ne le peut pas.
 *
 *  Ordre repris de Floating UI — flip sur l'axe principal, shift sur l'axe croisé. Lu, pas
 *  installé : l'emprunt est une question ouverte de la map #6. */
export function destPopoverPosition(
  btn: AnchorRect,
  popW: number,
  popH: number,
  vw: number,
  vh: number,
): PopoverPosition {
  // Vertical : au-dessus par défaut. Bascule en dessous seulement si ça ne tient pas au-dessus ET
  // qu'il y a plus de place en dessous — quand aucun des deux ne tient, garder le côté le plus
  // large perd le moins de contenu.
  const roomAbove = btn.top - POPOVER_GAP - POPOVER_MARGIN;
  const roomBelow = vh - btn.bottom - POPOVER_GAP - POPOVER_MARGIN;
  const flipBelow = popH > roomAbove && roomBelow > roomAbove;
  const top = flipBelow ? btn.bottom + POPOVER_GAP : btn.top - popH - POPOVER_GAP;
  // Horizontal : ne bascule jamais — le popover est aligné à gauche sur le bouton et ne fait que
  // se recaler dans la fenêtre.
  return {
    top: clampToViewport(top, popH, vh),
    left: clampToViewport(btn.left, popW, vw),
    placement: flipBelow ? "below" : "above",
  };
}

/** Position d'un popover ancré SOUS son déclencheur, ramené dans la fenêtre.
 *
 *  Jumelle de `destPopoverPosition`, et l'inverse de son défaut : celle-là sert un bouton ancré en
 *  bas de fenêtre (barre d'action), celle-ci un bouton en HAUT — le sélecteur de facette, qui vit
 *  en tête de la table. Réutiliser la première y donnait le mauvais côté : avec un bouton à ~360px
 *  du haut, `roomAbove` (≈344) dépasse la hauteur du popover (340), donc rien ne basculait et le
 *  panneau se serait ouvert vers le haut, par-dessus la barre unifiée.
 *
 *  Même ordre que sa jumelle — flip sur l'axe principal, shift sur l'axe croisé — seul le côté
 *  préféré change. */
export function anchoredBelowPosition(
  btn: AnchorRect,
  popW: number,
  popH: number,
  vw: number,
  vh: number,
): PopoverPosition {
  const roomBelow = vh - btn.bottom - POPOVER_GAP - POPOVER_MARGIN;
  const roomAbove = btn.top - POPOVER_GAP - POPOVER_MARGIN;
  const flipAbove = popH > roomBelow && roomAbove > roomBelow;
  const top = flipAbove ? btn.top - popH - POPOVER_GAP : btn.bottom + POPOVER_GAP;
  return {
    top: clampToViewport(top, popH, vh),
    left: clampToViewport(btn.left, popW, vw),
    placement: flipAbove ? "above" : "below",
  };
}

/** Maintient un segment de longueur `size` démarrant en `start` dans un axe long de `viewport`, en
 *  laissant `POPOVER_MARGIN` à chaque bout. La borne basse est appliquée EN DERNIER : un popover
 *  plus grand que la fenêtre se colle au bord haut au lieu d'être poussé au-delà. Inatteignable
 *  aujourd'hui (`minHeight` 640 dans `tauri.conf.json` contre un `max-height` de 340 px, mesuré à
 *  222 px dans la vraie fenêtre) — seulement si ce plancher baissait ou si le `max-height` montait. */
export function clampToViewport(start: number, size: number, viewport: number): number {
  return Math.max(POPOVER_MARGIN, Math.min(start, viewport - size - POPOVER_MARGIN));
}
