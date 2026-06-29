# Batch ↔ Detail Convergence Re-skin Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Re-skin the three Batch zones with the Detail screen's visual grammar, add per-format group selection, a single adaptive action button (Filer/Discarder ↔ Stop) anchored in the right rail, and move the destination selector into the right rail — all FRONT-ONLY.

**Architecture:** All work lives in `frontend/sift-live.ts` (the Batch view: `renderBatch`, `renderBatchRail`, the click/change wiring, the run lifecycle) plus a small CSS block in `frontend/styles.css`. No backend, no `shared/contracts.ts` change. Selection is split into two sets — `batchSel` (fileables → File) and `batchFakeSel` (fakes → Discard) — so the action button can be adaptive. The right rail (`#filfoot`) becomes the single home of: récap → destination pill → per-track progress → state button.

**Tech Stack:** Vite vanilla TypeScript, existing palette tokens (`col-h`, `inputCss` surfaces, `--color-background-info`/`-danger`/`-warning`, `--color-text-*`, `--border-radius-md`, 0.5px borders), Tabler icons (`ti`).

**Verification model:** This frontend has no unit-test runner; the established gate is `npx tsc --noEmit` (exit 0) + Antoine's live test (checklist a–f from the spec). Each task ends with a tsc step and names the live-test letter it satisfies.

**Out of scope / FLAGGED (needs backend, do NOT build here):** Zone A's per-row "Label · Année" — `QueueItem` (contracts.ts) has no `label`/`year`; surfacing it requires extending the Rust `QueueItem` struct + its SQL. Left out, flagged for the user.

---

## File Structure

- `frontend/sift-live.ts` — Batch view. State, render, wiring, run lifecycle.
  - State: add `const batchFakeSel = new Set<number>();` next to `batchSel`.
  - `renderBatch()` — Zone 1 re-skin + group-header tri-state + remove center action bar + remove per-format quick buttons.
  - `renderBatchRail()` — Zone 2 re-skin (col-h récap), destination pill (D), mount point for progress (Zone 3) + the adaptive/Stop state button (C).
  - new helpers: `groupHeaderHtml`, `groupState`, `actionButtonHtml`.
  - wiring: `batchgroup` (tri-state group toggle), `batchpickfake` (fake row tick), `batchaction` (adaptive run dispatch), `batchstop`, keep `batchbin` change.
  - `runBatchFile` / `runBatchDiscard` / `onFileBatchDone` — flip the rail button to Stop during a run, back to adaptive after.
  - `ensureBatchTracklistHost()` — re-target into `#filfoot` (Zone 3 placement).
- `frontend/styles.css` — `.sift-bgrp-*` (group header checkbox), `.sift-baction` (state button) classes using tokens only.

---

## Task 1: Split selection into fileables + fakes, with tri-state group headers (Zone B + Zone A re-skin of Ready/Review)

**Files:**
- Modify: `frontend/sift-live.ts` — `renderBatch` (~262-383), `batchall`/`batchpick` handlers (~877-896), add `batchFakeSel` state (~72), helpers near `railLabel` (~385).

- [ ] **Step 1: Add the fakes selection set**

After `const batchSel = new Set<number>();` (the existing fileables set), add:

```ts
// Fakes ticked for DISCARD (never filed — Sift never ranges a fake lossless). Kept separate from
// batchSel (fileables → File) so the rail action button can be adaptive (File n / Discard n / both).
const batchFakeSel = new Set<number>();
```

- [ ] **Step 2: Add group-state + group-header helpers**

Add near `railLabel` (~385):

```ts
/** Tri-state of a group's checkbox given its item ids and the active selection set. */
function groupState(ids: number[], sel: Set<number>): "empty" | "partial" | "full" {
  if (ids.length === 0) return "empty";
  let n = 0;
  for (const id of ids) if (sel.has(id)) n++;
  return n === 0 ? "empty" : n === ids.length ? "full" : "partial";
}

/** A group header row: tri-state checkbox + col-h label + count + optional right-aligned extra.
 *  `kind` routes the toggle to the right selection set ("file" → batchSel, "fake" → batchFakeSel). */
function groupHeaderHtml(kind: "file" | "fake", railKey: string, label: string, ids: number[], extra = ""): string {
  const st = groupState(ids, kind === "file" ? batchSel : batchFakeSel);
  const box =
    st === "full"
      ? `<span class="sift-bgrp-box on"><i class="ti ti-check"></i></span>`
      : st === "partial"
        ? `<span class="sift-bgrp-box partial"><i class="ti ti-minus"></i></span>`
        : `<span class="sift-bgrp-box"></span>`;
  return (
    `<div class="sift-bgrp-head" data-sift="batchgroup" data-kind="${kind}" data-rail="${esc(railKey)}" style="cursor:pointer">` +
    box +
    `<span class="col-h" style="margin:0">${esc(label)} · ${ids.length}</span>` +
    `<span style="flex:1"></span>${extra}</div>`
  );
}
```

