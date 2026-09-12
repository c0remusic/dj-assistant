// Shared, stateless rendering of Discogs candidate rows — used by both the Revue filing
// footer (filing.ts) and the Bibliothèque detail panel (library-detail.ts). Pure HTML
// builders + the "first result + N others" list layout; the stateful apply/changer wiring
// lives in each caller (it differs: filing edits canonical fields, the library edits a
// filed track's metadata). Keeps the candidate markup in one place (spec: zero duplication).
import type { Candidate } from "./ipc";
import { esc } from "./dom";
import { humanizeError } from "./errors";

/** Cover thumbnail (or vinyl placeholder) for a candidate row. */
function candCoverHtml(c: Candidate): string {
  if (c.cover_url) {
    return `<img src="${esc(c.cover_url)}" alt="" class="sift-cand-noart" loading="lazy">`;
  }
  return '<span class="sift-cand-noart"><i class="ti ti-vinyl" style="font-size:var(--text-xl);color:var(--color-text-tertiary)"></i></span>';
}

/** One candidate button row (sub-line: label · year · country · format). In an open listbox (Revue,
 *  fork F) it carries role=option + aria-selected via `opt`; the collapsed variant (Bibliothèque)
 *  omits it. */
function candRowHtml(c: Candidate, idx: number, opt?: { selected: boolean }): string {
  const sub = [c.label, c.year != null ? String(c.year) : null, c.country, c.format]
    .filter(Boolean)
    .join(" · ");
  const roleAttr = opt ? ` role="option" aria-selected="${opt.selected}"` : "";
  return (
    `<button class="sift-cand" data-cand="${idx}"${roleAttr}>` +
    candCoverHtml(c) +
    `<span class="sift-cand-meta"><span>${esc(c.artist)} — ${esc(c.title)}</span>` +
    (sub ? `<small>${esc(sub)}</small>` : "") +
    `</span></button>`
  );
}

/** La release CHOISIE, rendue SEULE après la fermeture de la liste (retours d'Antoine des
 *  2026-09-06/07 : une liste qui reste ouverte après le choix se lit comme inachevée, mais la
 *  release choisie, elle, doit rester visible). Même anatomie qu'une ligne candidate — pochette,
 *  « artiste — titre », sous-ligne — mais un <div> inerte : cliquer ne fait rien, permuter =
 *  re-cliquer Ré-identifier. `coverSrc` : URL Discogs (choix frais) ou fichier local converti
 *  (réopen). */
export function chosenRowHtml(r: {
  artist: string;
  title: string;
  sub: string;
  coverSrc: string | null;
}): string {
  const cover = r.coverSrc
    ? `<img src="${esc(r.coverSrc)}" alt="" class="sift-cand-noart">`
    : '<span class="sift-cand-noart"><i class="ti ti-vinyl" style="font-size:var(--text-xl);color:var(--color-text-tertiary)"></i></span>';
  return (
    `<div class="sift-cand sift-cand-chosen">` +
    cover +
    `<span class="sift-cand-meta"><span>${esc(r.artist)} — ${esc(r.title)}</span>` +
    (r.sub ? `<small>${esc(r.sub)}</small>` : "") +
    `</span></div>`
  );
}

/** Render candidates into `host`. Two layouts :
 *  - Revue (fork F, `opts.open`) : une LISTE OUVERTE (listbox) — tous les candidats visibles, le
 *    meilleur (`opts.selectedIdx`, défaut 0) pré-sélectionné (aria-selected). La décision centrale
 *    ne coûte pas un clic d'ouverture, et se navigue au clavier.
 *  - Bibliothèque (défaut) : premier résultat inline, le reste derrière un « N autres résultats ».
 *  Empty list → a neutral "no results" message (no warning styling). */
export function renderCandidates(
  host: HTMLElement,
  list: Candidate[],
  opts?: { open?: boolean; selectedIdx?: number },
): void {
  if (list.length === 0) {
    host.innerHTML = '<div class="sift-cands-msg">Rien sur Discogs.</div>';
    return;
  }
  if (opts?.open) {
    const sel = opts.selectedIdx ?? 0;
    host.innerHTML =
      `<div class="sift-cands-list" role="listbox" aria-label="Éditions Discogs">` +
      list.map((c, i) => candRowHtml(c, i, { selected: i === sel })).join("") +
      `</div>`;
    return;
  }
  const [first, ...rest] = list;
  const moreHtml = rest.length
    ? `<details class="sift-cand-more"><summary class="sift-cand-more-summary">▸ ${rest.length} autre${rest.length > 1 ? "s" : ""} résultat${rest.length > 1 ? "s" : ""}</summary>${rest.map((c, i) => candRowHtml(c, i + 1)).join("")}</details>`
    : "";
  host.innerHTML = candRowHtml(first, 0) + moreHtml;
}

