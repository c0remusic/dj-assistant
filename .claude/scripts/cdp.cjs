#!/usr/bin/env node
// Reusable CDP helper against the real `tauri dev` WebView2 window (port 9222,
// WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222 at launch —
// see CLAUDE.md "Vérification UI"). Replaces the ad-hoc .cdp-*.cjs scripts rewritten
// several times per session — this one is committed, not scratch.
//
// Usage:
//   node .claude/scripts/cdp.cjs eval "document.title"
//   node .claude/scripts/cdp.cjs eval --reload "document.title"   # full reload first
//   node .claude/scripts/cdp.cjs screenshot out.png
//   node .claude/scripts/cdp.cjs click ".sift-sg-toggle"
//   node .claude/scripts/cdp.cjs open-track   # nav Revue -> open first track -> expand Diagnostic audio
//   node .claude/scripts/cdp.cjs --port 9223 eval "document.title"  # explicit port (default 9222)
//
// Assumes a single open page target (true for this app). Node 22+ (native WebSocket/fetch).

const rawArgs = process.argv.slice(2);
let port = 9222;
let reload = false;
const args = [];
let anyTitle = false;
for (let i = 0; i < rawArgs.length; i++) {
  if (rawArgs[i] === "--port") port = Number(rawArgs[++i]);
  else if (rawArgs[i] === "--reload") reload = true;
  else if (rawArgs[i] === "--any-title") anyTitle = true;
  else args.push(rawArgs[i]);
}
const [cmd, ...cmdArgs] = args;

async function pageWsUrl() {
  const res = await fetch(`http://localhost:${port}/json`);
  const targets = await res.json();
  const page = targets.find((t) => t.type === "page");
  if (!page) throw new Error("no page target found — is tauri dev running with the CDP port open?");
  // GARDE D'IDENTITÉ (2026-09-09). Un port de débogage se fait reprendre par un autre projet Tauri
  // dès que Sift redémarre (édition .rs → restart → l'app ne revient pas → Tuple ou shaderlab
  // prend le port). Ce soir-là, huit mesures « Sift » ont mesuré shaderlab avant que quelqu'un
  // lise un titre. Le titre se vérifie ICI, à chaque appel — pas une fois en début de session.
  if (!anyTitle && !/Sift/i.test(page.title || "")) {
    throw new Error(
      `target on port ${port} is NOT Sift: "${page.title}" (${page.url}) — another project holds ` +
        `this port; re-check after every .rs edit. Pass --any-title to talk to it anyway.`,
    );
  }
  return page.webSocketDebuggerUrl;
}

function withSocket(run) {
  return new Promise(async (resolve, reject) => {
    const ws = new WebSocket(await pageWsUrl());
    const timeout = setTimeout(() => { reject(new Error("timeout")); ws.close(); }, 15000);
    ws.onopen = () => run(ws, resolve, reject, timeout);
    ws.onerror = (e) => { clearTimeout(timeout); reject(new Error("ws error: " + (e && e.message))); };
  });
}

function evaluate(ws, expression, awaitPromise = false) {
  return new Promise((resolve, reject) => {
    const id = Math.floor(Math.random() * 1e9);
    ws.addEventListener("message", function handler(ev) {
      const msg = JSON.parse(ev.data);
      if (msg.id !== id) return;
      ws.removeEventListener("message", handler);
      if (msg.error) return reject(new Error(JSON.stringify(msg.error)));
      resolve(msg.result?.result);
    });
    ws.send(JSON.stringify({
      id,
      method: "Runtime.evaluate",
      params: { expression, returnByValue: true, awaitPromise },
    }));
  });
}

function reloadPage(ws) {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => { cleanup(); resolve(); }, 8000); // fallback if the load event never arrives
    function cleanup() {
      clearTimeout(timeout);
      ws.removeEventListener("message", onMessage);
    }
    function onMessage(ev) {
      const msg = JSON.parse(ev.data);
      if (msg.id === 999998 && msg.error) { cleanup(); reject(new Error("Page.reload failed: " + JSON.stringify(msg.error))); return; }
      if (msg.method === "Page.loadEventFired") { cleanup(); resolve(); }
    }
    ws.addEventListener("message", onMessage);
    ws.send(JSON.stringify({ id: 999999, method: "Page.enable", params: {} }));
    ws.send(JSON.stringify({ id: 999998, method: "Page.reload", params: { ignoreCache: true } }));
  });
}

