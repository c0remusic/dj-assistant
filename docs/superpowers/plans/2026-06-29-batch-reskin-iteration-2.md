# Batch Re-skin Iteration 2 — Folder Explorer, In-Place Checkbox, Name Preview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** In batch mode, replace the destination `<select>` with the détail folder-tree explorer (`#fldz`), add a "file in place" checkbox under the tree, and show a representative final-name preview in the rail — all front-only.

**Architecture:** Reuse the existing exported `renderBins(fldz)` tree (filing.ts) by adding a small module-level *pick context* (`binPick = { selectedRel, onPick }`); when set, the tree highlights `selectedRel` and routes clicks to `onPick` instead of `state.binRel`. Batch owns `batchBin` (folder rel) + a new `batchInPlace` boolean; effective destination = `batchInPlace ? FILE_IN_PLACE : batchBin`. The in-place checkbox is a sibling element after `#fldz`, surviving the tree's `innerHTML` rebuilds.

**Tech Stack:** Vite vanilla TS, palette tokens, existing `renderBins`/`binNodeHtml`/`FILE_IN_PLACE`/`IN_PLACE_LABEL`/`.sift-fil-prev`.

**Verification model:** no front unit runner — gate is `npx tsc --noEmit` (exit 0) + Antoine's live test (a–f).

**BLOCKED — needs backend, NOT in this plan:** Spec #4 (per-group format chips feeding the filer). `file_batch` (ipc_filing.rs:251 → run_file_batch:334) hardcodes `override_target = None`; `plan_file` already accepts `override_target: Option<Target>` (filing.rs:233) but the IPC boundary doesn't expose it. Wiring chips to the encode target requires editing `ipc_filing.rs` + the `fileBatch` binding + contract = `src-tauri/`. Documented in the final section; awaiting user go-ahead.

---

## File Structure

- `frontend/filing.ts` — bin tree made reusable. Add `binPick` context + `selRel()`; mode-aware click handler; export `renderBinsForBatch`, `refreshBinsForBatch`, `clearBinPick`.
- `frontend/sift-live.ts` — batch destination switches from `<select>` to the tree: `batchInPlace` state, effective-dest helper, in-place checkbox under `#fldz`, rail récap + `.sift-fil-prev` preview, remove `binSelectHtml`/its change handler, stop hiding `#fldz`.

---

## Task 1: Make the bin tree reusable with a pick context (filing.ts)

**Files:** Modify `frontend/filing.ts` — near `binNodeHtml` (~189), `flatBinHtml` (~219), `renderBins` click wiring (~306-312), `refreshBins` (~1334).

- [ ] **Step 1: Add the pick context + selected-rel helper** (above `binNodeHtml`, ~188)

```ts
// Optional batch pick context: when set, the #fldz tree highlights `selectedRel` and routes a folder
// click to `onPick` (→ batchBin in sift-live) instead of detail's state.binRel. null = detail mode.
let binPick: { selectedRel: string | null; onPick: (rel: string) => void } | null = null;
/** The rel currently highlighted in the tree — batch pick context when active, else detail's. */
function selRel(): string | null {
  return binPick ? binPick.selectedRel : state.binRel;
}
```

- [ ] **Step 2: Use `selRel()` for the highlight in both renderers**

`binNodeHtml`: `const on = node.rel === state.binRel ? " on" : "";` → `const on = node.rel === selRel() ? " on" : "";`
`flatBinHtml`: `const on = b.rel === state.binRel ? " on" : "";` → `const on = b.rel === selRel() ? " on" : "";`

- [ ] **Step 3: Mode-aware folder-click handler** (replace filing.ts:306-312)

```ts
  fldz.querySelectorAll<HTMLElement>('[data-fil="bin"]').forEach((el) =>
    el.addEventListener("click", () => {
      const rel = el.dataset.rel ?? "";
      if (binPick) {
        binPick.onPick(rel); // batch: caller updates batchBin + re-renders tree/rail/preview
      } else {
        state.binRel = rel;
        renderBins(fldz);
        refreshFootButton();
      }
    }),
  );
```

- [ ] **Step 4: Export batch entry points** (next to `refreshBins`, ~1337)

```ts
/** Render the tree in batch pick mode (no reload — state.bins already loaded). */
export function renderBinsForBatch(
  fldz: HTMLElement,
  selectedRel: string | null,
  onPick: (rel: string) => void,
): void {
  binPick = { selectedRel, onPick };
  renderBins(fldz);
}

/** Load bins then render the tree in batch pick mode (entry when switching into batch). */
export async function refreshBinsForBatch(
  fldz: HTMLElement,
  selectedRel: string | null,
  onPick: (rel: string) => void,
): Promise<void> {
  binPick = { selectedRel, onPick };
  await loadBins();
  renderBins(fldz);
}

/** Leave batch pick mode → tree reverts to detail's state.binRel. */
export function clearBinPick(): void {
  binPick = null;
}
```

