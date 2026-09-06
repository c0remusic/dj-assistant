import { identify, applyIdentity, applyTags, trackFileTags, openUrl, revertBatch } from "./ipc";
import type { Candidate, AppliedIdentity } from "./ipc";
import { convertFileSrc } from "@tauri-apps/api/core";
import { identifyErrorHtml, renderCandidates } from "./identify-shared";
import { requireEl, esc } from "./dom";
import { state, openState } from "./filing-state";
import { toast } from "./filing-toast";
import { refreshPreview, updateHeaderName, titleCase } from "./filing-preview";
import { humanizeError } from "./errors";

// Exclusive accordion (shadcn Accordion reference, ui.shadcn.com/docs/components/base/accordion):
// opening Métadonnées closes Diagnostic and vice versa. Coordinated with report-view.ts (no
// shared ancestor passed down) via a document-level event — see the matching listener there for
// why a single module-load-time registration doesn't leak across track re-opens.
let closeMetaZone: (() => void) | null = null;
document.addEventListener("sift:accordion-open", (e) => {
  if ((e as CustomEvent).detail?.zone !== "metadonnees") closeMetaZone?.();
});

// Per-track Discogs release facts (label/year/country/format), captured when an identity is
// applied so they survive a close+reopen of the SAME track within the session. `reconcile` (the
// only open-time read) doesn't return them, and re-reading would need a new IPC — so we hold them
// in memory. Keyed by track id. Cross-session reopen won't repopulate this (a fresh process starts
// empty) — country/format additionally have no persisted backend column at all (unlike label/year,
// which trackRelease re-populates from the `metadata` table on a real reopen), so those two are
// session-only regardless of process lifetime.
export const releaseCache = new Map<
  number,
  { label: string | null; year: number | null; country: string | null; format: string | null }
>();

/** Render the genres into `.sift-genres` from `state.genres` (single source — set on open from
 *  track_release, or from `applied.styles` on identify) as plain text preceded by a tag glyph
 *  (fork F, 2026-08-24 — plus de chips colorées par famille). Empty list → empty box. */
export function renderGenres(): void {
  const el = document.querySelector<HTMLElement>(".sift-genres");
  if (!el) return; // editor not mounted
  el.innerHTML = state.genres.length
    ? `<i class="ti ti-tag sift-genres-tagicon" aria-hidden="true"></i>${esc(state.genres.join(", "))}`
    : "";
}

/** Join genres EXACTLY like write_tags_full (trim, drop empties, "A; B"), so the comparison against
 *  the file's single Genre field is like-for-like. */
const joinGenres = (g: string[]): string => g.map((s) => s.trim()).filter(Boolean).join("; ");

/** Which displayed tag fields would CHANGE the file if written — i.e. diverge from `state.fileTags`.
 *  Mirrors write_tags_full's semantics: artist/title are ALWAYS written (compare directly), while
 *  label/year/genres are only written when non-empty (an empty would-write never clears the file, so
 *  it is NOT a discrepancy). All comparison is in memory against the on-open snapshot — no disk read. */
function tagFieldDiffs(): { artist: boolean; title: boolean; label: boolean; year: boolean; genres: boolean; any: boolean } {
  const f = state.fileTags;
  const c = state.canonical;
  const none = { artist: false, title: false, label: false, year: false, genres: false, any: false };
  if (!f || !c) return none; // snapshot not loaded yet → show nothing rather than a false alarm
  const norm = (s: string | null | undefined): string => (s ?? "").trim();
  // Non-empty guard added (annotation: "quand les champs sont vides, je ne veux pas de texte en
  // italique") — an untyped/not-yet-identified field showing "stale" (italic+warning) read as a
  // real conflict when it was really just nothing entered yet. Same non-empty guard label/year/
  // genres already had below; artist/title never had it.
  const artistW = norm(c.artist);
  const artist = artistW !== "" && artistW !== norm(f.artist);
  // Mirrors naming::tag_title (Rust) — the ID3 Title tag now includes the version suffix on
  // write, so the comparison must too. Without this, editing ONLY the version field never
  // changed titleW vs f.title (version was silently ignored here), leaving "Appliquer" greyed
  // out no matter what was typed (annotation: "le bouton appliquer reste grisé si on edit la
  // version"). c.version is never sent to the write itself — building the same combined string
  // on both sides so they compare like-for-like, not two different sources deriving one value.
  const versionW = norm(c.version);
  const titleW = norm(versionW ? `${c.title} (${versionW})` : c.title);
  const title = titleW !== "" && titleW !== norm(f.title);
  // Label rides on Canonical now (editable) — compare the EDITED value against the file, like
  // artist/title. Same non-empty guard: a blank label never counts as a discrepancy (write_tags_full
  // won't clear the file's existing one either).
  const labelW = norm(c.label);
  const label = labelW !== "" && labelW !== norm(f.label);
  const yearW = state.year ?? 0;
  const year = yearW > 0 && yearW !== (f.year ?? 0);
  const genresW = joinGenres(state.genres);
  const genres = genresW !== "" && genresW !== norm(f.genre_joined);
  return { artist, title, label, year, genres, any: artist || title || label || year || genres };
}

