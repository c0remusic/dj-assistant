// Le site Vercel (https://sift-music.vercel.app) est le build Vite de l'app plus trois choses
// posées à côté : la page d'accueil, le manuel, le manuel en PDF.
//
// Deux phases, branchées dans package.json :
//   `prebuild`  → `node scripts/build-site.mjs prepare`
//   `postbuild` → `node scripts/build-site.mjs finish`
//
// prepare — enveloppe `docs/accueil.html` et `docs/manuel.html` d'un squelette HTML et les dépose
// dans `public/` avec le PDF et la capture d'écran ; Vite recopie `public/` verbatim dans `dist/`.
// Les deux sources sont écrites SANS `<!doctype>`/`<html>`/`<head>`/`<body>` : c'est la forme que
// demande l'outil d'artefact de Claude, qui les enveloppe lui-même. Servies nues, elles perdraient
// leur `<meta charset>` — mojibake sur les accents selon le navigateur.
//
// finish — SEULEMENT sur Vercel (`process.env.VERCEL`) : la racine du site devient la page
// d'accueil et l'app se déplace en `/app.html`. Partout ailleurs `dist/index.html` reste l'app :
// `npm run tauri build` lance `npm run build` et embarque `dist/` tel quel (`frontendDist`) — un
// swap inconditionnel livrerait le site d'accueil dans l'installeur.
//
// `public/` est généré et gitignoré : la source reste `docs/`.
import { copyFile, mkdir, readFile, rename, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const pub = join(root, "public");
const dist = join(root, "dist");

const PAGES = [
  {
    src: "accueil.html",
    out: "accueil.html",
    description: "Sift, app desktop gratuite pour DJ : faux lossless détectés au spectrogramme, doublons, rangement au format CDJ, export Rekordbox, clé USB. Windows et macOS.",
  },
  {
    src: "manuel.html",
    out: "manuel.html",
    description: "Manuel de Sift : installer, trois mots, les huit écrans, le clavier, ce que la détection laisse passer.",
  },
];

async function wrap(page) {
  const body = await readFile(join(root, "docs", page.src), "utf8");
  if (body.includes("<!doctype") || body.includes("<html")) {
    console.error(`build-site: docs/${page.src} porte déjà un squelette HTML — l'artefact et ce script attendent un fragment.`);
    process.exit(1);
  }
  const html =
    '<!doctype html>\n<html lang="fr">\n<head>\n<meta charset="utf-8">\n' +
    '<meta name="viewport" content="width=device-width,initial-scale=1">\n' +
    `<meta name="description" content="${page.description}">\n` +
    "</head>\n<body>\n" +
    body +
    "\n</body>\n</html>\n";
  await writeFile(join(pub, page.out), html, "utf8");
}

async function prepare() {
  await mkdir(join(pub, "screenshots"), { recursive: true });
  for (const page of PAGES) await wrap(page);
  await copyFile(join(root, "docs", "manuel.pdf"), join(pub, "manuel.pdf"));
  await copyFile(join(root, "docs", "screenshots", "revue.png"), join(pub, "screenshots", "revue.png"));
  console.log("build-site: public/{accueil.html, manuel.html, manuel.pdf, screenshots/revue.png}");
}

async function finish() {
  if (!process.env.VERCEL) {
    console.log("build-site: hors Vercel, dist/index.html reste l'app (Tauri l'embarque)");
    return;
  }
  await rename(join(dist, "index.html"), join(dist, "app.html"));
  await rename(join(dist, "accueil.html"), join(dist, "index.html"));
  console.log("build-site: Vercel — racine = accueil, app en /app.html");
}

const phase = process.argv[2];
if (phase === "prepare") await prepare();
else if (phase === "finish") await finish();
else {
  console.error("usage: node scripts/build-site.mjs prepare|finish");
  process.exit(1);
}
