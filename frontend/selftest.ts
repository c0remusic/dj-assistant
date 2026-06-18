// Headless player-load self-test (Tauri only). Lets us verify audio playback loads
// reliably across the WHOLE queue without any manual clicks. For each queued track it
// runs the real load path used by the player — fetch (Tauri asset protocol) →
// decodeAudioData → WAV blob → wavesurfer loadBlob → await "ready" — timing each stage
// and logging OK/FAIL per file to the Rust log via report_smoke (readable from the dev
// server output). Trigger automatically with VITE_SIFT_SELFTEST=1, or from devtools via
// window.__siftSelfTest().
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import WaveSurfer from "wavesurfer.js";
import { listQueue, analyzePath } from "./ipc";
import { audioBufferToWav } from "./report-view";

const log = (ok: boolean, detail: string) => void invoke("report_smoke", { ok, detail });
const ms = (a: number, b: number) => Math.round(b - a);

export async function runSelfTest(limit = 15): Promise<void> {
  let items;
  try {
    items = await listQueue();
  } catch (e) {
    log(false, `SELFTEST listQueue failed: ${String(e)}`);
    return;
  }
  const sample = items.slice(0, limit);
  log(true, `SELFTEST start — ${sample.length}/${items.length} tracks`);

  // Off-screen host for the throwaway players.
  const host = document.createElement("div");
  host.style.cssText = "position:fixed;left:-9999px;top:0;width:200px;height:40px";
  document.body.appendChild(host);

  let ctx: AudioContext | null = null;
  let ok = 0,
    fail = 0;

  for (const it of sample) {
    const base = it.path.split(/[\\/]/).pop() || it.path;
    const ext = (it.path.split(".").pop() || "").toLowerCase();
    const t0 = performance.now();
    try {
      // Mirror mountPlayer faithfully: get the analysis (peaks + duration), create the player
      // WITH peaks+duration, then load via the decode path. This reproduces the exact runtime
      // combination the player uses, so a flaky "ready" here = the real bug, headless.
      const r = await analyzePath(it.path, false);
      const tAna = performance.now();
      const resp = await fetch(convertFileSrc(it.path));
      if (!resp.ok) throw new Error(`fetch ${resp.status}`);
      const arr = await resp.arrayBuffer();
      const tFetch = performance.now();
      if (!ctx) ctx = new AudioContext();
      const buf = await ctx.decodeAudioData(arr);
      const tDec = performance.now();

      const ws = WaveSurfer.create({
        container: host,
        height: 1,
        normalize: true,
        peaks: r.peaks.length ? [r.peaks] : undefined,
        duration: r.duration_sec || undefined,
      });
      const ready = new Promise<string>((res) => {
        ws.on("ready", () => res("ready"));
        ws.on("error", (e) => res(`error:${String(e)}`));
        setTimeout(() => res("timeout"), 10000);
      });
      void ws.loadBlob(audioBufferToWav(buf));
      const outcome = await ready;
      const tEnd = performance.now();
      try {
        ws.destroy();
      } catch {
        /* ignore */
      }

      if (outcome === "ready") {
        ok++;
        log(
          true,
          `SELFTEST OK ${base} [${ext}] ana=${ms(t0, tAna)}ms fetch=${ms(tAna, tFetch)}ms decode=${ms(tFetch, tDec)}ms ready=${ms(tDec, tEnd)}ms dur=${Math.round(buf.duration)}s`,
        );
      } else {
        fail++;
        log(false, `SELFTEST FAIL(${outcome}) ${base} [${ext}] decoded-in=${ms(t0, tDec)}ms`);
      }
    } catch (e) {
      fail++;
      log(false, `SELFTEST FAIL(load) ${base} [${ext}]: ${String(e)}`);
    }
  }

  try {
    if (ctx) await ctx.close();
  } catch {
    /* ignore */
  }
  host.remove();
  log(true, `SELFTEST done — ${ok} ok / ${fail} fail (of ${sample.length})`);
}