async function cmdEval(expr) {
  await withSocket(async (ws, resolve, reject, timeout) => {
    try {
      if (reload) await reloadPage(ws);
      const result = await evaluate(ws, expr, /* awaitPromise */ expr.trim().startsWith("(async"));
      clearTimeout(timeout);
      console.log(result?.value !== undefined ? JSON.stringify(result.value) : JSON.stringify(result));
      ws.close();
      resolve();
    } catch (e) {
      clearTimeout(timeout);
      reject(e);
    }
  });
}

async function cmdScreenshot(outPath) {
  await withSocket((ws, resolve, reject, timeout) => {
    const fs = require("fs");
    ws.send(JSON.stringify({ id: 1, method: "Page.enable", params: {} }));
    ws.send(JSON.stringify({ id: 2, method: "Page.captureScreenshot", params: { format: "png" } }));
    ws.onmessage = (ev) => {
      const msg = JSON.parse(ev.data);
      if (msg.id === 2 && msg.result?.data) {
        clearTimeout(timeout);
        fs.writeFileSync(outPath, Buffer.from(msg.result.data, "base64"));
        console.log("saved", outPath);
        ws.close();
        resolve();
      }
    };
  });
}

// Le sélecteur est échappé par JSON.stringify aux DEUX endroits où il entre dans l'expression.
// Le message d'échec l'interpolait autrefois brut : un sélecteur à guillemets doubles (c'est-à-dire
// tous ceux de Sift, du type data-view="revue") fermait la chaîne JS et produisait un SyntaxError
// côté page, donc l'eval entier échouait et le clic n'avait jamais lieu. Audit 2026-07-28, SIMP-4.
// Attention en éditant cette fonction : elle est écrite dans un template literal, un backtick dans
// un commentaire à l'intérieur fermerait la chaîne et casserait tout le script.
async function cmdClick(selector) {
  const expr = `(() => {
    const el = document.querySelector(${JSON.stringify(selector)});
    if (!el) return "NOT FOUND: " + ${JSON.stringify(selector)};
    const rect = el.getBoundingClientRect();
    el.dispatchEvent(new MouseEvent("click", { clientX: rect.left + rect.width/2, clientY: rect.top + rect.height/2, bubbles: true }));
    return "clicked";
  })()`;
  await cmdEval(expr);
}

// Repo-specific shortcut: HMR reloads reset Sift's nav to Accueil. This re-navigates to
// Revue, opens the first visible track row/queue item, and expands "Diagnostic audio" —
// the exact 3-click sequence needed before most report-view.ts/filing.ts CDP checks.
async function cmdOpenTrack() {
  const expr = `(async () => {
    document.querySelector('[data-view="revue"]')?.dispatchEvent(new MouseEvent("click", {bubbles:true}));
    await new Promise(r => setTimeout(r, 400));
    const firstTrack = document.querySelector('.qi, .lr, [data-bib="row"]');
    firstTrack?.dispatchEvent(new MouseEvent("click", {bubbles:true}));
    await new Promise(r => setTimeout(r, 500));
    const toggle = document.querySelector(".sift-sg-toggle");
    const body = document.querySelector(".sift-sg-body");
    if (toggle && body && !body.classList.contains("is-open")) {
      toggle.dispatchEvent(new MouseEvent("click", {bubbles:true}));
    }
    await new Promise(r => setTimeout(r, 900));
    return JSON.stringify({
      firstTrackFound: !!firstTrack,
      toggleFound: !!toggle,
      canvasReady: !!document.querySelector(".sift-spectro-canvas"),
    });
  })()`;
  await cmdEval(expr);
}

(async () => {
  try {
    if (cmd === "eval") await cmdEval(cmdArgs[0]);
    else if (cmd === "screenshot") await cmdScreenshot(cmdArgs[0] || "screenshot.png");
    else if (cmd === "click") await cmdClick(cmdArgs[0]);
    else if (cmd === "open-track") await cmdOpenTrack();
    else {
      console.error("usage: cdp.cjs [--port 9222] [--reload] <eval|screenshot|click|open-track> [args]");
      process.exit(1);
    }
  } catch (e) {
    console.error("error:", e.message || e);
    process.exit(1);
  }
})();
