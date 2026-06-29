// Per-track progress for a filing batch. NO backend event: state is DERIVED from the ordered
// submitted id list + file:progress.done (the first `done` ids are processed, the (done)-th is in
// progress, the rest wait), reconciled at file:done (filed vs needs_validation). file:progress is a
// BURST event, so rows are created ONCE in startBatchTracklist and only MUTATED afterwards — never
// re-innerHTML'd in the update path (the front-events rule in CLAUDE.md).
type BtState = "wait" | "run" | "done" | "fail";

interface BtRow {
  id: number;
  el: HTMLElement;
  pill: HTMLElement;
  state: BtState;
}

let rows: BtRow[] = [];
let host: HTMLElement | null = null;

const PILL: Record<BtState, { cls: string; html: string }> = {
  wait: { cls: "sift-bt-wait", html: '<i class="ti ti-clock"></i>' },
  run: { cls: "sift-bt-run", html: '<span class="sift-bt-spin"></span>' },
  done: { cls: "sift-bt-done", html: '<i class="ti ti-check"></i>' },
  fail: { cls: "sift-bt-fail", html: '<i class="ti ti-alert-triangle"></i>' },
};

const esc = (s: string) =>
  s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);

/** One row's inner markup: pill + (ellipsised) name + an optional non-truncating right suffix
 *  (used in "file in place" mode to show each track's real source folder). */
function rowInner(name: string, suffix?: string): string {
  return (
    '<span class="sift-bt-pill"></span>' +
    `<span class="sift-bt-name">${esc(name)}</span>` +
    (suffix ? `<span class="sift-bt-src">${esc(suffix)}</span>` : "")
  );
}

/** Mount the list (ordered ids + display names, optional source-folder suffix) into `container`,
 *  replacing any prior list. The first track starts immediately (run), the rest wait. */
export function startBatchTracklist(
  container: HTMLElement,
  items: { id: number; name: string; suffix?: string }[],
): void {
  host = container;
  host.innerHTML = '<div class="sift-bt-head">Batch</div><div class="sift-bt-list"></div>';
  const list = host.querySelector<HTMLElement>(".sift-bt-list")!;
  rows = items.map(({ id, name, suffix }) => {
    const el = document.createElement("div");
    el.className = "sift-bt-row";
    el.innerHTML = rowInner(name, suffix);
    list.appendChild(el);
    const row: BtRow = { id, el, pill: el.querySelector<HTMLElement>(".sift-bt-pill")!, state: "wait" };
    paint(row, "wait");
    return row;
  });
  if (rows.length) paint(rows[0], "run");
}

function paint(row: BtRow, s: BtState): void {
  if (row.state === s) return;
  row.state = s;
  row.pill.className = `sift-bt-pill ${PILL[s].cls}`;
  row.pill.innerHTML = PILL[s].html;
}

/** file:progress.done = number of files processed → first `done` are done, the (done)-th is running,
 *  the rest wait. A row already marked fail (shouldn't happen mid-run) is left as fail. */
export function updateBatchTracklist(done: number): void {
  rows.forEach((row, i) => {
    if (i < done) {
      if (row.state !== "fail") paint(row, "done");
    } else if (i === done) {
      paint(row, "run");
    } else {
      paint(row, "wait");
    }
  });
}

/** file:done → final reconcile: filed ids = done, needs_validation ids = fail. Ids in neither set
 *  (a cancelled run never reached them) are left at their current wait/run state. */
export function finishBatchTracklist(filed: number[], needsValidation: number[]): void {
  const ok = new Set(filed);
  const bad = new Set(needsValidation);
  for (const row of rows) {
    if (bad.has(row.id)) paint(row, "fail");
    else if (ok.has(row.id)) paint(row, "done");
  }
}

export function clearBatchTracklist(): void {
  if (host) host.innerHTML = "";
  rows = [];
  host = null;
}
