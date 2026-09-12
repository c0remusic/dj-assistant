import { describe, expect, it } from "vitest";
import { ROOT_WARN_ID, rootWarningHtml } from "../frontend/rail-warn-card";

// Carte « racine non définie » du rail (issue #54 du 2026-09-02, direction A2). Elle remplace le
// bandeau `#sift-gate` supprimé le même jour.
//
// Ce qui est gelé ici, c'est ce qu'une inspection visuelle ne verrait PAS tomber : un attribut de
// navigation perdu (la carte devient décorative et le seul chemin vers Réglages hors du rail
// disparaît), une classe `.nv` prise au passage (la carte se lirait comme un huitième écran de
// navigation), un id instable (`paintRootWarning` s'appuie dessus pour rester idempotent — deux
// cartes empilées sinon, à chaque reconstruction de la section Sources).
//
// Le markup vit dans un module SANS import (`rail-warn-card.ts`) exactement pour être exécutable
// ici : la suite tourne en env Node, qui ne peut pas charger un module important `./ipc`.

describe("carte de racine manquante du rail", () => {
  it("navigue vers Réglages par data-view — le délégué de router.ts est le seul chemin", () => {
    expect(rootWarningHtml()).toContain('data-view="reglages"');
  });

  it("n'est PAS une entrée de navigation : aucune classe .nv", () => {
    const html = rootWarningHtml();
    // Test de la CLASSE, pas d'une sous-chaîne : « .sift-railwarn » ne contient pas « nv », mais un
    // futur `class="nv sift-railwarn"` doit tomber ici.
    const classAttr = /class="([^"]*)"/.exec(html)?.[1] ?? "";
    expect(classAttr.split(/\s+/)).toContain("sift-railwarn");
    expect(classAttr.split(/\s+/)).not.toContain("nv");
  });

  it("porte l'id stable dont le montage idempotent dépend", () => {
    expect(ROOT_WARN_ID).toBe("sift-railwarn");
    expect(rootWarningHtml()).toContain(`id="${ROOT_WARN_ID}"`);
  });

  it("est un vrai bouton nommé — type explicite et aria-label en une phrase", () => {
    const html = rootWarningHtml();
    expect(html.startsWith("<button ")).toBe(true);
    expect(html).toContain('type="button"');
    expect(/aria-label="[^"]{20,}"/.test(html)).toBe(true);
  });
});
