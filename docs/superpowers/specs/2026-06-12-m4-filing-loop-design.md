# M4 — Filing loop (encode + tag + file + trash + undo) — Design

**Status:** approved for planning
**Date:** 2026-06-12
**Milestone:** M4 (closes the first end-to-end useful loop)

## Goal

Turn a reviewed track into a filed library file in one move: convert (only when
needed), write a consistent name + tags, and move it into a destination bin — plus a
reversible reject/trash path and a safe undo. After M4 the app is usable daily: the
Revue tab stops being a mockup and drives the real backend.

## Scope

**In:**
- Encoder: MP3 320 CBR / AIFF 16-bit 44.1 kHz, convert **only when the file is not
  already conformant to its target**, no upscale ever.
- Single-source-of-truth metadata: canonical `{artist, title, version}` projects to
  **both** the output filename (template) and the embedded tags.
- Confidence-gated reconciliation (clean tags vs filename) → green (auto) / yellow
  (needs validation).
- Filing into bins = recursive subfolders under a configured library root.
- Reject (`resourcing`) and trash (centralized `.sift-trash/`, auto-purged) actions.
- Undo: LIFO stack (Ctrl+Z) **and** a consultable journal with per-entry revert,
  sharing one guarded `revert(action)` primitive.
- Frontend: make the **Revue tab live** (real queue + real report + filing controls),
  reusing the existing wavesurfer player.

**Out (later milestones):**
- Écartés tab UI + per-track "copy name for Soulseek" + buy links → **M4b**.
- Discogs/metadata enrichment, covers → **M6**.
- Library scan, dedup, Rekordbox guard → **M5/M8**.
- `à-retélécharger.txt` — **dropped** (Soulseek re-downloads are track-by-track; a per-
  track clipboard copy in M4b replaces it).
- Other mockup tabs (Accueil, Biblio, Rekordbox, Clé USB, Réglages) stay mocked.

## Key decisions (from brainstorm)

| Topic | Decision |
|---|---|
| Encoding | Convert **only if not conformant** to the target; conformant file = tag + move, no re-encode. |
| Target format | By detected rail: lossless source → AIFF 16-bit/44.1; lossy source → MP3 320 CBR. No rail crossing. Per-file override allowed; no-upscale guard always wins. |
| Bins | Library root (configured in Réglages) + **recursive** subfolders; click-only navigation; "+ nouveau" creates a subfolder. Mono-location (no physical duplicates). |
| Name + tags | One canonical record → filename (template, default `Artist - Title (Version)`) **and** tags derived together. They can never diverge. |
| Reconciliation | Build candidates from tags and from filename; score cleanliness; pick by confidence (not fixed rule). Tags win when clean; fall back to filename; conflict or no-clean-source → validate. Clean embedded tags are preserved (not rewritten unless a field changed). |
| Confidence routing | 🟢 high confidence = file in one click; 🟡 low = stays in queue for a quick validation pass. |
| Batch | One bin chosen for the selection; 🟢 filed immediately, 🟡 **remain in the queue** after the greens pass. |
| Reject/trash | `resourcing` = status only (Écartés view is M4b). `trash` = move into central `.sift-trash/`, auto-purged after N days, reversible. |
| Undo | **B + C**: Ctrl+Z LIFO stack and a consultable journal; one guarded `revert(action_id)` primitive; never overwrites — fails cleanly. |
| Frontend | Converge: wire the real backend into the mockup's Revue `renderMid`; reuse the finished player; other tabs stay mock. |

## Architecture

### Backend (Rust, `src-tauri/src/`)

New modules, each with one responsibility and a thin IPC surface:

- **`naming.rs`** — pure logic, no I/O. Tag-cleanliness heuristic, filename parser,
  reconciliation → `Canonical { artist, title, version, confidence }`, and template
  rendering (`render_filename`, with filesystem sanitization). Fully unit-testable.
- **`encode.rs`** — wraps ffmpeg-sidecar. `target_for(report) -> Target` (rail-based),
  `is_conformant(report, target) -> bool`, `encode(src, dst, target)` (MP3 320 CBR /
  AIFF s16 44.1), `no_upscale_guard`. Returns a typed error on cross-rail upscale.
- **`tagging.rs`** — read/write embedded tags via lofty. `write_tags(path, canonical)`;
  only called when a field changed. Preserves untouched fields.
- **`library.rs`** — bins under the root: list recursive subfolders, create a new
  subfolder, resolve a chosen bin to an absolute destination path. Library root stored
  as a setting.
- **`filing.rs`** — orchestrates the strict order ① convert (if needed) → ② tag + name
  on the converted file → ③ move into the bin. Each filesystem step appends an
  `actions` row. Returns the final path + the action ids.
- **`actions.rs`** — the undo engine: `record(...)`, `revert(action_id)` (guarded
  inverse), `undo_last()` (LIFO), `list_journal(since)`. Marks rows `undone`.

`ipc.rs` gains commands (see Contracts). `db.rs` gains migration v4.

### Frontend (`frontend/`)

- Extract the player from `report-view.ts` into a reusable `player.ts`
  (`mountPlayer(el, report)`) — no behavior change.
- New `revue.ts` (TypeScript) renders the **live** Revue detail pane into `#mid`:
  verdict + player + canonical-fields editor + bin navigator + Ranger/Re-sourcer.
  Driven by real IPC; replaces the mockup's fake `renderMid` for the Revue view only.
- `revue.ts` owns the confidence badge, the editable artist/title/version (which live-
  updates the previewed filename), recursive bin navigation, and the action buttons.
- `report-view.ts` keeps only the debug-modal role (`sift-test`), now delegating to
  `player.ts`. The mockup's other tabs are untouched.
- `ipc.ts` + `shared/contracts.ts` gain the new commands/types.

