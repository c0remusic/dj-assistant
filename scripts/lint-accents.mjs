#!/usr/bin/env node
// lint-accents.mjs — détecte les BLOCS de commentaire français écrits sans un seul accent,
// c'est-à-dire la forme réelle du bug de strip à l'écriture (issues #43 / #44).
//
// POURQUOI PAR BLOC, ET PAS PAR MOT. La contre-mesure que #43 proposait — « grep une courte liste
// de mots FR désaccentués » — a été écrite, mesurée, et écartée : 1932 hits sur le dépôt, dont
// `execute` 292, `cache` 276, `affiche` 113. Ce sont des homographes anglais et des identifiants de
// code, pas du français. Une deuxième tentative, par dictionnaire inverse (dénuder tous les mots
// accentués du dépôt puis chercher les formes nues), fait pire : 4590 hits, dont `reste` 288,
// `porte` 191, `mesure` 151 — les paires « reste »/« resté » que seul le sens tranche. Une gate à
// ce taux de faux positifs se désactive la première semaine.
//
// Le bloc, lui, porte une redondance statistique que le mot n'a pas : plusieurs lignes de français
// d'affilée sans le moindre accent, c'est improbable ; un mot sans accent, c'est courant. Au seuil
// retenu, le détecteur sort 2 blocs sur le dépôt entier là où le scan par mots en sortait 1932.
//
// CE QU'IL NE VOIT PAS, et c'est assumé : un bloc partiellement strippé (un mot nu au milieu de
// lignes correctement accentuées) et les blocs de moins de MIN_LIGNES lignes. Une gate qui attrape
// la forme dominante sans crier au loup vaut mieux qu'une gate exhaustive qu'on éteint.
//
// Ratchet à baseline versionnée, même contrat que lint-tokens.mjs : les blocs déjà présents ne
// font pas échouer (ce sont, au 2026-08-26, deux faux positifs — du français qui ne demande
// aucun accent) ; seule une HAUSSE échoue. Baisser le compte se grave par --write-baseline.

import { readFileSync, readdirSync, writeFileSync, existsSync } from 'node:fs';
import { join, resolve, relative, extname } from 'node:path';