- [ ] **Step 3: Rebuild `railGroup` to use the group header + keep dense rows**

Replace the `railGroup` arrow (~339-349) with a version whose header is the tri-state checkbox (output-format hint kept on the right, dense rows unchanged):

```ts
const railGroup = (rail: "lossless" | "lossy" | "unknown") => {
  const xs = ready.filter((it) => (it.rail ?? "unknown") === rail);
  if (!xs.length) return "";
  const fmt = `<span style="font-size:var(--text-xs);color:var(--color-text-tertiary)">→ ${outputFormat(rail)}</span>`;
  return (
    `<div style="margin:2px 0 6px">` +
    groupHeaderHtml("file", rail, railLabel(rail), xs.map((it) => it.id), fmt) +
    xs.map(readyRow).join("") +
    `</div>`
  );
};
```

- [ ] **Step 4: Give fakes their own selectable group + row ticks in NEEDS REVIEW**

Add `fakeRow` next to `reviewRow` (~313):

```ts
const fakeRow = (it: QueueItem) => {
  const on = batchFakeSel.has(it.id);
  return (
    `<div class="bx-row" data-sift="batchpickfake" data-id="${it.id}" style="display:flex;align-items:center;gap:9px;padding:7px 9px;border-radius:var(--border-radius-md);cursor:pointer;${
      on ? "background:rgba(255,255,255,.045)" : ""
    }">` +
    `<span class="bx-ck" style="flex:none;width:15px;height:15px;border-radius:4px;display:inline-flex;align-items:center;justify-content:center;border:1.5px solid ${
      on ? "var(--color-text-danger)" : "var(--color-border-secondary)"
    };background:${on ? "var(--color-background-danger)" : "transparent"}">${
      on ? '<i class="ti ti-check" style="font-size:var(--text-xs);color:var(--color-text-danger)"></i>' : ""
    }</span>` +
    verdictDot(it.verdict) +
    nameCell(it, true) +
    `<span style="flex:none;font-size:var(--text-2xs);font-weight:600;letter-spacing:.03em;padding:2px 7px;border-radius:999px;background:var(--color-background-danger);color:var(--color-text-danger)">FAKE</span>` +
    `<button data-sift="batchopen" data-id="${it.id}" style="flex:none;font-size:var(--text-xs);padding:2px 8px;color:var(--color-text-info)">open in Detail</button>` +
    `</div>`
  );
};
```

- [ ] **Step 5: Re-skin the section heads (Zone A) + remove the per-format quick button, keep Tout/Aucun**

Add the fakes split + replace `readyHead` (~350-358) so the only global control is a single "Tout / Aucun" toggle (B GLOBAL):

```ts
const fakes = review.filter((it) => it.verdict === "fake");
const reviewRest = review.filter((it) => it.verdict !== "fake");
const readyHead = sectionHead(
  "READY TO FILE",
  ready.length,
  `<button data-sift="batchall" style="font-size:var(--text-xs);padding:2px 8px;color:var(--color-text-info)">${
    allOn ? "Aucun (clear)" : "Tout"
  }</button>`,
);
```

- [ ] **Step 6: Assemble the new center body (Zone 1) and drop the center action bar**

Replace the `mid.innerHTML` block (~360-380) — NO action bar at the bottom anymore (moved to the rail in Task 2); fakes get a header + ticks; grey/unanalyzed stay read-only:

```ts
mid.innerHTML =
  `<div style="display:flex;flex-direction:column;height:100%;min-height:0">` +
  `<div style="flex:1;min-height:0;overflow-y:auto;padding-right:2px">` +
  (ready.length
    ? readyHead + railGroup("lossless") + railGroup("lossy") + railGroup("unknown")
    : '<div class="col-h" style="margin:0 0 6px">READY TO FILE · 0</div><div style="font-size:var(--text-md);color:var(--color-text-tertiary);padding:4px 9px 14px">Nothing clean to file yet.</div>') +
  (review.length
    ? `<div style="margin-top:16px"></div>` + sectionHead("NEEDS REVIEW", review.length) +
      (fakes.length ? groupHeaderHtml("fake", "fake", "Fakes", fakes.map((it) => it.id)) + fakes.map(fakeRow).join("") : "") +
      reviewRest.map(reviewRow).join("")
    : "") +
  `</div></div>`;
