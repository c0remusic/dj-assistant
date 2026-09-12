---
name: run-sift
description: Build, launch, screenshot and drive the real Sift desktop app (Tauri v2 + WebView2). Use when asked to run Sift, start the dev app, take a screenshot of a screen, measure computed styles, open a track, or verify a UI change in the actual window rather than in a browser.
---

Sift is a Tauri v2 desktop app (Rust backend, Vite + vanilla TypeScript frontend). Most of
its frontend **never executes in a plain browser**: `frontend/main.ts` only installs the
live wiring when `__TAURI_INTERNALS__` is present, so `sift-live.ts`, `filing*.ts`,
`report-view.ts` and every IPC path exist only in the real window. A screenshot from
`npm run dev` proves nothing about them.

Drive it with **`.claude/skills/run-sift/driver.mjs`**, which launches the window with a
CDP debug port and talks to it through the repo's own `.claude/scripts/cdp.cjs`.

All paths below are relative to the repo root. Verified on Windows 10, Node 22.

## Prerequisites

Bootstrap once. `fetch-ffmpeg` downloads the bundled sidecar into `src-tauri/binaries/`
(gitignored):

```bash
npm ci && npm run fetch-ffmpeg
```

Rust toolchain is pinned by `rust-toolchain.toml` at the repo **root** (not in
`src-tauri/`). Nothing else to install on Windows.

## Run — agent path

```bash
node .claude/skills/run-sift/driver.mjs status
```

```bash
node .claude/skills/run-sift/driver.mjs launch
```

`launch` picks a free debug port (9333+), starts `tauri dev`, and waits until a CDP target
**whose title is Sift** answers. It reuses an already-running window instead of starting a
second one. A cold or interrupted Rust rebuild genuinely takes 10–25 minutes; an
incremental one is far quicker.

Reach a state where the detail pane is actually painted — this is the prerequisite for
almost any UI measurement:

```bash
node .claude/skills/run-sift/driver.mjs open-track
```

```
{ "opened": true, "queueRows": 56, "mid": 1, "zones": 2 }
```

Evaluate anything in the page, and take screenshots:

```bash
node .claude/skills/run-sift/driver.mjs eval 'JSON.stringify({rows:document.querySelectorAll(".qi[data-id]").length})'
```

```bash
node .claude/skills/run-sift/driver.mjs shot shot.png
```

Typography floor check (no text below 10px — the HIG macOS minimum the project adopted on
2026-08-05). It always reports `nbTexts` and `minPx` next to the offenders, on purpose:

```bash
node .claude/skills/run-sift/driver.mjs floor
```

```
{ "nbTexts": 177, "minPx": 10, "under10": [] }
```

Real engine pseudo-states — `:hover` and `:focus-visible` CANNOT be triggered from the page
(`dispatchEvent` skips the engine's hit-testing; `focus()` only counts as focus-visible after a
real keyboard interaction). These inject true `Input.dispatch*` events over one continuous
WebSocket session (see probe.mjs's header for why cdp.cjs can't do it), measure rest/state/back,
and write 1:1 cropped screenshots when an output dir is given:

```bash
node .claude/skills/run-sift/driver.mjs hover ".nv" 1 C:/abs/out-dir
```

```bash
node .claude/skills/run-sift/driver.mjs focus ".sift-bib-facet-btn" C:/abs/out-dir
```

Gotchas of their own: an open modal STEALS focus (its trap wins over `focus()` — close it first),
and a state class added by hand (`el.classList.add("dim")`) is swept away by the component's next
re-render, so only states reachable by real interaction can be measured live.

Stop everything — npm parent, Vite, and the window:

```bash
node .claude/skills/run-sift/driver.mjs stop
```

## Run — human path

```bash
npm run tauri dev
```

Opens the window with hot reload. This is the project's default for visual work, and it is
useless headless: there is no debug port unless you set one at launch (see Gotchas).

