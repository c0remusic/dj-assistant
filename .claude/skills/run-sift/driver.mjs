#!/usr/bin/env node
// Launch + drive the REAL `tauri dev` window of Sift, from an agent.
//
// Why this exists on top of `.claude/scripts/cdp.cjs`: that helper evaluates JS against an
// ALREADY-RUNNING window. Everything before that is where the traps are — picking a debug port
// a neighbouring Tauri project has not squatted, not piping the dev server through `tail`,
// waiting for a signal that distinguishes "still compiling" from "build failed", and reaching a
// state where anything is actually painted. This file owns that part and shells out to cdp.cjs
// for the evaluation itself.
//
//   node .claude/skills/run-sift/driver.mjs status
//   node .claude/skills/run-sift/driver.mjs launch [--port N]
//   node .claude/skills/run-sift/driver.mjs eval "expr" [--port N]
//   node .claude/skills/run-sift/driver.mjs open-track [--port N]
//   node .claude/skills/run-sift/driver.mjs floor [--port N]
//   node .claude/skills/run-sift/driver.mjs shot out.png [--port N]
//   node .claude/skills/run-sift/driver.mjs stop
//
// Node 22+ (native fetch). Windows-first: this project ships Windows + macOS.

import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, openSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, resolve, join } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(HERE, "../../..");            // <repo>/.claude/skills/run-sift → <repo>
const CDP = resolve(REPO, ".claude/scripts/cdp.cjs");
// Machine state lives OUTSIDE the repo: this skill directory is whitelisted in .gitignore so the
// driver travels with a fresh clone, which means anything written next to it would be committed.
const STATE = join(tmpdir(), "sift-run-skill-state.json");
const LOG = resolve(REPO, "tauri-dev.log");        // ignored by the repo's `*.log` rule

const raw = process.argv.slice(2);
const args = [];
let portArg = null;
for (let i = 0; i < raw.length; i++) {
  if (raw[i] === "--port") portArg = Number(raw[++i]);
  else args.push(raw[i]);
}
const [cmd, ...rest] = args;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** CDP targets on a port, or null when nothing answers. */
async function targets(port) {
  try {
    const res = await fetch(`http://127.0.0.1:${port}/json/list`, {
      signal: AbortSignal.timeout(1500),
    });
    return await res.json();
  } catch {
    return null;
  }
}

/** A port belongs to Sift only if a target says so. 9222/9223 are routinely held by other
 *  Tauri projects on a dev machine — trusting "something answered" is how you end up measuring
 *  a neighbour's app and reporting it as yours. */
async function isSift(port) {
  const t = await targets(port);
  if (!t) return false;
  return t.some((x) => /Sift/i.test(x.title || "") || /localhost:5173/.test(x.url || ""));
}

/** L'URL WebSocket de la page Sift sur `port`, ou null. Résolue ICI et passée à probe.mjs :
 *  l'identité se juge par la règle d'`isSift` — titre « Sift » OU url du serveur de dev — alors que
 *  probe.mjs, laissé à lui-même, la rejuge sur le TITRE SEUL et refuse une fenêtre dont le titre
 *  n'est pas encore posé. Une cible déjà validée ne se revalide pas plus faiblement. */
async function siftPageWs(port) {
  const t = (await targets(port)) || [];
  const page = t.find(
    (x) => x.type === "page" && (/Sift/i.test(x.title || "") || /localhost:5173/.test(x.url || "")),
  );
  return page?.webSocketDebuggerUrl || null;
}

async function findSiftPort() {
  const saved = readState().port;
  const candidates = [portArg, saved, 9333, 9334, 9335, 9222, 9223].filter(Boolean);
  for (const p of new Set(candidates)) if (await isSift(p)) return p;
  return null;
}

async function findFreePort() {
  for (let p = 9333; p < 9360; p++) if ((await targets(p)) === null) return p;
  throw new Error("no free debug port in 9333..9359");
}

