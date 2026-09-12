# Sift UI Design Governance Skill

> Contenu canonique côté repo. Ce fichier était la source suivie par Git de la
> skill locale `.agents/skills/sift-ui-design-governance/SKILL.md`.
> ⚠️ Ce miroir **n'existe plus** (vérifié le 2026-08-05 : `.agents/skills/` ne
> contient plus qu'`impeccable`), et le harnais Claude Code ne liste pas cette
> skill. Aucun agent ne charge donc ce fichier automatiquement : les règles qui
> doivent agir vivent dans `CLAUDE.md`, celui-ci porte leur rationale et leurs
> sources. Le lire à la demande sur toute tâche UI.

```yaml
name: sift-ui-design-governance
description: Enforce Sift-specific UI/UX, design-system, theme, layout, and user-flow governance. Use for any Sift task that designs, implements, audits, polishes, or reviews UI in the real Tauri app; changes frontend/styles.css, frontend/*.ts rendering, visual hierarchy, spacing, colors, dark/light theme, navigation, Revue flow, "A finaliser", Diagnostic audio, Metadonnees, or final verification after UI work.
```

## Purpose

Use this skill to keep Sift UI work aligned with the real product architecture:
user need -> journey -> UX -> UI -> performance -> code.

This skill is a process guardrail. It does not replace the design system docs.
It tells Codex which Sift sources to read, which decisions to preserve, and what
must be true before a UI task is considered done.

## First Reads

Before acting, read only the relevant parts:

1. `.claude/rules/context-packs.md` for task-scoped packs. (There is no skill
   inventory file any more - `~/.claude/skills-view.md` was deleted by the
   2026-07-31 vanilla reset. Rely on the skills the harness actually lists.)
2. `docs/design-system/governance.md` for Sift UI process and verification.
3. `docs/design-system/foundations.md` for product/user intent.
4. `docs/design-system/components.md` and `docs/design-system/patterns.md` for
   the touched surface.
5. `docs/design-system/tokens.md` plus `frontend/styles.css` for theme/tokens.
6. `docs/design-system-states.md` for existing component states.
7. The real rendering files, usually `frontend/report-view.ts`,
   `frontend/filing.ts`, `frontend/chrome.ts`, and `frontend/styles.css`.

Do not treat a mockup as source of truth - mockups are exploration only. The real
one is `frontend/app.js`, loaded unconditionally by `frontend/main.ts`, so it runs
in production. ⚠️ `docs/mockups/`, cited here until 2026-08-05, **does not exist**:
there is no `.html` anywhere under `docs/`.

## Routing

Preserve the Sift routing model:

- Retouch/polish existing UI: use `impeccable`, then `interface-design`.
- Post-implementation design audit: use `design-review` when available.
- New significant UI chantier: use `design-flow` when available.
- Quick pre-code exploration of a direction: `enhance-prompt` ->
  `stitch-generate-design` -> look at the result -> if satisfying,
  `stitch-extract-static-html` and hand-port to vanilla TS/existing tokens.
- Accessibility/perf quick reference: use `ui-ux-pro-max` only for the relevant
  quick-reference parts.
- Refactor/legacy UI code: use `working-with-legacy-code`,
  `refactoring-patterns`, and `clean-code` when available.
- Never invoke `design-taste-frontend` on Sift (marketing/landing scope, out
  of scope for this dense desktop product).

If a listed skill is unavailable, say so briefly and continue from the project
docs. Never invent a parallel process.

## Workflow

1. State the user decision the UI must support.
2. Map the current real implementation and DOM/CSS contracts before editing.
3. Preserve existing product contracts, especially:
   - the real app is the design surface;
   - `frontend/styles.css` is the token source of truth;
   - `#filfoot` and `#fldz` remain siblings of `.mid` in Revue;
   - Tauri-only UI must be verified in the real app, not a browser mockup.
4. Make the smallest production-code change that improves the user journey.
5. Remove decorative structure before adding new structure.
6. Verify and update documentation when behavior, components, tokens, or patterns
   changed.

## Lexical Granularity: Concept Before Numbers

Design talk moves across three levels of granularity:

- **L1 (vibe)** - impressionistic: "feels cramped", "not serious enough".
- **L2 (design domain)** - named surfaces and patterns, no numbers: "collapse
  Diagnostic", "action rail", "empty state", "section spacing".
- **L3 (operational)** - `padding: 24px`, `#2563EB`, a specific token or
  `frontend/styles.css` line.

Rule: **do not answer an L1 request with an L3 edit.** Restate an
impressionistic request at L2 first - name the surface, the user decision it
serves, and two candidate directions - then drop to L3 for the chosen one.
Specificity only helps once the concept is agreed; introduced earlier it narrows
the search space around the wrong idea, and every later fix inherits it. This is
not a blocking gate: when the app makes the direction obvious, pick one, say
which and why, and continue.

Corollary when a visual fix does not land: the failure mode is rarely "not
enough detail". Go back up a level. Two failed L3 edits on the same surface mean
the L2 agreement was never made - stop editing, restate the surface and the
decision, offer directions again. This is the language-side twin of the
`CLAUDE.md` rule to measure the real `tauri dev` window after two failed visual
fixes: measuring fixes the evidence, going back up fixes the target.

Going back up is not the same as same-level tuning: `padding: 32px` ->
`padding: 24px` is iteration and needs no restatement.