/** Show/hide the "tags not written" banner and mark the diverging fields. Cheap (a few
 *  querySelectors + class toggles) — safe to call on open, on each field edit, and after Apply/File.
 *  Reads `state.fileTags` (the cached snapshot), never the disk. */
export function refreshDiscrepancy(): void {
  const editor = document.querySelector<HTMLElement>(".sift-fil-editor");
  if (!editor) return;
  const d = tagFieldDiffs();
  const banner = editor.querySelector<HTMLElement>(".sift-tag-warn");
  // Visibility via display ONLY (the banner has no `hidden` attribute — that conflicted with an
  // inline display and kept it stuck on). flex when there's a discrepancy, none otherwise.
  if (banner) banner.style.display = d.any ? "flex" : "none";
  const mark = (sel: string, on: boolean) =>
    editor.querySelector<HTMLElement>(sel)?.classList.toggle("sift-tag-stale", on);
  mark('[data-fil="artist"]', d.artist);
  mark('[data-fil="title"]', d.title);
  mark(".sift-genres", d.genres);
}

/** Apply an identity result to the editing fields + filename preview. Liste ouverte (fork F) :
 *  `host` + `chosenIdx` servent à MARQUER le candidat appliqué (aria-selected) dans la liste qui
 *  reste ouverte, sans la replier. `write` grave l'ID3 tout de suite (clic sur un match, décision
 *  F.2) ; l'auto-apply du meilleur match passe `write=false` — il PRÉ-REMPLIT sans graver (fork A). */
