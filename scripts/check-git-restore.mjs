// check-git-restore.mjs — hook PreToolUse (Bash) : refuse `git checkout -- <chemin>`,
// `git checkout <chemin>`, `git restore …` et `git stash …` quand l'arbre porte des modifications
// non commitées, AVANT que la commande parte.
// PAS de shebang ici : même raison que `check-gh-title.mjs` (Vitest importe le module).
//
// Le raté qu'il ferme (mémoire `never-git-checkout-to-undo-a-test-edit`, récidive du 2026-09-09) :
// pour défaire un contre-test, une ligne enchaînait `sed` (mutation) → `cargo test` →
// `git checkout -- worker.rs` → `sed` (restauration). `git checkout` restaure HEAD, pas l'état
// d'avant le contre-test : ~200 lignes d'éditions non commitées sont parties avec la mutation. La
// règle était écrite et chargée — c'est le cran hook qui manquait (CLAUDE.md § Un raté attrapé).
//
// Contrat hook Claude Code : JSON du tool call sur stdin ; exit 0 = laisser passer,
// exit 2 = BLOQUER (stderr revient à l'agent comme feedback). Fail-open sur toute erreur
// interne (mémoire `protective-layer-fail-open`).

import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';

/** Vrai si la commande contient un geste qui écrase ou cache du travail non commité.
 *  Exportée pour les vecteurs de test. Ce qu'on laisse passer : `git checkout <branche>` et
 *  `git checkout -b …` (git refuse lui-même d'écraser une modification en changeant de
 *  branche) ; ce qu'on bloque : `checkout --`, `checkout .`, `checkout <chemin>` (un chemin se
 *  reconnaît à son `/`, son `\` ou son extension), `restore`, `stash`. */
export function estDangereux(cmd) {
  const c = String(cmd);
  if (/\bgit\s+(?:-C\s+\S+\s+)?restore\b/.test(c)) return true;
  if (/\bgit\s+(?:-C\s+\S+\s+)?stash\b/.test(c)) return true;
  const co = /\bgit\s+(?:-C\s+\S+\s+)?checkout\s+(.*)/g;
  for (const m of c.matchAll(co)) {
    const reste = m[1].split(/\s*(?:&&|\|\||;|\|)\s*/)[0].trim();
    if (reste.startsWith('--')) return true;
    if (/^\.(\s|$)/.test(reste)) return true;
    if (/^-b\b|^-B\b|^-t\b|^--track\b|^--orphan\b|^-q\b/.test(reste)) continue;
    const cible = reste.split(/\s+/)[0] || '';
    if (/[\\/]/.test(cible) || /\.[A-Za-z0-9]{1,6}$/.test(cible)) return true;
  }
  return false;
}

/** Lignes de `git status --porcelain` qui portent du travail non commité (suivi) — les fichiers
 *  non suivis (`??`) ne sont pas touchés par un checkout. */
export function modifsSuivies(porcelain) {
  return String(porcelain)
    .split('\n')
    .filter((l) => l.trim() && !l.startsWith('??'));
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
  if (!estDangereux(cmd)) process.exit(0);
  let porcelain = '';
  try {
    porcelain = execFileSync('git', ['status', '--porcelain'], { encoding: 'utf8' });
  } catch {
    process.exit(0);
  }
  const sales = modifsSuivies(porcelain);
  if (!sales.length) process.exit(0);
  process.stderr.write(
    `check-git-restore : arbre sale (${sales.length} fichier(s) suivi(s) modifié(s)) — un ` +
      `checkout/restore/stash y efface ou cache du travail non commité. Committer d'abord, ou ` +
      `restaurer par édition inverse (sed retour, copie de sauvegarde). Mémoire : ` +
      `never-git-checkout-to-undo-a-test-edit.\n` +
      sales.slice(0, 8).map((l) => `  ${l}`).join('\n') + '\n',
  );
  process.exit(2);
}

// Ne s'exécute qu'en tant que hook (stdin), jamais à l'import par Vitest.
if (process.argv[1] && /check-git-restore\.mjs$/.test(process.argv[1])) main();