/** Is anything still compiling or running the app? Used as the liveness signal because the
 *  dev-server log is empty on Windows (see cmdLaunch). */
function buildAlive() {
  // Deliberately NOT matching node.exe: this machine always has node processes from other
  // projects, so including it would make the check always-true and therefore useless.
  //
  // 2026-09-09 : ne compter que les processus dont la LIGNE DE COMMANDE nomme ce dépôt. Un
  // `tasklist` nu voyait le `cargo.exe` de shaderlab et `launch` a attendu 25 minutes une fenêtre
  // qu'aucun build de Sift ne préparait. Sur Windows, seule WMI donne la ligne de commande.
  if (process.platform === "win32") {
    const repoRe = REPO.replace(/[\\/]/g, "[\\\\\\\\/]").replace(/[.+?^${}()|[\]]/g, "\\$&");
    const ps =
      `Get-CimInstance Win32_Process | Where-Object { $_.Name -match '^(cargo|rustc|sift)' ` +
      `-and $_.CommandLine -match '${repoRe}' } | Measure-Object | Select-Object -ExpandProperty Count`;
    const r = spawnSync("powershell", ["-NoProfile", "-NonInteractive", "-Command", ps], { encoding: "utf8" });
    return Number((r.stdout || "0").trim()) > 0;
  }
  const r = spawnSync("sh", ["-c", "ps ax"], { encoding: "utf8" });
  const lines = (r.stdout || "").split("\n").filter((l) => /cargo|rustc|sift/i.test(l));
  return lines.some((l) => l.includes(REPO));
}

function readState() {
  try {
    return JSON.parse(readFileSync(STATE, "utf8"));
  } catch {
    return {};
  }
}
const writeState = (s) => writeFileSync(STATE, JSON.stringify(s, null, 2));

/** Evaluate through the project's own committed helper rather than reimplementing the
 *  WebSocket dance. Returns stdout trimmed. */