## Test

⚠️ Never run these while `tauri dev` is compiling — they fight over the `target/` lock.

```bash
npx tsc --noEmit
```

```bash
npm run lint:tokens
```

```bash
npm run check:security
```

## Gotchas

- **Verify the debug port belongs to Sift.** On this machine ports **9222 and 9223 were
  both held by a different Tauri project** whose CDP answers normally. Measuring it and
  reporting the result as Sift's is a silent, total failure. `driver.mjs` only accepts a
  target whose title matches Sift; if you use `cdp.cjs` directly, check
  `curl http://127.0.0.1:<port>/json/list` first.
- **Never set the debug port in `tauri.conf.json`.** It would ship in distributed builds
  and overrides wry's own arguments. It goes in the environment at launch only:
  `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333`.
- **Never pipe a dev server through `tail`.** `tail` buffers until EOF, which never comes,
  so the log stays empty and a running build looks dead.
- **On Windows the dev log is empty anyway.** A detached `npm` does not write into the
  inherited descriptor: `tauri-dev.log` sits at 0 bytes through a perfectly healthy
  15-minute build. Use process liveness (`cargo`/`rustc`/`sift` in `tasklist`) as the
  progress signal — that is what `driver.mjs` does.
- **Stopping kills three processes, not one.** The npm parent, the orphan Vite on 5173
  (which then blocks the next launch), and the WebView2 window that keeps holding the debug
  port. Kill only the parent and `status` still reports *running*. Always by exact PID —
  never `taskkill /IM node.exe`, other projects on this machine use node.
- **And `stop` reporting three kills does not mean the app is gone.** Measured 2026-08-17:
  `driver.mjs stop` printed `killed pid` three times and exited clean, 5173 was correctly
  free — and a `sift.exe` was **still running**, confirmed mine by
  `ExecutablePath = C:\dev\sift\src-tauri\target\debug\sift.exe`. `stop` kills the PIDs it
  recorded at launch; the app binary `cargo` spawns underneath is not always among them.
  So the count printed by `stop` is not the check. **Always verify after stopping**, and
  only then kill what is left, by exact PID:
  ```
  Get-CimInstance Win32_Process -Filter "Name='sift.exe'" | Select ProcessId, ExecutablePath
  ```
  `ExecutablePath` is the only field that proves the process is yours — a Tauri binary's
  command line is the relative `"target\debug\sift.exe"` and names no project, and this
  machine runs several Tauri projects. A survivor left here is what makes the *next* launch
  find a stale window, which is the failure mode the Known limitation section blames for the
  2026-08-05 timeout.
- **A FAILED `launch` also leaves a Vite behind, and that is worse.** Measured 2026-08-14:
  `driver.mjs launch` reported *the launch died silently* with an empty log — but its Vite
  had already grabbed 5173 and survived. The next attempt then fails on `Error: Port 5173 is
  already in use`, which names a different culprit than the real one, and the loop repeats.
  Free 5173 and relaunch **in one command** so nothing can race in between, and only kill a
  holder whose `CommandLine` contains `dev\sift`.
- **`stop` then `launch` back to back loses the race, and the symptom points nowhere.** Measured
  2026-08-16. `driver.mjs stop` returns as soon as it has killed its three PIDs, but the old Vite
  is still releasing 5173. A `launch` fired immediately after finds the port taken, its own Vite
  never binds — and `tauri dev` **opens the window anyway**. Result: `sift.exe` alive, nothing
  listening on 5173, no CDP on any port in 9333+, `status` reporting `running: false`, and the
  `launch` process hanging until its own timeout. Nothing in that picture names the real cause.
  Diagnose with two checks, in this order: `Get-NetTCPConnection -LocalPort 5173 -State Listen`
  (empty = Vite is dead, so the window is on `ERR_CONNECTION_REFUSED`) and
  `Get-CimInstance Win32_Process -Filter "ProcessId=<pid>" | Select ExecutablePath` — the
  command line of a Tauri binary is the relative `"target\debug\sift.exe"` and names no project,
  so **only `ExecutablePath` proves the process is yours** on a machine running several Tauri
  projects. Then kill by exact PID, confirm 5173 is free, and only then relaunch.