function onIdentityApplied(
  applied: AppliedIdentity,
  chosen: Candidate,
  chosenIdx: number,
  editor: HTMLElement,
  mid: HTMLElement,
  host: HTMLElement,
  idBtn: HTMLButtonElement,
  write = true,
): void {
  if (!state.canonical) return;
  state.canonical.artist = applied.canonical.artist;
  // Split a trailing "(Version)" out of the Discogs title so it's never duplicated: the title
  // field gets the clean base, the version field gets the mix. Prefer the version Discogs put
  // in the title; otherwise keep the one parsed from the local name (Discogs search doesn't
  // always expose a per-track version). Fixes e.g. "Love Foolosophy (Knee Deep Remix) (Knee
  // Deep Remix)".
  const m = applied.canonical.title.match(/^(.*?)\s*\(([^()]+)\)\s*$/);
  const baseTitle = m ? m[1].trim() : applied.canonical.title.trim();
  const rawVersion = (m ? m[2].trim() : null) ?? state.canonical.version;
  const version = rawVersion ? titleCase(rawVersion) : null;
  state.canonical.title = baseTitle;
  state.canonical.version = version;

  // Update the editable inputs directly. Label included now that it's editable (it rides on
  // Canonical) — set the edited value here where state.canonical is freshly narrowed non-null,
  // alongside artist/title/version; its input is patched in place below with the others.
  state.canonical.label = applied.label;
  const aInp = editor.querySelector<HTMLInputElement>('[data-fil="artist"]');
  const tInp = editor.querySelector<HTMLInputElement>('[data-fil="title"]');
  const vInp = editor.querySelector<HTMLInputElement>('[data-fil="version"]');
  const lInp = editor.querySelector<HTMLInputElement>('[data-fil="label"]');
  if (aInp) aInp.value = applied.canonical.artist;
  if (tInp) tInp.value = baseTitle;
  if (vInp) vInp.value = version ?? "";
  if (lInp) lInp.value = applied.label ?? "";

  // Refresh the filename preview using the same logic as the input handler.
  refreshPreview();
  updateHeaderName(mid);

  // Read-only release facts from the chosen Discogs release. Cache them on the track so a
  // close+reopen within the session re-shows them (reconcile doesn't carry label/year). Choosing a
  // different candidate re-enters here with the new release → the line updates in place.
  state.label = applied.label;
  state.year = applied.year;
  // Country/format only ever exist on the search-result candidate (chosen), never on AppliedIdentity
  // (Rust apply_identity_cmd doesn't return them, and metadata has no column for either) — take them
  // from the same candidate object the click already had, so they don't vanish once the candidate
  // list is replaced by the "Identifié :" confirmation line (2026-07-06 annotation).
  state.releaseCountry = chosen.country;
  state.releaseFormat = chosen.format;
  state.coverPath = applied.cover_path;
  if (state.track) {
    releaseCache.set(state.track.id, {
      label: applied.label,
      year: applied.year,
      country: chosen.country,
      format: chosen.format,
    });
  }

  // Show the cover if we have a local path. Every match, not just the first — the Hero and the
  // player's mini header both carry this class now. Probe non-throw — the report pane may be
  // gone after the identify await / a navigation.
  if (applied.cover_path) {
    const src = convertFileSrc(applied.cover_path);
    mid.querySelectorAll<HTMLImageElement>(".sift-report-cover").forEach((covEl) => {
      // Discogs sometimes returns a placeholder ("no image") instead of real art — the file
      // downloads fine but fails to decode/display as a photo. Re-hide on error so the vinyl
      // ::before fallback shows instead of a broken-image glyph on top of it.
      covEl.onerror = () => { covEl.hidden = true; };
      covEl.src = src;
      covEl.hidden = false;
    });
  }

  // [m11] Genres: store the would-write list (single source) and render the chips. The list also
  // feeds the file-vs-display discrepancy check (joined form), so it must live in state, not only DOM.
  state.genres = applied.styles;
  renderGenres();

  // A Discogs match now exists → if the file is a fake/transcode, offer the rebuy search link.
  state.identified = true;
  refreshRebuyLink();

  // Liste ouverte (fork F) : au lieu de replier en ligne « Identifié », on MARQUE le candidat appliqué
  // (aria-selected) dans la liste qui reste visible, et on le flashe brièvement. Permuter = cliquer un
  // autre item. Plus d'identifiedLineHtml ni de bouton « changer ».
  host.hidden = false;
  host.querySelectorAll<HTMLElement>("[data-cand]").forEach((el) => {
    el.setAttribute("aria-selected", String(Number(el.dataset.cand) === chosenIdx));
    el.style.opacity = "";
    el.style.pointerEvents = "";
  });
  const selEl = host.querySelector<HTMLElement>(`[data-cand="${chosenIdx}"]`);
  if (selEl) {
    selEl.classList.add("sift-identified-flash");
    selEl.addEventListener("animationend", () => selEl.classList.remove("sift-identified-flash"), {
      once: true,
    });
  }
  // Read-only unidentified card (sift-ident-idle): the idle note ("Aucune correspondance…") is now
  // false — drop it, keeping the search button (relabelled Ré-identifier below).
  editor.querySelector(".sift-ident-idle-note")?.remove();

  // [C1] Relabel Identifier → Ré-identifier once an identity has been applied.
  // TEXTE SEUL, comme le premier rendu de ce bouton plus bas dans ce fichier : c'est le MÊME
  // bouton dans un autre état, et n'en corriger qu'un chemin ferait revenir l'icône dès qu'une
  // identité est appliquée. `textContent` et non `innerHTML` — il n'y a plus de balise à poser,
  // et le badge de raccourci `I` n'a jamais été rendu par ce chemin-ci.
  idBtn.textContent = "Ré-identifier";

  // The displayed identity just changed while the FILE keeps its old tags → surface the gap.
  refreshDiscrepancy();

  // Choisir un match (clic) grave l'ID3 tout de suite (décision F.2) — plus de bouton Appliquer,
  // l'écriture est automatique. `write` reste par sécurité (défaut true ; il n'y a plus d'appelant
  // à false depuis le retrait de l'auto-apply).
  if (write) void doApplyTags();
}

/** On reopen of an already-identified track, restore the hero cover (mid `.sift-report-cover`) from
 *  the Discogs cover path — the identity's cover isn't carried by the analysis report, so without
 *  this the hero/player cover stayed hidden until you re-ran Identify
 *  (docs/superpowers/reviews/2026-07-02-audit-fidelite-ecran-par-ecran.md décision #5). Discogs
 *  placeholder art can fail to decode → re-hide on error, same as onIdentityApplied. In direction B
 *  the identity itself is shown by the always-visible attribute inputs, so no "Identifié :" line is
 *  drawn on reopen — only the cover needs restoring. */
export function restoreCover(mid: HTMLElement, coverPath: string | null): void {
  if (!coverPath) return;
  const src = convertFileSrc(coverPath);
  mid.querySelectorAll<HTMLImageElement>(".sift-report-cover").forEach((covEl) => {
    covEl.onerror = () => { covEl.hidden = true; };
    covEl.src = src;
    covEl.hidden = false;
  });
}