function cdpEval(port, expr, extra = []) {
  const r = spawnSync(process.execPath, [CDP, "--port", String(port), ...extra, expr], {
    cwd: REPO,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  if (r.status !== 0) throw new Error(`cdp.cjs failed: ${(r.stderr || r.stdout || "").trim()}`);
  return (r.stdout || "").trim();
}
const evalJson = (port, expr) => {
  const out = cdpEval(port, expr, ["eval"]);
  // cdp.cjs prints the JSON-encoded return value; our expressions return JSON strings.
  try {
    return JSON.parse(JSON.parse(out));
  } catch {
    return out;
  }
};

async function cmdStatus() {
  const port = await findSiftPort();
  if (!port) {
    console.log(JSON.stringify({ running: false }, null, 2));
    return;
  }
  const t = await targets(port);
  console.log(
    JSON.stringify({ running: true, port, title: t?.[0]?.title, url: t?.[0]?.url }, null, 2),
  );
}

async function cmdLaunch() {
  const already = await findSiftPort();
  if (already) {
    console.log(`already running on ${already} — reusing (use 'stop' first to restart)`);
    writeState({ ...readState(), port: already });
    return;
  }
  const port = portArg || (await findFreePort());
  // NEVER pipe a dev server through `tail`: tail buffers until EOF, which never comes, so the
  // log stays empty and a running build looks like a dead one. Redirect to a file instead.
  const fd = openSync(LOG, "w");
  const child = spawn("npm", ["run", "tauri", "dev"], {
    cwd: REPO,
    env: { ...process.env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}` },
    stdio: ["ignore", fd, fd],
    detached: true,
    shell: true,
  });
  child.unref();
  writeState({ port, pid: child.pid, log: LOG });
  console.log(`launching on debug port ${port} (pid ${child.pid}), log → ${LOG}`);

  // Readiness needs TWO independent signals, because on Windows the log is usually EMPTY:
  // a detached `npm` does not write into the inherited descriptor, so `tauri-dev.log` stays at
  // 0 bytes while cargo happily compiles. Trusting the log alone means a healthy 15-minute build
  // is indistinguishable from a dead one. So: CDP presence = success, and the *absence of any
  // build process* = failure. The log is a bonus when it happens to contain something.
  const FAIL = /error\[E\d+\]|could not compile|panicked at|EADDRINUSE/i;
  const MAX_MS = 25 * 60 * 1000;                 // a cold/interrupted rebuild really does take this
  const t0 = Date.now();
  while (Date.now() - t0 < MAX_MS) {
    if (await isSift(port)) {
      console.log(`ready after ~${Math.round((Date.now() - t0) / 1000)}s on port ${port}`);
      return;
    }
    const log = existsSync(LOG) ? readFileSync(LOG, "utf8") : "";
    if (FAIL.test(log)) {
      console.error("BUILD FAILED — tail of log:");
      console.error(log.split("\n").slice(-15).join("\n"));
      process.exit(1);
    }
    if (!buildAlive()) {
      console.error("no build process left and no window — the launch died silently.");
      console.error(log ? log.split("\n").slice(-15).join("\n") : `(log empty — normal here: ${LOG})`);
      process.exit(1);
    }
    await sleep(3000);
  }
  console.error(`timeout: no Sift CDP on ${port} after 25min. Still building? check: tasklist | grep cargo`);
  process.exit(2);
}

/** Reach a state where the detail pane is actually painted.
 *  Two traps, both hit for real:
 *   - clicking app.js's own mode toggle repaints the MOCKUP over the live view (demo tracks);
 *     reload first so the live wiring owns the DOM.
 *   - the click returning does not mean the track opened. #mid stays empty for seconds while
 *     the report loads. Poll childElementCount, never trust the click. */
async function cmdOpenTrack(port) {
  cdpEval(port, '"reload"', ["--reload", "eval"]);
  await sleep(6000);
  evalJson(port, 'document.querySelector(\'[data-view="revue"]\').click(), JSON.stringify("nav")');
  await sleep(4000);
  const q = evalJson(port, 'JSON.stringify({rows:document.querySelectorAll(".qi[data-id]").length})');
  if (!q.rows) {
    console.error("no queue rows — the library is empty or still scanning. Add a source on Accueil.");
    process.exit(3);
  }
  evalJson(port, 'var r=document.querySelector(".qi[data-id]"); r&&r.click(), JSON.stringify("click")');
  for (let i = 0; i < 20; i++) {
    await sleep(1500);
    const s = evalJson(
      port,
      'JSON.stringify({mid:(document.getElementById("mid")||{}).childElementCount||0,zones:document.querySelectorAll(".sift-zone-toggle").length})',
    );
    if (s.mid > 0 && s.zones > 0) {
      console.log(JSON.stringify({ opened: true, queueRows: q.rows, ...s }, null, 2));
      return;
    }
  }
  console.error("track never painted (#mid stayed empty) — analysis may be slow or failing");
  process.exit(4);
}

/** Typography floor check. Always reports nbTexts/minPx alongside the offender list: an empty
 *  list on an unpainted screen is indistinguishable from compliance without them. */
const FLOOR_EXPR = `JSON.stringify((function(){
  var leaves=[].slice.call(document.querySelectorAll('*')).filter(function(e){return e.textContent&&e.textContent.trim()&&!e.children.length;});
  var px=function(e){return parseFloat(getComputedStyle(e).fontSize);};
  return {nbTexts:leaves.length,
          minPx:leaves.length?Math.min.apply(null,leaves.map(px)):null,
          under10:leaves.filter(function(e){return px(e)<10;}).map(function(e){
            var c=e.className; if(c&&c.baseVal!==undefined)c=c.baseVal;
            return [c||e.tagName,px(e),e.textContent.trim().slice(0,20)];})};
})())`;

async function cmdFloor(port) {
  const r = evalJson(port, FLOOR_EXPR);
  console.log(JSON.stringify(r, null, 2));
  if (!r.nbTexts) {
    console.error("nbTexts=0 — nothing was painted. This result proves NOTHING. Open a screen first.");
    process.exit(5);
  }
  if (r.under10.length) process.exit(6);
}

function listeners(port) {
  const isWin = process.platform === "win32";
  const r = isWin
    ? spawnSync(
        "powershell",
        ["-NoProfile", "-Command",
         `(Get-NetTCPConnection -LocalPort ${port} -State Listen -ErrorAction SilentlyContinue).OwningProcess`],
        { encoding: "utf8" },
      )
    : spawnSync("sh", ["-c", `lsof -ti :${port} || true`], { encoding: "utf8" });
  return (r.stdout || "").split(/\s+/).map(Number).filter((n) => Number.isInteger(n) && n > 0);
}

function cmdStop() {
  const s = readState();
  const isWin = process.platform === "win32";
  // THREE processes, not one. Killing the npm parent leaves both children alive:
  //  - Vite on 5173, which then blocks the next launch (orphan, seen for real);
  //  - the Tauri/WebView2 window itself, which keeps holding the debug port — so `status` still
  //    reports "running" after a stop that looked successful.
  // Kill by exact PID only; never `taskkill /IM node.exe`, other projects on this machine use node.
  const pids = new Set([...listeners(5173), ...listeners(s.port || 9333)]);
  if (s.pid) pids.add(s.pid);
  for (const pid of pids) {
    if (isWin) spawnSync("taskkill", ["/PID", String(pid), "/T", "/F"], { encoding: "utf8" });
    else process.kill(pid, "SIGTERM");
    console.log(`killed pid ${pid}`);
  }
  writeState({});
  if (!pids.size) console.log("nothing to stop");
}

const main = async () => {
  if (cmd === "status") return cmdStatus();
  if (cmd === "launch") return cmdLaunch();
  if (cmd === "stop") return cmdStop();

  const port = await findSiftPort();
  if (!port) {
    console.error("no Sift window found. Run: node .claude/skills/run-sift/driver.mjs launch");
    process.exit(1);
  }
  if (cmd === "eval") return void console.log(cdpEval(port, rest[0], ["eval"]));
  if (cmd === "shot") {
    const out = resolve(REPO, rest[0] || "shot.png");
    mkdirSync(dirname(out), { recursive: true });
    return void console.log(cdpEval(port, out, ["screenshot"]));
  }
  if (cmd === "open-track") return cmdOpenTrack(port);
  if (cmd === "floor") return cmdFloor(port);
  // Pseudo-états réels (:hover, :focus-visible) : session WebSocket continue obligatoire
  // (Input.dispatch*), donc un module dédié plutôt que cdp.cjs — voir l'en-tête de probe.mjs.
  if (cmd === "hover" || cmd === "focus") {
    // La cible est passée résolue (`--ws`) quand on l'a : le port reste fourni pour le repli et pour
    // les messages d'erreur.
    const ws = await siftPageWs(port);
    const probeArgs = [resolve(HERE, "probe.mjs"), String(port), cmd, ...rest];
    if (ws) probeArgs.push("--ws", ws);
    const r = spawnSync(process.execPath, probeArgs, {
      cwd: REPO,
      encoding: "utf8",
      maxBuffer: 32 * 1024 * 1024,
    });
    process.stdout.write(r.stdout || "");
    process.stderr.write(r.stderr || "");
    return void process.exit(r.status ?? 1);
  }
  console.error(
    "usage: status | launch | stop | eval <expr> | shot <file> | open-track | floor | hover <sel> [idx] [outDir] | focus <sel> [outDir]",
  );
  process.exit(1);
};

main().catch((e) => {
  console.error(e.message || e);
  process.exit(1);
});