```

- [ ] **Step 7: Update the selection handlers (Zone B gestures)**

Replace the `batchall` handler (~877-882) and add `batchgroup` + `batchpickfake` (the existing `batchpick` for ready rows is unchanged):

```ts
} else if (act === "batchall") {
  e.stopPropagation();
  const ready = currentItems.filter((it) => it.verdict === "ok");
  if (batchSel.size === ready.length) batchSel.clear();
  else for (const it of ready) batchSel.add(it.id);
  renderBatch();
} else if (act === "batchgroup") {
  e.stopPropagation();
  const kind = el.dataset.kind === "fake" ? "fake" : "file";
  const railKey = el.dataset.rail ?? "";
  const ids =
    kind === "fake"
      ? currentItems.filter((it) => it.verdict === "fake").map((it) => it.id)
      : currentItems.filter((it) => it.verdict === "ok" && (it.rail ?? "unknown") === railKey).map((it) => it.id);
  const sel = kind === "fake" ? batchFakeSel : batchSel;
  // empty/partial → check all; full → clear all (tri-state toggle).
  const full = ids.length > 0 && ids.every((id) => sel.has(id));
  for (const id of ids) if (full) sel.delete(id); else sel.add(id);
  renderBatch();
} else if (act === "batchpickfake") {
  e.stopPropagation();
  const id = Number(el.dataset.id);
  if (batchFakeSel.has(id)) batchFakeSel.delete(id);
  else batchFakeSel.add(id);
  renderBatch();
```

- [ ] **Step 8: Type-check**

Run: `cmd /c "npx tsc --noEmit"`
Expected: exit 0. (`renderBatch` still calls `renderBatchRail(review.length)` at its end — unchanged; that function is rewritten in Task 2.)

Satisfies live-test **a** + **b**; full **c** lands once the rail button exists (Task 2).

---

## Task 2: Right-rail récap re-skin + destination pill + adaptive/Stop action button (Zone A2 + C + D)

**Files:**
- Modify: `frontend/sift-live.ts` — `renderBatchRail` (~411-423), `binSelectHtml` re-style (~398-407), the `batchbin` change handler (~900-906, verify only), run lifecycle (`runBatchFile` ~453, `runBatchDiscard` ~555, `onFileBatchDone` ~519), the action dispatch wiring (~890-896).

- [ ] **Step 1: Destination single-source verification (read-only, D garde-fou)**

Confirmed by reading: `binLabel()` (detail) reads `state.binRel`/`state.bins`, returns a single label string for the tree-selected node; `binSelectHtml()` (batch) builds `<option>`s from `batchBins` + `FILE_IN_PLACE` + "Library root". Different data source, different widget (tree vs select), different output. Detail is unchanged by this spec. **Conclusion: do NOT factor** (simple > complexe). No code in this step.

- [ ] **Step 2: Re-style `binSelectHtml` as a full-width rail pill**

Replace the `<select>` styling (~406) so it fills the rail récap (keep all options + `data-sift="batchbin"` + `batchBin` state):

```ts
return `<select data-sift="batchbin" class="sift-bpill" style="font-size:var(--text-sm);padding:6px 9px;border-radius:var(--border-radius-md);background:var(--color-background-secondary);color:var(--color-text-secondary);border:0.5px solid var(--color-border-tertiary);width:100%;max-width:100%">${opts}</select>`;
```

- [ ] **Step 3: Add the adaptive action-button helper**

Add near `binSelectHtml` (~407):

```ts
/** The single rail action button. Adaptive before a run, "Stop" during one. fileN = batchSel.size,
 *  fakeN = batchFakeSel.size. `running` swaps to the Stop affordance (wired to onFileStop). */
function actionButtonHtml(running: boolean): string {
  if (running) {
    return `<button data-sift="batchstop" class="sift-baction" style="background:var(--color-background-danger);color:var(--color-text-danger)"><i class="ti ti-player-stop" style="font-size:var(--text-md);vertical-align:-2px"></i> Stop</button>`;
  }
  const fileN = batchSel.size;
  const fakeN = batchFakeSel.size;
  if (fileN === 0 && fakeN === 0)
    return `<button class="sift-baction" disabled style="background:var(--color-background-info);color:var(--color-text-info);opacity:.5;pointer-events:none">Filer (0)</button>`;
  if (fakeN === 0)
    return `<button data-sift="batchaction" class="sift-baction" style="background:var(--color-background-info);color:var(--color-text-info)"><i class="ti ti-corner-down-left" style="font-size:var(--text-md);vertical-align:-2px"></i> Filer (${fileN})</button>`;
  if (fileN === 0)
    return `<button data-sift="batchaction" class="sift-baction" style="background:var(--color-background-danger);color:var(--color-text-danger)"><i class="ti ti-trash" style="font-size:var(--text-md);vertical-align:-2px"></i> Discarder (${fakeN})</button>`;
  return `<button data-sift="batchaction" class="sift-baction" style="background:var(--color-background-info);color:var(--color-text-info)">Filer (${fileN}) · Discarder (${fakeN})</button>`;
}
```

- [ ] **Step 4: Rewrite `renderBatchRail` (récap col-h + destination pill + progress mount + action button)**

Replace `renderBatchRail` body (~411-423). The progress host + a per-run note are preserved across the rebuild so a running batch keeps its rows; the button reflects the live `batchRunning` flag (Step 5):

```ts
function renderBatchRail(reviewN: number) {
  const foot = requireEl("#filfoot", "renderBatchRail");
  const fldz = requireEl("#fldz", "renderBatchRail");
  fldz.style.display = reviewMode === "batch" ? "none" : "";
  const head = (label: string) => `<div class="col-h" style="margin:0 0 4px">${label}</div>`;
  // Preserve a live run's progress list + note across this wholesale rebuild.
  const keepTracks = foot.querySelector("#sift-batch-tracks");
  const keepNote = foot.querySelector("[data-file-note]");
  foot.innerHTML =
    `<div style="margin-bottom:14px">${head("Selection")}<div style="font-size:var(--text-md);color:var(--color-text-primary);font-weight:500">${batchSel.size} à filer${
      batchFakeSel.size ? ` · ${batchFakeSel.size} à jeter` : ""
    }</div></div>` +
    `<div style="margin-bottom:14px">${head("Destination")}${binSelectHtml()}</div>` +
    `<div style="margin-bottom:14px">${head("Excluded")}<div style="font-size:var(--text-sm);color:var(--color-text-tertiary)">${reviewN} need review · filed safely only when clean</div></div>` +
    `<div id="sift-batch-tracks"></div>` +
    `<div class="sift-baction-slot">${actionButtonHtml(batchRunning)}</div>`;
  if (keepNote) foot.insertAdjacentElement("afterbegin", keepNote);
  if (keepTracks) foot.querySelector("#sift-batch-tracks")!.replaceWith(keepTracks);
}
```

- [ ] **Step 5: Add the `batchRunning` flag**

Near `fileStopping` (~189) add:

```ts
let batchRunning = false;
```

- [ ] **Step 6: Drive the flag from the run lifecycle**

`runBatchFile` — after the early `if (ids.length === 0) return;`:
```ts
  batchRunning = true;
  renderBatchRail(currentItems.filter((it) => it.verdict !== "ok").length);
```
`onFileBatchDone` — after `fileStopping = false;`:
```ts
  batchRunning = false;
```
(its later `await refresh()` repaints `renderBatch` → `renderBatchRail`, returning the button to adaptive; the success note shows via the existing `fileNote`.)

`runBatchDiscard` — wrap the run (also switch its source set to `batchFakeSel`):
```ts
async function runBatchDiscard() {
  const ids = [...batchFakeSel];
  if (ids.length === 0) return;
  batchRunning = true;
  renderBatchRail(currentItems.filter((it) => it.verdict !== "ok").length);
  try {
    await rejectBatch(ids);
    batchFakeSel.clear();
  } catch (err) {
    console.error("reject_batch failed", err);
  } finally {
    batchRunning = false;
    await refresh();
  }
}
```

- [ ] **Step 7: Wire the single adaptive dispatch + Stop, remove the old center buttons' handlers**

Replace the old `batchfile`/`batchdiscard` arms (~890-896) with:

```ts
} else if (act === "batchaction") {
  e.stopPropagation();
  if (batchSel.size) void runBatchFile();
  else if (batchFakeSel.size) void runBatchDiscard();
} else if (act === "batchstop") {
  e.stopPropagation();
  onFileStop();
}
```

Note on "both ticked": the button reads "Filer (n) · Discarder (n)" and this dispatch runs the File batch (progress/Stop follows the file run); discard-only path runs when only fakes are ticked. Simultaneous file+discard in one click is deferred (surgical; file-only and discard-only are the live-test paths).

- [ ] **Step 8: Confirm the `batchbin` change handler still works (D)** — delegated on `#pa` (~900-906): updates `batchBin` + re-renders the rail. The select now renders inside the rail but the listener is delegated, so it keeps working. Read to confirm; no edit.

