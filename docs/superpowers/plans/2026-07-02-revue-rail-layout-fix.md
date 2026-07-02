# Revue rail layout + queue row + Identification card — fidelity fix

> **Statut : terminé (2026-07-02).** 5/5 tâches livrées via
> `superpowers:subagent-driven-development` (un subagent implémenteur par tâche,
> vérification `tsc --noEmit` + `npm run build` indépendante par le contrôleur après
> chaque livraison — pas de reviewer séparé, jugé disproportionné pour du CSS/markup
> sans suite de tests automatisés). Commits : `f5bd9ff`, `12b60cc`, `5662bf2`,
> `50052b0`, `d9af04d`. Déclenché par un vrai screenshot de l'app tournante montrant
> le rail d'action empilé verticalement au lieu d'une ligne horizontale — l'audit
> précédent (comparaison texte-code contre la maquette) n'avait pas capté ce défaut
> car il n'avait jamais examiné le rendu réel ni le corps complet de `renderEditor`/
> `renderFoot` dans `filing.ts`. Leçon retenue : la comparaison de code contre une
> maquette ne remplace pas un œil sur le rendu réel.

> **For agentic workers:** Fresh subagent per task, self-contained brief below each task
> header. Verification = `npx tsc --noEmit` + `npm run build` (this codebase has no
> automated UI test suite — that pair is the project's real test cycle, confirmed by
> `docs/refonte-ui-plan.md`'s established convention this session). No `src-tauri/`
> files are touched by this plan — no Rust rebuild needed.

**Goal:** Fix a real-app screenshot regression: the Revue-Détail (and Mode Lot) bottom
action rail renders as a vertical stack of full-width buttons instead of the maquette's
single horizontal row, the queue row template is one line instead of title+artist+verdict,
and the Identification card is a permanently-editable form instead of the maquette's
read-only Label/Année/Genre grid + "Modifier" popover.

**Architecture:** Pure CSS + markup restructuring of existing, already-wired functions
(`renderFoot`, `renderBatchRail`, `renderQueue`'s row template, `renderEditor`) — no new
IPC calls, no new state shape beyond what's noted per task. Behavior (click handlers,
what each button does) is preserved; only the DOM layout changes.

**Tech Stack:** Vanilla TypeScript, hand-written CSS (`frontend/styles.css`), no build
tooling beyond the existing Vite pipeline.

## Global Constraints

- 2 couleurs sémantiques seulement (vert `--color-text-success` / ambre
  `--color-text-warning`) — jamais une 3e teinte, jamais de bleu/rouge en dur.
- Sliders custom déjà conformes (report-view.ts) — ne pas y toucher.
- Jargon technique gardé en anglais (LOSSLESS, DUPLICATE, MATCH/CHECK MATCH, FAKE, kbps,
  kHz, MP3/AIFF/WAV) ; tout le reste en français.
- Ne jamais faire filer un FAKE (règle métier confirmée le 2026-07-01, `batchSel`/
  `batchFakeSel` restent deux sets séparés dans `sift-live.ts` — hors scope de ce plan,
  ne pas y toucher).
- L'édition manuelle artist/title/version de la carte Identification doit être
  **conservée** (décision utilisateur du 2026-07-02) — seul l'HABILLAGE change vers le
  look maquette (grille lecture-seule par défaut), pas la capacité fonctionnelle.
- Tokens CSS existants uniquement (`--space-*`, `--text-*`, `--color-*`,
  `--border-radius-*`) — ne pas inventer de nouvelles valeurs littérales.

---

### Task 1: Action rail — horizontal layout (CSS)

**Files:**
- Modify: `frontend/styles.css:145-154`

**Interfaces:**
- Consumes: nothing (pure CSS).
- Produces: `.sift-action-rail` becomes a horizontal flex row — Tasks 2 and 3 build
  their markup assuming this row layout is already in place (this task must land
  first).

**Current code** (`styles.css:143-154`):
```css
/* Action rail (was the persistent .dest column — now a slim bar anchored to the bottom of the
   inspector, matching the maquette: Destination popover trigger + format + File/Discard). */
.sift-action-rail{flex:none;display:flex;flex-direction:column;padding:11px 15px;background:var(--color-background-tertiary);border-top:0.5px solid var(--color-border-tertiary);overflow:visible}
.sift-dest-btn{display:flex;align-items:center;gap:8px;width:100%;margin-bottom:10px;padding:8px 10px;background:var(--color-background-secondary);border:0.5px solid var(--color-border-secondary);border-radius:var(--border-radius-md);color:var(--color-text-primary);font-size:var(--text-sm);cursor:pointer}
.sift-dest-btn-label{color:var(--color-text-tertiary);flex:none}
.sift-dest-btn .sift-fil-bin{flex:1;min-width:0;text-align:left;overflow:hidden;text-overflow:ellipsis;font-weight:500}
.sift-dest-btn-caret{flex:none;font-size:var(--text-sm);color:var(--color-text-tertiary)}
```

- [ ] **Step 1: Replace the rail + Destination-button rules with a horizontal row**

Root cause confirmed by reading the rendered app (not guessed): `.sift-action-rail{flex-direction:column}`
is why Destination/Format/Nom final/Ranger/Jeter each render as a separate full-width
row instead of the maquette's single line (`Sift.dc.html`'s ACTION RAIL block:
`display:flex;align-items:center;gap:14px`).