## Data model (migration v4)

```sql
-- track filing/metadata state
ALTER TABLE tracks ADD COLUMN target_format TEXT;     -- 'mp3_320' | 'aiff_16_44'
ALTER TABLE tracks ADD COLUMN confidence TEXT;        -- 'green' | 'yellow'
ALTER TABLE metadata ADD COLUMN version TEXT;         -- 'Original Mix', 'Remix'…
-- undo support
ALTER TABLE actions ADD COLUMN undone INTEGER NOT NULL DEFAULT 0;  -- 0/1
ALTER TABLE actions ADD COLUMN batch_id TEXT;         -- groups a filing's convert+move
-- settings
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
-- library_root, filename_template, trash_purge_days live here
```

`tracks.status` already exists (`pending | filed | resourcing | trash`); `tracks.folder`
holds the chosen bin (relative to root). `actions.type` reuses `convert | move | trash |
reject`. A single user action (e.g. "Ranger") produces several `actions` rows sharing a
`batch_id`, so `revert` undoes them as a unit.

## Core flows

### Reconciliation (naming.rs)

1. Read embedded tags (artist, title) and parse the filename (`Artist - Title
   (Version)` patterns, strip quality tokens: `320kbps`, `kHz`, `FLAC`, `[uploader]`,
   leading track numbers, underscores→spaces).
2. Score each source clean/dirty. Pick canonical:
   - both clean & agree → green, use tags;
   - one clean → green, use it;
   - both clean but disagree → yellow (show both);
   - neither clean → yellow (prefill best guess).
3. `confidence` persisted; the filename preview = `render_filename(template, canonical)`.

### Filing (filing.rs) — strict order

1. `target = override ?? target_for(report)`; guard no-upscale.
2. If `!is_conformant`: `encode(src, tmp, target)` → record `convert`. Else use src.
3. If any canonical field changed vs embedded tags: `write_tags`.
4. Resolve destination = `root / folder / render_filename(...).ext`; ensure unique
   (refuse silent overwrite). `move` file → record `move`. Set `status='filed'`,
   `folder`, clear from queue.

Batch: iterate the selection; greens run the full flow; yellows are skipped and stay
`pending`. Result reported as `{ filed, needs_validation }`.

### Reject / trash

- **Re-sourcer**: `status='resourcing'`, record `reject`. (No file move in M4; Écartés
  view is M4b.)
- **Jeter**: move file into `<root>/.sift-trash/<id>__<name>`, `status='trash'`, record
  `trash`. A startup purge deletes `.sift-trash` entries older than `trash_purge_days`.

### Undo (actions.rs)

- `revert(action_id)`: compute the inverse from the row (`move` ↔ move back; `convert` ↔
  delete the converted file; `trash` ↔ restore; `reject` ↔ status back). **Guards:**
  destination must be free, source file must still be where recorded, the action not
  already `undone`, and no newer non-undone action on the same track. On any guard
  failure → return a typed error, change nothing. On success → mark `undone`, restore
  `tracks` state.
- `undo_last()` = revert the most recent non-undone `batch_id` (LIFO).
- `list_journal(since)` = recent actions (session + last few days) for the journal panel;
  each entry calls `revert`.

## IPC contracts (new)

```
reconcile(track_id) -> Canonical { artist, title, version, confidence, filename_preview }
file_track(track_id, bin: string, override_format?: string, edited?: Canonical) -> FileResult { path, batch_id }
file_batch(track_ids: number[], bin: string) -> BatchResult { filed: number, needs_validation: number[] }
reject_track(track_id) -> void
trash_track(track_id) -> void
list_bins() -> Bin[]                 // recursive tree under root
create_bin(parent_rel: string, name) -> Bin
undo_last() -> UndoResult            // or typed error
revert_action(action_id) -> UndoResult
list_journal(since?) -> ActionLog[]
get_setting(key) / set_setting(key, value)   // library_root, filename_template…
```

All file I/O stays in Rust; the front never touches the filesystem. Commands return
typed errors (string variants) for: no library root set, no-upscale violation,
destination collision, unsafe revert, ffmpeg failure.

## Error handling

- **No library root configured** → filing commands return `NoLibraryRoot`; Revue shows a
  "choisis ta racine" prompt (folder picker) instead of bin chips.
- **Destination collision** → `DestExists`; never overwrite. UI offers rename/skip.
- **ffmpeg failure / truncated output** → `EncodeFailed(detail)`; track stays `pending`,
  nothing moved.
- **Unsafe revert** → `RevertBlocked(reason)`; nothing changes.
- Every successful filesystem mutation is journaled before the next step, so a crash
  mid-flow leaves a revertible trail.

## Testing

- `naming.rs`: unit tests for the cleanliness heuristic, filename parsing, reconciliation
  matrix (the four confidence cases), template rendering + sanitization.
- `encode.rs`: `target_for`/`is_conformant` unit tests; **equivalence** characterization
  test — convert a known file and assert the output's declared rail/sample rate/bit
  depth match the target and that a real MP3 is never upscaled to lossless.
- `filing.rs`: integration test on temp files — full convert→tag→move order, action rows
  written, mono-location (source gone, one copy at dest), collision refused.
- `actions.rs`: revert each action type restores prior state; guards block unsafe
  reverts; LIFO order; `undone` idempotence.
- Frontend: type-check; manual verification of the live Revue flow against real files.

## Open items (deferred, not blocking M4)

- Filename template editor UI (Réglages) — M4 ships a sensible default + a setting; the
  editor can come with the Réglages tab.
- Auto-rules ("MP3 < 320 → convert", "fake → reject") — M4 wires manual actions; the
  rules engine that consumes the same `file_track`/`reject` commands is M4+.
