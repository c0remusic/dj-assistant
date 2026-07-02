# Journal d'Actions — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a persistent Journal tab showing the current session's filing/trash/reject actions, with 3 revert scopes (per-track, per-category, last batch) and an extended page that trees all sessions.

**Architecture:** v8 DB migration adds `session_id TEXT` to `actions`; the session ID is generated at app launch and written into the `settings` table (no new managed-state struct, no ripple through call chains — the INSERT reads it via SQL subquery). `JournalEntry` gains `from_path`, `session_id`, and `track_count` fields; `list_journal` gains an optional session filter. A new `frontend/journal.ts` module owns the view and the extended page; `app.js` gets a `renderJournal` stub; `sift-live.ts` registers `window.__siftJournal`.

**Tech Stack:** Rust/rusqlite (SQLite subquery for session_id injection), Tauri IPC, Vite vanilla TypeScript, tokens-only CSS (no new deps).

**Constraints (CLAUDE.md):**
- FRONT PUR until Task 3 (Tasks 1–3 are Rust). After Task 3: NE PAS COMMITER until Antoine tests.
- Tokens only. Fail-fast. No fallback silencieux.
- L'onglet Jetés existant est inchangé.
- Confirmations uniquement sur les actions de masse, jamais sur ↩ par track.
- Bloc ⚙️ REBUILD BACKEND à la fin du Task 3 (les Tasks 1–3 touchent src-tauri/).

---

## File Map

| File | Action | What changes |
|------|--------|-------------|
| `src-tauri/src/db.rs` | Modify | v8 migration: `session_id TEXT` on `actions` |
| `src-tauri/src/settings.rs` | Modify | Add `CURRENT_SESSION_ID` constant |
| `src-tauri/src/lib.rs` | Modify | Generate + write session_id at startup; register `get_session_id` IPC |
| `src-tauri/src/actions.rs` | Modify | `JournalEntry` +3 fields; `record_with_meta` SQL subquery; `list_journal` signature; test updates |
| `src-tauri/src/ipc_filing.rs` | Modify | `list_journal` handler gains `session_id` param; new `get_session_id` handler |
| `shared/contracts.ts` | Modify | `JournalEntry` +3 fields |
| `frontend/ipc.ts` | Modify | `listJournal` + optional `sessionId`; add `getSessionId` |
| `index.html` | Modify | Add Journal nav tab |
| `frontend/app.js` | Modify | Add `renderJournal` stub + route in render() |
| `frontend/journal.ts` | Create | Current-session view + extended page (all logic) |
| `frontend/sift-live.ts` | Modify | Import + register `window.__siftJournal` |
| `frontend/styles.css` | Modify | Journal-specific CSS classes |

---

## Task 1 — v8 Migration + CURRENT_SESSION_ID constant

**Files:**
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/settings.rs`

- [ ] **Step 1: Add v8 migration to MIGRATIONS array in db.rs**

  After the existing `// v7 —` block (ends at line 109), append this entry to the `MIGRATIONS` array. The array currently has 7 entries; this becomes entry index 7 (v8).

  ```rust
  // v8 — Journal session grouping: tag each new action with the app session that produced it.
  // Actions from before this migration keep session_id = NULL → front shows them under "Antérieur".
  r#"
  ALTER TABLE actions ADD COLUMN session_id TEXT;
  "#,
  ```

- [ ] **Step 2: Add the settings key constant in settings.rs**

  After the `DISCOGS_TOKEN` constant (line 14), add:

  ```rust
  /// Key under which the current session's unique ID is stored at app launch.
  /// Written once at startup; read by the `actions` INSERT via SQL subquery.
  pub const CURRENT_SESSION_ID: &str = "current_session_id";
  ```

- [ ] **Step 3: Add a DB test for the v8 column**

  In `db.rs`, after the `actions_has_v7_meta_column` test, add:

  ```rust
  #[test]
  fn actions_has_v8_session_id_column() {
      let conn = Connection::open_in_memory().unwrap();
      run_migrations(&conn).unwrap();
      let acols: Vec<String> = conn
          .prepare("SELECT name FROM pragma_table_info('actions')")
          .unwrap()
          .query_map([], |r| r.get::<_, String>(0))
          .unwrap()
          .map(|r| r.unwrap())
          .collect();
      assert!(acols.contains(&"session_id".to_string()), "actions missing column session_id");
  }
  ```