const REPO_ROOT = resolve(new URL('.', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'), '..');
const BASELINE_FILE = resolve(REPO_ROOT, 'scripts', 'lint-accents-baseline.json');
const WRITE_BASELINE = process.argv.includes('--write-baseline');

// Mêmes exclusions que lint-tokens.mjs, et pour la même raison mesurée : un worktree sous .claude/
// est un checkout complet, donc laissé visible il doublerait silencieusement chaque compte.
const EXCLUDE_DIRS = new Set(['node_modules', 'dist', '.git', 'target', '.claude']);
const SCAN_EXTS = new Set(['.ts', '.tsx', '.js', '.mjs', '.cjs', '.rs', '.css', '.toml', '.yml', '.yaml', '.sh']);

const MIN_LIGNES = 3;
const MIN_OUTILS = 6;

// Mots-outils SANS homographe anglais. « a », « on », « si », « plus », « non », « son » sont
// exclus à dessein : avec eux, le CSS et les commentaires anglais matchent, et styles.css — pourtant
// purgé par a2dd5d6 — remontait 300 lignes.
const OUTILS =
  /\b(le|la|les|une|des|du|qui|que|dont|pour|dans|sous|avec|sans|pas|ne|est|sont|cet|cette|ces|leur|leurs|elle|nous|vous|ils|elles|donc|mais|car|moins|tous|toute|toutes|meme|deja|encore|jamais|toujours|quand|comme|alors|ainsi|puis|entre|vers|apres|avant|depuis|selon|hors|faut|fait|faire|etre|avoir|peut|doit|celui|celle|ceux|aucun|aucune|chaque|plutot|parce|lorsque|afin|rien|tout|lui)\b/gi;
const ACCENTS = /[éèêëàâäùûüîïôöçœÉÈÊËÀÂÄÙÛÜÎÏÔÖÇŒ]/;
const COMMENT = /^\s*(\/\/\/|\/\/!|\/\/|\/\*|\*\/|\*|#(?!!)|<!--)/;

function walk(dir, out) {
  let entries;
  try {
    entries = readdirSync(dir, { withFileTypes: true });
  } catch {
    return out;
  }
  for (const e of entries) {
    const abs = join(dir, e.name);
    if (e.isDirectory()) {
      if (EXCLUDE_DIRS.has(e.name)) continue;
      if (relative(REPO_ROOT, abs).split(/[\\/]/).join('/').includes('src-tauri/target')) continue;
      walk(abs, out);
    } else if (SCAN_EXTS.has(extname(e.name))) {
      out.push(abs);
    }
  }
  return out;
}

/** Les blocs de commentaire d'un fichier : suites de lignes de commentaire consécutives. */
function blocsSansAccent(lines) {
  const trouves = [];
  let i = 0;
  while (i < lines.length) {
    if (!COMMENT.test(lines[i])) {
      i += 1;
      continue;
    }
    let j = i;
    while (j < lines.length && COMMENT.test(lines[j])) j += 1;
    const chunk = lines.slice(i, j);
    const texte = chunk.join('\n');
    if (chunk.length >= MIN_LIGNES && !ACCENTS.test(texte)) {
      const outils = new Set((texte.match(OUTILS) || []).map((w) => w.toLowerCase()));
      if (outils.size >= MIN_OUTILS) trouves.push({ ligne: i + 1, lignes: chunk.length, outils: outils.size });
    }
    i = j;
  }
  return trouves;
}

const files = walk(REPO_ROOT, []);
const parFichier = {};
let total = 0;
const details = [];
for (const abs of files) {
  let txt;
  try {
    txt = readFileSync(abs, 'utf8');
  } catch {
    continue;
  }
  const rel = relative(REPO_ROOT, abs).split(/[\\/]/).join('/');
  const blocs = blocsSansAccent(txt.split(/\r?\n/));
  if (blocs.length) {
    parFichier[rel] = blocs.length;
    total += blocs.length;
    for (const b of blocs) details.push(`${rel}:${b.ligne}  (${b.lignes} lignes, ${b.outils} mots-outils)`);
  }
}

if (WRITE_BASELINE) {
  writeFileSync(BASELINE_FILE, `${JSON.stringify({ total, parFichier }, null, 2)}\n`, 'utf8');
  console.log(`lint-accents: baseline écrite — ${total} bloc(s) dans ${Object.keys(parFichier).length} fichier(s).`);
  process.exit(0);
}

const baseline = existsSync(BASELINE_FILE) ? JSON.parse(readFileSync(BASELINE_FILE, 'utf8')) : { total: 0, parFichier: {} };

console.log(`lint-accents: ${total} bloc(s) de commentaire français sans accent (baseline ${baseline.total}).`);
for (const d of details) console.log(`  ${d}`);

// Le ratchet porte sur le TOTAL et sur chaque FICHIER : sans le second, réparer un fichier et en
// casser un autre passerait inaperçu à total constant.
const hausses = [];
for (const [f, n] of Object.entries(parFichier)) {
  const ref = baseline.parFichier[f] ?? 0;
  if (n > ref) hausses.push(`${f} : ${ref} → ${n}`);
}
if (total > baseline.total || hausses.length) {
  console.error('\nlint-accents: ÉCHEC — de nouveaux blocs de commentaire français sont écrits sans accent.');
  for (const h of hausses) console.error(`  ${h}`);
  console.error(
    "\nC'est le bug de strip à l'écriture (issues #43/#44), pas un style : réaccentuer le bloc.\n" +
      "Si le bloc est de l'anglais ou une citation verbatim, graver le nouveau compte par\n" +
      '  node scripts/lint-accents.mjs --write-baseline',
  );
  process.exit(1);
}
if (total < baseline.total) {
  console.log('lint-accents: sous la baseline — graver le gain par `node scripts/lint-accents.mjs --write-baseline`.');
}
console.log('lint-accents: dans la baseline — pass.');