/** Ce qu'un échec d'identification Discogs dit à l'utilisateur, en un seul endroit.
 *
 *  Les deux appelants (`filing-identify.ts`, `library-detail.ts`) portaient la même cascade de
 *  branches, dupliquée — et donc les mêmes deux défauts, deux fois :
 *
 *  - **A9** (issue #15) : le texte NO_TOKEN parlait de « recherches anonymes ». Il n'en existe
 *    aucune. `ipc_identify.rs` rend `NO_TOKEN` AVANT tout appel réseau et `settings.rs` le dit —
 *    « Empty/unset = identification disabled ». Sans jeton l'identification n'est pas dégradée,
 *    elle est absente.
 *  - **A10** (issue #15) : un jeton refusé (401/403) arrivait ici en `NETWORK:` et s'affichait
 *    « Discogs injoignable », envoyant l'utilisateur vérifier une connexion qui allait bien.
 *    `BAD_TOKEN:` existe désormais côté Rust ; il se dit comme ce qu'il est.
 *
 *  Ce n'est PAS la table code -> message qu'`errors.ts` refuse : là-bas le refus porte sur un
 *  humanisateur générique deviné, appliqué à toutes les erreurs de l'app. Ici les quatre codes
 *  sont produits par un seul `ProviderError::code()`, à trois `match` de distance, et l'un d'eux
 *  porte une donnée à afficher (les secondes du débit).
 *
 *  La chaîne brute part en console ICI, une fois, via `humanizeError` — c'est le seul point qui
 *  voit toutes les branches, donc le seul où la garantie ne peut pas être oubliée par une branche
 *  ajoutée plus tard. */
export function identifyErrorHtml(err: unknown): { html: string; gotoReglages: boolean } {
  const { texte, grave, gotoReglages } = identifyErrorText(err);
  const classe = grave ? "sift-cands-msg sift-cands-error" : "sift-cands-msg";
  const icone = grave ? `<i class="ti ti-alert-triangle sift-cand-error-icon"></i>` : "";
  return { html: `<div class="${classe}">${icone}${esc(texte)}</div>`, gotoReglages };
}

/** La MÊME phrase, sans balise — pour une surface qui n'est pas la liste de candidats.
 *
 *  Extrait le 2026-08-18 en câblant le bouton « Vérifier » des Réglages (impasse A11, issue #15).
 *  Le second appelant aurait sinon recopié les quatre branches, et c'est exactement le défaut que
 *  cette fonction-ci a été écrite pour corriger : la cascade dupliquée portait deux fois les mêmes
 *  deux erreurs de texte (A9 et A10).
 *
 *  `grave` sépare ce qui accuse le fichier ou le jeton de ce qui demande d'attendre — c'est la
 *  seule distinction dont une surface a besoin pour choisir son ton. `humanizeError` reste appelé
 *  ICI, une fois, pour que la chaîne brute parte en console quelle que soit la branche. */
export function identifyErrorText(err: unknown): {
  texte: string;
  grave: boolean;
  gotoReglages: boolean;
} {
  const msg = String(err);
  humanizeError(err, msg, "identify");
  if (msg.includes("NO_TOKEN")) {
    return {
      texte:
        "L'identification Discogs demande un jeton — sans lui, Sift n'interroge pas Discogs du tout. Il est gratuit et se colle dans Réglages.",
      grave: false,
      gotoReglages: true,
    };
  }
  if (msg.includes("BAD_TOKEN")) {
    return {
      texte:
        "Discogs a refusé le jeton — il est invalide, expiré ou révoqué. Ce n'est pas la connexion : réessayer ne changera rien.",
      grave: true,
      gotoReglages: true,
    };
  }
  const rl = msg.match(/RATE_LIMITED:(\d+)/);
  if (rl) {
    return {
      texte: `Discogs limite le débit — réessaie dans ${rl[1]}s.`,
      grave: false,
      gotoReglages: false,
    };
  }
  return { texte: "Discogs injoignable.", grave: true, gotoReglages: false };
}