- [ ] **Step 9: Type-check**

Run: `cmd /c "npx tsc --noEmit"`
Expected: exit 0.

Satisfies live-test **c** (adaptive button), **d** (Stop), **e** (destination in rail).

---

## Task 3: Move the per-track progress list into the right rail (Zone 3 / C ordering)

**Files:**
- Modify: `frontend/sift-live.ts` — `ensureBatchTracklistHost` (~493-503).

- [ ] **Step 1: Re-target the host into `#filfoot`'s `#sift-batch-tracks` slot**

```ts
function ensureBatchTracklistHost(): HTMLElement {
  let el = document.getElementById("sift-batch-tracks");
  if (!el) {
    // Rail slot not mounted yet — create a detached node; renderBatchRail preserves it on rebuild.
    el = document.createElement("div");
    el.id = "sift-batch-tracks";
    document.getElementById("filfoot")?.appendChild(el);
  }
  return el;
}
```

- [ ] **Step 2: Type-check + confirm preservation**

Run: `cmd /c "npx tsc --noEmit"`
Expected: exit 0. Progress rows now render in the right rail, under the récap, above the Stop button. `renderBatchRail`'s `keepTracks` preservation keeps them alive across re-renders during a run.

Satisfies live-test **d** (progress bars appear À DROITE under the récap).

