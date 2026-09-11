#!/usr/bin/env node
// lint-tokens.mjs — detects hardcoded colors / z-index / px-spacing values that bypass
// the CSS custom-property tokens declared in frontend/styles.css.
//
// Approach: plain regex scanning, no CSS AST parser (per project constraint). This is a
// first-pass heuristic tool, not a full parser — see "known limitations" in the report.
//
// Spacing-token px set: tokens in styles.css are declared directly in px (--space-4:4px,
// --text-*:Npx, --h-40:40px, --border-radius-*). We do NOT convert rem->px here because
// styles.css does not use a rem-based spacing scale — every relevant numeric token is
// already a literal px value. If that changes, add a REM_BASE constant and convert.

import { readFileSync, readdirSync, statSync, writeFileSync, existsSync } from 'node:fs';
import { join, resolve, relative, extname } from 'node:path';

const REPO_ROOT = resolve(new URL('.', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'), '..');
const TOKEN_FILE = resolve(REPO_ROOT, 'frontend', 'styles.css');
const BASELINE_FILE = resolve(REPO_ROOT, 'scripts', 'lint-tokens-baseline.json');

// Ratchet mode: don't fail CI on the ~540 pre-existing findings (undoable in one pass, tracked
// separately in docs/superpowers/changes/2026-07-19-spacing-scale-sweep/design.md) — fail only
// when a run introduces MORE findings than the last recorded baseline, per category. Paying down
// debt lowers the ratchet (via --write-baseline); it can never silently climb back up unnoticed.
const WRITE_BASELINE = process.argv.includes('--write-baseline');

// '.claude' excludes native worktrees too (created under .claude/worktrees/ per
// ~/.claude/CLAUDE.md § Isolation native) — each one is a full checkout of frontend/, so
// leaving it unexcluded silently doubles (or worse) every count depending on how many worktree
// agents happen to be running on the machine at scan time. Caught by verify-gate crosscheck
// (2026-07-24): a baseline recorded while a worktree was present was ~2x the real count.
// 'docs' : le manuel et le site (docs/manuel.html, docs/accueil.html, docs/variantes/) copient les
// VALEURS RÉSOLUES de styles.css par construction — ils ne peuvent pas référencer un token de
// l'app, ils sont servis hors de l'app. Les scanner faisait tomber build.yml sur main depuis le
// 2026-09-10 (d1f5a3f) sans que personne ne regarde la job lint-tokens. Le périmètre du lint est
// l'UI de l'app : frontend/, shared/, test/.
const EXCLUDE_DIRS = new Set(['node_modules', 'dist', '.git', 'target', '.claude', 'docs', 'public']);
// src-tauri\target — matched by checking the relative path contains src-tauri/target.
const SCAN_EXTS = new Set(['.css', '.ts', '.tsx']);

function shouldSkipDir(absDir) {
  const rel = relative(REPO_ROOT, absDir).split(/[\\/]/);
  if (rel.some((seg) => EXCLUDE_DIRS.has(seg))) return true;
  if (rel.join('/').includes('src-tauri/target')) return true;
  return false;
}

function walk(dir, out) {
  let entries;
  try {
    entries = readdirSync(dir, { withFileTypes: true });
  } catch {
    return out;
  }
  for (const entry of entries) {
    const abs = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (shouldSkipDir(abs)) continue;
      walk(abs, out);
    } else if (entry.isFile()) {
      if (SCAN_EXTS.has(extname(entry.name))) out.push(abs);
    }
  }
  return out;
}

// ---- Step 1: parse tokens from styles.css --------------------------------------------

const tokenSrc = readFileSync(TOKEN_FILE, 'utf8');

// Matches `--name: value;` (value = anything up to the next `;`).
const TOKEN_DECL_RE = /--([a-zA-Z0-9-]+)\s*:\s*([^;]+);/g;

const colorTokens = new Map(); // name -> value (for reporting nearest match, best-effort)
const spacingPxValues = new Map(); // px number -> token name (first one wins)

let m;
while ((m = TOKEN_DECL_RE.exec(tokenSrc))) {
  const name = m[1];
  const value = m[2].trim();

  if (/^(color|overlay)-/.test(name) || /^oklch\(|^rgba?\(|^#/.test(value)) {
    colorTokens.set(name, value);
  }

  // Pull every literal `Npx` out of the value (covers --space-*, --text-*, --h-*,
  // --border-radius-base, and calc() expressions that reference literal px numbers).
  const pxRe = /(-?\d+(?:\.\d+)?)px/g;
  let pm;
  while ((pm = pxRe.exec(value))) {
    const px = parseFloat(pm[1]);
    if (!spacingPxValues.has(px)) spacingPxValues.set(px, `--${name}`);
  }
}

// ---- Step 2: scan files ----------------------------------------------------------------

// Scan styles.css too (not just other files) — most real drift lives THERE, in the
// component rules below the token declarations, not in other files bypassing it. We
// blank out the :root/dark-mode token-declaration blocks (same length, newlines kept,
// so line numbers stay correct) so a token's own value never flags itself, while the
// rest of the file still gets scanned. Fixes a codex-crosscheck HAUTE finding
// (2026-07-19): the previous version excluded the whole file, missing the majority of
// off-scale spacing/color/z-index sites — the file most in need of this lint.
// `:root` suivi d'un nombre quelconque de qualificatifs : attribut (`[data-theme="dark"]`) ET
// pseudo-classe fonctionnelle (`:not([data-theme="light"])`). L'ancien motif n'acceptait que
// l'attribut, donc le bloc `@media (prefers-color-scheme:dark) { :root:not(...) }` n'était pas
// reconnu et toutes ses déclarations de tokens étaient comptées comme des couleurs en dur.
const ROOT_SEL = String.raw`:root(?:(?::not\([^)]*\))|(?:\[[^\]]*\]))*`;
const TOKEN_BLOCK_RE = new RegExp(
  `(@media[^{]*\\{\\s*${ROOT_SEL}\\s*\\{[^{}]*\\}\\s*\\})|(${ROOT_SEL}\\s*\\{[^{}]*\\})`,
  'g',
);
const CSS_COMMENT_RE = /\/\*[\s\S]*?\*\//g;

// Blanking keeps length AND newlines, so every later offset and line number stays exact.
const blank = (s) => s.replace(/[^\n]/g, ' ');

// Les commentaires sont neutralisés AVANT la recherche des blocs de tokens, pour deux raisons
// distinctes — la première est un bug, la seconde une nuisance :
//
// 1. TOKEN_BLOCK_RE exige `[^{}]*` entre les accolades du bloc. Un commentaire contenant une
//    accolade — et styles.css en a un, `bg-{info,danger,success,warning}` dans le bloc `:root` de
//    base — coupe le motif en deux : le bloc n'est PAS reconnu, donc pas neutralisé, donc toutes
//    ses déclarations de tokens sont comptées comme des couleurs codées en dur. Mesure du
//    2026-07-28 : 1 bloc reconnu sur 3, et l'essentiel des 122 dérives couleur de la baseline
//    étaient en réalité des déclarations de tokens. Un ratchet qui compte surtout du bruit ne
//    protège de rien : il empêche seulement d'AJOUTER un token.
// 2. Un littéral cité dans un commentaire pour expliquer ce qu'on vient d'en retirer n'est pas
//    une dérive. Sans ce traitement, documenter une correction la fait échouer.
//
// Audit multi-passes 2026-07-28, finding SYS-4(b).
function blankOutTokenBlocks(text) {
  const noComments = text.replace(CSS_COMMENT_RE, blank);
  let out = noComments;
  out = out.replace(TOKEN_BLOCK_RE, blank);
  return out;
}

/// Nombre de blocs de déclaration de tokens reconnus dans un texte CSS. Sert au garde-fou
/// ci-dessous : une régression silencieuse de TOKEN_BLOCK_RE se traduirait par une explosion du
/// nombre de findings, ce qui ressemble à de la vraie dérive.
function countTokenBlocks(text) {
  return (text.replace(CSS_COMMENT_RE, blank).match(TOKEN_BLOCK_RE) || []).length;
}

const files = walk(REPO_ROOT, []);

const COLOR_RE = /(#(?:[0-9a-fA-F]{3}){1,2}\b|#[0-9a-fA-F]{8}\b|\brgba?\([^)]*\)|\boklch\([^)]*\))/g;
const ZINDEX_RE = /z-index\s*:\s*(-?\d+(?:\.\d+)?)/g;
// La déclaration se termine par `;` OU par le `}` du bloc — en LOOKAHEAD, sans consommer le
// délimiteur. Le motif exigeait un `;` littéral jusqu'au 2026-08-13, ce qui rendait invisible
// toute déclaration DERNIÈRE de son bloc : 112 déclarations d'espacement sur 451, dont 41 en px
// dur, jamais examinées. Silencieusement — elles n'étaient pas ignorées, elles n'existaient pas
// pour le linter, et la baseline avait été gravée sur ce compte tronqué. C'est la forme la plus
// courante d'une règle mono-propriété, donc précisément celle qu'un `max-width` prend : les trois
// `max-width:560px` de la feuille passaient tous les trois (issue #29).
const SPACING_PROP_RE = /\b(padding|margin|width|height|gap)(-(?:top|right|bottom|left|inline|block)(?:-(?:start|end))?)?\s*:\s*([^;{}]+)(?=[;}])/g;
const PX_VALUE_RE = /(-?\d+(?:\.\d+)?)px/g;

const findings = []; // { file, line, category, value, suggestion }
// Compte POSITIF de ce qui a été examiné, pas seulement de ce qui a été trouvé. Une liste de
// findings vide et un motif qui ne matche plus rien se ressemblent exactement — c'est ce qui a
// laissé passer #29 pendant toute la vie du script. Ce nombre est le témoin qui les sépare.
let spacingDeclsSeen = 0;

function nearestSpacingToken(px) {
  if (spacingPxValues.has(px)) return spacingPxValues.get(px);
  let best = null;
  let bestDist = Infinity;
  for (const [val, name] of spacingPxValues) {
    const d = Math.abs(val - px);
    if (d < bestDist) {
      bestDist = d;
      best = name;
    }
  }
  return best && bestDist <= 2 ? `${best} (off by ${bestDist}px)` : 'no matching token — new value';
}

function lineAt(text, index) {
  let line = 1;
  for (let i = 0; i < index; i++) {
    if (text.charCodeAt(i) === 10) line++;
  }
  return line;
}

for (const file of files) {
  let text;
  try {
    text = readFileSync(file, 'utf8');
  } catch {
    continue;
  }
  if (resolve(file) === resolve(TOKEN_FILE)) {
    // Garde-fou : styles.css déclare TROIS blocs de thème (`:root`, le bloc
    // `@media (prefers-color-scheme:dark)`, et `:root[data-theme="dark"]`). Si TOKEN_BLOCK_RE
    // cesse d'en reconnaître un — comme c'était le cas jusqu'au 2026-07-28 où il n'en voyait
    // qu'un — le nombre de findings explose et se lit comme une vraie dérive. Échouer ici,
    // bruyamment, coûte une minute ; laisser passer coûte une baseline de bruit qu'on re-grave
    // ensuite à chaque commit.
    const blocks = countTokenBlocks(text);
    if (blocks !== 3) {
      console.error(
        `lint-tokens: ${blocks} bloc(s) de tokens reconnu(s) dans ${TOKEN_FILE}, 3 attendus.\n` +
          `  TOKEN_BLOCK_RE ne matche plus la structure du fichier — corriger le motif, PAS la baseline.`,
      );
      process.exit(2);
    }
    text = blankOutTokenBlocks(text);
  }
  const rel = relative(REPO_ROOT, file).split('\\').join('/');

  // --- colors ---
  let cm;
  COLOR_RE.lastIndex = 0;
  while ((cm = COLOR_RE.exec(text))) {
    const value = cm[0];
    // skip url() contents crudely: if immediately preceded by "url(" ignore (best-effort).
    const before = text.slice(Math.max(0, cm.index - 5), cm.index);
    if (/url\($/.test(before)) continue;
    findings.push({
      file: rel,
      line: lineAt(text, cm.index),
      category: 'color',
      value,
      suggestion: 'use a --color-*/--overlay-* token instead of a literal color',
    });
  }

  // --- z-index ---
  let zm;
  ZINDEX_RE.lastIndex = 0;
  while ((zm = ZINDEX_RE.exec(text))) {
    const num = parseFloat(zm[1]);
    if (num === 0 || num === 1 || num === -1) continue;
    findings.push({
      file: rel,
      line: lineAt(text, zm.index),
      category: 'z-index',
      value: zm[1],
      suggestion: 'use --z-popover/--z-toast/--z-modal (or add a new named layer token) instead of a literal z-index',
    });
  }

  // --- px spacing on padding/margin/width/height/gap ---
  let sm2;
  SPACING_PROP_RE.lastIndex = 0;
  while ((sm2 = SPACING_PROP_RE.exec(text))) {
    spacingDeclsSeen++;
    const declValue = sm2[3];
    let pxm;
    PX_VALUE_RE.lastIndex = 0;
    while ((pxm = PX_VALUE_RE.exec(declValue))) {
      const px = parseFloat(pxm[1]);
      if (!spacingPxValues.has(px)) {
        const offset = sm2.index + sm2[0].indexOf(declValue);
        findings.push({
          file: rel,
          line: lineAt(text, offset),
          category: 'px-spacing',
          value: `${px}px`,
          suggestion: nearestSpacingToken(px),
        });
      }
    }
  }
}

// ---- Step 3: report --------------------------------------------------------------------

const byFile = new Map();
for (const f of findings) {
  if (!byFile.has(f.file)) byFile.set(f.file, []);
  byFile.get(f.file).push(f);
}

const counts = { color: 0, 'z-index': 0, 'px-spacing': 0 };
for (const f of findings) counts[f.category]++;

// Le témoin part dans la baseline AVEC les counts, mais il ne vit pas dans `counts` : le
// cliquet de l'étape 4 y cherche une HAUSSE, alors qu'une chute du témoin est le signal.
// Deux sens de lecture opposés, deux emplacements.
const witness = { spacingDeclsSeen };

if (WRITE_BASELINE) {
  writeFileSync(BASELINE_FILE, JSON.stringify({ ...counts, ...witness }, null, 2) + '\n');
  console.log(`lint-tokens: baseline written to ${relative(REPO_ROOT, BASELINE_FILE)}:`, {
    ...counts,
    ...witness,
  });
  process.exit(0);
}

if (findings.length === 0) {
  console.log('lint-tokens: no hardcoded values found bypassing tokens.');
  process.exit(0);
}

console.log(`lint-tokens: ${findings.length} finding(s) across ${byFile.size} file(s)\n`);

for (const [file, list] of [...byFile.entries()].sort()) {
  console.log(file + ':');
  for (const f of list.sort((a, b) => a.line - b.line)) {
    console.log(`  ${f.line}: [${f.category}] ${f.value} -> ${f.suggestion}`);
  }
  console.log('');
}

console.log('Summary:');
console.log(`  colors:      ${counts.color}`);
console.log(`  z-index:     ${counts['z-index']}`);
console.log(`  px-spacing:  ${counts['px-spacing']} (sur ${spacingDeclsSeen} déclarations examinées)`);

// ---- Step 4: ratchet against baseline ---------------------------------------------------

if (!existsSync(BASELINE_FILE)) {
  console.log(
    `\nlint-tokens: no baseline at ${relative(REPO_ROOT, BASELINE_FILE)} — run with ` +
    `--write-baseline once to record the current count as the starting ratchet, then commit it.`,
  );
  process.exit(1);
}

const baseline = JSON.parse(readFileSync(BASELINE_FILE, 'utf8'));

// ---- Step 4a: le témoin, dans le sens inverse du cliquet -----------------------------------
//
// Une CHUTE du nombre de déclarations examinées veut dire que le motif a cessé de voir une
// partie de la feuille. C'est le mode de panne exact de #29 : le 2026-08-13, SPACING_PROP_RE
// exigeait un `;` littéral et rendait invisibles les 112 déclarations dernières de leur bloc.
// Les findings tombent EUX AUSSI, donc le cliquet de l'étape 4 lit une amélioration et passe.
//
// Vérifié par mutation le 2026-08-18, en remettant le motif dans sa forme d'avant le
// correctif : 762 -> 584 déclarations examinées, 70 -> 59 findings, et la gate sortait 0 en
// annonçant « below baseline — nothing blocking ». Le nombre était imprimé depuis le
// correctif de #29 et comparé à rien : un témoin que personne ne confronte ne sépare rien.
//
// Toute baisse bloque. Une réduction légitime (suppression réelle de CSS) se valide par
// --write-baseline, ce qui force quelqu'un à regarder le nombre — c'est précisément le
// geste qui manquait.
if (typeof baseline.spacingDeclsSeen !== 'number') {
  console.log(
    `\nlint-tokens: la baseline ne porte pas encore spacingDeclsSeen — re-lancer avec ` +
    `--write-baseline pour enregistrer le témoin. Non bloquant pour cette exécution.`,
  );
} else if (spacingDeclsSeen < baseline.spacingDeclsSeen) {
  console.log(
    `\nlint-tokens: le nombre de déclarations d'espacement EXAMINÉES a chuté : ` +
    `${baseline.spacingDeclsSeen} -> ${spacingDeclsSeen} ` +
    `(-${baseline.spacingDeclsSeen - spacingDeclsSeen}).`,
  );
  console.log(
    `  Deux causes possibles, et une seule est bénigne :\n` +
    `    1. SPACING_PROP_RE ne voit plus une partie de la feuille — c'est le bug #29, et\n` +
    `       il rend les findings faussement bas. Corriger le motif.\n` +
    `    2. Du CSS a réellement été supprimé. Alors --write-baseline pour l'enregistrer.`,
  );
  process.exit(1);
}

const regressed = [];
for (const cat of Object.keys(counts)) {
  if (counts[cat] > (baseline[cat] ?? 0)) {
    regressed.push(`${cat}: ${baseline[cat] ?? 0} -> ${counts[cat]} (+${counts[cat] - (baseline[cat] ?? 0)})`);
  }
}

if (regressed.length > 0) {
  console.log('\nlint-tokens: NEW findings beyond the recorded baseline — this run is a regression:');
  for (const r of regressed) console.log(`  ${r}`);
  console.log(
    '\nFix the new drift, or if this reduction/addition is deliberate and reviewed, ' +
    're-run with --write-baseline to update the ratchet.',
  );
  process.exit(1);
}

const improved = Object.keys(counts).filter((cat) => counts[cat] < (baseline[cat] ?? 0));
if (improved.length > 0) {
  console.log(
    `\nlint-tokens: below baseline on [${improved.join(', ')}] — nothing blocking, but consider ` +
    `re-running with --write-baseline to lock in the improvement.`,
  );
}
console.log(`\nlint-tokens: within baseline (${relative(REPO_ROOT, BASELINE_FILE)}) — pass.`);
process.exit(0);
