// Joint la vérité terrain (`labels.json` de make-corpus.mjs) avec la mesure du détecteur (sortie
// CSV de `analysis::corpus::corpus_scan`) et rend la matrice de confusion.
//
//   node scripts/score-corpus.mjs <labels.json> <scan.csv>
//
// Ce que ce tableau dit et que « les tests passent » ne dit pas :
//   * FAUX POSITIF (authentique jugé Fake) — le plus cher : un bon fichier part à re-sourcer.
//   * FAUX NÉGATIF (faux jugé Ok) — un transcodage entre en bibliothèque sans être vu.
//   * la ventilation PAR ENCODEUR, qui localise les angles morts au lieu de les moyenner. C'est
//     la seule colonne qui répond à « le détecteur marche-t-il », parce que la moyenne d'un
//     détecteur aveugle à une famille entière et parfait sur une autre ne veut rien dire.
//
// Depuis le 2026-08-18 il mesure AUSSI les deux bandes de platitude, que `verdict()` lit
// désormais. Ces chiffres-là vivaient dans des scripts ad-hoc jetés avec leur session, et le
// review a fini par citer des taux qu'on ne savait plus refaire — 77 % puis 68 %, sans commande
// pour les rejouer. Ils se recalculent maintenant d'un coup, à partir du même CSV.

import { readFileSync } from "node:fs";

const [LABELS, CSV] = process.argv.slice(2);
if (!LABELS || !CSV) {
  console.error("usage: node scripts/score-corpus.mjs <labels.json> <scan.csv>");
  process.exit(1);
}

const labels = JSON.parse(readFileSync(LABELS, "utf8"));

/** Les seuils se LISENT dans le code qui JUGE, ils ne se recopient pas ici.
 *
 *  Et c'est `verdict.rs`, pas `report-figures.ts` : les bornes du TS servent à situer la mesure
 *  dans l'affichage et viennent de `hf-flatness-probe.mjs` (mono 44,1 kHz, DFT naïve, 200 trames),
 *  pas du chemin qui décide. Les scorer ici ferait mesurer un détecteur avec les seuils d'un autre
 *  — l'erreur exacte qui a fait passer un achat en Douteux le 2026-08-18. */
function judgeThreshold(name) {
  const src = readFileSync("src-tauri/src/analysis/verdict.rs", "utf8");
  const m = src.match(new RegExp(`const ${name}: f32 = (-?[0-9.]+);`));
  if (!m) {
    throw new Error(`${name} introuvable dans src-tauri/src/analysis/verdict.rs — seuil non lisible`);
  }
  return Number(m[1]);
}
const HF_REF_LO = judgeThreshold("HF_FIXED_FLOOR_DB");
const HF_TOP_REF_LO = judgeThreshold("HF_TOP_FLOOR_DB");

const lines = readFileSync(CSV, "utf8").split(/\r?\n/);
const header = lines.find((l) => l.startsWith("rail;"));
// Fail fast plutôt que deviner : un CSV d'avant les colonnes de platitude produirait des `null`
// silencieux, donc un tableau de zéros indiscernable de « aucune détection ».
if (!header || !header.includes("hf_flat_db")) {
  console.error("CSV sans colonnes de platitude — re-scanner avec le corpus_scan a jour");
  process.exit(1);
}
const NCOL = header.split(";").length - 1; // le nom de fichier est le dernier champ

const rows = lines
  .filter((l) => l.includes(";") && !l.startsWith("rail;") && !l.startsWith("--"))
  .map((l) => {
    // Le nom est en DERNIER et se prend comme « tout ce qui reste » : un `;` dans un titre ne
    // peut donc plus décaler les colonnes. Mesuré le 2026-08-18 — 4 lignes sur 967 étaient
    // tordues quand le nom venait en premier, et le verdict lu était un bout de titre.
    const parts = l.split(";");
    const [rail, bitrate, cutoff, verdict, est, hf, hfTop] = parts;
    const file = parts.slice(NCOL).join(";");
    const num = (s) => (s === "-" || s === undefined || s === "" ? null : Number(s));
    return {
      file,
      rail,
      bitrate,
      cutoff: Number(cutoff),
      verdict,
      est,
      hf: num(hf),
      hfTop: num(hfTop),
    };
  });

const byFile = new Map(rows.map((r) => [r.file, r]));

const missing = [];
const joined = [];
for (const l of labels) {
  const m = byFile.get(l.file);
  if (!m) {
    missing.push(l.file);
    continue;
  }
  joined.push({ ...l, ...m });
}