- [ ] **Step 5: Type-check** — `cmd /c "npx tsc --noEmit"` → exit 0.

---

## Task 2: Batch uses the tree for destination + in-place checkbox + preview (sift-live.ts)

**Files:** Modify `frontend/sift-live.ts` — state (~67), filing import, `binSelectHtml` (remove ~398), `renderBatchRail` (~485), `setReviewMode` (~507), `runBatchFile` dest (~551), `batchbin` change handler (remove ~1015).

- [ ] **Step 1: In-place state + import**

Near `const batchFakeSel` (~67):
```ts
// Batch "file in place" toggle (FILE_IN_PLACE). Kept apart from batchBin so the picked folder is
// remembered while in-place is on. Effective destination = batchInPlace ? FILE_IN_PLACE : batchBin.
let batchInPlace = false;
```
Add `import { renderBinsForBatch, refreshBinsForBatch, clearBinPick } from "./filing";` (or extend the existing filing import).

- [ ] **Step 2: Effective-destination + pick callback** (near `batchTrackName`)

```ts
/** The destination actually passed to the filer + shown in the récap/preview. */
function batchDest(): string {
  return batchInPlace ? FILE_IN_PLACE : batchBin;
}
function batchDestLabel(): string {
  return batchInPlace ? IN_PLACE_LABEL : batchBin || "Library root";
}
function onBatchBinPick(rel: string): void {
  batchBin = rel;
  batchInPlace = false; // picking a folder turns off in-place
  const fldz = document.getElementById("fldz");
  if (fldz) renderBinsForBatch(fldz, batchBin, onBatchBinPick);
  renderBatchRail(currentItems.filter((it) => it.verdict !== "ok").length);
}
/** Representative batch final-name preview: count of fileables → destination folder. */
function batchPreview(): string {
  const n = batchSel.size;
  if (n === 0) return "—";
  return `${n} morceau${n > 1 ? "x" : ""} → ${batchDestLabel()}/…`;
}
```

- [ ] **Step 3: Remove `binSelectHtml` + its change arm**

Delete `binSelectHtml` (~398-407) and the `select[data-sift="batchbin"]` arm of the `#pa` change handler (~1015-1022).

- [ ] **Step 4: `ensureBatchDestUI` — in-place checkbox under the tree**

```ts
/** Ensure the batch destination UI around #fldz: the tree is in batch pick mode, and a "file in
 *  place" checkbox sits right under it (a sibling, so renderBins' innerHTML rebuild can't wipe it). */
function ensureBatchDestUI(): void {
  const fldz = document.getElementById("fldz");
  if (!fldz) return;
  fldz.style.opacity = batchInPlace ? ".45" : "";
  fldz.style.pointerEvents = batchInPlace ? "none" : "";
  let box = document.getElementById("sift-inplace");
  if (!box) {
    box = document.createElement("label");
    box.id = "sift-inplace";
    box.style.cssText =
      "display:flex;align-items:center;gap:7px;margin-top:8px;font-size:var(--text-sm);color:var(--color-text-secondary);cursor:pointer";
    fldz.parentElement?.insertBefore(box, fldz.nextSibling);
  }
  box.innerHTML =
    `<input type="checkbox" data-sift="inplace"${batchInPlace ? " checked" : ""} style="accent-color:var(--color-text-info)"> ${esc(IN_PLACE_LABEL)}`;
}
```

- [ ] **Step 5: Rewrite `renderBatchRail`** (text destination + `.sift-fil-prev` preview, no select)