/** Wire clicks on rendered candidate buttons.
 * Extracted so it can be called after initial render AND after "changer" re-shows the list. */
function wireCandidateClicks(
  host: HTMLElement,
  candidates: Candidate[],
  editor: HTMLElement,
  mid: HTMLElement,
  idBtn: HTMLButtonElement,
): void {
  host.querySelectorAll<HTMLElement>("[data-cand]").forEach((el) => {
    const idx = Number(el.dataset.cand);
    el.addEventListener("click", () => {
      const c = candidates[idx];
      if (!c || !state.track) return;
      el.style.opacity = "0.5";
      el.style.pointerEvents = "none";
      // FIX-21: openState.openSeq-guarded, same pattern as openFilingInto/setApplyIdle — without it, a
      // slow applyIdentity resolving after the user already navigated to a different track would
      // write the fetched metadata onto the WRONG track's pane (state.canonical, cover, DOM).
      const myseq = openState.openSeq;
      void applyIdentity(state.track.id, c)
        .then((applied) => {
          if (myseq !== openState.openSeq) return; // a newer open started while we awaited — drop this result
          onIdentityApplied(applied, c, idx, editor, mid, host, idBtn);
        })
        .catch((e) => {
          if (myseq !== openState.openSeq) return;
          el.style.opacity = "";
          el.style.pointerEvents = "";
          // [m10] errors get a warning icon to distinguish from "no results"
          host.innerHTML = `<div class="sift-cands-msg sift-cands-error"><i class="ti ti-alert-triangle sift-cand-error-icon"></i>${esc(humanizeError(e, "Impossible d'appliquer cette release — réessaie", "apply_identity"))}</div>`;
        });
    });
  });
}

/** ↑/↓ dans la liste ouverte : déplace le focus entre les candidats (le gate de queue-panel laisse
 *  passer quand le focus est dans .sift-cands). Attaché à la listbox FRAÎCHE de chaque render, donc
 *  pas d'accumulation de handlers d'un search à l'autre. */
function wireListboxArrows(host: HTMLElement): void {
  const listbox = host.querySelector<HTMLElement>(".sift-cands-list");
  if (!listbox) return;
  listbox.addEventListener("keydown", (e) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    const btns = Array.from(listbox.querySelectorAll<HTMLElement>("[data-cand]"));
    const cur = btns.indexOf(document.activeElement as HTMLElement);
    const next = e.key === "ArrowDown" ? cur + 1 : cur - 1;
    if (next >= 0 && next < btns.length) {
      e.preventDefault();
      btns[next].focus();
    }
  });
}

/** Run the Discogs identify flow for the current track. */
async function doIdentify(
  btn: HTMLButtonElement,
  host: HTMLElement,
  editor: HTMLElement,
  mid: HTMLElement,
): Promise<void> {
  if (!state.track) return;
  const trackId = state.track.id;
  // FIX-21: openState.openSeq-guarded — identify's await can outlive the user navigating to another
  // track (openFilingInto bumps openState.openSeq on every open); without this a slow/late response
  // painted candidates/errors from THIS track's search into a pane now showing a different one.
  const myseq = openState.openSeq;
  const origLabel = btn.innerHTML;
  btn.disabled = true;
  btn.innerHTML = '<i class="ti ti-loader-2 sift-spin sift-searching-icon"></i> Recherche…';
  host.hidden = false;
  host.innerHTML = '<div class="sift-cands-msg">Recherche…</div>';

  let candidates: Candidate[] = [];
  try {
    candidates = await identify(trackId);
    if (myseq !== openState.openSeq) return; // a newer open started while we awaited — drop this result
    renderCandidates(host, candidates, { open: true, selectedIdx: 0 });
    wireCandidateClicks(host, candidates, editor, mid, btn);
    wireListboxArrows(host); // ↑/↓ déplace le focus entre candidats (le gate de queue-panel laisse passer)
    // PAS d'auto-apply (retour Antoine : un match auto appliqué à tort abîmerait le fichier). La
    // recherche AFFICHE les candidats, elle ne remplit rien — l'utilisateur clique un match pour
    // graver. On focus le meilleur (candidat 0) pour que ↑/↓ navigue la liste tout de suite.
    host.querySelector<HTMLElement>(".sift-cand")?.focus();
  } catch (err) {
    if (myseq !== openState.openSeq) return;
    // [C2/m5] expliquer POURQUOI + donner une action directe vers Réglages. La cascade de branches
    // vit dans `identifyErrorHtml` depuis le 2026-08-17 : elle était dupliquée à l'identique dans
    // `library-detail.ts`, donc chacun de ses deux défauts (A9, A10 — issue #15) existait en deux
    // exemplaires. Le `console.error` reste garanti par `humanizeError`.
    const { html, gotoReglages } = identifyErrorHtml(err);
    host.innerHTML =
      html +
      (gotoReglages
        ? `<button class="sift-cand-jump sift-goto-reglages" data-fil="goto-reglages"><i class="ti ti-arrow-right"></i> Ouvrir Réglages</button>`
        : "");
    host.querySelector<HTMLElement>('[data-fil="goto-reglages"]')?.addEventListener("click", () => {
      // Navigate to the Réglages view via the existing nav click handler in app.js
      requireEl('[data-view="reglages"]', "filing goto-reglages").dispatchEvent(
        new MouseEvent("click", { bubbles: true }),
      );
    });
  } finally {
    btn.disabled = false;
    btn.innerHTML = origLabel;
  }
}

