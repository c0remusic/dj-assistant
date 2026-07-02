// Écartés (Discarded) view (Tauri only): the real rejected/trashed tracks with re-source links.
// Extracted from sift-live.ts (audit P-3). Row actions (Soulseek copy / send-to-bin / restore /
// empty-bin / store link) are handled by the delegated #pa click handler in sift-live, which
// re-renders via this module's renderEcartes.
import { listEcartes } from "./ipc";
import type { EcarteItem } from "../shared/contracts";
import { requireEl } from "./dom";
import { emptyStateHtml, wireEmptyState } from "./empty-state";

const esc = (s: string) =>
  s.replace(/[&<>"']/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : c === '"' ? "&quot;" : "&#39;",
  );

/** Reason chip for an écarté track (truncated → tronqué, fake → faux, else à re-sourcer). Uses the
 *  shared `.sift-vchip` component (Revue-Détail's evidence chips) so tone/shape stay consistent
 *  across screens instead of ad-hoc inline styles. */
function ecReason(it: EcarteItem): string {
  if (it.truncated)
    return '<span class="sift-vchip" style="background:var(--color-background-warning);color:var(--color-text-warning);flex:none"><i class="ti ti-cut" style="font-size:var(--text-2xs)"></i> tronqué</span>';
  if (it.verdict === "fake")
    return '<span class="sift-vchip" style="background:var(--color-background-danger);color:var(--color-text-danger);flex:none"><i class="ti ti-alert-triangle" style="font-size:var(--text-2xs)"></i> faux</span>';
  return '<span class="sift-vchip" style="background:var(--color-background-danger);color:var(--color-text-danger);flex:none"><i class="ti ti-alert-circle" style="font-size:var(--text-2xs)"></i> à re-sourcer</span>';
}

/** The "Artiste Titre" string to paste into Soulseek (single space; no dash). */
function ecSlsk(it: EcarteItem): string {
  if (it.artist && it.title) return `${it.artist} ${it.title}`;
  return (it.filename || it.path).replace(/\.[^.]+$/, "");
}

// Buy-link stores: a search URL built from the track's query (q is already encoded).
const EC_STORES: [string, (q: string) => string][] = [
  ["Beatport", (q) => `https://www.beatport.com/search?q=${q}`],
  ["Traxsource", (q) => `https://www.traxsource.com/search?term=${q}`],
  ["Juno", (q) => `https://www.junodownload.com/search/?q%5Ball%5D%5B%5D=${q}`],
  ["Bandcamp", (q) => `https://bandcamp.com/search?q=${q}`],
  ["Amazon", (q) => `https://www.amazon.fr/s?k=${q}&i=digital-music`],
  ["Apple Music", (q) => `https://music.apple.com/fr/search?term=${q}`],
];

/** Buy-link row for a track: store names that open a search in the default browser. */
function ecStoreLinks(it: EcarteItem): string {
  const q = encodeURIComponent(ecSlsk(it));
  return EC_STORES.map(
    ([label, fn]) =>
      `<a data-ec="store" data-url="${encodeURIComponent(fn(q))}" style="font-size:var(--text-xs);color:var(--color-text-info);cursor:pointer;text-decoration:none;white-space:nowrap">${label}</a>`,
  ).join('<span style="color:var(--color-border-secondary);margin:0 3px">·</span>');
}

/** Live Écartés view: replaces #content with the real rejected (à re-sourcer) + trashed
 * tracks. Soulseek copy + send-to-bin / restore / empty-bin wired via the #pa handler. */
export async function renderEcartes() {
  const content = requireEl("#content", "renderEcartes");
  let items: EcarteItem[] = [];
  try {
    items = await listEcartes();
  } catch (e) {
    console.error("listEcartes failed", e);
    return;
  }
  const res = items.filter((i) => i.status === "resourcing");
  const trash = items.filter((i) => i.status === "trash");
  const name = (it: EcarteItem) =>
    esc(it.artist && it.title ? `${it.artist} — ${it.title}` : it.filename || it.path);
  const fileLine = (it: EcarteItem) =>
    `<div style="font-size:var(--text-xs);color:var(--color-text-tertiary);font-family:var(--font-mono);white-space:nowrap;overflow:hidden;text-overflow:ellipsis">${esc(
      it.filename || it.path,
    )}</div>`;

  const resRows = res
    .map(
      (it) =>
        `<div style="padding:7px 4px;border-bottom:0.5px solid var(--color-border-tertiary)"><div style="display:flex;align-items:center;gap:7px"><div style="flex:1;min-width:0"><div style="white-space:nowrap;overflow:hidden;text-overflow:ellipsis;font-size:var(--text-md);font-weight:500">${name(
          it,
        )}</div>${fileLine(it)}</div>${ecReason(
          it,
        )}<button class="lk" data-ec="requeue" data-id="${it.id}" title="Restaurer — remettre en file"><i class="ti ti-arrow-back-up" style="font-size:var(--text-base);color:var(--color-text-tertiary)"></i></button><button class="lk" data-ec="trash" data-id="${it.id}" title="Envoyer à la corbeille"><i class="ti ti-trash" style="font-size:var(--text-md);color:var(--color-text-tertiary)"></i></button></div><div style="margin-top:5px;display:flex;flex-wrap:wrap;align-items:center;gap:4px"><button data-ec="slsk" data-q="${esc(
          ecSlsk(it),
        )}" title="Copier « Artiste Titre » pour chercher sur Soulseek" style="font-size:var(--text-xs);padding:2px 7px;color:var(--color-text-secondary)"><i class="ti ti-copy" style="font-size:var(--text-xs);vertical-align:-1px"></i> Copier le nom</button><span style="color:var(--color-border-secondary)">·</span>${ecStoreLinks(
          it,
        )}</div></div>`,
    )
    .join("");

  const trashRows = trash
    .map(
      (it) =>
        `<div style="display:flex;align-items:center;gap:7px;padding:7px 4px;border-bottom:0.5px solid var(--color-border-tertiary)"><div style="flex:1;min-width:0"><div style="white-space:nowrap;overflow:hidden;text-overflow:ellipsis;font-size:var(--text-md)">${name(
          it,
        )}</div>${fileLine(it)}</div><button data-ec="restore" data-id="${it.id}" title="Restaurer — remettre en file" style="font-size:var(--text-xs);padding:2px 8px;color:var(--color-text-info)">Restaurer</button></div>`,
    )
    .join("");

  content.innerHTML =
    '<div class="h1">Écartés</div>' +
    (items.length === 0
      ? emptyStateHtml({
          title: "Rien dans Écartés",
          note: "Les pistes que tu écartes depuis Revue apparaissent ici, avec possibilité de les restaurer.",
          backToRevue: true,
        })
      : '<div style="display:flex;gap:7px;margin-bottom:12px;flex-wrap:wrap;align-items:center">' +
        `<span class="pill" style="background:var(--color-background-danger);color:var(--color-text-danger)"><i class="ti ti-alert-circle" style="font-size:var(--text-xs)"></i> ${res.length} à re-sourcer</span>` +
        `<span class="pill"><i class="ti ti-trash" style="font-size:var(--text-xs)"></i> ${trash.length} en corbeille</span>` +
        (trash.length
          ? `<button data-ec="purge" title="Purger — suppression définitive" style="font-size:var(--text-xs);padding:2px 8px;color:var(--color-text-danger)">Purger la corbeille (${trash.length})</button>`
          : "") +
        "</div>" +
        (res.length ? `<div class="col-h">À re-sourcer</div>${resRows}` : "") +
        (trash.length ? `<div class="col-h" style="margin-top:14px">Corbeille</div>${trashRows}` : ""));
  wireEmptyState(content);
}
