// La hauteur d'une rangée de file doit être DÉTERMINISTE, et rien dans le code ne le disait.
//
// Mode de défaillance, mesuré dans la vraie fenêtre le 2026-08-26 (issue #45) : avec
// `line-height:normal`, le moteur dérive la hauteur de la ligne des glyphes réellement présents
// dans le titre. Sur 47 rangées, 46 mesuraient 45px et une 46px, à `font-size` identique. Rien ne
// tombe, rien ne s'affiche en rouge — et `measureQueueRowHeight` met UNE hauteur en cache pour
// toute la file, donc le calcul de fenêtre de la virtualisation se décale sur des milliers de
// lignes à partir d'un pixel.
//
// Ce test lit les deux vrais fichiers, comme `font-weights.test.ts` et pour la même raison : la
// valeur vit dans `styles.css`, le markup qui en dépend vit dans `queue-panel.ts`, et une gate qui
// n'en lirait qu'un seul laisserait la divergence passer.
import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";

const CSS = readFileSync("frontend/styles.css", "utf8");
const PANEL = readFileSync("frontend/queue-panel.ts", "utf8");

/** Le corps de la règle CSS portant exactement ce sélecteur, en tête de son bloc de déclarations. */
function regle(selecteur: string): string {
  const m = CSS.match(new RegExp(`(?:^|\\n)${selecteur.replace(".", "\\.")}\\{([^}]*)\\}`));
  if (!m) throw new Error(`règle ${selecteur} introuvable dans styles.css`);
  return m[1];
}

describe("hauteur de rangée de file (#45)", () => {
  // Les deux valeurs viennent du kit macOS Big Sur (`docs/design-refs/Styleguide.pdf`, § 05
  // Typography) et non d'un arbitrage : Callout 12pt/15pt pour le titre, Caption 1 10pt/13pt pour
  // le sous-texte. Les changer demande de changer la référence, pas seulement le nombre.
  it("pose un interligne EXPLICITE en px sur la ligne de titre — Callout 12/15", () => {
    const decl = regle(".qi");
    expect(decl).toMatch(/font-size:12px/);
    expect(decl).toMatch(/line-height:15px/);
  });

  it("pose un interligne EXPLICITE en px sur la seconde ligne — Caption 1 10/13", () => {
    const decl = regle(".qi-sub");
    expect(decl).toMatch(/font-size:var\(--text-xs\)/);
    expect(decl).toMatch(/line-height:13px/);
  });

  it("laisse `normal` hors des deux règles — c'est lui qui rendait la hauteur non déterministe", () => {
    expect(regle(".qi")).not.toMatch(/line-height:\s*normal/);
    expect(regle(".qi-sub")).not.toMatch(/line-height:\s*normal/);
  });

  // La sonde de `measureQueueRowHeight` reconstruit le markup de la rangée. Si elle recopiait les
  // styles au lieu de partager la classe, elle mesurerait une rangée qui n'existe pas — et le
  // décalage serait invisible, puisque la sonde comme le rendu resteraient « corrects » chacun de
  // leur côté.
  it("fait partager la MÊME classe à la rangée peinte et à la sonde qui la mesure", () => {
    const occurrences = PANEL.match(/class="qi-sub"/g) ?? [];
    expect(occurrences.length).toBe(2);
    const sonde = PANEL.slice(PANEL.indexOf("function measureQueueRowHeight"));
    expect(sonde.slice(0, 1200)).toMatch(/class="qi-sub"/);
  });

  it("garde la seconde ligne hors des styles inline, où la sonde ne la verrait pas", () => {
    expect(PANEL).not.toMatch(/style="padding-left:15px;font-size:var\(--text-xs\)/);
  });
});