Source: Sato, D. (2026), "From Vibe to Code - and Back: Lexical Oscillation in
the Formation of Design Intent with Generative AI", arXiv:2607.23126v1 -
preprint, qualitative, N = 5. The L1/L2/L3 lens and "conceptual alignment
precedes operational alignment" are taken from it. The paper explicitly scopes
its findings out of programming practice, so it is borrowed here as vocabulary
for the design conversation, not as evidence about code.

## A Wireframe Must Contain What the Decision Is Judged Against

Measured 2026-08-14, issue #23. A nine-cell matrix compared three surface planes x three
groupings, drawn from the real `styles.css` tokens at the widths issue #9 had settled. It
was rejected - "il me faut de vrais mockups" - and it deserved to be: **it did not show
the nav rail**.

A surface's plane is not readable on its own. It reads *against the chrome* - the nav
rail, the queue column, the action bar. A thumbnail that omits them compares greys in a
vacuum, and stays structurally unable to settle the very thing it claims to settle, no
matter how faithful its tokens are.

The counter-measure, measured the same day: the four planes injected **live into the real
window** over CDP - no file touched, `git status` empty before and after - settled in one
minute what nine thumbnails had not. And the captures were stacked at the same position,
not laid side by side: a few points of lightness only read when you flip between two
images in place. Side-by-side lets the eye compensate.

Rule: **before drawing a wireframe, name what the decision is judged against, and check
that it is inside the frame.** When that something is the app itself, do not draw - inject
into the real window (skill `run-sift`).

## Design Theater: Claims Must Be Checkable

Generative UI tools narrate what they built. That narration reads as evidence of
deliberate work whether or not it matches the artifact. Measured across 120
generated interfaces: roughly 25% of stated design rationales were not
implemented, rising to 34% on functional requirements. Claude scored best on that
alignment (0.87) yet implemented only 6% of the *implicit* functional UX
principles - visibility of system status, user control and freedom, error
prevention and recovery - when a prompt implied them instead of naming them.

Regime caveat: that benchmark is one-shot whole-interface generation in vanilla
HTML/CSS/JS at default settings. Sift work is incremental editing of an existing
codebase with `CLAUDE.md` loaded, `npx tsc --noEmit`, fixed tokens, and CDP
verification against the real window. The numbers do not transfer. The failure
mode does.

Operating rule (see `CLAUDE.md` § Methode): every sentence in a report claiming
something is done must attach to citable evidence - `file:line`, command output,
screenshot. Borrow the benchmark's extraction test: a claim that cannot be scored
1.0 / 0.5 / 0.0 against the code is not a claim, it is decoration. Unverified work
is stated as unverified, never in the past tense.

Watch the invisible tier first. A wrong color shows up in a screenshot; a missing
state, an untabbable control, or a dead-end error path does not. That is exactly
where the measured gap concentrates.

Countermeasures Sift already has - do not rebuild them: `confirm-modal.ts` with
`BATCH_CONFIRM_THRESHOLD` and armed, timestamped confirmation (user control);
`journal.ts` with mass revert (error recovery); and the `CLAUDE.md` ban on UI
drawn from training memory, which is also what keeps generated interfaces from
converging on the same cross-tool defaults.

Source: Imteyaz, K., Imteyaz, K., Rajpal, N., Shaikh, K., Muller, M., Savage, S.
(2026), "Design Theater: Evaluating the Gap Between User-Facing Design Reasoning
and Implementation in Generative UI Tools", arXiv:2607.22928v2 - 24 tasks x 5
tools = 120 interfaces, two independent raters.

## Sift UI Rules

- Favor continuous surfaces over card stacks.
- Use panels only for real structural or floating surfaces.
- Keep Diagnostic audio above Metadonnees in the Revue decision flow unless a
  fresh product reason says otherwise.
- Keep Destination visible in "A finaliser"; it answers where the track goes.
- Show Nom final after Format, because Format changes the extension.
- Do not duplicate warnings across sections.
- Do not color neutral categories like "Signal" and "Conteneur" as warnings.
- Do not use permanent saturated success color for an already-confirmed state.
- Do not use `window.confirm()`, `alert()`, or `prompt()` for destructive or
  costly actions.
- Do not add side-stripe accent borders.
- Animate `transform` or `opacity`, not layout properties.

## Done Checklist

Before final response on a UI/design task:

1. The implementation matches the real app architecture, not only a mockup.
2. The user flow is explicit: what decision became easier?
3. Tokens come from `frontend/styles.css`; no parallel theme was created.
4. Existing component states were preserved or intentionally updated.
5. Layout spacing is coherent by category: screen, section, group, row, value.
6. Color communicates state, not decoration.
7. Keyboard/action rail contracts still make sense.
8. `npx tsc --noEmit` was run if TypeScript changed, or the skipped verification
   is stated.
9. `docs/design-system-*` or `docs/design-system/` were updated if the design
   system changed. (`docs/INDEX.json` is gone as of 2026-08-05 - there is no
   catalogue to update; a new doc needs its negation in `.gitignore` instead.)
10. An impressionistic (L1) request was restated at L2 - surface, decision,
    directions - before any token or px value moved.
11. Every "done" claim in the final report carries citable evidence; anything
    unverified is labelled unverified, not written in the past tense.

