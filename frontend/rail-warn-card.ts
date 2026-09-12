// Markup SEUL de la carte « racine de bibliothèque non définie » du rail (issue #54 du
// 2026-09-02, direction A2). Aucun import, aucun `document`, aucun état.
//
// Séparé de `rail-root-warning.ts` (qui lit le réglage et monte le nœud) pour la même raison que
// `popover-position.ts` l'est de `filing-bins.ts` : la suite Vitest tourne en env Node, qui ne peut
// pas charger un module important `./ipc`. C'est ce fichier-ci que le test et la story exécutent —
// jamais une copie de son markup, qui ne pourrait que diverger.

/** Id du nœud monté. Exporté parce que le montage (`rail-root-warning.ts`) l'interroge pour rester
 *  idempotent, et que le test le vérifie : un id instable ferait empiler les cartes. */
export const ROOT_WARN_ID = "sift-railwarn";

/** La carte : un bouton `data-view="reglages"` — le délégué unique de `router.ts` sur `#pa` couvre
 *  le rail, donc naviguer ne demande aucune dépendance au routeur.
 *
 *  PAS de classe `.nv` : ce n'est pas une entrée de navigation du rail mais une carte d'état, et
 *  `.nv` lui donnerait le gabarit (hauteur, encre tertiaire, survol) des items de nav — elle se
 *  lirait comme un huitième écran.
 *
 *  Texte entièrement littéral : aucune donnée n'entre ici, donc rien à passer par `esc()`.
 *  `aria-label` explicite parce que le contenu visible est en deux nœuds et que le nom accessible
 *  concaténé se terminerait par un chevron typographique. */
export function rootWarningHtml(): string {
  return (
    `<button id="${ROOT_WARN_ID}" class="sift-railwarn" type="button" data-view="reglages" ` +
    `aria-label="Racine de bibliothèque non définie — ouvrir les Réglages pour la choisir">` +
    `<strong>Racine non définie</strong>` +
    `<span>Choisir dans Réglages ›</span>` +
    `</button>`
  );
}
