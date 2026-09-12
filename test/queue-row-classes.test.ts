// La ligne de file a rendu en JetBrains Mono 10px bordée, haute de 30px là où ses voisines font
// 46px, du jour où elle a porté le curseur clavier — parce que ce curseur était marqué `.kbd`, le
// nom de la classe GLOBALE de badge de touche (`styles.css`, `font-family:var(--font-mono)`,
// `font-size:10px`, bordure, rayon). Deux sens pour un nom, dans deux fichiers qui ne se lisent pas
// l'un l'autre. La file sautait de 16px à chaque flèche. Corrigé le 2026-08-26 en `.qi-kbd`.
//
// Aucune gate du dépôt ne pouvait le voir : `tsc` type les deux usages pareil, `eslint` ne connaît
// pas la feuille, `lint:tokens` cherche des littéraux et non des noms, et 10px est PILE le plancher
// typographique — `driver.mjs floor` passait donc au vert lui aussi. Il a fallu mesurer la vraie
// fenêtre.
//
// Ce test tient la place laissée vide. Il LIT les deux fichiers plutôt que d'importer le module :
// `queue-panel.ts` importe `./ipc`, que l'env Node de Vitest ne peut pas charger (même raison qui a
// fait extraire `popover-position.ts`). Modèle : `test/font-weights.test.ts`, qui confronte de même
// `styles.css` et `main.ts`.
//
// LIMITE ASSUMÉE : la portée est la ligne de file, pas le dépôt. Le test lit le seul `class="…"` du
// `<div>` racine de `queueRowHtml` et ne suit aucune classe ajoutée après coup par du JS
// (`classList.add`), ni les autres composants. Sous-couverture connue ; jamais un faux positif.
import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";

const QUEUE = "frontend/queue-panel.ts";
const CSS = "frontend/styles.css";

/** Classe racine du composant : sa règle nue dans `styles.css` est sa propre définition. */
const RACINE = "qi";

/** Classes que la ligne de file a le droit de porter sans préfixe : le composant et son état de
 *  sélection. Tout le reste doit être préfixé `qi-` — convention déjà tenue par `.qi-ck`,
 *  `.qi-dup`. */
const CLASSES_NUES_ADMISES = new Set([RACINE, "cur"]);

/** Les classes posées sur le `<div>` racine de `queueRowHtml`.
 *
 *  Le markup est un template littéral — `class="qi${a ? " cur" : ""}${b ? " qi-kbd" : ""}"` — donc
 *  les classes conditionnelles vivent dans des chaînes À L'INTÉRIEUR de l'attribut. On isole
 *  l'attribut par son voisin de droite (`" id=`), qui est stable et ne peut pas apparaître dans un
 *  ternaire de classe, puis on prend le littéral de tête et chaque chaîne entre guillemets.
 *
 *  ⚠️ La recherche part de `function queueRowHtml` et non du début du fichier. Elle partait du
 *  début jusqu'au 2026-08-26, et le premier `class="qi…` du fichier a cessé d'être le bon le jour
 *  où `measureQueueRowHeight` — déclarée PLUS HAUT — a reçu `class="qi-sub"` (issue #45) : la
 *  capture non-greedy s'étendait alors de la sonde jusqu'à l'attribut de `queueRowHtml`, et
 *  rendait 5010 « classes ». Le test tombait, à raison sur sa propre extraction et à tort sur le
 *  markup. Ancrer sur la fonction rend la lecture indifférente à ce qui vit ailleurs. */
function classesDeLaLigne(): string[] {
  const fichier = readFileSync(QUEUE, "utf8");
  const debut = fichier.indexOf("function queueRowHtml");
  if (debut < 0) throw new Error(`${QUEUE} : fonction queueRowHtml introuvable`);
  const src = fichier.slice(debut);
  const m = /class="(qi.*?)" id="qi-\$\{/s.exec(src);
  if (!m) throw new Error(`${QUEUE} : attribut class du <div> racine de queueRowHtml introuvable`);
  const blob = m[1];
  const tete = blob.split("${")[0]; // "qi"
  const conditionnelles = [...blob.matchAll(/"([^"]*)"/g)].map((c) => c[1]);
  return [tete, ...conditionnelles]
    .flatMap((s) => s.split(/\s+/))
    .map((s) => s.trim())
    .filter(Boolean);
}

/** Classes que `styles.css` définit par une règle NUE — `.foo{…}`, seule, sans contexte de
 *  composant — ET qui y posent une typographie. Ce sont celles qu'un nœud ne peut pas porter sans
 *  avaler leur rendu, quel que soit le composant où il vit : la forme exacte du bug de `.kbd`.
 *  `.qi.cur{…}` n'en fait pas partie, son rendu est contextualisé. */
function classesNuesTypographiques(): Set<string> {
  const css = readFileSync(CSS, "utf8");
  const sansCommentaires = css.replace(/\/\*[\s\S]*?\*\//g, "");
  const out = new Set<string>();
  for (const m of sansCommentaires.matchAll(/(?:^|\})\s*\.([a-z][a-z0-9-]*)\s*\{([^}]*)\}/g)) {
    if (/font-family|font-size/.test(m[2])) out.add(m[1]);
  }
  return out;
}

describe("classes de la ligne de file", () => {
  it("extrait des classes réelles (le test est branché sur le vrai markup)", () => {
    const classes = classesDeLaLigne();
    expect(classes.length).toBeGreaterThanOrEqual(2);
    expect(classes).toContain("qi");
  });

  it("n'utilise que `qi`, `cur` ou un préfixe `qi-`", () => {
    const horsConvention = classesDeLaLigne().filter(
      (c) => !CLASSES_NUES_ADMISES.has(c) && !c.startsWith("qi-"),
    );
    expect(horsConvention).toEqual([]);
  });

  // Le critère exact, et il n'est PAS « cette classe est globale » : `.qi` l'est aussi, et c'est sa
  // propre définition. Ce qui a mordu, c'est une règle NUE (`.kbd{…}`, sans contexte de composant)
  // qui pose une typographie et que d'autres écrans posent sur leurs propres nœuds — la ligne
  // l'avalait en entier. `.cur`, portée elle aussi par les lignes de Bibliothèque, ne mord pas :
  // elle n'a aucune règle nue, seulement `.qi.cur{…}` et `.lr.cur{…}`. Un nom partagé est sain
  // tant que son rendu est contextualisé ; c'est le rendu nu qui traverse les composants.
  it("ne porte, hors sa classe racine, aucune classe à règle NUE qui pose une typographie", () => {
    const nues = classesNuesTypographiques();
    // Compte positif : une feuille mal lue rendrait cet ensemble vide et le test garderait le vide
    // en passant au vert — le mode de défaillance que le dépôt refuse (« une mesure vide n'est pas
    // un pass »). `.kbd`, la classe du bug d'origine, doit y figurer.
    expect(nues.size).toBeGreaterThan(0);
    expect(nues).toContain("kbd");
    const collisions = classesDeLaLigne().filter((c) => c !== RACINE && nues.has(c));
    expect(collisions).toEqual([]);
  });
});
