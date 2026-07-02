// Shared empty-state component (DESIGN.md "État vide"): a real dead-end screen — top-aligned
// (never vertically centred), title + explanatory note, and for Bibliothèque/Écartés a
// "Aller à Revue →" link (Revue itself is the entry point, so it never gets the link). Single
// source of markup so the three callers (filing.ts, ecartes-view.ts, sift-live.ts) render the
// exact same structure instead of three ad hoc variants.
import { requireEl } from "./dom";

const esc = (s: string) =>
  s.replace(/[&<>"']/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : c === '"' ? "&quot;" : "&#39;",
  );

export interface EmptyStateOpts {
  /** Short heading, e.g. "Rien dans Écartés". */
  title: string;
  /** One line of explanatory copy. */
  note: string;
  /** Show the "Aller à Revue →" link. Omit for Revue itself — already the entry point. */
  backToRevue?: boolean;
}

/** Markup for the empty state. Insert into the view's content container; call `wireEmptyState`
 *  afterwards (once, on the same container) to hook up the optional back-to-Revue link. */
export function emptyStateHtml(opts: EmptyStateOpts): string {
  const link = opts.backToRevue
    ? `<button type="button" data-empty="revue" class="sift-empty-link">Aller à Revue →</button>`
    : "";
  return (
    `<div class="sift-empty-state">` +
    `<div class="sift-empty-title">${esc(opts.title)}</div>` +
    `<div class="sift-empty-note">${esc(opts.note)}</div>` +
    link +
    `</div>`
  );
}

/** Wire the "Aller à Revue →" link (a no-op if the markup didn't include one). Navigates via the
 *  same nav-click pattern already used elsewhere (filing.ts goto-reglages): dispatch a click on
 *  the real nav item rather than duplicating the router. */
export function wireEmptyState(root: ParentNode): void {
  root.querySelector<HTMLElement>('[data-empty="revue"]')?.addEventListener("click", () => {
    requireEl('[data-view="revue"]', "empty-state goto-revue").dispatchEvent(
      new MouseEvent("click", { bubbles: true }),
    );
  });
}