Replace the block above with:

```css
/* Action rail (was the persistent .dest column — now a slim horizontal bar anchored to
   the bottom of the inspector, matching the maquette's single-row ACTION RAIL: Destination
   popover trigger, format chips, a flex spacer, key hints, secondary, primary — all in one
   line, wrapping only if the window gets too narrow. */
.sift-action-rail{flex:none;display:flex;flex-wrap:wrap;align-items:center;gap:var(--space-12);padding:11px 15px;background:var(--color-background-tertiary);border-top:0.5px solid var(--color-border-tertiary);overflow:visible}
.sift-dest-btn{display:flex;align-items:center;gap:8px;flex:none;width:auto;max-width:220px;padding:8px 10px;background:var(--color-background-secondary);border:0.5px solid var(--color-border-secondary);border-radius:var(--border-radius-md);color:var(--color-text-primary);font-size:var(--text-sm);cursor:pointer}
.sift-dest-btn-label{color:var(--color-text-tertiary);flex:none}
.sift-dest-btn .sift-fil-bin{flex:1;min-width:0;text-align:left;overflow:hidden;text-overflow:ellipsis;font-weight:500;max-width:120px}
.sift-dest-btn-caret{flex:none;font-size:var(--text-sm);color:var(--color-text-tertiary)}
/* Format group: label + chips inline together, no longer a stacked "col-h" block. */
.sift-rail-fmt-group{display:flex;align-items:center;gap:6px;flex:none}
.sift-rail-fmt-group .col-h{margin:0}
/* Pushes secondary/primary to the right edge of the rail. */
.sift-rail-spacer{flex:1;min-width:8px}
.sift-ranger-btn{flex:none;white-space:nowrap}
.sift-secondary-resource,.sift-secondary-trash{flex:none;white-space:nowrap}
```

- [ ] **Step 2: Verify**

Run: `npx tsc --noEmit` (must stay clean — this is CSS-only, tsc should be unaffected;
run it anyway as the project-standard smoke check before every commit) and
`npm run build`.
Expected: both exit 0, no new warnings beyond the pre-existing
`INEFFECTIVE_DYNAMIC_IMPORT` note (unrelated, do not try to fix it).

- [ ] **Step 3: Commit**

```bash
git add frontend/styles.css
git commit -m "fix(styles): action rail — horizontal row layout matching maquette"
```

---

### Task 2: Detail rail markup + Nom final relocation to the verdict card

**Files:**
- Modify: `frontend/filing.ts:728-795` (`renderFoot`)
- Modify: `frontend/filing.ts:420-426` (`refreshPreview`)
- Modify: `frontend/report-view.ts` (`verdictCardHtml`, currently ~line 206-240 —
  re-check the exact line with `grep -n "export function verdictCardHtml" frontend/report-view.ts`
  before editing, this session already moved things around once)

**Interfaces:**
- Consumes: Task 1's `.sift-action-rail` row layout, `.sift-rail-fmt-group`,
  `.sift-rail-spacer` classes (already in `styles.css` after Task 1 — do not redefine
  them here).