---

## Task 4: CSS for the new batch classes (tokens only) + final pass

**Files:**
- Modify: `frontend/styles.css` — add a block near the existing `.sift-bt-*` block.

- [ ] **Step 1: Add the classes**

```css
/* Batch group-header tri-state checkbox (Ready-to-file format groups + Fakes). */
.sift-bgrp-head{display:flex;align-items:center;gap:8px;padding:3px 9px}
.sift-bgrp-box{flex:none;width:15px;height:15px;border-radius:4px;display:inline-flex;align-items:center;justify-content:center;border:1.5px solid var(--color-border-secondary);background:transparent;color:var(--color-text-tertiary);font-size:var(--text-xs)}
.sift-bgrp-box.on{border-color:var(--color-text-success);background:var(--color-text-success);color:var(--color-background-primary)}
.sift-bgrp-box.partial{border-color:var(--color-text-info);color:var(--color-text-info)}
/* The single rail action button (adaptive / Stop). */
.sift-baction{width:100%;font-size:var(--text-sm);font-weight:600;padding:9px 14px;border-radius:var(--border-radius-md);border:none;cursor:pointer}
.sift-baction-slot{margin-top:8px}
```

- [ ] **Step 2: Token audit** — confirm no invented hex in edited regions. Pre-existing literals NOT touched (gold check `#1a1a18`, `rgba(255,255,255,.045)` row highlight) left as-is. The removed center action bar deletes the old `#2f6fe0/#e5eeff` literals as a side effect.

- [ ] **Step 3: Final type-check** — Run: `cmd /c "npx tsc --noEmit"` → exit 0.

- [ ] **Step 4: Self-review a–f** — a→T1/T2, b→T1, c→T1+T2, d→T2+T3, e→T2, f→Detail untouched (no edit in `filing.ts`).

---

## Self-Review (author pass)

- **Spec coverage:**
  - A Zone1 (re-skin Ready, keep density, col-h headers, CTA moved) → T1 + T2. **Per-row "Label · Année" → FLAGGED out (backend).**
  - A Zone2 (rail récap col-h + destination pill) → T2.
  - A Zone3 (progress into right rail) → T3.
  - B (tri-state group headers; fakes→discard; keep only Tout/Aucun) → T1. **Group chevron collapse → deferred** (current center has no collapse state; optional polish, flag for user).
  - C (adaptive button + Filer→Stop, single rail location, remove center buttons) → T2.
  - D (destination into rail, no factoring, detail unchanged) → T2.
- **Placeholder scan:** none — every step has concrete code.
- **Type consistency:** `batchFakeSel` (T1) used by `groupHeaderHtml`/`actionButtonHtml`/handlers; `batchRunning` (T2 S5) used by `renderBatchRail`/`actionButtonHtml`; `groupState`/`groupHeaderHtml` signatures consistent.
- **Deferred sub-items to confirm with user:** (1) per-row Label·Année (backend), (2) group chevron collapse (front, optional), (3) simultaneous file+discard in one click (currently file-run when both ticked).
