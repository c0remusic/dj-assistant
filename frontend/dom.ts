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