- **`status` reads the CDP *target* title, not the live document.** A dead page still reports
  `running: true` with the correct `Sift — prépa sons DJ` title. The live check is
  `driver.mjs eval 'document.title'`: a page whose Vite died returns `"localhost"`, and its
  body reads `ERR_CONNECTION_REFUSED`. Never trust `status` alone before measuring.
- **Never truncate the pipe of a foregrounded `npm run tauri dev`.** `| head -N` or
  PowerShell's `| Select-Object -First N` closes the pipe at the Nth line, which kills npm,
  which kills Vite — the window stays open on an error page while `status` still says
  running. Same blindness as the `tail` gotcha above, opposite cause.
- **`shot <name>` ignores the cwd** and writes to the repo root — it polluted `C:\dev\sift`
  with a stray PNG before being caught by `git status`. Pass an absolute path into the
  session scratchpad.
- **An empty measurement is not a pass.** Filtering for offenders returns `[]` both when a
  screen is compliant and when nothing was painted. Every check must carry a positive count
  (`nbTexts`, `minPx`). This produced three false "conformant" results before being caught.
- **Assert which screen you are on before capturing, not after.** A capture run that never
  navigated silently photographed Réglages for four "Revue" variants. Make the view identity
  a precondition that throws: `document.querySelector(".nv.on").textContent` — note it carries
  the badge count, so match with `startsWith`, not equality.
- **`driver.mjs eval` prints `JSON.stringify(value)`.** A value that is *already* a JSON string
  comes back double-encoded, so `JSON.parse` yields a string, not an object — and a guard reading
  its fields sees `undefined` and aborts on a healthy app. Parse twice, or return a bare object.
- **Clicking a queue row does not mean the track opened.** `#mid` stays empty for seconds
  while the report loads. Poll `childElementCount`, never trust the click returning.
- **`cdp.cjs open-track` fails on the live DOM** — it returns
  `{"firstTrackFound":false}`. Use `driver.mjs open-track`, which reloads first and polls.
- **`frontend/app.js` is a mockup that runs in production.** It is imported
  unconditionally. Clicking *its* mode toggle repaints the mockup **over** the live view,
  with demo tracks (Mr. Fingers, Chez Damier…). If you see those names, you are driving the
  mockup. Reload and use the real path.
- **Git Bash mangles `tasklist /FI`** into a path via MSYS conversion. Use the PowerShell
  tool for Windows process queries.
- **`src-tauri/fixtures/*` is gitignored.** On a fresh clone the `analysis::decode` tests
  fail with *file not found* — not a real bug. Regenerate:
  `node scripts/make-fixtures.mjs`.
- **`.claude/*` is gitignored** with a narrow whitelist. This skill is re-allowed
  explicitly in `.gitignore`; a new file elsewhere under `.claude/` is invisible to git by
  default, silently.

## Troubleshooting