- Produces: `verdictCardHtml(r: AnalysisReport)` keeps its exact exported signature
  (no new parameter) but its returned HTML now includes an element with class
  `sift-verdict-finalname` — `refreshPreview()` must keep this in sync alongside the
  existing `.sift-fil-prev` write, since later tasks and existing call sites
  (`openFilingInto`, format-chip clicks) rely on `refreshPreview()` being the single
  place the final name string is pushed to the DOM.

**Current code** (`filing.ts:759-769`, the `foot.innerHTML` assignment inside `renderFoot`):
```ts
  foot.innerHTML =
    `<button data-fil="destbtn" class="sift-dest-btn"><span class="sift-dest-btn-label">Destination</span><span class="sift-fil-bin">${esc(binLabel())}</span><i class="ti ti-chevron-down sift-dest-btn-caret"></i></button>` +
    `<div class="col-h sift-col-h-tight">Format</div>` +
    `<div class="sift-fmt-chips">${chips}</div>` +
    `<div class="col-h sift-col-h-tight">Nom final</div>` +
    `<div class="sift-fil-prev">→ ${esc(previewName())}</div>` +
    `<button data-fil="ranger" class="sift-ranger-btn"><i class="ti ti-corner-down-left sift-icon-inline-md"></i> Ranger → <span class="sift-fil-bin">${esc(binLabel())}</span> <span class="kbd">⏎</span></button>` +
    secondary +
    // Keyboard hints anchored to the bottom rail (maquette's keyHints), not the scrollable
    // detail content — moved here from report-view.ts, which used to inject them under the hero.
    keyboardHintsHtml();
  if (filedBanner) foot.append(filedBanner); // restore the banner below the freshly-rendered controls
```

- [ ] **Step 1: Rebuild the rail as one row, drop "Nom final" from here**

Replace that block with:

```ts
  foot.innerHTML =
    `<button data-fil="destbtn" class="sift-dest-btn"><span class="sift-dest-btn-label">Destination</span><span class="sift-fil-bin">${esc(binLabel())}</span><i class="ti ti-chevron-down sift-dest-btn-caret"></i></button>` +
    `<div class="sift-rail-fmt-group"><span class="col-h">Format</span><div class="sift-fmt-chips">${chips}</div></div>` +
    `<div class="sift-rail-spacer"></div>` +
    // Keyboard hints anchored to the bottom rail (maquette's keyHints), not the scrollable
    // detail content — moved here from report-view.ts, which used to inject them under the hero.
    keyboardHintsHtml() +
    secondary +
    `<button data-fil="ranger" class="sift-ranger-btn"><i class="ti ti-corner-down-left sift-icon-inline-md"></i> Ranger → <span class="sift-fil-bin">${esc(binLabel())}</span> <span class="kbd">⏎</span></button>`;
  if (filedBanner) foot.append(filedBanner); // restore the banner below the freshly-rendered controls
```

Note the reordering: `secondary` now comes before the primary Ranger button (maquette:
secondary sits left of primary, both right-aligned after the spacer) — this matches
`Sift.dc.html`'s ACTION RAIL block (`onSecondary` div, then `onPrimary` div, in that
source order).

- [ ] **Step 2: Add the final-name slot to the verdict card**

Read the current `verdictCardHtml` function in `frontend/report-view.ts` first
(`grep -n "export function verdictCardHtml" -A 20 frontend/report-view.ts`) — it was
last touched on 2026-07-02 (verdict-reorder task) and returns something shaped like:

```ts
export function verdictCardHtml(r: AnalysisReport): string {
  const map = { /* ok/fake/grey → icon, label, fg, panelBg */ } as const;
  const [icon, label, fg, panelBg] = map[r.verdict];
  const rq = realQuality(r);
  const qualityChip = /* ... */;
  return (
    `<div class="sift-verdict-card" style="background:${panelBg}">` +
    `<div class="sift-verdict-head"><i class="ti ${icon}" style="color:${fg}"></i><span class="sift-verdict-label" style="color:${fg}">${label}</span></div>` +
    `<div class="sift-vchips sift-vchips-row">${qualityChip}</div>` +
    `</div>`
  );
}
```

Change the returned markup to add a final-name column on the right, matching the
maquette's CONCLUSION band (`Sift.dc.html`: label "Nom final" uppercase micro-label +
the name in the verdict's own color, right-aligned, `max-width:46%`). The name itself
starts empty — `refreshPreview()` (Step 3 below) fills it once `filing.ts` has a
canonical name to show, exactly like it already does for `.sift-fil-prev` today:

```ts
export function verdictCardHtml(r: AnalysisReport): string {
  const map = { /* unchanged */ } as const;
  const [icon, label, fg, panelBg] = map[r.verdict];
  const rq = realQuality(r);
  const qualityChip = /* unchanged */;
  return (
    `<div class="sift-verdict-card" style="background:${panelBg}">` +
    `<div class="sift-verdict-head"><i class="ti ${icon}" style="color:${fg}"></i><span class="sift-verdict-label" style="color:${fg}">${label}</span></div>` +
    `<div class="sift-vchips sift-vchips-row">${qualityChip}</div>` +
    `<div class="sift-verdict-finalname-col">` +
    `<div class="sift-verdict-finalname-label">Nom final</div>` +
    `<div class="sift-verdict-finalname" style="color:${fg}"></div>` +
    `</div>` +
    `</div>`
  );
}
```

Add matching CSS to `frontend/styles.css` (append near the other `.sift-verdict-*`
rules — `grep -n "sift-verdict-card\|sift-verdict-head" frontend/styles.css` to find
them):

```css
.sift-verdict-card{display:flex;align-items:center;gap:16px}
.sift-verdict-finalname-col{flex:none;max-width:46%;text-align:right;margin-left:auto}
.sift-verdict-finalname-label{font-size:9.5px;letter-spacing:.08em;text-transform:uppercase;color:var(--color-text-tertiary);margin-bottom:3px}
.sift-verdict-finalname{font-family:var(--font-mono);font-size:11.5px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
```

(If `.sift-verdict-card` already has `display:flex;flex-direction:column` or similar
from the existing CSS, adjust rather than duplicate — read the existing rule first;
the goal is: head+chips stack on the left, final-name column pinned right, same row.)

- [ ] **Step 3: `refreshPreview` writes both slots**

Current code (`filing.ts:423-426`):
```ts
function refreshPreview(): void {
  const prev = document.querySelector<HTMLElement>(".sift-fil-prev");
  if (prev) prev.textContent = `→ ${previewName()}`;
}
```

Replace with:
```ts
function refreshPreview(): void {
  const name = previewName();
  const prev = document.querySelector<HTMLElement>(".sift-fil-prev");
  if (prev) prev.textContent = `→ ${name}`;
  const verdictName = document.querySelector<HTMLElement>(".sift-verdict-finalname");
  if (verdictName) verdictName.textContent = `→ ${name}`;
}
```

`.sift-fil-prev` no longer exists in the rail after Step 1 — the `prev` lookup above
will simply find nothing there and no-op, which is fine and requires no special-casing
(`querySelector` returning `null` is the normal "not mounted" case already handled
everywhere else in this file, e.g. `refreshReleaseLine`).

- [ ] **Step 4: Verify**

