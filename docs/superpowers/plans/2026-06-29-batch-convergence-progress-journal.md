# Batch Convergence — Per-Track Progress + Actions Journal — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Give live per-track feedback during a filing batch, and an after-the-fact Actions Journal (session tab + extended tree) with three-scope revert.

**Architecture:** VOLET 1 is front-only — derive per-track state from the ordered submitted id list + the existing `file:progress.done` count (done-first = done, next = in-progress, rest = waiting), reconciled at `file:done`. VOLET 2 adds a `session_id` column (migration v8) stamped on every journaled action via a process-global set at app launch, an extended journal IPC returning `session_id`, a Journal session tab + extended tree page, and per-kind revert routing (`convert|move`→revert_batch, `trash`→restore_track, `reject`→requeue_track).

**Tech Stack:** Tauri v2 (Rust), Vite vanilla TS front, rusqlite, existing `progress-zone` + `actions` journal.

---

## PROJECT ADAPTATIONS (override the generic skill cadence)

- **Do NOT `git commit`** — Antoine tests live. Replace every "commit" step with a **checkpoint**: run `cmd /c "npx tsc --noEmit"` (front) and hand off for a live test.
- **Claude cannot run cargo / tauri dev.** Rust unit tests are written but executed on Antoine's machine. After ANY `src-tauri/` edit, end the message with the ⚙️ REBUILD BACKEND block.
- **Tokens only** (`--color-*`, `--text-*`, `--space-*`), no invented hex.
- **Fail-fast, no silent fallback.** Unknown `kind` in revert routing → throw, not no-op.

---

## File Structure

**VOLET 1 (front only)**
- Create: `frontend/batch-tracklist.ts` — the per-track progress list (create-once/mutate).
- Modify: `frontend/sift-live.ts` — capture the ordered id→name map at submit; call the tracklist lifecycle from the batch handlers.
- Modify: `frontend/styles.css` — `.sift-bt-*` row/pill styles + the indeterminate-spin keyframe.

**VOLET 2**
- Modify: `src-tauri/src/db.rs` — migration v8 `ALTER TABLE actions ADD COLUMN session_id TEXT;` + test.
- Modify: `src-tauri/src/actions.rs` — process-global `SESSION_ID`; `set_session_id()`; `record_with_meta` writes it; new `list_journal_full()` + `JournalRow`.
- Modify: `src-tauri/src/lib.rs` — generate + set session id in `run()`; register the new command.
- Modify: `src-tauri/src/ipc_filing.rs` — `journal_full` command.
- Modify: `shared/contracts.ts` — `JournalRow` mirror.
- Modify: `frontend/ipc.ts` — `journalFull()` binding.
- Create: `frontend/journal-view.ts` — session tab + extended tree + per-kind revert router + `confirmAction`.
- Modify: `frontend/sift-live.ts` — register a "Journal" view following the Écartés pattern.
- Modify: `frontend/styles.css` — `.sift-jn-*` + `.sift-confirm-*`.

---

# PHASE 1 — VOLET 1: PER-TRACK PROGRESS (front only)

### Task 1: Batch tracklist component

**Files:** Create `frontend/batch-tracklist.ts`; Modify `frontend/styles.css`.

- [ ] **Step 1: Create the component**

