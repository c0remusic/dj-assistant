// Strict DOM lookup for the live layer (P-4). The live wiring augments app.js's shell and its
// own freshly-rendered templates; when an element it depends on is absent (a renamed shell id,
// a missing render container, a broken cross-module contract) the old `if (!x) return` / `?.`
// pattern made it no-op SILENTLY. `requireEl` turns that into a loud, located failure instead.
//
// Use ONLY for elements proven to always exist when the code runs (the OBLIGATOIRE accesses in
// audit/p4-recensement.md): the cross-file/cross-module "shell" contract and each render
// container. NEVER for conditional, optional, idempotent-probe or async-filled elements — those
// stay `if (x)` / `?.` on purpose.
//
// `selector`  CSS selector (use "#id" for what was getElementById).
// `context`   short caller label (function/view) so the thrown message situates the problem.
// `root`      optional scope for a scoped query (defaults to `document`).
export function requireEl<T extends Element = HTMLElement>(
  selector: string,
  context: string,
  root: ParentNode = document,
): T {
  const el = root.querySelector<T>(selector);
  if (!el) {
    throw new Error(`requireEl: élément introuvable "${selector}" (${context})`);
  }
  return el;
}

/** HTML-escapes untrusted string data (filenames, tags, Discogs/master.db fields) before
 *  interpolating it into a template string assigned via innerHTML. Every render helper that
 *  builds markup from data not fully owned by Sift's own code must run it through this first —
 *  a file that skips it is a stored-XSS gap (found in journal.ts, 2026-07-10 security audit). */
export function esc(s: string): string {
  return s.replace(/[&<>"']/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : c === '"' ? "&quot;" : "&#39;",
  );
}

/** « 1 piste » / « 3 pistes » — le COMPTE avec son nom accordé, jamais le nom seul : l'utilisateur
 *  lit un résultat, pas le libellé de ce qu'il a cliqué.
 *
 *  Le pluriel s'accorde à partir de 2, comme en français courant — `n > 1`, donc « 0 piste » et non
 *  « 0 pistes ». `many` est déduit en ajoutant un « s » ; le passer explicitement sert aux mots qui
 *  ne se pluralisent pas comme ça (« un dossier surveillé » → « des dossiers surveillés »). */
/** Une durée de `styles.css` (`--duration-fast|base|slow`, écrites en `ms`) lue en millisecondes,
 *  pour différer un re-rendu jusqu'à la fin de la transition qu'il tuerait (un nœud reconstruit
 *  par `innerHTML` n'anime rien — CLAUDE.md § Front). Repli 150 ms hors DOM ou token absent :
 *  ne jamais bloquer un rendu sur une valeur illisible. Miroir de lecture, pas de valeur : la
 *  durée reste déclarée une seule fois, dans `styles.css`. */
export function durationMs(token: "--duration-fast" | "--duration-base" | "--duration-slow"): number {
  if (typeof document === "undefined") return 150;
  const raw = getComputedStyle(document.documentElement).getPropertyValue(token).trim();
  const ms = raw.endsWith("ms") ? parseFloat(raw) : raw.endsWith("s") ? parseFloat(raw) * 1000 : NaN;
  return Number.isFinite(ms) ? ms : 150;
}

export function plural(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n > 1 ? many : one}`;
}
