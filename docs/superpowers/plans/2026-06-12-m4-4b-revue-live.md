# M4-4b — Live Revue filing UI — Implementation Plan

> **For agentic workers:** executes the frontend half of the approved M4 spec
> (`docs/superpowers/specs/2026-06-12-m4-filing-loop-design.md`). M4-4a (contracts +
> IPC bindings) is already merged.

**Goal:** Make the Revue tab file real tracks — live destination bins, editable canonical
metadata, format override, and Ranger / Re-sourcer / Écarter actions wired to the backend,
with an undo affordance.

**Architecture:** Follow the *actual* codebase pattern, not the spec's proposed file
layout. The mockup (`frontend/app.js`) renders the Revue shell (`#ql` queue, `#mid` detail
pane, `#fldz` destination column). `frontend/sift-live.ts` augments it under Tauri only
(plain-browser demo untouched). M4-4b adds `frontend/filing.ts` — the live Revue controller —
which (a) renders the destination bins into `#fldz` from `list_bins`, with a NoLibraryRoot
picker gate, and (b) renders the analysis report (via `report-view.renderReportInto`) plus a
filing footer (editable fields + format + actions) into `#mid`. `sift-live.ts` routes queue
clicks to `filing.ts` instead of `report-view` directly, threading the real `track_id`.

**Tech Stack:** TypeScript, Tauri invoke (M4-4a bindings), wavesurfer (existing player),
tabler icons + the project's CSS vars. No new deps. Verify with `npx tsc --noEmit`.

---

## File structure

- **Modify** `frontend/report-view.ts` — export `renderReportInto` (already exported) and a
  small `analyze`+render helper reused by `filing.ts`; keep its debug-modal role.
- **Create** `frontend/filing.ts` — the live Revue controller:
  - `renderBins(fldz, state)` — list_bins → chips, selection, "+ nouveau" (createBin), and
    the NoLibraryRoot picker (folder dialog → setSetting('library_root') → reload).
  - `openFilingInto(mid, item, state)` — analysis report + canonical-fields editor
    (seeded by reconcile, confidence badge) + format-override chips + Ranger/Re-sourcer/
    Écarter footer; wires fileTrack / rejectTrack / trashTrack; shows an undo toast.
  - small shared `RevueState` (selected bin rel, current track id/path, edited canonical).
- **Modify** `frontend/sift-live.ts` — give queue rows `data-id`; on click call
  `filing.openFilingInto(mid, item, state)`; after `renderQueue`, call
  `filing.renderBins(fldz, state)`; subscribe undo to Ctrl+Z.

## Slices (each ends with `npx tsc --noEmit` green + a commit)

### Slice 1 — Library-root gate + live bins in `#fldz`
- `getSetting('library_root')`; if null, render a picker prompt in `#fldz`
  (button → `@tauri-apps/plugin-dialog` open directory → `setSetting` → refresh).
- Else `listBins()` → chips (indent by `depth`), click selects (`state.binRel`),
  "+ nouveau" → inline input → `createBin(parentRel, name)` → refresh + select it.
- Replace the mockup `#fldz` content only under Tauri (mirror the renderQueue pattern).

### Slice 2 — Filing footer in `#mid` (reconcile + edit + format)
- After `report-view.renderReportInto`, append a footer: confidence badge + editable
  artist / title / version inputs (seeded by `reconcile(track_id)`), a live filename
  preview, and format chips (MP3 320 / AIFF) defaulting to the rail, overriding `target`.
- No actions yet — just state capture into `state.edited` / `state.target`.

### Slice 3 — Actions: Ranger / Re-sourcer / Écarter
- "Ranger → <bin>" → `fileTrack(track_id, state.binRel, state.target, state.edited)`;
  on `NoLibraryRoot`/`DestExists`/`EncodeFailed`/`Upscale` show the message, keep the track.
- Verdict-aware secondary button: fake → "Re-sourcer" (`rejectTrack`); else "Écarter"
  (`trashTrack`). On success the backend emits `queue:changed` → queue refreshes; advance
  to the next pending item.

### Slice 4 — Undo
- After any filing action, show a toast "Rangé — Annuler" → `undoLast()`.
- Global Ctrl+Z (when not editing a field) → `undoLast()` → refresh.

## Out of scope (later)
- Batch-mode wiring (`renderBatch`) — M4-4c if wanted.
- Écartés tab + Soulseek copy + buy links — M4b.
- Journal panel UI — M4-4c (binding `listJournal`/`revertBatch` already exists).
- Réglages library-root row — the Revue picker covers setting it for now.
