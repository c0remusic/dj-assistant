import { describe, expect, it } from "vitest";
import {
  DURATION_MISMATCH_SEC,
  HF_REF_HI,
  HF_REF_LO,
  decodedShortfallText,
  hfDensityParts,
  hfTopDensityParts,
  HF_TOP_REF_LO,
} from "../frontend/report-figures";

// `fmt` est injecté par report-view (virgule décimale française). Ici on en passe une version
// minimale : ce qui est testé est CE QUI EST DIT, pas le formatage des nombres.
const fmt = (v: number, d: number) => v.toFixed(d);

// Depuis la synthèse du 2026-09-07, chaque densité rend une PAIRE {value, ref} — la valeur en
// encre normale, la référence en tertiaire sur la même ligne. Les garanties de l'ancien texte
// unique valent pour la paire recomposée.
const joined = (p: { value: string; ref: string }) => `${p.value} ${p.ref}`;

describe("densité de l'aigu", () => {
  it("dit la mesure ET sa référence — une valeur seule ne situe rien", () => {
    const p = hfDensityParts(-3.2, fmt);
    expect(p.value).toContain("-3.2 dB");
    expect(p.ref).toContain(String(HF_REF_LO));
    expect(p.ref).toContain(String(HF_REF_HI));
  });

  it("situe sans juger : « dans » / « sous » la plage, jamais un verdict", () => {
    expect(hfDensityParts(-3.2, fmt).ref).toContain("dans la plage");
    expect(hfDensityParts(-12.4, fmt).ref).toContain("sous la plage");
  });

  // Le risque réel de cette ligne n'est pas un mauvais chiffre, c'est un mot qui transforme une
  // mesure en accusation. Un master volontairement sombre donne la même valeur qu'un transcodage :
  // le texte ne doit donc RIEN affirmer sur l'histoire du fichier.
  it("n'accuse jamais, même très en dessous de la plage", () => {
    const t = joined(hfDensityParts(-43.8, fmt)).toLowerCase();
    for (const mot of ["fake", "faux", "suspect", "transcod", "mp3", "lossy"]) {
      expect(t).not.toContain(mot);
    }
  });

  it("la borne basse elle-même compte comme DANS la plage", () => {
    // Elle est le minimum OBSERVÉ chez les authentiques, donc un fichier qui l'atteint exactement
    // est encore un cas connu — l'exclure inventerait une sévérité que la mesure ne porte pas.
    expect(hfDensityParts(HF_REF_LO, fmt).ref).toContain("dans la plage");
  });

  it("le fait « dans/sous » vit dans la RÉFÉRENCE, jamais dans la valeur", () => {
    // La valeur reste un chiffre nu (`-3.5 dB`) : c'est elle qui s'aligne en mono dans la
    // grille ; un mot dedans casserait la colonne et dupliquerait la référence.
    const p = hfDensityParts(-12.4, fmt);
    expect(p.value).toBe("-12.4 dB");
    expect(p.value).not.toContain("plage");
  });
});

describe("durée décodée (rangée Intégrité, conditionnelle)", () => {
  it("null quand les deux s'accordent — la rangée ne se rend pas", () => {
    expect(decodedShortfallText(212.4, 212.4, fmt)).toBeNull();
  });

  it("tolère l'écart de bourrage d'encodeur sans le montrer", () => {
    const justeEnDessous = 212.4 - DURATION_MISMATCH_SEC + 0.01;
    expect(decodedShortfallText(212.4, justeEnDessous, fmt)).toBeNull();
  });

  it("dit les deux durées quand elles divergent — c'est le désaccord qui informe", () => {
    const t = decodedShortfallText(400.0, 40.0, fmt);
    expect(t).toContain("40.0 s");
    expect(t).toContain("400.0 s annoncées");
  });

  // Un rapport écrit avant que la mesure existe porte 0 (le `#[serde(default)]` côté Rust).
  // L'afficher dirait « 0 s décodées » sur un fichier parfaitement sain : une absence de mesure
  // présentée comme une mesure, exactement le défaut que ce dépôt corrige partout.
  it("traite 0 comme « pas mesuré », jamais comme une durée nulle", () => {
    expect(decodedShortfallText(212.4, 0, fmt)).toBeNull();
  });
});

describe("densité du haut du spectre", () => {
  it("situe sans juger, comme l'autre bande", () => {
    expect(hfTopDensityParts(-3.0, fmt).ref).toContain("dans la plage");
    expect(hfTopDensityParts(-25.0, fmt).ref).toContain("sous la plage");
  });

  // Même garde que pour la bande fixe : la mesure ne distingue pas un master sombre d'un
  // transcodage, donc le texte ne doit rien affirmer sur l'histoire du fichier.
  it("n'accuse jamais", () => {
    const t = joined(hfTopDensityParts(-31.7, fmt)).toLowerCase();
    for (const mot of ["fake", "faux", "suspect", "transcod", "opus", "lossy"]) {
      expect(t).not.toContain(mot);
    }
  });

  // Les deux bandes ont des références DIFFÉRENTES, et les confondre ferait passer pour normal un
  // haut de spectre éteint (-8 dB est dans la plage relative, mais bien sous la plage fixe).
  it("n'utilise pas les bornes de la bande fixe", () => {
    expect(HF_TOP_REF_LO).not.toBe(HF_REF_LO);
    expect(hfTopDensityParts(-8.0, fmt).ref).toContain("dans la plage");
    expect(hfDensityParts(-8.0, fmt).ref).toContain("sous la plage");
  });
});
