// Global background-task progress zone — lives at the BOTTOM of the nav sidebar (#nav), outside
// #content, so it survives view switches (the sidebar is static; only #content re-renders).
// One row per active task: icon + name + done/total + a thin accent bar. Hidden when idle.
//
// Generic by design (the `task:progress { kind, done, total, state }` contract): this module is a
// small encapsulated store + renderer. It subscribes to NOTHING itself — callers push state via
// `setTask`/`clearTask` (analyse is wired in sift-live; identify/file are kept ready, not wired).
import { requireEl } from "./dom";

/** Which long task a row represents. Doubles as the Map key (one active run per kind). */
export type TaskKind = "analyze" | "identify" | "file";

/** Lifecycle of a task run. `done`/`error` are terminal; the caller decides when to clear. */
export type TaskState = "running" | "done" | "error";

export interface TaskProgress {
  done: number;
  total: number;
  state: TaskState;
  /** Optional transient flag: the run is being cancelled (the user clicked Stop). The row shows
   *  "Stopping…" and hides its Stop button until the terminal update arrives. */
  stopping?: boolean;
}

/** Tabler icon per kind (analyse = wave, identify = tag, file = move-right). */
const ICONS: Record<TaskKind, string> = {
  analyze: "ti-wave-sine",
  identify: "ti-tag",
  file: "ti-arrow-big-right-lines",
};

/** Human label shown next to the icon. */
const LABELS: Record<TaskKind, string> = {
  analyze: "Analyzing",
  identify: "Identifying",
  file: "Filing",
};

// Encapsulated state: at most one active run per kind. Insertion order = display order.
const tasks = new Map<TaskKind, TaskProgress>();

// Optional per-kind cancel actions. A kind with a registered handler gets a Stop button on its row
// while running; the click is routed to the handler here (the IPC call lives in the caller, keeping
// this module generic). Only "file" registers one today (stop-net cancel); analyse has none.
const cancelHandlers = new Map<TaskKind, () => void>();

/** Register the Stop action for `kind` — shown as a button on the row while it is running. */
export function setCancelHandler(kind: TaskKind, fn: () => void): void {
  cancelHandlers.set(kind, fn);
}

/** Lazily create (once) the persistent zone container at the bottom of #nav. Fail-fast (P-4):
 * if the sidebar shell is missing, `requireEl` throws with the selector + context. */
function ensureZone(): HTMLElement {
  let zone = document.getElementById("sift-progress-zone");
  if (!zone) {
    const foot = requireEl(".nav-foot", "progress-zone ensureZone");
    zone = document.createElement("div");
    zone.id = "sift-progress-zone";
    zone.className = "sift-progress-zone";
    // Inserted ABOVE Settings inside the bottom group (`.nav-foot` carries the margin-top:auto). The
    // zone grows UPWARD into the rail's free space, so Settings stays pinned to the very bottom and no
    // nav item shifts when a background task starts.
    foot.prepend(zone);
    // One delegated listener for the per-row Stop buttons; routes to the registered cancel handler.
    zone.addEventListener("click", (e) => {
      const btn = (e.target as HTMLElement).closest<HTMLElement>("[data-pz-cancel]");
      if (btn) cancelHandlers.get(btn.dataset.pzCancel as TaskKind)?.();
    });
  }
  return zone;
}

/** Structural signature of a row: everything that fixes its STRUCTURE / static text (and whether the
 * Stop button exists) — but NOT the two values that move every tick (done/total + bar width). Same
 * sig ⇒ update in place; changed sig ⇒ rebuild this row only. */
function rowSig(kind: TaskKind, p: TaskProgress): string {
  const showStop = p.state === "running" && !p.stopping && cancelHandlers.has(kind);
  const label = p.stopping ? "Stopping…" : LABELS[kind];
  return `${p.state}|${p.stopping ? 1 : 0}|${showStop ? 1 : 0}|${label}`;
}

/** Outer class for a row (only the error modifier varies). */
function rowClassOf(p: TaskProgress): string {
  return p.state === "error" ? "sift-pz-row error" : "sift-pz-row";
}

/** INNER HTML of a row (head + track) — the outer `.sift-pz-row` is the cached rowEl. Built ONCE on
 * creation, and again only when the signature changes; NOT on every tick. While `stopping`, the label
 * reads "Stopping…" and the Stop button is omitted. */
