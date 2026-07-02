// Self-hosted UI fonts — Outfit (UI: 400 body, 600 titles/labels) + JetBrains Mono (numbers).
// Bundled via @fontsource so the desktop app needs no network. See system.md (Typographie).
import "@fontsource/outfit/400.css";
import "@fontsource/outfit/600.css";
import "@fontsource/jetbrains-mono/400.css";
import "./app.js";
import { invoke } from "@tauri-apps/api/core";
import { appInfo, dbHealth, ffmpegVersion } from "./ipc";
import { installLiveWiring } from "./sift-live";

// Only exercise the IPC layer inside the Tauri app. In a plain browser (e.g. the
// Vercel web demo) there is no Tauri runtime — skip it so the UI renders cleanly.
const inTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

if (inTauri) {
  installLiveWiring();
  (async () => {
    try {
      const info = await appInfo();
      const health = await dbHealth();
      const ff = await ffmpegVersion();
      const detail = `${info.name} v${info.version} · db schema=${health.schema_version} tables=${health.tables} · ffmpeg=${ff}`;
      console.log("Sift IPC contract OK", detail);
      await invoke("report_smoke", { ok: true, detail });
    } catch (e) {
      console.error("IPC smoke failed", e);
      await invoke("report_smoke", { ok: false, detail: String(e) });
    }
  })();

  // Headless playback self-test: exercises the real audio-load path on every queued track
  // and logs OK/FAIL per file (no manual clicks). Auto-runs with VITE_SIFT_SELFTEST=1; also
  // exposed as window.__siftSelfTest() to trigger from devtools.
  void import("./selftest").then((m) => {
    (window as { __siftSelfTest?: () => void }).__siftSelfTest = () => void m.runSelfTest();
    if ((import.meta as { env?: Record<string, string> }).env?.VITE_SIFT_SELFTEST === "1") {
      setTimeout(() => void m.runSelfTest(), 2500);
    }
  });
}