```ts
function renderBatchRail(reviewN: number) {
  const foot = requireEl("#filfoot", "renderBatchRail");
  const fldz = requireEl("#fldz", "renderBatchRail");
  fldz.style.display = ""; // batch now shows the tree (no longer hidden)
  ensureBatchDestUI();
  const head = (label: string) => `<div class="col-h" style="margin:0 0 4px">${label}</div>`;
  const keepTracks = foot.querySelector("#sift-batch-tracks");
  const keepNote = foot.querySelector("[data-file-note]");
  foot.innerHTML =
    `<div style="margin-bottom:14px">${head("Selection")}<div style="font-size:var(--text-md);color:var(--color-text-primary);font-weight:500">${
      batchSel.size
    } à filer${batchFakeSel.size ? ` · ${batchFakeSel.size} à jeter` : ""}</div></div>` +
    `<div style="margin-bottom:14px">${head("Destination")}<div style="font-size:var(--text-md);color:var(--color-text-secondary)">${esc(batchDestLabel())}</div></div>` +
    `<div style="margin-bottom:14px">${head("Final name")}<div class="sift-fil-prev" style="font-size:var(--text-xs);color:var(--color-text-tertiary);font-family:var(--font-mono);word-break:break-all;line-height:1.5">${esc(batchPreview())}</div></div>` +
    `<div style="margin-bottom:14px">${head("Excluded")}<div style="font-size:var(--text-sm);color:var(--color-text-tertiary)">${reviewN} need review · filed safely only when clean</div></div>` +
    `<div id="sift-batch-tracks"></div>` +
    `<div class="sift-baction-slot">${actionButtonHtml(batchRunning)}</div>`;
  if (keepNote) foot.insertAdjacentElement("afterbegin", keepNote);
  if (keepTracks) foot.querySelector("#sift-batch-tracks")!.replaceWith(keepTracks);
}
```

- [ ] **Step 6: Wire the in-place checkbox change** (add an arm in the `#pa` change handler)

```ts
    const ip = (e.target as HTMLElement).closest<HTMLInputElement>('input[data-sift="inplace"]');
    if (ip) {
      batchInPlace = ip.checked;
      const fldz = document.getElementById("fldz");
      if (fldz) renderBinsForBatch(fldz, batchBin, onBatchBinPick);
      renderBatchRail(currentItems.filter((it) => it.verdict !== "ok").length);
      return;
    }
```

- [ ] **Step 7: Render the tree on entering batch, restore on leaving** (`setReviewMode`)

`m === "batch"` branch — after `renderBatch();` inside the `.then`:
```ts
        renderBatch();
        const fldz = document.getElementById("fldz");
        if (fldz) void refreshBinsForBatch(fldz, batchBin, onBatchBinPick);
```
`else` branch (leaving batch) — before `void renderQueue(true);`:
```ts
    clearBinPick();
    document.getElementById("sift-inplace")?.remove();
    const fldzEl = document.getElementById("fldz");
    if (fldzEl) {
      fldzEl.style.opacity = "";
      fldzEl.style.pointerEvents = "";
    }
```

- [ ] **Step 8: Filer gets the effective destination** — in `runBatchFile`, `await fileBatch(ids, batchBin);` → `await fileBatch(ids, batchDest());`.

- [ ] **Step 9: Type-check** — `cmd /c "npx tsc --noEmit"` → exit 0.

Satisfies a, b, c, e (f = detail untouched: default null ctx).

---

## Task 3: Final pass

- [ ] **Step 1: tsc** → exit 0.
- [ ] **Step 2: Self-review a–f** — a→T2, b→T2, c→T2, d→BLOCKED #4, e→T2, f→T1.
- [ ] **Step 3: No dead refs** — `binSelectHtml` / `select[data-sift="batchbin"]` removed; `FILE_IN_PLACE`/`IN_PLACE_LABEL` still imported (used by `batchDest`/`batchDestLabel`/checkbox).

---

## BLOCKED — Spec #4 (per-group format chips) — needs a small backend change

Not implemented (FRONT-PUR garde-fou). To make per-group chips drive the encode target:
1. `frontend/ipc.ts`: `fileBatch(trackIds, binRel, targets?)` carrying a `Record<trackId, Target>` (or per-rail map).
2. `shared/contracts.ts`: type the target map.
3. `src-tauri/src/ipc_filing.rs`: `file_batch`/`run_file_batch` accept the map; at `run_file_batch:334` pass `Some(target)` to `plan_file` instead of `None` (`plan_file`/`file_track` already accept `override_target` — no deeper change).
4. Front: render chips on each group header (reuse `chip`/`chip on` + the détail lossy-greying rule renderFoot:712-713), store per-rail chosen target, build the map at submit.

Display-only chips that don't affect the encode = a silent no-op (violates fail-fast) — deferred until the backend hop is approved.

---

## Self-Review

- **Spec coverage:** #1 → T1+T2; #2 → T2; #3 → T2 (representative preview, choice explained); #4 → BLOCKED. Detail unchanged → T1 default ctx.
- **Placeholders:** none.
- **Type consistency:** `binPick`/`selRel`/`renderBinsForBatch`/`refreshBinsForBatch`/`clearBinPick` (T1) consumed by `onBatchBinPick`/`setReviewMode`/`ensureBatchDestUI` (T2); `batchInPlace`/`batchDest`/`batchDestLabel`/`batchPreview` consistent.
- **Decisions to confirm:** (1) #4 backend hop yes/no; (2) preview form chosen = "N morceaux → dest/…" (front-only honest).
