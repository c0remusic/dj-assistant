// check-gh-title.mjs — hook PreToolUse (Bash) : refuse un `gh … --title "…"` (ou `-t`) dont le
// titre est du français désaccentué, AVANT que la commande parte.
// PAS de shebang ici : le hook est invoqué `node scripts/check-gh-title.mjs` (settings.json), et
// la ligne `#!` cassait l'import du module par Vitest — vite-node exécute le transform dans un
// corps de fonction où un shebang non strippé est un SyntaxError V8 (« Invalid or unexpected
// token », CI rouge du 2026-08-27, invisible à esbuild seul qui le préserve).
//
// Ferme le dernier canal du strip d'accents (issue #43, cause mesurée le 2026-08-26) : le texte
// destiné à un ARGUMENT de ligne de commande est rédigé en ASCII par prudence de quoting.
// `git commit` est gardé par le hook commit-msg (`lint-commit-msg.mjs`) ; `gh --title` — six
// titres strippés dans l'historique — ne l'était par rien. Même moteur : le titre est écrit dans
// un fichier temporaire et passé à `lint-commit-msg.mjs`, aucune liste dupliquée.
//
// Contrat hook Claude Code : JSON du tool call sur stdin ; exit 0 = laisser passer,
// exit 2 = BLOQUER (stderr revient à l'agent comme feedback). Fail-open sur toute erreur
// interne : un garde secondaire ne doit jamais casser le travail (mémoire
// `protective-layer-fail-open`).

import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

/** Les valeurs de `--title`/`-t` d'une commande shell, gérant `"…"`, `'…'` et l'absence de quotes.
 *  Exportée pour les vecteurs de test — le hook lui-même n'est que ce parse + un spawn. */
export function extraireTitres(cmd) {
  const out = [];
  const re = /(?:--title|(?<=\s)-t)(?:\s+|=)("((?:[^"\\]|\\.)*)"|'([^']*)'|(\S+))/g;
  for (const m of cmd.matchAll(re)) {
    const brut = m[2] !== undefined ? m[2].replace(/\\(.)/g, '$1') : m[3] !== undefined ? m[3] : m[4];
    if (brut) out.push(brut);
  }
  return out;
}

function main() {
  let entree = '';
  try {
    entree = readFileSync(0, 'utf8');
  } catch {
    process.exit(0);
  }
  let cmd = '';
  try {
    const data = JSON.parse(entree);
    if (data.tool_name !== 'Bash') process.exit(0);
    cmd = String(data.tool_input?.command ?? '');
  } catch {
    process.exit(0);
  }
  if (!/\bgh\s+(issue|pr|release|repo|label)\b/.test(cmd)) process.exit(0);
  const titres = extraireTitres(cmd);
  if (!titres.length) process.exit(0);

  const scripts = dirname(fileURLToPath(import.meta.url));
  const lint = join(scripts, 'lint-commit-msg.mjs');
  const dir = mkdtempSync(join(tmpdir(), 'gh-title-'));
  const fautifs = [];
  for (const t of titres) {
    const f = join(dir, 'titre.txt');
    writeFileSync(f, t, 'utf8');
    try {
      execFileSync(process.execPath, [lint, f], { stdio: 'pipe' });
    } catch (e) {
      if (e.status === 1) fautifs.push(t);
      // tout autre statut = panne interne du linter : fail-open, on laisse passer
    }
  }
  if (!fautifs.length) process.exit(0);
  console.error(
    'check-gh-title: titre gh en français désaccentué — le canal exact du strip #43.\n' +
      fautifs.map((t) => `  « ${t} »`).join('\n') +
      '\nRéaccentuer le titre (le canal transporte l\'UTF-8 sans perte, vérifié sur 12 titres le 2026-08-26).',
  );
  process.exit(2);
}

// Exécuté seulement en CLI directe — un import (Vitest) ne déclenche rien.
if (process.argv[1] && /check-gh-title\.mjs$/.test(process.argv[1].replace(/\\/g, '/'))) main();
