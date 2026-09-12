#!/usr/bin/env node
// Refuse deux régressions de configuration que rien d'autre ne rattrape : ni `tsc`, ni `clippy`,
// ni la suite de tests ne lisent `tauri.conf.json`, et les deux se réintroduisent en une seule
// ligne pendant un debug (« je remets `**` deux minutes pour voir si c'est ça »).
//
// 1. `assetProtocol.scope` avec un wildcard. Le protocole `asset:` sert des fichiers du disque à
//    la webview ; `["**"]` veut dire « tous ». Sift affiche des tags et des noms de fichiers
//    venant de fichiers inconnus, et a déjà livré un XSS stocké une fois — un wildcard ici
//    transforme cette classe de bug en lecture arbitraire de fichiers. Le scope doit rester vide
//    et se remplir à l'exécution (`lib.rs` pour le cache de pochettes, `ipc::playback_url` pour
//    le fichier en cours de lecture, un par un).
// 2. `'unsafe-eval'` dans `script-src`. Rien dans Sift n'évalue de chaîne ; sa seule fonction
//    serait d'offrir l'escalade à une injection.
//
// Limite assumée : `'unsafe-inline'` reste autorisé, Vite injectant des scripts inline (HMR en
// dev, préchargement de modules en build). Ce n'est pas un oubli — le retirer demande de vérifier
// un build de prod réel, pas une relecture de config.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const confPath = join(root, "src-tauri", "tauri.conf.json");
const conf = JSON.parse(readFileSync(confPath, "utf8"));
const security = conf?.app?.security ?? {};
const problems = [];

const scope = security.assetProtocol?.scope;
if (!Array.isArray(scope)) {
  problems.push("app.security.assetProtocol.scope absent ou non-tableau");
} else {
  for (const entry of scope) {
    const s = typeof entry === "string" ? entry : entry?.path;
    if (typeof s === "string" && s.includes("*")) {
      problems.push(
        `app.security.assetProtocol.scope contient un motif large (${JSON.stringify(s)}) — ` +
          "le scope doit rester vide et être accordé fichier par fichier à l'exécution",
      );
    }
  }
}

const csp = typeof security.csp === "string" ? security.csp : "";
if (!csp) {
  problems.push("app.security.csp absent — la webview tournerait sans CSP");
} else if (/script-src[^;]*'unsafe-eval'/.test(csp)) {
  problems.push("app.security.csp autorise 'unsafe-eval' dans script-src");
}

if (problems.length) {
  console.error("check-tauri-security: configuration refusée");
  for (const p of problems) console.error(`  - ${p}`);
  process.exit(1);
}
console.log("check-tauri-security: scope asset et CSP conformes");