// Compte positif AVANT tout jugement : un tableau de zéros et un corpus non mesuré se ressemblent,
// et c'est le défaut que ce dépôt corrige partout ailleurs.
console.log(`etiquettes: ${labels.length}   mesures: ${rows.length}   jointes: ${joined.length}`);
console.log(`seuils lus dans verdict.rs: fixe ${HF_REF_LO}   relative ${HF_TOP_REF_LO}`);
if (missing.length) {
  const head = missing.slice(0, 6).join(", ");
  console.log(`NON MESURES (${missing.length}): ${head}${missing.length > 6 ? " ..." : ""}`);
}
const errored = joined.filter((j) => j.rail === "ERREUR");
if (errored.length) {
  // Une ligne en erreur n'est ni un Ok ni un Fake : la sortir du dénominateur EXPLICITEMENT,
  // plutôt que la laisser diluer un taux qu'on va citer.
  console.log(`EN ERREUR D'ANALYSE (${errored.length}): ${errored.map((e) => e.file).join(", ")}`);
}
if (!joined.length) {
  console.log("aucune jointure — rien a conclure");
  process.exit(1);
}

const scored = joined.filter((j) => j.rail !== "ERREUR");
const cell = (truth, verdict) => scored.filter((j) => j.truth === truth && j.verdict === verdict).length;
const nGen = scored.filter((j) => j.truth === "genuine").length;
const nFake = scored.filter((j) => j.truth === "fake").length;

console.log("\n=== MATRICE DE CONFUSION ===");
console.log("verite \\ verdict        Ok    Grey    Fake   total");
for (const t of ["genuine", "fake"]) {
  const [ok, grey, fake] = ["Ok", "Grey", "Fake"].map((v) => cell(t, v));
  console.log(
    `${t.padEnd(20)} ${String(ok).padStart(4)} ${String(grey).padStart(7)} ` +
      `${String(fake).padStart(7)} ${String(ok + grey + fake).padStart(7)}`,
  );
}

const pct = (n, d) => (d ? ` = ${((100 * n) / d).toFixed(1)} %` : "");
console.log(`\nFAUX POSITIFS (authentique -> Fake) : ${cell("genuine", "Fake")}/${nGen}${pct(cell("genuine", "Fake"), nGen)}`);
console.log(`  dont zone grise (authentique -> Grey) : ${cell("genuine", "Grey")}/${nGen}`);
console.log(`FAUX NEGATIFS (faux -> Ok)          : ${cell("fake", "Ok")}/${nFake}${pct(cell("fake", "Ok"), nFake)}`);
console.log(`  rattrapes en Grey (faux -> Grey)      : ${cell("fake", "Grey")}/${nFake}`);

console.log("\n=== PAR ENCODEUR (cote faux uniquement) ===");
console.log("variante        n   detecte  rate   Grey   cutoff min..max");
const variants = [...new Set(scored.filter((j) => j.truth === "fake").map((j) => j.via))].sort();
for (const v of variants) {
  const g = scored.filter((j) => j.truth === "fake" && j.via === v);
  const det = g.filter((j) => j.verdict === "Fake").length;
  const miss = g.filter((j) => j.verdict === "Ok").length;
  const grey = g.filter((j) => j.verdict === "Grey").length;
  const cuts = g.map((j) => j.cutoff).filter((c) => Number.isFinite(c));
  const range = cuts.length ? `${Math.min(...cuts).toFixed(0)}..${Math.max(...cuts).toFixed(0)}` : "-";
  console.log(
    `${v.padEnd(14)} ${String(g.length).padStart(3)} ${String(det).padStart(8)} ` +
      `${String(miss).padStart(6)} ${String(grey).padStart(6)}   ${range}${miss > 0 ? "  <-- RATE" : ""}`,
  );
}

console.log("\n=== COTE AUTHENTIQUE, cutoff par fichier ===");
for (const j of scored.filter((x) => x.truth === "genuine")) {
  console.log(`${j.verdict.padEnd(5)} ${String(j.cutoff).padStart(6)}  ${j.source}`);
}

// ── PLATITUDE — les deux bandes, telles que le verdict les lit ──────────────────────────────
//
// Ce bloc isole ce que les bandes font SEULES, alors que la matrice du haut mélange leur effet
// avec celui de la coupure. Il sert à répondre « laquelle voit quoi », pas « le détecteur
// détecte-t-il » — et un fichier peut y compter comme vu tout en étant déjà Fake par sa falaise.

const measured = scored.filter((j) => j.hf !== null && j.hfTop !== null);
const unmeasured = scored.length - measured.length;
console.log("\n=== BANDES DE PLATITUDE (hors verdict livre) ===");
console.log(
  `mesurees: ${measured.length}/${scored.length}` +
    (unmeasured ? `   SANS PLATITUDE: ${unmeasured}` : ""),
);

const fakes = measured.filter((j) => j.truth === "fake");
const gens = measured.filter((j) => j.truth === "genuine");
const underFixed = (j) => j.hf < HF_REF_LO;
const underTop = (j) => j.hfTop < HF_TOP_REF_LO;
const union = (j) => underFixed(j) || underTop(j);

