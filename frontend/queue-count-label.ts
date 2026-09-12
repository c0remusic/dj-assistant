// Libellé du compte de la file, seul compte de l'écran Revue (barre unifiée, contre le titre).
//
// Module SANS DOM, comme `popover-position.ts` et `source-color.ts` : `queue-panel.ts` importe
// `./ipc`, qui ne se charge pas en env Node — la règle qui décide CE QUE le compte dit vit donc ici
// pour être gelée par Vitest, et `paintBarCount` ne garde que l'écriture DOM.
//
// MOTIF MAIL (`docs/design-refs/03-mail.png`, fenêtre claire) : l'en-tête de colonne de Mail tient
// deux lignes — « Inbox » en gras, « 34 messages » dessous — et le BOUTON DE FILTRE est sur cette
// même rangée, à droite du compte. Chez Apple le compte est donc le voisin immédiat du filtre, et il
// décrit ce que la colonne CONTIENT. Sift a déplacé le nom de la liste et son compte dans la barre
// unifiée (règle tranchée le 2026-08-26 : le compte va contre le nom de la liste, à un seul
// endroit), et le compte y disait la file ENTIÈRE quel que soit le filtre posé — c'était l'écart au
// motif, et la perte suivie par l'issue #49.
//
// ⚠️ Ce que Sift n'hérite PAS de Mail, et que le mot « filtrées » compense : chez Mail le pulldown
// qui nomme le critère est à quelques dizaines de pixels du compte, donc un nombre nu s'y lit sans
// ambiguïté. Dans Sift le compte est dans la barre et le pulldown (« Faux + Doublons ») en tête de
// colonne, à ~1 000 px : un « 139 pistes » nu se lirait comme la taille de la file. Le qualificatif
// rétablit ce que la distance a coûté, SANS ajouter un second nombre — la spec (§ Zone A) interdit
// deux nombres côte à côte dans ce nœud, qui se liraient comme une fraction.
export type QueueCountInput = {
  /** Mode de revue courant. En Lot le compte dit la SÉLECTION et remplace le total (spec § Zone A). */
  mode: "detail" | "batch";
  /** Taille de la sélection du mode Lot. Ignoré hors mode Lot. */
  selected: number;
  /** File entière (`currentItems`), avant recherche et facettes. */
  total: number;
  /** Ce que la colonne montre : après recherche ET facettes (`visibleQueueItems()`). */
  visible: number;
  /** Vrai dès qu'une facette est cochée OU qu'une recherche est saisie. */
  filtered: boolean;
};

/** Accord français du dépôt : 0 et 1 prennent le singulier, 2 et au-delà le pluriel. Même règle que
 *  le compte jumeau de la Bibliothèque (`bibliotheque-view.ts`, `.sift-bib-count`). */
function s(n: number): string {
  return n > 1 ? "s" : "";
}

/** Le texte exact de `#sift-tb-count`. Trois états, dans cet ordre de priorité :
 *
 *  1. **mode Lot** — « N sélectionnée(s) ». Il REMPLACE le total au lieu de s'y ajouter, et il ne
 *     tient aucun compte du filtre : la sélection est ce que le lot va traiter, filtre ou pas.
 *  2. **filtre ou recherche posé** — « N piste(s) filtrée(s) », N = ce que la colonne montre.
 *  3. **rien de posé** — « N piste(s) », N = la file entière.
 *
 *  L'état 2 est le retour chiffré du filtre (issue #49) : le pulldown résume la COMBINAISON cochée
 *  en toutes lettres, ce compte dit ce qu'elle LAISSE VOIR. */
export function queueCountLabel(i: QueueCountInput): string {
  if (i.mode === "batch") return `${i.selected} sélectionnée${s(i.selected)}`;
  if (i.filtered) return `${i.visible} piste${s(i.visible)} filtrée${s(i.visible)}`;
  return `${i.total} piste${s(i.total)}`;
}