```ts
// Per-track progress for a filing batch. NO backend event: state derived from the ORDERED
// submitted id list + file:progress.done (first `done` processed, next in progress, rest wait),
// reconciled at file:done (filed vs needs_validation). file:progress is a BURST event → create
// rows ONCE, then mutate; never innerHTML in the update path.
type BtState = "wait" | "run" | "done" | "fail";
interface BtRow { id: number; name: string; el: HTMLElement; pill: HTMLElement; state: BtState; }
let rows: BtRow[] = [];
let host: HTMLElement | null = null;

const PILL: Record<BtState, { cls: string; html: string }> = {
  wait: { cls: "sift-bt-wait", html: '<i class="ti ti-clock"></i>' },
  run: { cls: "sift-bt-run", html: '<span class="sift-bt-spin"></span>' },
  done: { cls: "sift-bt-done", html: '<i class="ti ti-check"></i>' },
  fail: { cls: "sift-bt-fail", html: '<i class="ti ti-alert-triangle"></i>' },
};

export function startBatchTracklist(container: HTMLElement, items: { id: number; name: string }[]): void {
  host = container;
  host.innerHTML = '<div class="sift-bt-head">Batch</div><div class="sift-bt-list"></div>';
  const list = host.querySelector<HTMLElement>(".sift-bt-list")!;
  rows = items.map(({ id, name }) => {
    const el = document.createElement("div");
    el.className = "sift-bt-row";
    el.innerHTML = `<span class="sift-bt-pill"></span><span class="sift-bt-name"></span>`;
    el.querySelector<HTMLElement>(".sift-bt-name")!.textContent = name;
    list.appendChild(el);
    const row: BtRow = { id, name, el, pill: el.querySelector<HTMLElement>(".sift-bt-pill")!, state: "wait" };
    setRow(row, "wait");
    return row;
  });
  if (rows.length) setRow(rows[0], "run");
}

function setRow(row: BtRow, s: BtState): void {
  if (row.state === s) return;
  row.state = s;
  row.pill.className = `sift-bt-pill ${PILL[s].cls}`;
  row.pill.innerHTML = PILL[s].html;
}

export function updateBatchTracklist(done: number): void {
  rows.forEach((row, i) => {
    if (i < done) { if (row.state !== "fail") setRow(row, "done"); }
    else if (i === done) setRow(row, "run");
    else setRow(row, "wait");
  });
}

export function finishBatchTracklist(filed: number[], needsValidation: number[]): void {
  const ok = new Set(filed), bad = new Set(needsValidation);
  for (const row of rows) {
    if (bad.has(row.id)) setRow(row, "fail");
    else if (ok.has(row.id)) setRow(row, "done");
  }
}

export function clearBatchTracklist(): void { if (host) host.innerHTML = ""; rows = []; host = null; }
```

- [ ] **Step 2: Styles** — append to `frontend/styles.css`:
```css
.sift-bt-head{font-size:var(--text-2xs);text-transform:uppercase;letter-spacing:.04em;color:var(--color-text-tertiary);margin:var(--space-8) 0 var(--space-4)}
.sift-bt-list{display:flex;flex-direction:column;gap:2px}
.sift-bt-row{display:flex;align-items:center;gap:7px;font-size:var(--text-sm);color:var(--color-text-secondary)}
.sift-bt-name{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.sift-bt-pill{width:16px;height:16px;flex:none;display:inline-flex;align-items:center;justify-content:center;border-radius:50%;font-size:11px}
.sift-bt-wait{color:var(--color-text-tertiary)}
.sift-bt-run{color:var(--color-text-info)}
.sift-bt-done{color:var(--color-text-success)}
.sift-bt-fail{color:var(--color-text-warning)}
.sift-bt-spin{width:10px;height:10px;border:2px solid var(--color-border-secondary);border-top-color:var(--color-text-info);border-radius:50%;animation:sift-bt-rot .7s linear infinite}
@keyframes sift-bt-rot{to{transform:rotate(360deg)}}
```

- [ ] **Step 3: Checkpoint** — `cmd /c "npx tsc --noEmit"` → exit 0.

### Task 2: Wire into the batch flow

**Files:** Modify `frontend/sift-live.ts`.

**Discovery first:** read `sift-live.ts` batch submit (`const ids = [...batchSel]; await fileBatch(ids, batchBin)`, ~444-453), `pushFileProgress` (~182), `onFileBatchDone` (~478), stop (~200-217). Find the variable holding the latest `QueueItem[]` snapshot (for names). Confirm `batchSel` iteration order == submitted order.

- [ ] **Step 1: Imports + module fields**
```ts
import { startBatchTracklist, updateBatchTracklist, finishBatchTracklist, clearBatchTracklist } from "./batch-tracklist";
```
Near `const batchSel` (~59):
```ts
let batchTrackIds: number[] = [];
let lastProgressDone = 0;
```