export function renderEditor(host: HTMLElement, mid: HTMLElement, rail: string): void {
  void rail;
  const c = state.canonical;
  if (!c) {
    host.innerHTML = "";
    return;
  }
  host.innerHTML =
    // Header statique (spec revue.md § Zone C, direction B validée 2026-08-21) : Métadonnées est
    // TOUJOURS visible, sans accordéon NI bascule read-only/édition. Les valeurs s'éditent EN PLACE —
    // chaque ligne d'attribut porte son input, stylé comme du texte au repos, révélé au survol et au
    // focus. Un seul rendu quel que soit l'état : c'est tout le point de la direction B. Le bouton
    // "Identifier" ne bascule plus un mode, il lance la recherche Discogs qui remplit ces mêmes
    // champs. Ce header ne porte PLUS de badge CDJ : il a été retiré avec le passage au header
    // statique (2026-08-25), et `report.tags_cdj_ok` n'a aujourd'hui aucun consommateur dans
    // l'UI réelle — le seul rendu qui reste est celui de la maquette `app.js`, qui ne fait pas
    // autorité. Le critère backend, lui, est bien recâblé (#46, 2026-09-01).
    `<div class="sift-meta-header">` +
    `<span class="sift-meta-title">Métadonnées</span>` +
    `</div>` +
    `<div class="sift-meta-body">` +
    // Résultats Discogs — vide au repos, rempli le temps d'une recherche (doIdentify), au-dessus des
    // attributs pour que le choix d'une release précède l'édition. onIdentityApplied remplit ensuite
    // les inputs data-fil ci-dessous en place, sans re-render.
    `<div class="sift-cands sift-cands-host" hidden></div>` +
    // Liste d'attributs éditable en place : la valeur EST un input (data-fil écouté par `upd` à la
    // saisie et par onIdentityApplied au remplissage), stylé comme du texte tant qu'on ne le touche
    // pas. Labels persistants — annotation "on ne sait pas à quoi correspondent les champs".
    // Placeholder "—" quand vide, jamais une ligne vide.
    `<div class="sift-attr-list">` +
    `<div class="sift-attr"><span class="sift-attr-k">Artiste</span><input data-fil="artist" placeholder="—" value="${esc(c.artist)}" class="sift-attr-input" aria-label="Artiste"></div>` +
    `<div class="sift-attr"><span class="sift-attr-k">Titre</span><input data-fil="title" placeholder="—" value="${esc(c.title)}" class="sift-attr-input" aria-label="Titre"></div>` +
    `<div class="sift-attr"><span class="sift-attr-k">Version</span><input data-fil="version" placeholder="—" value="${esc(c.version ?? "")}" class="sift-attr-input" aria-label="Version"></div>` +
    // Label — fait de release Discogs, ÉDITABLE EN PLACE (retour vérif visuelle 2026-08-25 :
    // l'utilisateur veut corriger le label). La valeur EST un input `data-fil="label"`, câblé comme
    // Artiste/Titre/Version : `upd` le lit vers state.canonical.label (label voyage désormais DANS
    // Canonical), et il se grave au fichier (blur/Entrée) via doApplyTags → write_tags_full (+
    // persiste metadata.label). Rempli en place par onIdentityApplied. Placeholder "—" quand vide.
    `<div class="sift-attr"><span class="sift-attr-k">Label</span><input data-fil="label" placeholder="—" value="${esc(c.label ?? "")}" class="sift-attr-input" aria-label="Label"></div>` +
    `<div class="sift-attr"><span class="sift-attr-k">Genres</span><span class="sift-genres"></span></div>` +
    `</div>` +
    // Bouton Identifier SOUS les attributs, aligné au bord gauche des valeurs — décision 1b
    // d'Antoine (2026-09-06, wireframe avant/après) : l'ordre de lecture est l'ordre d'usage — on
    // lit les valeurs, elles clochent, l'action est là. Il vivait au bord droit de l'en-tête,
    // que la largeur du panneau envoyait à ~1300px du titre. Patron formulaire (action en pied,
    // sous les champs — Réglages Système). Coût assumé au wireframe : pendant une recherche, les
    // candidats s'insèrent au-dessus des attributs, loin de ce bouton.
    // Loupe retirée le 2026-08-26 (CTA à label descriptif = texte seul) ; le badge `I` RESTE — il
    // porte le raccourci clavier, une information que le libellé ne donne pas.
    `<div class="sift-meta-actions">` +
    `<button data-fil="identifier" class="sift-meta-ident-btn" title="Rechercher les métadonnées sur Discogs (pochette, label, année, genres)">${c.artist && c.title ? "Ré-identifier" : "Identifier"} <span class="kbd sift-kbd-hint-id">I</span></button>` +
    `</div>` +
    // Plus de bouton « Appliquer » (retour Antoine 2026-08-25) : les tags ID3 se gravent
    // AUTOMATIQUEMENT quand on finit d'éditer un champ (blur/Entrée) ou qu'on choisit un match
    // Discogs — voir doApplyTags, déclenché depuis le wiring des inputs et onIdentityApplied.
    // Rebuy link slot — filled by refreshRebuyLink() only for a fake track that also has a Discogs
    // match (empty, no gap, otherwise). Placed after genres so the identity block reads whole first.
    `<div class="sift-rebuy"></div>` +
    // Plus de ligne « Tags ID3 » ici (spec docs/ui-specs/revue.md § Zone C, point 4, annotation
    // d'Antoine « supprimé ») : `report.id3_version` était un drapeau de PRÉSENCE de tag conteneur
    // — le backend ne le renseignait que pour .mp3, et il y valait la chaîne « ID3 »
    // (analysis/tags.rs) — si bien que la ligne rendue disait « Tags ID3 : ID3 ». Tautologique.
    // Depuis le 2026-09-01 (issue #46) le champ porte le TYPE réel du porteur (« Id3v2 »,
    // « RiffInfo »…), donc la ligne ne serait plus tautologique — la décision de la retirer, elle,
    // n'a pas été rouverte. Elle avait déjà été
    // renommée une fois (« Version ID3 », annotation 2026-07-06) parce que le mot « version »
    // partagé avec le champ Version de Discogs juste au-dessus laissait croire qu'appliquer une
    // identité la remplissait, ce qu'elle n'a jamais fait : le renommage n'a pas suffi, la ligne
    // part. Ne pas la restaurer sans rouvrir la décision. Ce que la piste vaut pour un CDJ ne se
    // dit nulle part dans cette vue : le badge de l'en-tête Métadonnées est parti le 2026-08-25 et
    // rien ne l'a remplacé. Le bandeau `.sift-tag-warn` ci-dessous n'en tient pas lieu — il compare
    // l'affichage aux tags du fichier (`refreshDiscrepancy`), il ne lit pas `tags_cdj_ok`.
    // Elle était le SEUL usage du rapport d'analyse dans cet éditeur : le paramètre `report` de
    // renderEditor et l'import de `row` (report-view) sont partis avec elle.
    // Discrepancy banner — sits JUST BELOW Apply. Hidden by default via inline display:none; the LONE
    // visibility mechanism is refreshDiscrepancy toggling style.display (no `hidden`+display conflict).
    // Look lives in .sift-tag-warn (styles.css). Shown only when the display diverges from the file.
    `<div class="sift-tag-warn" role="status" aria-live="polite" style="display:none"><i class="ti ti-alert-triangle sift-icon-inline-md sift-icon-flex-none"></i><span>Artiste et Titre pas encore gravés dans le fichier (seulement identifiés ci-dessus) — un CDJ ne peut pas les lire tant que ce n'est pas fait. <strong>Convertir</strong> ou <strong>Appliquer les tags</strong> pour corriger.</span></div>` +
    `</div>`; // ferme .sift-meta-body

  // Métadonnées ne se replie plus (spec revue.md § Zone C) : rien à fermer quand Diagnostic s'ouvre,
  // donc l'accordéon exclusif est neutralisé côté Métadonnées (closeMetaZone reste null). Diagnostic
  // garde son propre repli, indépendant.
  closeMetaZone = null;

  const upd = () => {
    const a = host.querySelector<HTMLInputElement>('[data-fil="artist"]');
    const t = host.querySelector<HTMLInputElement>('[data-fil="title"]');
    const v = host.querySelector<HTMLInputElement>('[data-fil="version"]');
    const l = host.querySelector<HTMLInputElement>('[data-fil="label"]');
    if (!state.canonical) return;
    state.canonical.artist = a?.value ?? "";
    state.canonical.title = t?.value ?? "";
    state.canonical.version = v?.value.trim() ? v.value.trim() : null;
    // Label rides on Canonical now (empty → null, same discipline as version). Graved by doApplyTags.
    state.canonical.label = l?.value.trim() ? l.value.trim() : null;
    refreshPreview();
    refreshDiscrepancy(); // editing a field may make the display diverge from the file (or re-converge)
  };
  // Le titre en haut (hero, `.sift-report-name`) ne se met à jour qu'à la FIN de l'édition — au blur
  // ou sur Entrée — pas à chaque frappe (Antoine 2026-08-21 : « pas en même temps »). Le Nom final du
  // rail, lui, suit en direct (refreshPreview dans upd).
  const commitTitle = (): void => updateHeaderName(mid);
  // Entrée dans un champ = appliquer les tags au fichier (Antoine 2026-08-21). upd() d'abord pour que
  // state.canonical porte la dernière saisie, puis on grave dès qu'il y a une vraie divergence avec le
  // fichier (`tagFieldDiffs().any`) — plus fiable que l'état `.disabled` du bouton, qui reste
  // « Annuler » après l'apply auto d'une identification et ne se ré-arme pas. Même doApplyTags que le
  // bouton. preventDefault : l'Entrée globale « Convertir » est de toute façon gardée hors des INPUT
  // (filing.ts:570, DESIGN.md § 9).
  const applyOnEnter = (e: KeyboardEvent): void => {
    if (e.key !== "Enter") return;
    e.preventDefault();
    upd();
    commitTitle();
    if (tagFieldDiffs().any) void doApplyTags();
    (e.currentTarget as HTMLInputElement).blur();
  };
  host
    .querySelectorAll<HTMLInputElement>('[data-fil="artist"],[data-fil="title"],[data-fil="version"],[data-fil="label"]')
    .forEach((el) => {
      let focusVal = el.value;
      // Drapeau « ce blur-ci vient d'Échap, il ne grave RIEN ». Posé juste avant `el.blur()` du
      // chemin Échap, lu par le handler de blur ci-dessous. Corrige le bug du 2026-08-25 : Échap
      // annulait à l'écran (revert à `focusVal`) puis appelait `blur()`, dont le handler grave dès
      // que l'affichage diverge du fichier — donc ANNULER ÉCRIVAIT sur le disque. Le cas qui le rend
      // visible : une piste identifiée mais pas encore gravée, où `focusVal` diverge DÉJÀ du fichier,
      // si bien que le revert ne fait pas retomber `tagFieldDiffs().any`.
      // Drapeau plutôt que retrait/repose du listener : `blur()` est dispatché SYNCHRONEMENT, donc
      // la durée de vie du drapeau est exactement celle de l'appel et il ne peut pas fuir sur un blur
      // ultérieur ; un retrait/repose, lui, laisserait le champ définitivement sans écriture si un
      // throw traversait entre les deux. Ne touche pas `applyingTags` (garde anti-double-fire de
      // l'Entrée, doApplyTags), qui répond à une autre question.
      let escapeCancel = false;
      el.addEventListener("focusin", () => {
        focusVal = el.value;
      });
      el.addEventListener("input", upd);
      el.addEventListener("keydown", (e) => {
        if (e.key === "Enter") {
          applyOnEnter(e);
        } else if (e.key === "Escape") {
          // Échap = annuler l'édition : revert à la valeur du focus-in, resync l'état (upd), et
          // stopPropagation pour ne pas remonter fermer un popover/la fenêtre (couche 1, shortcuts.ts).
          e.preventDefault();
          e.stopPropagation();
          el.value = focusVal;
          upd();
          escapeCancel = true;
          try {
            el.blur();
          } finally {
            // `finally` et pas une remise à false dans le handler : si `el` n'a plus le focus,
            // `blur()` est un no-op, AUCUN événement blur n'est dispatché — et le drapeau resté
            // levé avalerait la prochaine vraie fin d'édition.
            escapeCancel = false;
          }
        }
      });
      el.addEventListener("blur", () => {
        // Graver EN FINISSANT l'édition (retour Antoine : plus de bouton Appliquer) — si un champ a
        // divergé du fichier. doApplyTags se garde contre le double-fire avec l'Entrée (applyingTags).
        // `commitTitle` reste appelé même sur Échap : le titre du hero doit afficher la valeur
        // RESTAURÉE, pas celle que l'annulation vient de jeter.
        commitTitle();
        if (escapeCancel) return; // Échap : annuler ne grave jamais (voir escapeCancel plus haut)
        if (tagFieldDiffs().any) void doApplyTags();
      });
    });

  const idBtn = host.querySelector<HTMLButtonElement>('[data-fil="identifier"]');
  const candsHost = host.querySelector<HTMLElement>(".sift-cands");
  if (idBtn && candsHost) {
    idBtn.addEventListener("click", () => void doIdentify(idBtn, candsHost, host, mid));
  }


  refreshRebuyLink(); // rebuy-on-Beatport link when the open track is fake AND already identified
}

