// Prépare le manuel pour le site Vercel (`npm run build` → `dist/`, servi à
// https://dj-assistantapp.vercel.app/manuel.html et /manuel.pdf).
//
// `docs/manuel.html` est écrit SANS squelette HTML (`<!doctype>`, `<html>`, `<head>`, `<body>`) :
// c'est la forme que demande l'outil d'artefact de Claude, qui l'enveloppe lui-même à la
// publication. Servi tel quel par un serveur web, le fichier s'affiche mais sans `<meta charset>`
// ni viewport — les accents passent en mojibake selon le navigateur. Ce script pose l'enveloppe et
// dépose le résultat dans `public/`, que Vite recopie verbatim dans `dist/`. Le PDF suit.
//
// `public/` est généré et gitignoré : la source reste `docs/manuel.html` (+ `docs/manuel.pdf`).
// Branché en `prebuild` dans package.json ; l'intégration Git de Vercel lance `npm run build`.
import { mkdir, readFile, writeFile, copyFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const src = join(root, "docs", "manuel.html");
const outDir = join(root, "public");

const body = await readFile(src, "utf8");
if (body.includes("<!doctype") || body.includes("<html")) {
  console.error("build-manuel: docs/manuel.html porte déjà un squelette HTML — l'artefact et ce script attendent un fragment.");
  process.exit(1);
}
const page =
  '<!doctype html>\n<html lang="fr">\n<head>\n<meta charset="utf-8">\n' +
  '<meta name="viewport" content="width=device-width,initial-scale=1">\n' +
  '<meta name="description" content="Manuel de Sift : installer, trois mots, les huit écrans, le clavier, ce que la détection laisse passer.">\n' +
  '</head>\n<body>\n' +
  body +
  "\n</body>\n</html>\n";

await mkdir(outDir, { recursive: true });
await writeFile(join(outDir, "manuel.html"), page, "utf8");
await copyFile(join(root, "docs", "manuel.pdf"), join(outDir, "manuel.pdf"));
console.log("build-manuel: public/manuel.html + public/manuel.pdf");
