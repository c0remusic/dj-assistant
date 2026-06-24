// Shared, stateless rendering of Discogs candidate rows — used by both the Revue filing
// footer (filing.ts) and the Bibliothèque detail panel (library-detail.ts). Pure HTML
// builders + the "first result + N others" list layout; the stateful apply/changer wiring
// lives in each caller (it differs: filing edits canonical fields, the library edits a
// filed track's metadata). Keeps the candidate markup in one place (spec: zero duplication).
import type { Candidate } from "./ipc";

const esc = (s: string) =>
  s.replace(/[&<>"']/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : c === '"' ? "&quot;" : "&#39;",
  );

/** Cover thumbnail (or vinyl placeholder) for a candidate row. */
function candCoverHtml(c: Candidate): string {
  if (c.cover_url) {
    return `<img src="${esc(c.cover_url)}" alt="" class="sift-cand-noart" loading="lazy">`;
  }
  return '<span class="sift-cand-noart"><i class="ti ti-vinyl" style="font-size:18px;color:var(--color-text-tertiary)"></i></span>';
}

/** One candidate button row (sub-line: label · year · country · format). */
export function candRowHtml(c: Candidate, idx: number): string {
  const sub = [c.label, c.year != null ? String(c.year) : null, c.country, c.format]
    .filter(Boolean)
    .join(" · ");
  return (
    `<button class="sift-cand" data-cand="${idx}">` +
    candCoverHtml(c) +
    `<span class="sift-cand-meta"><span>${esc(c.artist)} — ${esc(c.title)}</span>` +
    (sub ? `<small>${esc(sub)}</small>` : "") +
    `</span></button>`
  );
}

/** Render candidates into `host`: first result inline, the rest behind a "N autres résultats"
 * disclosure. Empty list → a neutral "no results" message (no warning styling). */
export function renderCandidates(host: HTMLElement, list: Candidate[]): void {
  if (list.length === 0) {
    host.innerHTML = '<div class="sift-cands-msg">Rien sur Discogs.</div>';
    return;
  }
  const [first, ...rest] = list;
  const moreHtml = rest.length
    ? `<details class="sift-cand-more"><summary class="sift-cand-more-summary">▸ ${rest.length} autre${rest.length > 1 ? "s" : ""} résultat${rest.length > 1 ? "s" : ""}</summary>${rest.map((c, i) => candRowHtml(c, i + 1)).join("")}</details>`
    : "";
  host.innerHTML = candRowHtml(first, 0) + moreHtml;
}