const rate = (set, f) => `${set.filter(f).length}/${set.length}${pct(set.filter(f).length, set.length)}`;
console.log(`bande fixe seule      faux ${rate(fakes, underFixed)}   authentiques touches ${rate(gens, underFixed)}`);
console.log(`bande relative seule  faux ${rate(fakes, underTop)}   authentiques touches ${rate(gens, underTop)}`);
console.log(`UNION                 faux ${rate(fakes, union)}   authentiques touches ${rate(gens, union)}`);

console.log("\n--- union par encodeur ---");
console.log("variante        n   fixe  relative  union");
for (const v of variants) {
  const g = fakes.filter((j) => j.via === v);
  if (!g.length) continue;
  console.log(
    `${v.padEnd(14)} ${String(g.length).padStart(3)} ${String(g.filter(underFixed).length).padStart(6)} ` +
      `${String(g.filter(underTop).length).padStart(9)} ${String(g.filter(union).length).padStart(6)}`,
  );
}

// ── CE QUE L'UNION RATE, ET DE COMBIEN ──────────────────────────────────────────────────────
//
// La distance au seuil sépare deux diagnostics qu'un taux confond : un raté à 0,2 dB est une
// MARGE — le signal voit la chose, la borne est au mauvais endroit — là où un raté à 15 dB est un
// TROU : ce fichier ressemble vraiment à un master sur cet axe.
const missed = fakes.filter((j) => !union(j));
const distance = (j) => Math.min(j.hf - HF_REF_LO, j.hfTop - HF_TOP_REF_LO);
console.log(`\n=== CE QUE L'UNION RATE : ${missed.length}/${fakes.length}${pct(missed.length, fakes.length)} ===`);
console.log("variante        ratés   distance au seuil le plus proche (dB)");
for (const v of variants) {
  const g = missed.filter((j) => j.via === v);
  if (!g.length) continue;
  const d = g.map(distance);
  const n = fakes.filter((j) => j.via === v).length;
  console.log(
    `${v.padEnd(14)} ${String(g.length).padStart(4)}/${String(n).padEnd(4)} ` +
      `${Math.min(...d).toFixed(2)} à ${Math.max(...d).toFixed(2)}`,
  );
}
if (missed.length) {
  const d = missed.map(distance).sort((a, b) => a - b);
  const under = (x) => d.filter((v) => v < x).length;
  console.log(
    `répartition: ${under(1)} à moins d'un dB, ${under(2)} à moins de deux, max ${d[d.length - 1].toFixed(2)}`,
  );
}

// ── LES RÈGLES DE DÉCISION, MESURÉES ────────────────────────────────────────────────────────
//
// ⚠️ Les règles 2 à 6 tirent leur référence des SEULS authentiques du corpus (10 fichiers, une
// famille musicale). C'est exactement le défaut qui a fait publier un 77 % payé par des faux
// positifs non mesurés : leur colonne « faux positifs » est auto-référentielle par construction et
// ne dit rien du comportement sur un authentique jamais vu.
const sum = (j) => j.hf + j.hfTop;
const mean = (xs) => xs.reduce((a, b) => a + b, 0) / xs.length;
const std = (xs) => {
  const m = mean(xs);
  return Math.sqrt(mean(xs.map((x) => (x - m) ** 2)));
};
const fixedVals = gens.map((j) => j.hf);
const topVals = gens.map((j) => j.hfTop);
const mFixed = mean(fixedVals);
const sFixed = std(fixedVals) || 1;
const mTop = mean(topVals);
const sTop = std(topVals) || 1;
const zMin = (j) => Math.min((j.hf - mFixed) / sFixed, (j.hfTop - mTop) / sTop);
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)];
const medFixed = median(fixedVals);
const medTop = median(topVals);
const bothUnderMedian = (j) => j.hf < medFixed && j.hfTop < medTop;

const minSum = Math.min(...gens.map(sum));
const minZ = Math.min(...gens.map(zMin));
const RULES = [
  ["OU des deux seuils (actuel)", union],
  ["somme des deux axes < min authentique", (j) => sum(j) < minSum],
  ["OU des trois (seuils + somme)", (j) => union(j) || sum(j) < minSum],
  ["z-score minimal < min authentique", (j) => zMin(j) < minZ],
  ["les deux sous la médiane authentique", bothUnderMedian],
  ["OU des seuils, OU les deux sous la médiane", (j) => union(j) || bothUnderMedian(j)],
];
console.log("\n=== RÈGLES DE DÉCISION (références tirées des authentiques DU CORPUS) ===");
console.log("règle                                          détection   faux positifs");
for (const [name, f] of RULES) {
  const det = fakes.filter(f).length;
  const fp = gens.filter(f).length;
  console.log(
    `${name.padEnd(44)} ${`${det}/${fakes.length}${pct(det, fakes.length)}`.padStart(14)}   ${fp}/${gens.length}`,
  );
}
