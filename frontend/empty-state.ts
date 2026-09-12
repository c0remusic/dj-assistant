// Shared empty-state component (DESIGN.md "État vide"): a real dead-end screen — top-aligned
// (never vertically centred), title + explanatory note, and for Bibliothèque/Écartés a
// "Ouvrir Revue" link (Revue itself is the entry point, so it never gets the link). Single
// source of markup so the three callers (filing.ts, ecartes-view.ts, sift-live.ts) render the
// exact same structure instead of three ad hoc variants.
import { requireEl, esc } from "./dom";

export interface EmptyStateOpts {
  /** Short heading, e.g. "Rien dans Écartés". */
  title: string;
  /** One line of explanatory copy. */
  note: string;
  /** Show the "Ouvrir Revue" link. Omit for Revue itself — already the entry point. */
  backToRevue?: boolean;
  /** Pre-built button/link markup for a screen-specific action (e.g. Rekordbox's "Lier un
   *  fichier XML Rekordbox"). Rendered after the back-to-Revue link, if both are present. The
   *  caller is responsible for its own click wiring (e.g. a `data-bib`/`data-sift` attribute
   *  already handled by an existing delegate) — wireEmptyState() does not touch it. */
  actionHtml?: string;
}

/** Markup for the empty state. Insert into the view's content container; call `wireEmptyState`
 *  afterwards (once, on the same container) to hook up the optional back-to-Revue link.
 *  `actionHtml` (if provided) needs no extra wiring call — the caller already owns its click
 *  handler. */
export function emptyStateHtml(opts: EmptyStateOpts): string {
  const link = opts.backToRevue
    ? `<button type="button" data-empty="revue" class="sift-empty-link">Ouvrir Revue</button>`
    : "";
  return (
    `<div class="sift-empty-state">` +
    `<div class="sift-empty-title">${esc(opts.title)}</div>` +
    `<div class="sift-empty-note">${esc(opts.note)}</div>` +
    link +
    (opts.actionHtml ?? "") +
    `</div>`
  );
}

/** Wire the "Ouvrir Revue" link (a no-op if the markup didn't include one). Navigates via the
 *  same nav-click pattern already used elsewhere (filing.ts goto-reglages): dispatch a click on
 *  the real nav item rather than duplicating the router. */
export function wireEmptyState(root: ParentNode): void {
  root.querySelector<HTMLElement>('[data-empty="revue"]')?.addEventListener("click", () => {
    requireEl('[data-view="revue"]', "empty-state goto-revue").dispatchEvent(
      new MouseEvent("click", { bubbles: true }),
    );
  });
}