- [ ] **Step 2: At submit, mount the list** (just before `await fileBatch(ids, batchBin)`):
```ts
batchTrackIds = ids;
lastProgressDone = 0;
const nameOf = (id: number): string => {
  const it = LATEST_ITEMS.find((q) => q.id === id); // replace LATEST_ITEMS with the real snapshot var
  return (it && ([it.artist, it.title].filter(Boolean).join(" — ") || it.filename || it.path)) || `#${id}`;
};
startBatchTracklist(ensureBatchTracklistHost(), ids.map((id) => ({ id, name: nameOf(id) })));
```

- [ ] **Step 3: Drive updates**

In `pushFileProgress`, in the running branch after `setTask("file", …)`:
```ts
lastProgressDone = p.done;
updateBatchTracklist(p.done);
```
In `onFileBatchDone`, after the general `file` task update:
```ts
const processed = res.cancelled ? batchTrackIds.slice(0, lastProgressDone) : batchTrackIds;
const failed = new Set(res.needs_validation);
finishBatchTracklist(processed.filter((id) => !failed.has(id)), res.needs_validation);
```
Extend the existing `fileClearTimer` callback (the one that calls `clearTask("file")`) to also call `clearBatchTracklist()`.

- [ ] **Step 4: Host container** (sibling under the progress zone):
```ts
function ensureBatchTracklistHost(): HTMLElement {
  let el = document.getElementById("sift-batch-tracks");
  if (!el) {
    el = document.createElement("div");
    el.id = "sift-batch-tracks";
    const zone = document.querySelector(".sift-progress-zone");
    zone?.parentElement?.insertBefore(el, zone.nextSibling);
  }
  return el;
}
```

- [ ] **Step 5: Checkpoint** — `cmd /c "npx tsc --noEmit"` → exit 0. LIVE TEST a/b/c/d.

---

# PHASE 2 — VOLET 2: ACTIONS JOURNAL

### Task 3: Migration v8 — `session_id`

**Files:** Modify `src-tauri/src/db.rs`.

- [ ] **Step 1: Append v8** after the v7 entry in `MIGRATIONS`:
```rust
    // v8 — session journal: stamp each action with the app-launch session id (NULL for rows
    // written before this migration → grouped under "Antérieur" in the extended journal).
    r#"
    ALTER TABLE actions ADD COLUMN session_id TEXT;
    "#,
```

- [ ] **Step 2: Test** (mirror `actions_has_v7_meta_column`):
```rust
    #[test]
    fn actions_has_v8_session_column() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let cols: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('actions')").unwrap()
            .query_map([], |r| r.get::<_, String>(0)).unwrap()
            .map(|r| r.unwrap()).collect();
        assert!(cols.contains(&"session_id".to_string()), "actions missing column session_id");
    }
```

### Task 4: Stamp `session_id` on every action

**Files:** Modify `src-tauri/src/actions.rs`, `src-tauri/src/lib.rs`.

- [ ] **Step 1: Global in `actions.rs`** (top):
```rust
use std::sync::OnceLock;
static SESSION_ID: OnceLock<String> = OnceLock::new();
/// Set once at launch (lib.rs `run`). Stamped on every journaled action. Tests never set it → None.
pub fn set_session_id(id: String) { let _ = SESSION_ID.set(id); }
fn session_id() -> Option<&'static str> { SESSION_ID.get().map(|s| s.as_str()) }
```

- [ ] **Step 2: `record_with_meta` INSERT** → add the column:
```rust
    conn.execute(
        "INSERT INTO actions(track_id, type, from_path, to_path, batch_id, meta, session_id)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![track_id, kind, from_path, to_path, batch_id, meta, session_id()],
    )?;
