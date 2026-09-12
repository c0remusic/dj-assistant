// Rappel « racine de bibliothèque non définie », logé au rail — direction A2 de l'issue #54
// (2026-09-02). Il remplace le bandeau pleine largeur `#sift-gate`, retiré dans le même geste.
//
// Pourquoi il change de forme : depuis #54 la racine n'est plus un prérequis de la conversion —
// on convertit EN PLACE ou vers un dossier externe sans en avoir aucune. Un bandeau permanent
// au-dessus de la charpente entière criait donc un prérequis qui ne mord qu'à une destination
// précise (un bac de l'arbre). Le rappel reste, mais logé : carte ambre compacte sous la section
// Sources, là où on ajoute déjà des dossiers.
//
// Ce module ne dépend que d'`./ipc` et du markup pur : la navigation passe par
// `data-view="reglages"`, capté par le délégué unique de `router.ts` sur `#pa` (le rail en fait
// partie). Aucun import de `router` ni de `rail-sources` — c'est ce qui permet à Réglages ET au
// popover de destination de le rafraîchir sans refermer un cycle d'import.
//
// Le markup vit dans `rail-warn-card.ts`, sans aucun import : c'est ce qui le rend exécutable par
// la story ET par la suite Vitest en env Node, qui ne peut pas charger un module important `./ipc`.
import { getSetting } from "./ipc";
import { ROOT_WARN_ID, rootWarningHtml } from "./rail-warn-card";

const HOST_ID = "sift-rail-sources";

/** Dernier fait MESURÉ. `false` au démarrage : tant que le réglage n'a pas été lu, on n'affirme
 *  pas qu'il manque une racine — une carte peinte sur une lecture qui n'a pas eu lieu serait la
 *  même faute que la porte qui s'affichait sur un échec de lecture (impasse A8, issue #15). */
let rootMissing = false;

/** (Re)peint la carte depuis l'état déjà mesuré, sans relire le réglage.
 *
 *  Appelée par `rail-sources.ts` après chaque reconstruction de la section : ce rendu-là écrase la
 *  section par `innerHTML`, donc la carte part avec. Sur le chemin rapide de ce même rendu (celui
 *  qui ne fait que muter les lignes), la carte survit et cette fonction n'a rien à faire — d'où
 *  l'idempotence : elle ne crée le nœud que s'il manque. */
export function paintRootWarning(): void {
  const host = document.getElementById(HOST_ID);
  if (!host) return;
  const existing = document.getElementById(ROOT_WARN_ID);
  if (!rootMissing) {
    existing?.remove();
    return;
  }
  if (existing) return;
  host.insertAdjacentHTML("beforeend", rootWarningHtml());
}

/** Relit `library_root` et repeint. Le réglage est la source de vérité, jamais un état local :
 *  Réglages et le popover de destination peuvent en poser une à tout moment, et le rappel doit
 *  tomber tout de suite — c'est ce que l'ancienne porte ne faisait pas (elle n'était relue qu'au
 *  démarrage, donc restait à l'écran après un premier réglage réussi). */
export async function refreshRootWarning(): Promise<void> {
  let root: string | null;
  try {
    root = await getSetting("library_root");
  } catch (e) {
    // Échec de lecture : ne RIEN affirmer. Peindre la carte dirait « aucune racine », un fait non
    // mesuré ; la retirer dirait l'inverse. On laisse l'état précédent et on journalise.
    console.error("getSetting(library_root) failed", e);
    return;
  }
  rootMissing = !(root && root.trim());
  paintRootWarning();
}
