// Teinte d'identité d'une source du rail — logique pure, SANS import de `./ipc`, pour rester
// chargeable par Vitest en env Node (même séparation que `popover-position.ts`).
//
// Portée depuis l'ex-`home-sources.ts` (4befc09), perdue à la fusion 1 : ab64074 a réécrit le
// rendu du rail avec un repli neutre sur `color_key` null, et l'inventaire des survivants de
// 6d1cc85 (pickAndAddFolder, scanFailures) ne l'a pas comptée. DESIGN.md § 4 réserve les accents
// catégoriels aux taxonomies — « couleur de dossier source » en est une — et § 15 (fusion 1)
// prescrit « leur pastille de couleur » : la pastille identifie, un gris uniforme n'identifie
// rien, surtout en rail replié où elle est tout ce qui reste visible d'une source.
import type { Source } from "../shared/contracts";

/** Ordre gelé : il fixe quelle teinte reçoit chaque position d'ajout. Miroir des classes
 *  `.sift-rail-src-dot-<teinte>` de `styles.css` — épinglé par `test/source-color.test.ts`. */
export const SOURCE_HUE_CYCLE = ["indigo", "purple", "pink", "teal", "yellow"] as const;

/** Couleur d'identité d'une source : son override manuel si posé (`set_source_color`), sinon la
 *  teinte à sa position dans l'ordre d'ajout (id croissant — stable quel que soit l'ordre
 *  d'affichage de la liste passée), en cyclant sur les 5 teintes catégorielles. */
export function resolveSourceColorKey(sources: Source[], source: Source): string {
  if (source.color_key) return source.color_key;
  const sorted = [...sources].sort((a, b) => a.id - b.id);
  const idx = sorted.findIndex((s) => s.id === source.id);
  return SOURCE_HUE_CYCLE[idx % SOURCE_HUE_CYCLE.length];
}
