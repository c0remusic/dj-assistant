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

/** Lazily create (once) the persistent zone container at the bottom of #nav. Fail-fast (P-4):
 * if the sidebar shell is missing, `requireEl` throws with the selector + context. */
function ensureZone(): HTMLElement {
  let zone = document.getElementById("sift-progress-zone");
  if (!zone) {
    const nav = requireEl("#nav", "progress-zone ensureZone");
    zone = document.createElement("div");
    zone.id = "sift-progress-zone";
    zone.className = "sift-progress-zone";
    // Appended after the `margin-top:auto` Settings item ⇒ pinned to the very bottom of the rail.
    nav.appendChild(zone);
  }
  return zone;
}

/** One task row: icon + name on the left, tabular done/total on the right, thin bar below. */
function row(kind: TaskKind, p: TaskProgress): string {
  const pct = p.total > 0 ? Math.min(100, Math.round((p.done / p.total) * 100)) : 0;
  const rowClass = p.state === "error" ? "sift-pz-row error" : "sift-pz-row";
  return (
    `<div class="${rowClass}">` +
    `<div class="sift-pz-head">` +
    `<span class="sift-pz-name"><i class="ti ${ICONS[kind]}" aria-hidden="true"></i>${LABELS[kind]}</span>` +
    `<span class="sift-pz-count">${p.done}/${p.total}</span>` +
    `</div>` +
    `<div class="sift-pz-track"><div class="sift-pz-fill" style="width:${pct}%"></div></div>` +
    `</div>`
  );
}

/** Redraw the zone from the current Map. Empty ⇒ the zone is hidden (no placeholder). */
function render(): void {
  const zone = ensureZone();
  if (tasks.size === 0) {
    zone.style.display = "none";
    zone.innerHTML = "";
    return;
  }
  zone.style.display = "";
  zone.innerHTML = [...tasks.entries()].map(([kind, p]) => row(kind, p)).join("");
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