/** Beatport search URL for the open track's identified artist + title. A search page (not an API):
 *  robust to spelling/pressing variants — the user picks the authentic release themselves. Null when
 *  there's nothing worth searching. */
function beatportSearchUrl(): string | null {
  const c = state.canonical;
  if (!c || !c.title.trim()) return null;
  const q = [c.artist, c.title].map((s) => (s ?? "").trim()).filter(Boolean).join(" ");
  return q ? `https://www.beatport.com/search?q=${encodeURIComponent(q)}` : null;
}

/** Show a "chercher sur Beatport" link ONLY when the open track is a fake/transcode AND a Discogs
 *  identity exists (state.identified) — searching a raw filename is useless. Fills a create-once
 *  `.sift-rebuy` container; empty (no link, no gap) otherwise. Called on open, on renderEditor, and
 *  after a fresh identify (onIdentityApplied). */
function refreshRebuyLink(): void {
  const el = document.querySelector<HTMLElement>(".sift-rebuy");
  if (!el) return; // editor not mounted
  const url = state.track?.verdict === "fake" && state.identified ? beatportSearchUrl() : null;
  if (!url) {
    el.innerHTML = "";
    return;
  }
  el.innerHTML =
    `<button class="sift-rebuy-btn" data-fil="rebuy" title="Ce fichier est un faux — chercher une version authentique sur Beatport">` +
    `<i class="ti ti-shopping-cart sift-icon-inline-md"></i> Chercher sur Beatport</button>`;
  el.querySelector('[data-fil="rebuy"]')?.addEventListener("click", () => {
    void openUrl(url).catch((e) => console.error("openUrl (rebuy) failed", e));
  });
}