- [ ] **Step 4: Run DB tests**

  ```
  cargo test --manifest-path src-tauri/Cargo.toml -p sift_lib db::tests
  ```

  Expected: all db tests pass. (`migrations_create_all_tables` still asserts 7 tables — v8 adds a column only, not a table.)

---

## Task 2 — actions.rs: JournalEntry + SQL changes

**Files:**
- Modify: `src-tauri/src/actions.rs`

- [ ] **Step 1: Extend JournalEntry with three new fields**

  Replace the existing `JournalEntry` struct (around line 229):

  ```rust
  /// One entry of the consultable journal: a live batch, summarized by its FIRST action.
  /// `track_count` = number of distinct tracks in the batch (used by the front to gate
  /// "last batch" confirmation on > 10 tracks). `session_id` = NULL for pre-migration rows.
  #[derive(Debug, Clone, PartialEq, Serialize)]
  pub struct JournalEntry {
      pub batch_id: String,
      pub track_id: Option<i64>,
      /// The batch's FIRST action type (convert|move|trash|reject) — determines the display
      /// category. MIN instead of MAX so a convert+trash filing shows as "convert", not "trash".
      pub kind: String,
      pub from_path: Option<String>,
      pub to_path: Option<String>,
      pub ts: String,
      pub session_id: Option<String>,
      pub track_count: i64,
  }
  ```

- [ ] **Step 2: Inject session_id in record_with_meta via SQL subquery**

  In `record_with_meta` (around line 61), change the INSERT SQL from:

  ```rust
  conn.execute(
      "INSERT INTO actions(track_id, type, from_path, to_path, batch_id, meta)
       VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
      params![track_id, kind, from_path, to_path, batch_id, meta],
  )?;
  ```

  To:

  ```rust
  conn.execute(
      "INSERT INTO actions(track_id, type, from_path, to_path, batch_id, meta, session_id)
       VALUES(?1, ?2, ?3, ?4, ?5, ?6,
              (SELECT value FROM settings WHERE key='current_session_id'))",
      params![track_id, kind, from_path, to_path, batch_id, meta],
  )?;
  ```

  No parameter count change — the session_id is pulled from the settings table directly in SQL.

- [ ] **Step 3: Update list_journal signature and SQL**

  Replace the entire `list_journal` function (from line ~241 to ~268):

  ```rust
  /// Recent live (not-yet-undone) batches, newest first, one entry per batch (summarized by
  /// the batch's FIRST action row — MIN id — so a convert+trash filing shows kind="convert").
  /// `session_id_filter` = Some(sid) to restrict to one session; None = all sessions.
  /// `tag_edit` batches are excluded (they have no category in the Journal view).
  pub fn list_journal(conn: &Connection, limit: i64, session_id_filter: Option<&str>) -> Vec<JournalEntry> {
      let mut stmt = match conn.prepare(
          "SELECT a.batch_id, a.track_id, a.type, a.from_path, a.to_path, a.ts,
                  a.session_id, g.cnt
           FROM actions a
           JOIN (
               SELECT batch_id, MIN(id) AS mid, count(DISTINCT track_id) AS cnt
               FROM actions
               WHERE undone=0 AND batch_id IS NOT NULL AND type NOT IN ('tag_edit')
               GROUP BY batch_id
           ) g ON a.id = g.mid
           WHERE (?2 IS NULL OR a.session_id = ?2)
           ORDER BY a.id DESC
           LIMIT ?1",
      ) {
          Ok(s) => s,
          Err(_) => return Vec::new(),
      };
      let rows = stmt.query_map(params![limit, session_id_filter], |r| {
          Ok(JournalEntry {
              batch_id: r.get(0)?,
              track_id: r.get(1)?,
              kind: r.get(2)?,
              from_path: r.get(3)?,
              to_path: r.get(4)?,
              ts: r.get(5)?,
              session_id: r.get(6)?,
              track_count: r.get(7)?,
          })
      });
      match rows {
          Ok(it) => it.filter_map(|r| r.ok()).collect(),
          Err(_) => Vec::new(),
      }
  }
  ```

