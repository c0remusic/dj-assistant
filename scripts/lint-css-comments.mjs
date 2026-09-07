// Gate : `*/` orphelin dans styles.css — le motif payé DEUX FOIS le 2026-09-06/07.
//
// Le raté exact : un bloc de commentaire édité en deux temps garde son `*/` intermédiaire, et le
// texte qui suit se retrouve HORS commentaire. Le navigateur ne jette PAS d'erreur : il lit le
// texte nu comme un sélecteur invalide et ignore en silence la règle qui suit — c'est ce qui a
// éteint `.sift-pz-row` puis failli éteindre `.sift-qfoot-go[hidden]`, attrapés les deux fois
// parce qu'une mesure CDP contredisait la feuille. Un parseur CSS classique ne le voit pas non
// plus (sélecteur bizarre + bloc {} = syntaxiquement valide) ; ce qui le voit, c'est l'automate
// de commentaires : un `*/` rencontré ALORS QU'AUCUN commentaire n'est ouvert est toujours une
// erreur, et c'est précisément la signature du raté.
//
// Périmètre : chaînes ("…"/'…') sautées — un `*/` dans un content:"…" serait légitime.
import { readFileSync } from "node:fs";

const path = new URL("../frontend/styles.css", import.meta.url);
const css = readFileSync(path, "utf8");

const errors = [];
let inComment = false;
let quote = null; // '"' | "'" | null
let line = 1;

for (let i = 0; i < css.length; i++) {
  const c = css[i];
  if (c === "\n") line++;
  if (inComment) {
    if (c === "*" && css[i + 1] === "/") {
      inComment = false;
      i++;
    }
    continue;
  }
  if (quote) {
    if (c === "\\") i++;
    else if (c === quote) quote = null;
    continue;
  }
  if (c === '"' || c === "'") {
    quote = c;
    continue;
  }
  if (c === "/" && css[i + 1] === "*") {
    inComment = true;
    i++;
    continue;
  }
  if (c === "*" && css[i + 1] === "/") {
    errors.push(line);
    i++;
  }
}
if (inComment) errors.push(-1);

if (errors.length) {
  for (const l of errors) {
    if (l === -1) console.error("lint-css-comments: commentaire jamais refermé en fin de fichier");
    else
      console.error(
        `lint-css-comments: frontend/styles.css:${l} — \`*/\` hors commentaire : le texte qui précède est HORS /* */, le navigateur ignorera en silence la règle suivante`,
      );
  }
  process.exit(1);
}
console.log("lint-css-comments: pass.");