// Apply button — une seule action « Appliquer » (grave les tags ID3 en place). Plus de bascule inline
// vers « Annuler » (Antoine 2026-08-21) : l'apply est journalisé (tag_edit, actions.rs), donc l'undo
// vit dans Ctrl+Z (undoLast) et l'écran Journal — le bouton inline faisait doublon.
/** Write the current edited tags onto the file in place (apply_tags). Déclenché AUTOMATIQUEMENT quand
 *  on finit d'éditer un champ (blur/Entrée) ou qu'on choisit un match Discogs — plus de bouton
 *  « Appliquer » (retour Antoine 2026-08-25 : les métadonnées se gravent quand on a fini de les
 *  éditer). Sur succès le fichier == l'affichage → re-snapshot pour effacer le marqueur. Gardé contre
 *  le double-fire (l'Entrée appelle blur() → deux déclenchements) par `applyingTags`.
 *  openState.openSeq-guarded : un open ultérieur ne repeint jamais l'état/UI de cette piste. */
let applyingTags = false;
async function doApplyTags(): Promise<void> {
  if (applyingTags || !state.track || !state.canonical) return;
  applyingTags = true;
  const trackId = state.track.id;
  const edited = state.canonical;
  const myseq = openState.openSeq;
  try {
    const batchId = await applyTags(trackId, edited);
    const snap = await trackFileTags(trackId); // file changed → refresh the in-memory snapshot
    if (myseq !== openState.openSeq) return; // another track opened meanwhile — leave its state/UI alone
    state.fileTags = snap;
    refreshDiscrepancy(); // file == display now → marker clears
    // Filet « Rétablir » en TOAST (décision F.2) : graver est un geste auto, on met l'undo ciblé à
    // portée immédiate. revertBatch(CE tag_edit) ; le Journal / Ctrl+Z restent le filet durable.
    toast("Tags gravés dans le fichier", true, () =>
      void revertBatch(batchId).catch((e) => console.error("revertBatch (tag_edit) failed", e)),
    );
  } catch (e) {
    console.error("apply_tags failed", e);
    toast("Échec de l'écriture des tags", false);
  } finally {
    applyingTags = false;
  }
}