```

- [ ] **Step 3: Set it in `lib.rs` `run()`** (before `generate_handler!`):
```rust
    let session = format!("s{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0));
    crate::actions::set_session_id(session);
```

### Task 5: Extended journal query + IPC

**Files:** Modify `actions.rs`, `ipc_filing.rs`, `lib.rs`, `shared/contracts.ts`, `frontend/ipc.ts`.

- [ ] **Step 1: `JournalRow` + `list_journal_full` in `actions.rs`** (after `list_journal`):
```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JournalRow {
    pub session_id: Option<String>,
    pub batch_id: Option<String>,
    pub track_id: Option<i64>,
    pub kind: String,
    pub to_path: Option<String>,
    pub ts: String,
}
pub fn list_journal_full(conn: &Connection, limit: i64) -> Vec<JournalRow> {
    let mut stmt = match conn.prepare(
        "SELECT session_id, batch_id, track_id, type, to_path, ts FROM actions
         WHERE undone=0 AND batch_id IS NOT NULL ORDER BY id DESC LIMIT ?1",
    ) { Ok(s) => s, Err(_) => return Vec::new() };
    let rows = stmt.query_map(params![limit], |r| Ok(JournalRow {
        session_id: r.get(0)?, batch_id: r.get(1)?, track_id: r.get(2)?,
        kind: r.get(3)?, to_path: r.get(4)?, ts: r.get(5)?,
    }));
    match rows { Ok(it) => it.filter_map(|r| r.ok()).collect(), Err(_) => Vec::new() }
}
```

- [ ] **Step 2: `journal_full` in `ipc_filing.rs`** (next to `list_journal`):
```rust
#[tauri::command]
pub fn journal_full(conn: State<'_, Mutex<Connection>>, limit: i64) -> Result<Vec<actions::JournalRow>, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    Ok(actions::list_journal_full(&conn, limit))
}
```

- [ ] **Step 3: Register** `ipc_filing::journal_full` in `lib.rs` `generate_handler!`.

- [ ] **Step 4: Contracts + binding.** `shared/contracts.ts`:
```ts
export interface JournalRow {
  session_id: string | null;
  batch_id: string | null;
  track_id: number | null;
  kind: "convert" | "move" | "trash" | "reject" | "tag_edit";
  to_path: string | null;
  ts: string;
}
```
`frontend/ipc.ts` (import `JournalRow`, add):
```ts
export const journalFull = (limit = 500): Promise<JournalRow[]> => invoke("journal_full", { limit });
```

### Task 6: Journal view (session tab + extended tree + revert routing)

**Files:** Create `frontend/journal-view.ts`; Modify `frontend/styles.css`, `frontend/sift-live.ts`.

**Discovery first:** find the view/tab registry (search `renderEcartes` in `sift-live.ts`, the rail nav `data-view` switching) to add a "Journal" entry the same way.

- [ ] **Step 1: `confirmAction` + `revertEntry` router** — `frontend/journal-view.ts`:
```ts
import { journal, journalFull, revertBatch, restoreTrack, requeueTrack } from "./ipc";
import type { JournalEntry, JournalRow } from "../shared/contracts";

function confirmAction(message: string): Promise<boolean> {
  return new Promise((resolve) => {
    const back = document.createElement("div");
    back.className = "sift-confirm-back";
    back.innerHTML =
      `<div class="sift-confirm"><div class="sift-confirm-msg"></div>` +
      `<div class="sift-confirm-row"><button data-c="no">Annuler</button>` +
      `<button data-c="yes" class="sift-confirm-yes">Confirmer</button></div></div>`;
    back.querySelector<HTMLElement>(".sift-confirm-msg")!.textContent = message;
    const close = (v: boolean) => { back.remove(); resolve(v); };
    back.querySelector('[data-c="no"]')!.addEventListener("click", () => close(false));
    back.querySelector('[data-c="yes"]')!.addEventListener("click", () => close(true));
    back.addEventListener("click", (e) => { if (e.target === back) close(false); });
    document.body.appendChild(back);
  });
}

type Cat = "filed" | "trashed" | "rejected";
const CAT_OF: Record<string, Cat | undefined> = {
  convert: "filed", move: "filed", trash: "trashed", reject: "rejected",
  // tag_edit = standalone "Apply ID3 tags" — not a file move; intentionally excluded from these
  // categories (revertable from the editor's Apply button instead).
};
const CAT_LABEL: Record<Cat, string> = { filed: "Filés", trashed: "Jetés", rejected: "Rejetés" };
const CAT_VERB: Record<Cat, string> = { filed: "défiler", trashed: "restaurer", rejected: "remettre en file" };

async function revertEntry(e: { kind: string; batch_id: string | null; track_id: number | null }): Promise<void> {
  switch (e.kind) {
    case "convert": case "move":
      if (!e.batch_id) throw new Error("filed action without batch_id");
      return revertBatch(e.batch_id);
    case "trash":
      if (e.track_id == null) throw new Error("trash action without track_id");
      return restoreTrack(e.track_id);
    case "reject":
      if (e.track_id == null) throw new Error("reject action without track_id");
      return requeueTrack(e.track_id);
    default: throw new Error(`journal revert: unhandled kind ${e.kind}`);
  }
}
function baseName(p: string): string { return p.split(/[\\/]/).pop() || p; }
```

- [ ] **Step 2: `renderJournal(host)` (session tab)** — grouped, per-line (no confirm), category "tout" (visible-only + confirm), last-batch (confirm if >10):
```ts
export async function renderJournal(host: HTMLElement): Promise<void> {
  const entries = await journal(50);
  const groups: Record<Cat, JournalEntry[]> = { filed: [], trashed: [], rejected: [] };
  for (const e of entries) { const c = CAT_OF[e.kind]; if (c) groups[c].push(e); }

  host.innerHTML =
    `<div class="sift-jn-top"><h2 class="sift-jn-h">Journal de session</h2>` +
    `<button data-jn="extended" class="sift-jn-link">Historique complet →</button></div>` +
    `<button data-jn="lastbatch" class="sift-jn-last">Annuler le dernier batch</button>` +
    `<div class="sift-jn-groups"></div>`;
  const groupsEl = host.querySelector<HTMLElement>(".sift-jn-groups")!;

  (Object.keys(groups) as Cat[]).forEach((cat) => {
    const list = groups[cat]; if (!list.length) return;
    const sec = document.createElement("section"); sec.className = "sift-jn-cat";
    sec.innerHTML =
      `<div class="sift-jn-cathead"><button class="sift-jn-toggle" aria-expanded="true">▾ ${CAT_LABEL[cat]} (${list.length})</button>` +
      `<button class="sift-jn-all">Tout ${CAT_VERB[cat]} (${list.length})</button></div><div class="sift-jn-rows"></div>`;
    const rowsEl = sec.querySelector<HTMLElement>(".sift-jn-rows")!;
    for (const e of list) {
      const row = document.createElement("div"); row.className = "sift-jn-row";
      row.innerHTML = `<span class="sift-jn-name"></span><button class="sift-jn-revert" title="${CAT_VERB[cat]}"><i class="ti ti-arrow-back-up"></i></button>`;
      row.querySelector<HTMLElement>(".sift-jn-name")!.textContent = e.to_path ? baseName(e.to_path) : `#${e.track_id ?? "?"}`;
      row.querySelector(".sift-jn-revert")!.addEventListener("click", async () => {
        try { await revertEntry(e); await renderJournal(host); } catch (err) { console.error("journal revert failed", err); }
      });
      rowsEl.appendChild(row);
    }
    sec.querySelector(".sift-jn-toggle")!.addEventListener("click", (ev) => {
      const b = ev.currentTarget as HTMLElement; const open = b.getAttribute("aria-expanded") === "true";
      b.setAttribute("aria-expanded", String(!open)); rowsEl.style.display = open ? "none" : "";
      b.textContent = `${open ? "▸" : "▾"} ${CAT_LABEL[cat]} (${list.length})`;
    });
    sec.querySelector(".sift-jn-all")!.addEventListener("click", async () => {
      if (!(await confirmAction(`${CAT_VERB[cat]} les ${list.length} morceaux affichés ?`))) return;
      for (const e of list) { try { await revertEntry(e); } catch (err) { console.error("mass revert failed", err); } }
      await renderJournal(host);
    });
    groupsEl.appendChild(sec);
  });

  host.querySelector('[data-jn="lastbatch"]')!.addEventListener("click", async () => {
    const newest = entries[0]; if (!newest) return;
    const sameBatch = entries.filter((e) => e.batch_id === newest.batch_id);
    if (sameBatch.length > 10 && !(await confirmAction(`Annuler le dernier batch (${sameBatch.length} morceaux) ?`))) return;
    try { await revertEntry(newest); await renderJournal(host); } catch (err) { console.error("last-batch revert failed", err); }
  });
  host.querySelector('[data-jn="extended"]')!.addEventListener("click", () => void renderJournalExtended(host));
}
```
NOTE: `journal(limit)` returns one entry per batch, so a category's visible rows == its batches; the "tout" count == visible rows → never touches off-screen actions.

- [ ] **Step 3: `renderJournalExtended(host)` (tree)** — Session → Batch → tracks; NULL session → "Antérieur":
```ts
export async function renderJournalExtended(host: HTMLElement): Promise<void> {
  const rows = await journalFull(500);
  const bySession = new Map<string, JournalRow[]>();
  for (const r of rows) { const k = r.session_id ?? "Antérieur"; if (!bySession.has(k)) bySession.set(k, []); bySession.get(k)!.push(r); }
  host.innerHTML = `<div class="sift-jn-top"><button data-jn="back" class="sift-jn-link">← Journal de session</button><h2 class="sift-jn-h">Historique complet</h2></div><div class="sift-jn-tree"></div>`;
  const tree = host.querySelector<HTMLElement>(".sift-jn-tree")!;
  for (const [session, srows] of bySession) {
    const batches = new Map<string, JournalRow[]>();
    for (const r of srows) { const b = r.batch_id ?? "—"; if (!batches.has(b)) batches.set(b, []); batches.get(b)!.push(r); }
    const sec = document.createElement("section"); sec.className = "sift-jn-session";
    sec.innerHTML = `<div class="sift-jn-sesshead">${session === "Antérieur" ? "Antérieur" : "Session"}</div>`;
    for (const [, brows] of batches) {
      const latest = brows[0];
      const wrap = document.createElement("div"); wrap.className = "sift-jn-batch";
      wrap.innerHTML = `<div class="sift-jn-batchhead">${CAT_LABEL[CAT_OF[latest.kind] ?? "filed"] ?? latest.kind} · ${brows.length}</div>`;
      for (const r of brows) {
        const row = document.createElement("div"); row.className = "sift-jn-row";
        row.innerHTML = `<span class="sift-jn-name"></span><button class="sift-jn-revert"><i class="ti ti-arrow-back-up"></i></button>`;
        row.querySelector<HTMLElement>(".sift-jn-name")!.textContent = r.to_path ? baseName(r.to_path) : `#${r.track_id ?? "?"}`;
        row.querySelector(".sift-jn-revert")!.addEventListener("click", async () => {
          try { await revertEntry(r); await renderJournalExtended(host); } catch (err) { console.error(err); }
        });
        wrap.appendChild(row);
      }
      sec.appendChild(wrap);
    }
    tree.appendChild(sec);
  }
  host.querySelector('[data-jn="back"]')!.addEventListener("click", () => void renderJournal(host));
}
```

- [ ] **Step 4: Styles** — append `.sift-jn-*` and `.sift-confirm-*` to `styles.css`, tokens only (group heads `--color-text-tertiary`; `Tout`/`yes` button on `--color-background-danger`/`--color-text-danger`; modal scrim `rgba(0,0,0,.5)` overlay is acceptable). Mirror existing `.sift-cands`/`.sift-genre-chip` spacing conventions.

- [ ] **Step 5: Register the Journal view** in `sift-live.ts` mirroring Écartés (nav entry + `void renderJournal(viewHost)` on switch). Écartés stays UNCHANGED.

- [ ] **Step 6: Checkpoint** — `cmd /c "npx tsc --noEmit"` → exit 0. ⚙️ REBUILD BACKEND (migration + new command). LIVE TEST e/f/g/h/i/j.

---

## Self-Review

**Spec coverage:** VOLET 1 3-state/no-event/no-%/placement/auto-hide → Tasks 1-2; stop & single-track edges → Task 2 Step 3. VOLET 2 (a) session tab → Task 6.2; (b) extended tree → 6.3; (c) migration+launch+stamp+return+"Antérieur" → Tasks 3-5; (d) three scopes + route-by-kind → Task 6.1-6.3; (e) Écartés untouched → 6.5. Guardrails (confirm on mass only, tokens, idempotent migration, fail-fast routing, no commits) honored.

**Open discovery items the worker MUST resolve (not guess):**
1. The latest-queue-snapshot variable name in `sift-live.ts` (for `nameOf`; placeholder `LATEST_ITEMS`).
2. Exact running-branch of `pushFileProgress` for `lastProgressDone`/`updateBatchTracklist`.
3. The view/tab registry location for the "Journal" tab.
4. Confirm `BatchResult` exposes no `filed_ids` (front derives them as specified).
5. Confirm `actions` import path in `lib.rs` for `set_session_id`.

**Type consistency:** Rust `JournalRow` ↔ TS `JournalRow` fields match; `revertEntry` takes the `{kind, batch_id, track_id}` subset common to `JournalEntry` and `JournalRow`. `CAT_OF/CAT_LABEL/CAT_VERB` keyed consistently.
