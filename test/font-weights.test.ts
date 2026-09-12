// Un poids de fonte demandé par le CSS et jamais chargé ne produit aucune erreur : le moteur
// synthétise un gras approximatif ou retombe sur la famille suivante, et la page a l'air
// normale. C'est le mode de défaillance de l'issue #33 — 24 déclarations `font-weight:500`
// qui ne peignaient pas parce que la face n'était jamais importée.
//
// Le correctif de #33 a ajouté les imports manquants dans `frontend/main.ts`. Il n'a rien posé
// qui retienne la divergence : ajouter `font-weight:700` au CSS sans son
// `@fontsource/inter/700.css` rejoue le ticket, en silence.
//
// Le test lit les DEUX vrais fichiers et les confronte, sur le modèle de
// `analysis::spectrum::tests::css_data_measure_matches_max_cols` (issue #30), dont le
// commentaire écarte nommément l'alternative : « assumer la duplication avec un simple
// commentaire croisé — écarté, un commentaire ne tombe pas ».
//
// LIMITE ASSUMÉE : ce test ne résout pas la cascade. Il lit la famille déclarée DANS le même
// bloc que le poids, et rattache tout le reste à la famille par défaut du document
// (`html,body{font-family:var(--font-ui)}`, styles.css:366). Un bloc qui hérite de `--font-mono`
// d'un ancêtre sans le redéclarer serait donc classé en `--font-ui` — sous-couverture connue,
// jamais un faux positif.
import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";

const CSS = "frontend/styles.css";
const MAIN = "frontend/main.ts";

/** Paquet @fontsource porteur de chaque token de famille. */
const PAQUET: Record<string, string> = {
  "--font-ui": "inter",
  "--font-mono": "jetbrains-mono",
};

interface Besoin {
  paquet: string;
  poids: number;
  selecteur: string;
}

/** Poids numériques réellement DEMANDÉS par la feuille, avec le paquet qui doit les fournir. */
function besoins(): Besoin[] {
  const css = readFileSync(CSS, "utf8");
  const out: Besoin[] = [];

  // Découpage grossier en blocs `sélecteur { corps }`. Suffisant ici : la feuille est écrite en
  // une déclaration par ligne compactée, sans imbrication.
  for (const bloc of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const selecteur = bloc[1].trim().split(/\s*\n\s*/).pop() ?? "";
    const corps = bloc[2];

    // Un `@font-face` DÉCLARE une face, il n'en demande pas. Son `font-weight` est l'étiquette
    // du fichier fourni — le compter inverserait le sens du test.
    if (selecteur.startsWith("@font-face")) continue;

    const m = corps.match(/font-weight\s*:\s*(\d{3})\b/);
    if (!m) continue;

    const token = corps.includes("var(--font-mono)") ? "--font-mono" : "--font-ui";
    out.push({ paquet: PAQUET[token], poids: Number(m[1]), selecteur });
  }
  return out;
}

/** Faces réellement importées, telles que Vite les bundlera. */
function importes(): Set<string> {
  const ts = readFileSync(MAIN, "utf8");
  const set = new Set<string>();
  for (const m of ts.matchAll(/@fontsource\/([a-z0-9-]+)\/(\d{3})\.css/g)) {
    set.add(`${m[1]}/${m[2]}`);
  }
  return set;
}

describe("poids de fonte (issue #33)", () => {
  it("charge une face pour chaque poids que la feuille demande", () => {
    const charges = importes();
    const manquants = besoins()
      .filter((b) => !charges.has(`${b.paquet}/${b.poids}`))
      .map((b) => `${b.selecteur} demande ${b.paquet} ${b.poids}`);

    expect([...new Set(manquants)]).toEqual([]);
  });

  it("lit des imports réels — sinon le test passerait sur une feuille vide", () => {
    // Témoin, même raison que `spacingDeclsSeen` dans lint-tokens.mjs : une liste de manquants
    // vide et un parseur qui ne matche plus rien se ressemblent exactement.
    expect(importes().size).toBeGreaterThanOrEqual(4);
    expect(besoins().length).toBeGreaterThanOrEqual(30);
  });
});