Run: `npx tsc --noEmit` — must be clean (this step touches 2 files with a shared
contract: `verdictCardHtml`'s new DOM structure and `refreshPreview`'s new querySelector
must agree on the class name `sift-verdict-finalname` — a typo here won't be caught by
tsc since it's a string literal, so also grep-verify: `grep -n "sift-verdict-finalname"
frontend/*.ts` must show exactly 3 hits — 1 in report-view.ts (the div), 2 in filing.ts
(the two querySelectors in Step 3, one of which is the label class check — read
carefully, `sift-verdict-finalname-label` and `sift-verdict-finalname-col` are
DIFFERENT classes from the one `refreshPreview` targets, don't conflate them).
Then `npm run build`.

- [ ] **Step 5: Commit**

```bash
git add frontend/filing.ts frontend/report-view.ts frontend/styles.css
git commit -m "fix(filing): detail rail — one row, Nom final moves into the verdict conclusion"
```

---

### Task 3: Mode Lot rail — same horizontal treatment

**Files:**
- Modify: `frontend/sift-live.ts` (`renderBatchRail`, `grep -n "function renderBatchRail" frontend/sift-live.ts` to find the current line — it moved during the 2026-07-01 Mode Lot rework)

**Interfaces:**
- Consumes: Task 1's `.sift-action-rail` row CSS (same element, `#filfoot`, shared
  between Detail and Batch — confirmed by reading both `renderFoot` and
  `renderBatchRail`, both target `document.getElementById("filfoot")`/`requireEl("#filfoot", ...)`).
- Produces: nothing new consumed by later tasks.

**Current shape** (read via `grep -n "foot.innerHTML =" -A 12 frontend/sift-live.ts`
inside `renderBatchRail` before starting — it was last rewritten 2026-07-01 for the
Mode Lot maquette-fidelity pass and stacks: Destination button → Selection block →
Format block (segmented, added 2026-07-01) → Final name → progress slot → tracks slot →
action button, each a separate full-width `<div>`).

- [ ] **Step 1: Read the current `renderBatchRail` body in full before editing**

This function was substantially rewritten earlier in this same session (Mode Lot
maquette-fidelity task) and its exact current shape must be re-read, not assumed —
`Read` the function fully first.

- [ ] **Step 2: Restructure into one row**

Keep every existing behavior (Destination popover toggle, format segmented control,
Selection count, action button adaptive Filer/Écarter label, progress zone slot,
per-track list) — only change the container markup from stacked full-width `<div>`s
to the same row pattern as Task 2: Destination button, then a compact "Selection · N à
filer" label (small, not a full block), then the format segmented control, then a
`<div class="sift-rail-spacer"></div>`, then the action button. Move "Final name" out of
the rail into a lightweight strip ABOVE the rail (not into a verdict card — Mode Lot has
no single-track verdict card) — e.g. a `<div class="sift-batch-final-name">` sitting
just above `.sift-action-rail` in the DOM (a sibling `<div>` inside `#mid`'s parent,
NOT inside `#filfoot`, since `#filfoot` is now a single flex row with no room for a
second text line). The progress zone slot (`#sift-batch-progress`) and per-track list
(`#sift-batch-tracks`) stay exactly where they are today (both render conditionally,
only visible during/after a run — they don't need to fit in the idle one-row layout,
CSS `flex-wrap:wrap` from Task 1 lets them drop to their own line when present).

Because this task depends on reading code not reproduced in this brief, the
implementer subagent should report the exact before/after `foot.innerHTML` shape in
its DONE report so the controller can review the specific diff (this is the one task
in this plan without complete verbatim target code — flag this in the report as
expected, not a gap).

- [ ] **Step 3: Verify**

Run: `npx tsc --noEmit` and `npm run build`. Manually re-read the diff against
`design_handoff_sift_refonte/Sift.dc.html`'s ACTION RAIL block (`grep -n "ACTION RAIL"
-A 60 design_handoff_sift_refonte/Sift.dc.html`) to confirm the row order matches:
Destination → Format (batch has no keyHints slot when a run is active, per the existing
`taskEntries` conditional already in the maquette) → spacer → secondary (Écarter) →
primary (Filer/Déplacer la sélection).

- [ ] **Step 4: Commit**

```bash
git add frontend/sift-live.ts
git commit -m "fix(batch): rail — one row, matching the Detail rail restructure"
```

---

### Task 4: Queue row — two-line title/artist + verdict word

**Files:**
- Modify: `frontend/sift-live.ts:152-167` (`renderQueue`'s `ql.innerHTML` row template
  — re-confirm the line range with `grep -n "ql.innerHTML" -A 15 frontend/sift-live.ts`
  before editing, line numbers have shifted this session)

**Interfaces:**
- Consumes: `QueueItem` shape from `shared/contracts.ts` (fields used: `id`, `path`,
  `filename`, `artist`, `title`, `verdict`, `dup` — all already read by the existing
  row template, no new fields needed).
- Produces: nothing consumed elsewhere.

**Current code** (verify exact line range first, shape is):
```ts
  ql.innerHTML =
    (items
      .map(
        (it) =>
          `<div class="qi" data-id="${it.id}" data-path="${esc(it.path)}" title="Listen and file" style="display:flex;align-items:center;gap:8px;cursor:pointer">${verdictDot(
            it.verdict,
          )}<span style="flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis">${esc(
            it.filename || it.path,
          )}</span>${
            it.dup
              ? '<i class="ti ti-copy" title="Possible duplicate (same name)" style="flex:none;font-size:var(--text-md);color:var(--color-text-secondary)"></i>'
              : ""
          }<i class="ti ti-chevron-right" style="flex:none;color:var(--color-text-tertiary);font-size:var(--text-lg)"></i></div>`,
      )
      .join("") ||
      '<div style="font-size:var(--text-md);color:var(--color-text-tertiary);padding:6px 4px">Queue empty.</div>');
```

- [ ] **Step 1: Replace the row template**

Maquette reference (`design_handoff_sift_refonte/Sift.dc.html`, the `showTrackRows`
block): dot + title (bold) + duplicate glyph `⧉` on the first line, artist in grey on
an indented second line, a right-aligned colored verdict WORD (not just a dot) —
`lossless`/`fake`/`analyse…` in the row's own semantic color.

```ts
  const verdictWord = (v: string | null): [string, string] =>
    v === "fake"
      ? ["fake", "var(--color-text-warning)"]
      : v === "grey"
        ? ["à vérifier", "var(--color-text-warning)"]
        : v === "ok"
          ? ["lossless", "var(--color-text-success)"]
          : ["analyse…", "var(--color-text-tertiary)"];

  ql.innerHTML =
    (items
      .map((it) => {
        const [word, wordColor] = verdictWord(it.verdict);
        const title = esc(it.filename || it.path);
        const artist = it.artist ? esc(it.artist) : "";
        return (
          `<div class="qi" data-id="${it.id}" data-path="${esc(it.path)}" title="Listen and file" style="display:flex;align-items:center;gap:8px;cursor:pointer;padding:5px 7px">` +
          `<div style="flex:1;min-width:0;display:flex;flex-direction:column;gap:2px">` +
          `<div style="display:flex;align-items:center;gap:6px;min-width:0">` +
          verdictDot(it.verdict) +
          `<span style="flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;font-weight:500">${title}</span>` +
          (it.dup
            ? '<span title="Possible duplicate (same name)" style="flex:none;font-size:var(--text-sm);color:var(--color-text-secondary)">⧉</span>'
            : "") +
          `</div>` +
          (artist
            ? `<div style="padding-left:15px;font-size:var(--text-xs);color:var(--color-text-tertiary);overflow:hidden;text-overflow:ellipsis;white-space:nowrap">${artist}</div>`
            : "") +
          `</div>` +
          `<span style="flex:none;font-size:var(--text-xs);color:${wordColor}">${word}</span>` +
          `</div>`
        );
      })
      .join("") ||
      '<div style="font-size:var(--text-md);color:var(--color-text-tertiary);padding:6px 4px">File vide.</div>');
```

Note: `verdictDot` is an existing function in the same file (`sift-live.ts`) already
imported/defined in scope — reuse it as-is, do not redefine. The empty-queue string
also gets translated to French here (`"Queue empty."` → `"File vide."`) since it was
missed in the earlier French-translation passes this session (confirmed leftover
English, not a new decision).

- [ ] **Step 2: Verify**

Run: `npx tsc --noEmit` and `npm run build`. Grep-check no other code references the
now-removed `ti-chevron-right` on `.qi` rows expecting it to exist (`grep -n "chevron-right"
frontend/*.ts` — if a click handler or CSS rule specifically targeted that icon inside
`.qi`, it needs removing too; if it's a generic chevron icon used elsewhere as well,
leave those other usages alone).

- [ ] **Step 3: Commit**

```bash
git add frontend/sift-live.ts
git commit -m "fix(queue): row — two-line title/artist + verdict word, matching maquette"
```

---

### Task 5: Identification card — read-only grid + click-to-edit, keep manual editing

**Files:**
- Modify: `frontend/filing.ts:855-936` (`renderEditor`)
- Modify: `frontend/styles.css` (new `.sift-ident-*` rules, additive)

**Interfaces:**
- Consumes: `state.canonical` (`{artist, title, version, confidence}`), `state.label`,
  `state.year`, `state.genres` — all already read by the current `renderEditor`, same
  shape, no changes to `state`'s type.
- Produces: nothing new consumed by other tasks — this is the last task, independent
  of 1-4 (touches `filing.ts` but a disjoint function from Task 2's `renderFoot`; still
  run it AFTER Task 2 since both touch `filing.ts` and sequential subagents avoid
  merge friction).

**Explicit product decision (2026-07-02, read this before writing any code):**
Keep the manual artist/title/version editing capability — it is load-bearing for
tracks Discogs can't match or gets wrong. Do NOT remove the `<input>` fields or the
`upd()` live-sync logic. Change ONLY the default visual presentation: show a read-only
Label/Année/Genre-style grid by default (matching the maquette's Identification card),
and reveal the existing editable inputs on demand (clicking the title/artist text, or a
small edit affordance) rather than showing three always-visible `<input>` boxes
stacked full-width as today.

- [ ] **Step 1: Read the current `renderEditor` in full**

Already reproduced in this session's investigation — `filing.ts:855-936`. Re-`Read` it
fresh before editing (this plan file is not a substitute for the actual current
source).

- [ ] **Step 2: Add a display/edit toggle state**

Add a module-level `let identEditing = false;` near the other module-level `state`
declarations in `filing.ts` (`grep -n "^let \|^const state" frontend/filing.ts` to find
the right neighborhood). Reset it to `false` at the top of `openFilingInto` (so opening
a different track always starts in read-only display, never mid-edit from a previous
track) — find `state.filedConfirm = null;` in `openFilingInto` (around line 1275 per
this session's earlier reads) and add `identEditing = false;` on the next line.

- [ ] **Step 3: Rewrite `renderEditor`'s returned HTML**

Replace the `host.innerHTML = ...` block (currently lines ~878-908) with a version
that branches on `identEditing`:

- **Display mode** (`identEditing === false`, the default): a card matching the
  maquette's Identification card — title/artist shown as read-only text (not inputs),
  a small "Modifier" affordance (pencil icon button, `title="Modifier manuellement"`)
  that sets `identEditing = true` and re-renders, the existing Label/Année line
  (`.sift-release`, unchanged, `refreshReleaseLine()` still owns it), Genres
  (`.sift-genres`, unchanged), Compatibilité CDJ / Version ID3 rows (unchanged), and
  the existing "Récupérer les métadonnées Discogs" / "Appliquer les tags ID3" buttons
  unchanged in behavior.
- **Edit mode** (`identEditing === true`): today's exact `<input>` fields (artist,
  title, version) plus a small "Terminé" button that sets `identEditing = false` and
  re-renders (calling `renderEditor` again, which the click handler already has access
  to via closure — check how other toggle-style buttons in this file re-invoke their
  own render function, e.g. `setReviewMode` in `sift-live.ts` is NOT in this file, so
  instead follow this file's own pattern: `grep -n "renderEditor(" frontend/filing.ts`
  to find all call sites and confirm `host`/`mid`/`rail`/`report` are all in scope at
  the click-handler closure so a direct `renderEditor(host, mid, rail, report)` re-call
  works without threading new parameters through).

Concretely, the new `host.innerHTML` (display mode branch):

```ts
  const displayName = c.artist && c.title ? `${c.artist} — ${c.title}${c.version ? ` (${c.version})` : ""}` : "Non identifié";
  host.innerHTML =
    `<div class="sift-ident-head">` +
    `<span class="col-h sift-editor-title">Identification · Discogs</span>` +
    `<button data-fil="ident-edit" class="sift-ident-edit-btn" title="Modifier manuellement"><i class="ti ti-pencil"></i></button>` +
    `</div>` +
    (identEditing
      ? `<div class="sift-editor-badge-row">${badge}</div>` +
        `<button data-fil="identifier" class="sift-id-btn sift-id-btn-full" title="Rechercher les métadonnées sur Discogs (pochette, label, année, genres)"><i class="ti ti-search sift-icon-inline-sm"></i> Récupérer les métadonnées Discogs <span class="kbd sift-kbd-hint-id">I</span></button>` +
        `<div class="sift-cands sift-cands-host" hidden></div>` +
        `<div class="sift-editor-fields">` +
        `<input data-fil="artist" placeholder="Artist" value="${esc(c.artist)}" class="${inputCss}">` +
        `<input data-fil="title" placeholder="Title" value="${esc(c.title)}" class="${inputCss}">` +
        `<input data-fil="version" placeholder="Version" value="${esc(c.version ?? "")}" class="${inputCss}">` +
        `</div>` +
        `<button data-fil="ident-done" class="sift-ident-done-btn">Terminé</button>`
      : `<div class="sift-ident-display">${esc(displayName)}</div>`) +
    `<div class="sift-release"></div>` +
    `<div class="col-h sift-col-h-tight">Genres</div>` +
    `<div class="sift-genres sift-genres-box"></div>` +
    (report
      ? `<div class="sift-spectro-rows">` +
        row("Compatibilité CDJ", yn(report.tags_cdj_ok)) +
        row("Version ID3", report.id3_version || "—") +
        `</div>`
      : "") +
    `<button data-fil="applytags" class="sift-applytags-btn" title="Écrire ces tags dans le fichier en place — pas de déplacement, pas d'encodage, réversible"><i class="ti ti-tag sift-icon-inline-md"></i> Appliquer les tags ID3</button>` +
    `<div class="sift-tag-warn" style="display:none"><i class="ti ti-alert-triangle sift-icon-inline-md sift-icon-flex-none"></i><span>Tags non écrits dans le fichier — <strong>Ranger</strong> ou <strong>Appliquer</strong> pour les graver</span></div>`;
```

Keep every existing `querySelector`/`addEventListener` wiring below this block exactly
as today (the `upd`, `idBtn`, `applyBtn`, `refreshReleaseLine()` calls) — they already
guard with `host.querySelector(...)` returning `null` when the input fields aren't
mounted (display mode), so no new null-guards are needed beyond what's already there.
Add two new listeners:

```ts
  host.querySelector<HTMLButtonElement>('[data-fil="ident-edit"]')?.addEventListener("click", () => {
    identEditing = true;
    renderEditor(host, mid, rail, report);
  });
  host.querySelector<HTMLButtonElement>('[data-fil="ident-done"]')?.addEventListener("click", () => {
    identEditing = false;
    renderEditor(host, mid, rail, report);
  });
```

- [ ] **Step 4: Add the display-mode CSS**

Append to `frontend/styles.css` (near the other `.sift-editor-*`/`.sift-ident-*` rules —
`grep -n "sift-editor-title\|sift-editor-badge-row" frontend/styles.css`):

```css
.sift-ident-head{display:flex;align-items:center;justify-content:space-between;gap:10px;margin-bottom:8px}
.sift-ident-edit-btn{flex:none;display:inline-flex;align-items:center;justify-content:center;width:26px;height:26px;padding:0;border:0.5px solid var(--color-border-secondary);border-radius:var(--border-radius-md);background:var(--color-background-secondary);color:var(--color-text-secondary);cursor:pointer}
.sift-ident-edit-btn:hover{color:var(--color-text-primary)}
.sift-ident-display{font-size:var(--text-lg);font-weight:500;margin-bottom:10px}
.sift-ident-done-btn{align-self:flex-start;margin-bottom:10px;padding:5px 12px;border:0.5px solid var(--color-border-secondary);border-radius:var(--border-radius-md);background:var(--color-background-secondary);color:var(--color-text-primary);font-size:var(--text-sm);cursor:pointer}
```

- [ ] **Step 5: Verify**

Run: `npx tsc --noEmit` — `identEditing` must be a module-level mutable `let`, not
re-declared per call (a `const` or a re-declaration inside `renderEditor` would silently
reset it every render, defeating the toggle — tsc won't catch this logic bug, manually
re-read the diff to confirm `identEditing` is declared exactly ONCE, outside any
function). Then `npm run build`. Manually exercise the toggle path by reading the two
new listeners against Step 2's reset-on-open — confirm opening a new track always
starts in display mode even if the previous track was left in edit mode (this is the
one behavior a subagent should explicitly reason through in its self-review, not just
compile-check).

- [ ] **Step 6: Commit**

```bash
git add frontend/filing.ts frontend/styles.css
git commit -m "fix(filing): Identification card — read-only display by default, edit on demand"
```

---

## Self-Review Notes (controller, not a task)

- Task ordering matters: 1 before 2 and 3 (CSS dependency); 2 before 5 (both touch
  `filing.ts`, avoid parallel edits to the same file); 4 is independent, can run
  anywhere after 1 but is placed after 3 here only to keep `sift-live.ts` edits
  adjacent in the commit history.
- Task 3 is intentionally less prescriptive than 2/4/5 (no complete verbatim target
  code) because `renderBatchRail`'s exact current shape depends on this session's
  earlier Mode Lot rework and must be re-read fresh, not assumed from a stale
  transcript — the plan says so explicitly rather than risk stale code in the brief.