function rowInner(kind: TaskKind, p: TaskProgress): string {
  const pct = p.total > 0 ? Math.min(100, Math.round((p.done / p.total) * 100)) : 0;
  const label = p.stopping ? "Stopping…" : LABELS[kind];
  // Stop button only while actively running, not already stopping, and a cancel action exists.
  const showStop = p.state === "running" && !p.stopping && cancelHandlers.has(kind);
  const stop = showStop
    ? `<button class="sift-pz-cancel" type="button" data-pz-cancel="${kind}" title="Stop" aria-label="Stop ${LABELS[kind]}"><i class="ti ti-x" aria-hidden="true"></i></button>`
    : "";
  return (
    `<div class="sift-pz-head">` +
    `<span class="sift-pz-name"><i class="ti ${ICONS[kind]}" aria-hidden="true"></i>${label}</span>` +
    `<span class="sift-pz-end"><span class="sift-pz-count">${p.done}/${p.total}</span>${stop}</span>` +
    `</div>` +
    `<div class="sift-pz-track"><div class="sift-pz-fill" style="width:${pct}%"></div></div>`
  );
}

/** Live DOM handles for one rendered row, kept ACROSS renders so ticks update in place instead of
 * rebuilding. `sig` is the structural signature; while it is unchanged only countEl/fillEl change. */
interface RowCache {
  rowEl: HTMLElement;
  countEl: HTMLElement;
  fillEl: HTMLElement;
  sig: string;
}
const rowCache = new Map<TaskKind, RowCache>();

/** Reconcile the zone from the current Map — CREATE ONCE, UPDATE IN PLACE. A row's structure (and its
 * Stop button) is built only on first appearance or when its signature changes; every other tick
 * writes just the two moving values. So a burst of `analyze` ticks never rebuilds the `file` row's
 * Stop button. Empty ⇒ the zone is hidden (no placeholder). */
function render(): void {
  const zone = ensureZone();
  if (tasks.size === 0) {
    zone.style.display = "none";
    zone.innerHTML = "";
    rowCache.clear();
    return;
  }
  zone.style.display = "";

  // Drop rows whose kind is no longer active.
  for (const [kind, cached] of rowCache) {
    if (!tasks.has(kind)) {
      cached.rowEl.remove();
      rowCache.delete(kind);
    }
  }

  // Create or update each active row, in insertion order (= display order).
  for (const [kind, p] of tasks) {
    const sig = rowSig(kind, p);
    const cached = rowCache.get(kind);
    if (!cached) {
      // New row: build the outer element + structure ONCE, append at the end (insertion order).
      const rowEl = document.createElement("div");
      rowEl.className = rowClassOf(p);
      rowEl.innerHTML = rowInner(kind, p);
      zone.appendChild(rowEl);
      rowCache.set(kind, {
        rowEl,
        countEl: requireEl<HTMLElement>(".sift-pz-count", "progress-zone row", rowEl),
        fillEl: requireEl<HTMLElement>(".sift-pz-fill", "progress-zone row", rowEl),
        sig,
      });
    } else if (cached.sig !== sig) {
      // Structure/label/button changed (start, Stop click, done) → rebuild THIS row's content only;
      // the rowEl node stays in place, so order and the zone's delegated listener are untouched.
      cached.rowEl.className = rowClassOf(p);
      cached.rowEl.innerHTML = rowInner(kind, p);
      cached.countEl = requireEl<HTMLElement>(".sift-pz-count", "progress-zone row", cached.rowEl);
      cached.fillEl = requireEl<HTMLElement>(".sift-pz-fill", "progress-zone row", cached.rowEl);
      cached.sig = sig;
    } else {
      // Same structure → write only the two moving values. No innerHTML, no node churn.
      const pct = p.total > 0 ? Math.min(100, Math.round((p.done / p.total) * 100)) : 0;
      cached.countEl.textContent = `${p.done}/${p.total}`;
      cached.fillEl.style.width = `${pct}%`;
    }
  }
}

/** Set/replace the active run for `kind` and redraw. */
export function setTask(kind: TaskKind, p: TaskProgress): void {
  tasks.set(kind, p);
  render();
}

/** Remove the run for `kind` (e.g. after it finished) and redraw. No-op if absent. */
export function clearTask(kind: TaskKind): void {
  if (tasks.delete(kind)) render();
}

/** Debug-only: inject a couple of fake runs to eyeball the rendering. Never called in committed
 * code — invoke from the devtools console (`import(...).then(m => m.__seedDebugTasks())`) to verify
 * layout, then `clearTask` to dismiss. Kept as an explicit, inert debug hook. */
export function __seedDebugTasks(): void {
  setTask("analyze", { done: 12, total: 40, state: "running" });
  setTask("identify", { done: 3, total: 3, state: "done" });
}