- [ ] **Step 4: Update tests that call list_journal and check kind**

  In the test `journal_lists_batches_newest_first` (around line 624), change:

  ```rust
  let entries = list_journal(&conn, 10);
  ```
  to:
  ```rust
  let entries = list_journal(&conn, 10, None);
  ```

  And change the kind assertion from:
  ```rust
  assert_eq!(entries[0].kind, "move"); // representative (latest) action of the batch
  ```
  to:
  ```rust
  assert_eq!(entries[0].kind, "convert"); // representative (first) action of the batch
  ```

- [ ] **Step 5: Run actions tests**

  ```
  cargo test --manifest-path src-tauri/Cargo.toml -p sift_lib actions::tests
  ```

  Expected: all actions tests pass.

---

## Task 3 — ipc_filing.rs + lib.rs startup

**Files:**
- Modify: `src-tauri/src/ipc_filing.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Update list_journal IPC handler in ipc_filing.rs**

  Replace the existing `list_journal` handler (lines ~490–496):

  ```rust
  /// Recent live (not-yet-undone) batches, newest first. `session_id` = None → all sessions
  /// (used by the extended journal page); Some(sid) → current session only (Journal tab).
  #[tauri::command]
  pub fn list_journal(
      conn: State<'_, Mutex<Connection>>,
      limit: i64,
      session_id: Option<String>,
  ) -> Result<Vec<JournalEntry>, String> {
      let conn = conn.lock().map_err(|e| e.to_string())?;
      Ok(actions::list_journal(&conn, limit, session_id.as_deref()))
  }
  ```

- [ ] **Step 2: Add get_session_id IPC handler in ipc_filing.rs**

  After the `list_journal` handler, add:

  ```rust
  /// The current app session ID (generated at launch, persisted in settings). Used by the
  /// Journal tab front to filter list_journal to the current session only.
  #[tauri::command]
  pub fn get_session_id(conn: State<'_, Mutex<Connection>>) -> Result<String, String> {
      let conn = conn.lock().map_err(|e| e.to_string())?;
      settings::get(&conn, settings::CURRENT_SESSION_ID)
          .map_err(|e| e.to_string())?
          .ok_or_else(|| "no session_id in settings".to_string())
  }
  ```

- [ ] **Step 3: Generate + write session_id at app startup in lib.rs**

  In `lib.rs`, inside the `setup` closure, after `let conn = db::open(...)` and before `app.manage(Mutex::new(conn))`, add:

  ```rust
  let session_id = format!(
      "{}-{}",
      std::time::SystemTime::now()
          .duration_since(std::time::UNIX_EPOCH)
          .unwrap_or_default()
          .as_millis(),
      std::process::id()
  );
  settings::set(&conn, settings::CURRENT_SESSION_ID, &session_id)
      .expect("session_id write failed");
  ```

- [ ] **Step 4: Register get_session_id in the invoke_handler in lib.rs**

  In the `tauri::generate_handler!` macro call, after `ipc_filing::list_journal,`, add:

  ```rust
  ipc_filing::get_session_id,
  ```

- [ ] **Step 5: Verify full Rust build + tests**

  ```
  cargo test --manifest-path src-tauri/Cargo.toml
  ```

  Expected: all tests pass; no compile errors.

---

⚙️ REBUILD BACKEND

Tasks 1–3 modified `src-tauri/`. Build before testing frontend changes:

```
# In the project root (dj-assistant-m6a):
# 1. Stop tauri dev if running (Ctrl+C in the terminal).
# 2. Restart:
npm run tauri dev
```

If you get LNK1120 (Windows linker error): `cargo clean --manifest-path src-tauri/Cargo.toml` then restart.

---

## Task 4 — Contracts + IPC frontend wiring

**Files:**
- Modify: `shared/contracts.ts`
- Modify: `frontend/ipc.ts`

- [ ] **Step 1: Extend JournalEntry in shared/contracts.ts**

  Replace the existing `JournalEntry` interface (around lines 157–164):

  ```typescript
  /** One consultable undo-journal entry (a live batch, summarized by its first action). */
  export interface JournalEntry {
    batch_id: string;
    track_id: number | null;
    /** First action type of the batch — determines display category.
     *  "convert"|"move" → Filés; "trash" → Jetés; "reject" → Rejetés. */
    kind: "convert" | "move" | "trash" | "reject";
    from_path: string | null;
    to_path: string | null;
    ts: string;
    session_id: string | null;
    /** Distinct track count in the batch — used to gate the last-batch confirmation on > 10. */
    track_count: number;
  }
  ```

- [ ] **Step 2: Update listJournal and add getSessionId in ipc.ts**

  Replace the existing `listJournal` export (around line 150):
  ```typescript
  export const listJournal = (limit = 20): Promise<JournalEntry[]> =>
    invoke("list_journal", { limit });
  ```
  With:
  ```typescript
  /** Recent live batches, newest first. `sessionId` = current session → Journal tab;
   *  omit (undefined) → all sessions → extended journal page. */
  export const listJournal = (limit = 50, sessionId?: string): Promise<JournalEntry[]> =>
    invoke("list_journal", { limit, sessionId: sessionId ?? null });
  ```

  Then add immediately after:
  ```typescript
  /** The session ID generated at this app launch (from settings). Used to filter
   *  list_journal to the current session in the Journal tab. */
  export const getSessionId = (): Promise<string> => invoke("get_session_id");
  ```

- [ ] **Step 3: Type-check**

  ```
  npx tsc --noEmit
  ```

  Expected: 0 errors.

---

## Task 5 — Nav tab + app.js stub

**Files:**
- Modify: `index.html`
- Modify: `frontend/app.js`

- [ ] **Step 1: Add Journal nav tab in index.html**

  Between the `ecarts` and `biblio` nav items, insert:

  ```html
  <div class="nv" data-view="journal" title="Action journal"><i class="ti ti-history" aria-hidden="true"></i><span>Journal</span></div>
  ```

- [ ] **Step 2: Add renderJournal function in app.js**

  Find `function renderEcarts(){` (around line 230) and add immediately BEFORE it:

  ```javascript
  function renderJournal(){block();content.innerHTML='';if(window.__siftJournal)window.__siftJournal();}
  ```

- [ ] **Step 3: Add journal case to render() in app.js**

  Find line 36 of app.js. It ends with:
  ```
  …if(view==="ecarts")return renderEcarts();return renderReglages();
  ```

  Change that suffix to:
  ```
  …if(view==="ecarts")return renderEcarts();if(view==="journal")return renderJournal();return renderReglages();
  ```

---

## Task 6 — journal.ts: current-session view + per-track revert + category + last-batch reverts

**Files:**
- Create: `frontend/journal.ts`

- [ ] **Step 1: Create frontend/journal.ts**

  ```typescript
  // Journal d'actions post-batch. Vue permanente (onglet Journal) montrant la session
  // courante, catégories repliables, revert à 3 portées. Page étendue = arbre SESSION→BATCH.
  // BURST RULE (CLAUDE.md) : les clics ↩ sont délégués — pas d'addEventListener par ligne.
  import { requireEl } from "./dom";
  import { listJournal, getSessionId, revertBatch, restoreTrack, requeueTrack } from "./ipc";
  import type { JournalEntry } from "../shared/contracts";

  // ── State ──────────────────────────────────────────────────────────────────────────────
  let currentSessionId: string | null = null;
  // Flat list of displayed entries; index matches data-jrnl-idx on DOM rows.
  let _entries: JournalEntry[] = [];

  // ── Helpers ────────────────────────────────────────────────────────────────────────────

  /** Extract the bare filename from an absolute path (handles / and \ separators). */
  function basename(p: string): string {
    return p.replace(/^.*[\\/]/, "");
  }

  /** Human-readable display name for a journal row. */
  function rowName(e: JournalEntry): string {
    const p = e.kind === "reject" ? e.from_path : e.to_path;
    return p ? basename(p) : `track #${e.track_id ?? "?"}`;
  }

  /** Short destination label shown muted next to the name. */
  function rowDest(e: JournalEntry): string {
    if (e.kind === "reject") return "Rejeté";
    if (e.kind === "trash") return "Corbeille";
    if (e.to_path) {
      const parts = e.to_path.replace(/\\/g, "/").split("/");
      return parts.length > 1 ? parts[parts.length - 2] : parts[0];
    }
    return "";
  }

  /** IPC call for a single-row revert; routed by kind. No confirmation. */
  async function revertRow(e: JournalEntry): Promise<void> {
    if (e.kind === "convert" || e.kind === "move") {
      await revertBatch(e.batch_id);
    } else if (e.kind === "trash") {
      if (e.track_id == null) throw new Error("trash row missing track_id");
      await restoreTrack(e.track_id);
    } else if (e.kind === "reject") {
      if (e.track_id == null) throw new Error("reject row missing track_id");
      await requeueTrack(e.track_id);
    }
  }

  // ── HTML builders ──────────────────────────────────────────────────────────────────────

  const esc = (s: string) =>
    s.replace(/[&<>"']/g, (c) => ({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#39;"}[c]!));

  function rowHtml(e: JournalEntry, idx: number): string {
    const label = e.kind === "convert" || e.kind === "move" ? "Défiler"
                : e.kind === "trash"                         ? "Restaurer"
                :                                              "Re-sourcer";
    return (
      `<div class="jrnl-row">` +
      `<span class="jrnl-row-name" title="${esc(rowName(e))}">${esc(rowName(e))}</span>` +
      `<span class="jrnl-row-dest">${esc(rowDest(e))}</span>` +
      `<button class="jrnl-act" data-jrnl-revert="${idx}" title="${label}">↩</button>` +
      `</div>`
    );
  }

  interface Category {
    id: "filed" | "trash" | "reject";
    label: string;
    entries: JournalEntry[];
    allLabel: string;
    allConfirmTpl: string; // "{n}" replaced with count
  }

  function buildCategories(entries: JournalEntry[]): Category[] {
    const filed  = entries.filter((e) => e.kind === "convert" || e.kind === "move");
    const trash  = entries.filter((e) => e.kind === "trash");
    const reject = entries.filter((e) => e.kind === "reject");
    return [
      { id: "filed",  label: "Filés",   entries: filed,  allLabel: "Tout défiler",   allConfirmTpl: "Défiler les {n} morceaux affichés ?" },
      { id: "trash",  label: "Jetés",   entries: trash,  allLabel: "Tout restaurer",  allConfirmTpl: "Restaurer les {n} morceaux affichés ?" },
      { id: "reject", label: "Rejetés", entries: reject, allLabel: "Tout re-sourcer", allConfirmTpl: "Re-sourcer les {n} morceaux affichés ?" },
    ];
  }

  function categoryHtml(cat: Category, offset: number): string {
    if (!cat.entries.length) return "";
    const rows = cat.entries.map((e, i) => rowHtml(e, offset + i)).join("");
    const allBtn = `<button class="jrnl-cat-all" data-jrnl-cat="${cat.id}">${cat.allLabel} (${cat.entries.length})</button>`;
    return (
      `<div class="jrnl-cat">` +
      `<div class="jrnl-cat-head">` +
      `<span class="col-h jrnl-cat-label">${cat.label}</span>` +
      `<div class="jrnl-all">${allBtn}</div>` +
      `</div>` +
      `<div class="jrnl-cat-rows">${rows}</div>` +
      `</div>`
    );
  }

  // ── Main render ────────────────────────────────────────────────────────────────────────

  export async function renderJournal(): Promise<void> {
    const content = requireEl("#content", "renderJournal");
    content.style.display = "block";
    content.style.padding = "14px 18px";
    content.style.overflowY = "auto";

    try {
      if (!currentSessionId) currentSessionId = await getSessionId();
      _entries = await listJournal(50, currentSessionId);
    } catch (err) {
      content.innerHTML = `<div class="jrnl-err">Impossible de charger le journal.</div>`;
      console.error("renderJournal fetch failed", err);
      return;
    }

    const cats = buildCategories(_entries);
    let offset = 0;
    const catHtmls: string[] = [];
    for (const cat of cats) {
      catHtmls.push(categoryHtml(cat, offset));
      offset += cat.entries.length;
    }

    const hasAny = _entries.length > 0;
    const lastBatchBtn = hasAny
      ? `<button class="jrnl-last-batch" data-jrnl-lastbatch>Annuler le dernier batch</button>`
      : "";

    content.innerHTML =
      `<div id="sift-journal-wrap">` +
      `<div class="jrnl-header">` +
      `<span class="h1">Journal</span>` +
      `<div class="jrnl-header-right">` +
      `${lastBatchBtn}` +
      `<button class="jrnl-see-all" data-jrnl-seeall>Voir tout</button>` +
      `</div>` +
      `</div>` +
      (hasAny ? catHtmls.join("") : `<div class="jrnl-empty">Aucune action dans cette session.</div>`) +
      `</div>`;

    installDelegate(content);
  }

  // ── Delegated click handler ────────────────────────────────────────────────────────────
  // Installed ONCE per renderJournal/renderJournalExtended. User-initiated click only —
  // not a burst event. Reads _entries by index so no DOM rebuild occurs during the handler.

  function installDelegate(el: HTMLElement): void {
    // Replace to avoid stacking listeners across re-renders.
    el.removeEventListener("click", journalClick);
    el.addEventListener("click", journalClick);
  }

  function journalClick(e: Event): void {
    const t = e.target as HTMLElement;

    // Per-track revert (↩ button) — no confirmation
    const revertBtn = t.closest<HTMLButtonElement>("[data-jrnl-revert]");
    if (revertBtn) {
      const idx = Number(revertBtn.dataset.jrnlRevert);
      const entry = _entries[idx];
      if (!entry) return;
      revertBtn.disabled = true;
      void revertRow(entry)
        .then(() => renderJournal())
        .catch((err) => {
          revertBtn.disabled = false;
          console.error("revert failed", err);
          alert(`Échec du revert : ${String(err)}`);
        });
      return;
    }

    // Per-category revert — with confirmation
    const catBtn = t.closest<HTMLButtonElement>("[data-jrnl-cat]");
    if (catBtn) {
      const catId = catBtn.dataset.jrnlCat as Category["id"];
      const cat = buildCategories(_entries).find((c) => c.id === catId);
      if (!cat || !cat.entries.length) return;
      if (!confirm(cat.allConfirmTpl.replace("{n}", String(cat.entries.length)))) return;
      catBtn.disabled = true;
      void revertAll(cat.entries)
        .then(() => renderJournal())
        .catch((err) => {
          catBtn.disabled = false;
          console.error("category revert failed", err);
          alert(`Échec : ${String(err)}`);
        });
      return;
    }

    // Last-batch revert — with confirmation if > 10 tracks
    const lastBtn = t.closest<HTMLButtonElement>("[data-jrnl-lastbatch]");
    if (lastBtn) {
      void handleLastBatch(lastBtn);
      return;
    }

    // Extended page
    const seeAll = t.closest<HTMLElement>("[data-jrnl-seeall]");
    if (seeAll) { void renderJournalExtended(); return; }

    // Back to current-session view
    const back = t.closest<HTMLElement>("[data-jrnl-back]");
    if (back) { void renderJournal(); return; }
  }

  async function revertAll(entries: JournalEntry[]): Promise<void> {
    for (const e of entries) await revertRow(e);
  }

  async function handleLastBatch(btn: HTMLButtonElement): Promise<void> {
    if (!_entries.length) return;
    const lastBatchId = _entries[0].batch_id;
    const batchTrackCount = _entries[0].track_count;
    const batchEntries = _entries.filter((e) => e.batch_id === lastBatchId);
    if (batchTrackCount > 10) {
      if (!confirm(`Annuler le dernier batch ? (${batchTrackCount} morceaux)`)) return;
    }
    btn.disabled = true;
    try {
      await revertAll(batchEntries);
      await renderJournal();
    } catch (err) {
      btn.disabled = false;
      console.error("last-batch revert failed", err);
      alert(`Échec : ${String(err)}`);
    }
  }

  // ── Extended journal page ──────────────────────────────────────────────────────────────

  export async function renderJournalExtended(): Promise<void> {
    const content = requireEl("#content", "renderJournalExtended");
    content.style.display = "block";
    content.style.padding = "14px 18px";
    content.style.overflowY = "auto";

    let allEntries: JournalEntry[];
    try {
      allEntries = await listJournal(500); // no session filter = all sessions
    } catch (err) {
      content.innerHTML = `<div class="jrnl-err">Impossible de charger l'historique.</div>`;
      console.error("renderJournalExtended failed", err);
      return;
    }

    // Group by session_id; null → "Antérieur"
    const sessionMap = new Map<string, JournalEntry[]>();
    for (const e of allEntries) {
      const key = e.session_id ?? "__anterior__";
      if (!sessionMap.has(key)) sessionMap.set(key, []);
      sessionMap.get(key)!.push(e);
    }

    // Current session first, then others, "Antérieur" last.
    const orderedKeys: string[] = [];
    if (currentSessionId && sessionMap.has(currentSessionId)) orderedKeys.push(currentSessionId);
    for (const k of sessionMap.keys()) {
      if (k !== currentSessionId && k !== "__anterior__") orderedKeys.push(k);
    }
    if (sessionMap.has("__anterior__")) orderedKeys.push("__anterior__");

    _entries = allEntries; // extended page reuses the same delegate
    let globalOffset = 0;

    let html =
      `<div id="sift-journal-wrap">` +
      `<div class="jrnl-header">` +
      `<button class="jrnl-back" data-jrnl-back>← Journal</button>` +
      `<span class="h1">Historique complet</span>` +
      `</div>`;

    for (const sessionKey of orderedKeys) {
      const sessEntries = sessionMap.get(sessionKey)!;
      const sessLabel =
        sessionKey === "__anterior__"   ? "Antérieur"
        : sessionKey === currentSessionId ? "Session courante"
        : `Session ${sessionKey.split("-")[0]}`;

      const batchMap = new Map<string, JournalEntry[]>();
      for (const e of sessEntries) {
        if (!batchMap.has(e.batch_id)) batchMap.set(e.batch_id, []);
        batchMap.get(e.batch_id)!.push(e);
      }

      let sessHtml = `<div class="jrnl-session">`;
      sessHtml += `<div class="col-h jrnl-session-label">${esc(sessLabel)}</div>`;
      for (const [, batchEntries] of batchMap) {
        const first = batchEntries[0];
        const batchLabel = first.ts.slice(0, 16).replace("T", " ");
        sessHtml += `<div class="jrnl-batch">`;
        sessHtml += `<div class="jrnl-batch-head">Batch <span class="jrnl-batch-ts">${esc(batchLabel)}</span></div>`;
        for (const e of batchEntries) sessHtml += rowHtml(e, globalOffset++);
        sessHtml += `</div>`;
      }
      sessHtml += `</div>`;
      html += sessHtml;
    }

    if (!allEntries.length) html += `<div class="jrnl-empty">Aucune action journalisée.</div>`;
    html += `</div>`;
    content.innerHTML = html;
    installDelegate(content);
  }
  ```

---

## Task 7 — sift-live.ts wiring

**Files:**
- Modify: `frontend/sift-live.ts`

- [ ] **Step 1: Import renderJournal**

  Near the other view imports at the top of `sift-live.ts` (e.g. near `import { renderEcartes }`), add:

  ```typescript
  import { renderJournal } from "./journal";
  ```

- [ ] **Step 2: Register window.__siftJournal**

  Where `window.__siftEcarts` and `window.__siftBiblio` are set (around lines 1036–1038), add:

  ```typescript
  window.__siftJournal = () => void renderJournal();
  ```

- [ ] **Step 3: Type declaration (if TypeScript complains)**

  If the project has a `global.d.ts` or a `declare global { interface Window {...} }` block for `__siftEcarts`/`__siftBiblio`, add:

  ```typescript
  __siftJournal?: () => void;
  ```

  in the same block. If no explicit declaration exists (the codebase uses `window as any`-style access), skip this step.

---

## Task 8 — styles.css journal CSS

**Files:**
- Modify: `frontend/styles.css`

- [ ] **Step 1: Append journal rules**

  ```css
  /* ── Journal d'actions ──────────────────────────────────────────────── */
  #sift-journal-wrap{display:flex;flex-direction:column;gap:var(--space-lg)}
  .jrnl-header{display:flex;align-items:center;gap:var(--space-md);margin-bottom:var(--space-sm)}
  .jrnl-header .h1{flex:1;margin:0}
  .jrnl-header-right{display:flex;align-items:center;gap:var(--space-sm)}
  .jrnl-cat-head{display:flex;align-items:center;justify-content:space-between;padding-bottom:var(--space-xs)}
  .jrnl-cat-head .col-h{margin:0}
  .jrnl-all{display:flex;align-items:center;gap:var(--space-xs)}
  .jrnl-cat-all,.jrnl-last-batch{font-size:var(--text-xs)}
  .jrnl-see-all{font-size:var(--text-xs);color:var(--color-text-info);background:none;border:none;cursor:pointer;padding:0;text-decoration:underline}
  .jrnl-row{display:flex;align-items:center;gap:var(--space-sm);padding:var(--space-xs) 0;border-bottom:0.5px solid var(--color-border-tertiary);font-size:var(--text-sm)}
  .jrnl-row-name{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .jrnl-row-dest{color:var(--color-text-tertiary);font-size:var(--text-xs);flex:none;max-width:30%;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .jrnl-act{flex:none;font-size:var(--text-xs);padding:2px 7px}
  .jrnl-empty,.jrnl-err{font-size:var(--text-sm);color:var(--color-text-tertiary)}
  .jrnl-err{color:var(--color-text-danger)}
  .jrnl-back{font-size:var(--text-xs);background:none;border:none;cursor:pointer;color:var(--color-text-secondary);padding:0}
  .jrnl-session{margin-bottom:var(--space-xl)}
  .jrnl-session-label{display:block;margin-bottom:var(--space-sm)}
  .jrnl-batch{margin-left:var(--space-md);margin-bottom:var(--space-sm)}
  .jrnl-batch-head{font-size:var(--text-xs);color:var(--color-text-tertiary);margin-bottom:2px}
  .jrnl-batch-ts{font-family:var(--font-mono)}
  ```

- [ ] **Step 2: Final type-check**

  ```
  npx tsc --noEmit
  ```

  Expected: 0 errors.

---

## Test Live Checklist (spec §TEST LIVE)

- [ ] **a)** Filer un batch → ouvrir Journal → morceaux dans "Filés", session courante.
- [ ] **b)** ↩ sur une ligne → ce morceau seul est défilé (revient pending dans la Revue), Journal se rafraîchit.
- [ ] **c)** "Tout défiler" sur Filés → confirmation "Défiler les N morceaux affichés ?", seuls les affichés sont défilés.
- [ ] **d)** "Annuler le dernier batch" avec un batch de > 10 morceaux → confirmation, batch annulé.
- [ ] **e)** Jeter / rejeter un morceau → apparaît dans "Jetés" / "Rejetés" avec les bons boutons.
- [ ] **f)** "Voir tout" → arbre Session → Batch → morceaux ; "Antérieur" si des actions préexistaient.
- [ ] **g)** Onglet Jetés (ecarts) marche toujours normalement.
- [ ] **h)** Relancer l'app → nouvelle session dans "Voir tout" ; l'ancienne passe en historique.

---

## Self-Review

**Spec coverage:**

| Spec item | Task |
|-----------|------|
| A — Migration `session_id` | 1 |
| A — session_id au lancement | 3 (lib.rs) |
| A — list_journal renvoie session_id | 2 + 3 |
| A — fail-fast si migration/write échoue | 3 (`.expect(...)`) |
| B — onglet Journal | 5 (index.html + app.js) |
| B — session courante seulement | 6 (sessionId filter) |
| B — catégories (Filés/Jetés/Rejetés) | 6 (buildCategories) |
| B — nom + destination + bouton | 6 (rowHtml) |
| B — limit ≈ 50, newest-first | 4 + SQL ORDER BY DESC |
| C.1 — ↩ par track sans confirmation | 6 (revertRow, no confirm) |
| C.1 — routage par kind | 6 (revertRow if/else) |
| C.2 — "Tout" par catégorie avec confirmation | 6 (catBtn handler + confirm) |
| C.2 — porte sur affichés seulement | 6 (buildCategories reads _entries) |
| C.3 — dernier batch avec confirmation si > 10 | 6 (handleLastBatch + track_count) |
| D — page étendue SESSION → BATCH | 6 (renderJournalExtended) |
| D — Antérieur pour session_id NULL | 6 (__anterior__ key) |
| Onglet Jetés inchangé | ecartes-view.ts untouched |
| Tokens only | 6, 8 |

**Type consistency:** `JournalEntry.kind` is `"convert"|"move"|"trash"|"reject"` in contracts.ts, `actions.rs`, `buildCategories`, and `revertRow`. `track_count: number` TS ↔ `track_count: i64` Rust. `from_path`/`session_id` nullable in both layers.

**No placeholders confirmed.**