| Symptom | Cause and fix |
|---|---|
| `status` says `running: false` right after `launch` | Rust still compiling. `tasklist \| grep -i cargo` — if processes are there, wait; a cold rebuild reaches 25 min. |
| `no free debug port in 9333..9359` | Another Tauri project is squatting the range. Pass one explicitly: `--port 9400`. |
| `open-track` exits with *no queue rows* | The library is empty or still scanning. Add a source folder on Accueil first — the OS folder picker is native and cannot be driven by CDP. |
| `open-track` exits with *track never painted* | Analysis is slow on a cold cache (`analyze_path` runs the decode + FFT inline, synchronously). Retry, or pick an already-analysed track. |
| `floor` exits non-zero with `nbTexts: 0` | Nothing was painted — the result is meaningless. Run `open-track` or navigate to a screen first. |
| `launch` reports *the launch died silently* | **The message itself can be FALSE.** Measured 2026-08-19: it printed this and exited 1 while cargo/vite/tauri were all alive — the window came up on its own minutes later. Before believing it: `tasklist \| grep -i cargo` (PowerShell tool on Windows); processes there = build still running, wait, then `status` + `eval 'document.title'`. Only if nothing is alive: the build failed with an empty log — run `npm run tauri dev` in the foreground to see the real error. |
| `launch` times out but the app is actually **open** | Seen once on 2026-08-05, **did not reproduce** the same day (see Known limitation). Before blaming `launch`, check for a stale `sift.exe` from an earlier run and confirm the port range is free — that is the explanation the evidence supports. Fallback that always works: `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333 npm run tauri dev` in the foreground. |
| You need to read the app's **boot log** | A detached `npm` writes nothing into the inherited descriptor, so `tauri-dev.log` stays at 0 bytes and `driver.mjs launch` cannot show you the boot. Launch it in the foreground (command above) with the output captured — that is the only path where `SMOKE OK`, panics and unhandled rejections are actually readable. |
| An `eval` returns `error: timeout` and nothing else | `cdp.cjs` closes its socket after **15 s** (`.claude/scripts/cdp.cjs:39`). An `(async …)` IIFE that walks all 8 rail views with a 3 s settle needs 24 s and dies with no partial result. Split into batches of ≤ 4 views. Found 2026-08-05 while measuring painted font sizes. |

## Known limitation

**Lifted 2026-08-05, same day it was written.** `launch` was exercised end to end and
succeeded: `node driver.mjs launch --port 9333` reported *ready after ~8s*, the CDP port
answered, and `document.title` was `Sift — prépa sons DJ`. The app was then driven, rebuilt
by the watcher, and came back on the same port ~10 s later — the port survives a rebuild
cycle. Every subcommand is now verified against the real window.

The 2026-08-05 failure therefore **did not reproduce**, and its stated cause was measured
false (below). The most likely explanation of the original observation is the third
possibility, not the first: a stale `sift.exe` from an earlier run, or the port range being
squatted that day — 9222 *and* 9223 were both held by other processes at the time. Always
confirm identity before trusting a session; never assume a live `sift.exe` is yours.

### What was RULED OUT on 2026-08-05 — do not re-test it

The obvious theory was that `spawn(..., {shell: true, detached: true})` drops the
environment variable on Windows. **Measured, and false.** A probe spawned a child under all
four combinations of `{shell, detached}` and had it report its own environment: the variable
arrived intact in **all four**, including `shell:true` + `detached:true`.

Two false readings were produced before that verdict, and both are worth knowing because
they look exactly like a positive result:

1. Using `process.execPath` as the command — under `shell: true`, `C:\Program
   Files\nodejs\node.exe` splits on its space and the child dies before reading anything.
2. Letting the child `console.log` into an inherited descriptor — under `detached: true` the
   output never reaches the file, so the probe reads empty and scores it "lost". This is the
   *same* blindness that keeps `tauri-dev.log` at 0 bytes, and it makes an absence look like
   an answer. The fix is for the child to write its own result file.

So the variable reaches the process. The failure is downstream — WebView2 not applying it,
a readiness check racing the window, or the observed portless `sift.exe` having been a stale
process from an earlier launch. Start there, not at the spawn options.

Corollary worth knowing before killing anything: this machine runs **several Tauri
projects at once**. On 2026-08-05, four of six live `cargo.exe` belonged to two *other*
projects. Their command lines are relative (`"cargo" run …`) and carry no project path — a
naive `taskkill` would have destroyed a neighbour's build. Identify by walking
`ParentProcessId` up to a command line that names the project, or by `ExecutablePath` for
the app binary.
