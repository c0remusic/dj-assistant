// Shared analysis-report view (Tauri only): verdict, signals, waveform, on-demand
// spectrogram. Can render inline into a container (Revue #mid pane) or as a modal
// (debug button on an arbitrary picked file). Queries are scoped to a root element so
// inline + modal can't clash on ids.
import { analyzePath } from "./ipc";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import WaveSurfer from "wavesurfer.js";
import type { AnalysisReport } from "../shared/contracts";

const PEAKS_WINDOW = 512; // must match analysis::PEAKS_WINDOW

// Single live player at a time — destroyed before any re-render so audio never lingers.
let currentWs: WaveSurfer | null = null;
function destroyPlayer() {
  if (currentWs) {
    try {
      currentWs.destroy();
    } catch {
      /* already gone */
    }
    currentWs = null;
  }
}

/** Toggle play/pause on the current report player (for the Space keyboard shortcut). */
export function togglePlay() {
  void currentWs?.playPause();
}

// One-time hover styling for the clickable time display (inline styles can't do :hover).
function ensureStyles() {
  if (document.getElementById("sift-report-style")) return;
  const st = document.createElement("style");
  st.id = "sift-report-style";
  st.textContent =
    ".sift-time:hover{color:var(--color-text-primary)!important}" +
    "@keyframes sift-spin{to{transform:rotate(360deg)}}" +
    ".sift-spin{display:inline-block;animation:sift-spin 1s linear infinite}" +
    // Custom tempo thumb: grey at neutral, blue once nudged either way (accent-color only
    // tints the fill on one side of 0, so we colour the thumb explicitly via a class).
    ".sift-tempo{-webkit-appearance:none;appearance:none;background:transparent;cursor:pointer}" +
    ".sift-tempo::-webkit-slider-runnable-track{width:4px;border-radius:3px;background:var(--color-border-secondary)}" +
    // Turntable-style pitch-fader cap: wide flat handle with a centre marker line, recentred
    // on the thin track (margin-left = (track − thumb) / 2).
    ".sift-tempo::-webkit-slider-thumb{-webkit-appearance:none;appearance:none;width:20px;height:9px;margin-left:-8px;border-radius:2px;border:0.5px solid var(--color-border-secondary);background:linear-gradient(var(--color-text-tertiary) 0 44%,rgba(0,0,0,.5) 44% 56%,var(--color-text-tertiary) 56% 100%)}" +
    ".sift-tempo.sift-active::-webkit-slider-thumb{background:linear-gradient(var(--color-text-info) 0 44%,rgba(0,0,0,.5) 44% 56%,var(--color-text-info) 56% 100%)}";
  document.head.appendChild(st);
}

const mmss = (s: number) => {
  if (!Number.isFinite(s)) return "0:00";
  const m = Math.floor(s / 60);
  return `${m}:${String(Math.floor(s % 60)).padStart(2, "0")}`;
};

const esc = (s: string) =>
  s.replace(/[&<>"']/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : c === '"' ? "&quot;" : "&#39;",
  );
const fmt = (n: number, d = 1) => (Number.isFinite(n) ? n.toFixed(d) : String(n));

/** The file's REAL quality (what the audio actually is), derived from the analysis — shown
 * next to what it was declared as. */
function realQuality(r: AnalysisReport): { label: string; bg: string; fg: string } {
  // Real quality of a transcode, expressed as the equivalent MP3 bitrate inferred from the
  // measured low-pass cutoff (LAME-style: 16k≈128, 17k≈160, 19k≈192, 20k≈256, 20.5k≈320).
  // The exact cutoff stays in the foldable "infos" — here we show what the audio is worth.
  const estKbps = (hz: number) =>
    hz >= 20000 ? 320 : hz >= 19000 ? 256 : hz >= 18000 ? 192 : hz >= 16500 ? 160 : 128;
  if (r.verdict === "fake") {
    return {
      label: `MP3 ≈ ${estKbps(r.cutoff_hz)} kbps`,
      bg: "var(--color-background-danger)",
      fg: "var(--color-text-danger)",
    };
  }
  if (r.verdict === "grey")
    return { label: `MP3 ≈ ${estKbps(r.cutoff_hz)} kbps — check`, bg: "var(--color-background-warning)", fg: "var(--color-text-warning)" };
  // genuine: describe the actual quality, not a yes/no
  const real =
    r.declared_rail === "lossless"
      ? "lossless · full-band"
      : r.declared_bitrate
        ? `${r.declared_bitrate} kbps actual`
        : "genuine quality";
  return { label: real, bg: "var(--color-background-success)", fg: "var(--color-text-success)" };
}

function spectroCaption(v: AnalysisReport["verdict"]): string {
  if (v === "fake") return "sharp cutoff = likely transcode";
  if (v === "grey") return "inspect visually";
  return "full-band energy = compliant encoding";
}

function drawSpectrogram(canvas: HTMLCanvasElement, r: AnalysisReport) {
  const ctx = canvas.getContext("2d");
  const sg = r.spectrogram;
  if (!ctx || sg.frames === 0 || sg.bins === 0) return;
  const w = canvas.width, h = canvas.height;
  const img = ctx.createImageData(w, h);
  for (let x = 0; x < w; x++) {
    const f = Math.min(sg.frames - 1, Math.floor((x / w) * sg.frames));
    for (let y = 0; y < h; y++) {
      const b = Math.min(sg.bins - 1, Math.floor(((h - 1 - y) / h) * sg.bins));
      const val = sg.mag_db[f * sg.bins + b] || 0;
      const i = (y * w + x) * 4;
      img.data[i] = val;
      img.data[i + 1] = val;
      img.data[i + 2] = Math.min(255, 60 + val);
      img.data[i + 3] = 255;
    }
  }
  ctx.putImageData(img, 0, 0);
  const nyquist = sg.bins * sg.hz_per_bin;
  if (r.cutoff_hz > 0 && nyquist > 0) {
    const y = h - (r.cutoff_hz / nyquist) * h;
    ctx.strokeStyle = "#ff5050";
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    ctx.moveTo(0, y);
    ctx.lineTo(w, y);
    ctx.stroke();
    ctx.fillStyle = "#ff5050";
    ctx.font = "11px monospace";
    ctx.fillText(`cutoff ${(r.cutoff_hz / 1000).toFixed(1)} kHz`, 6, Math.max(12, y - 4));
  }
}

function peaksCoverage(r: AnalysisReport): string {
  const sr = r.sample_rate || 44100;
  const covered = (r.peaks.length * PEAKS_WINDOW) / sr;
  const pct = r.duration_sec > 0 ? (covered / r.duration_sec) * 100 : 0;
  return `${r.peaks.length} pts ≈ ${covered.toFixed(1)}s / ${r.duration_sec.toFixed(1)}s (${pct.toFixed(0)}%)`;
}

function row(label: string, value: string): string {
  return `<div style="display:flex;justify-content:space-between;gap:16px;padding:3px 0;border-bottom:0.5px solid var(--color-border-tertiary)"><span style="color:var(--color-text-tertiary)">${label}</span><span style="font-family:var(--font-mono);text-align:right;color:var(--color-text-secondary)">${value}</span></div>`;
}

// ── HTML helpers ────────────────────────────────────────────────────────────

function nameHeaderHtml(name: string, path: string, closeBtn: boolean): string {
  return (
    `<div style="display:flex;justify-content:space-between;align-items:flex-start;gap:12px;margin-bottom:12px">` +
    `<img class="sift-report-cover" hidden alt="" style="width:40px;height:40px;border-radius:4px;object-fit:cover;flex:none">` +
    `<div style="min-width:0;flex:1"><div class="sift-report-name" style="font-size:13px;font-weight:600;word-break:break-all;color:var(--color-text-primary)">${esc(name)}</div>` +
    `<div style="font-size:10px;color:var(--color-text-tertiary);font-family:var(--font-mono);word-break:break-all;margin-top:2px">${esc(path)}</div></div>` +
    (closeBtn ? `<button class="sift-close" style="flex:none;font-size:13px;padding:4px 10px">close</button>` : "") +
    `</div>`
  );
}

/** Son-first hero for the inline (#mid) detail: enlarged cover (filled on identify) + the
 *  proposed/clean name + raw path. Keeps the `.sift-report-cover` / `.sift-report-name` hooks
 *  that filing.ts writes into (cover src on identify, clean displayName on reconcile). */
function heroHtml(name: string, path: string): string {
  return (
    `<div style="display:flex;align-items:center;gap:14px;margin-bottom:18px">` +
    `<img class="sift-report-cover" hidden alt="" style="width:64px;height:64px;border-radius:var(--border-radius-lg);object-fit:cover;flex:none;background:var(--color-background-secondary)">` +
    `<div style="min-width:0;flex:1">` +
    `<div class="sift-report-name" style="font-size:26px;font-weight:600;line-height:1.15;color:var(--color-text-primary);word-break:break-word">${esc(name)}</div>` +
    `<div class="sift-report-sub" style="font-size:13px;color:var(--color-text-secondary);margin-top:2px"></div>` +
    `<div style="font-size:10px;color:var(--color-text-tertiary);font-family:var(--font-mono);word-break:break-all;margin-top:5px">${esc(path)}</div>` +
    `</div></div>`
  );
}

/** Keyboard-hint footer under the detail, matching the board's `kbd` line. */
function keyboardHintsHtml(): string {
  const k = (key: string, what: string) =>
    `<span><b style="font-weight:600;color:var(--color-text-secondary)">${key}</b> ${what}</span>`;
  return (
    `<div style="margin-top:18px;font-size:10px;letter-spacing:.03em;color:var(--color-text-tertiary);display:flex;gap:16px;flex-wrap:wrap">` +
    k("SPACE", "listen") + k("ENTER", "file") + k("BKSP", "discard") + k("↑↓", "navigate") +
    `</div>`
  );
}

function playerRowHtml(): string {
  return (
    `<div style="display:flex;align-items:center;gap:12px;margin-bottom:11px;padding:8px 11px;min-height:80px;background:var(--color-background-secondary);border-radius:var(--border-radius-md)">` +
    `<div style="flex:none;align-self:stretch;width:62px;position:relative;display:flex;flex-direction:column;align-items:center;justify-content:center">` +
    `<button class="sift-play" title="Play / pause (space)" style="flex:none;width:30px;height:30px;border-radius:50%;display:inline-flex;align-items:center;justify-content:center;padding:0"><i class="ti ti-player-play" style="font-size:13px"></i></button>` +
    `<span class="sift-time" title="Click: elapsed ⇄ remaining" style="position:absolute;bottom:0;left:50%;transform:translateX(-50%);white-space:nowrap;font-family:var(--font-mono);font-size:9px;color:var(--color-text-secondary);cursor:pointer;transition:color .15s;display:inline-flex;align-items:center;justify-content:center;gap:3px"><span class="sift-time-val">0:00 / 0:00</span></span>` +
    `</div>` +
    `<div class="sift-wave" style="flex:1;min-width:0;align-self:center;cursor:pointer"></div>` +
    `<div style="flex:none;align-self:stretch;width:64px;display:flex;flex-direction:column;align-items:center;justify-content:space-between;gap:6px;padding:3px 0">` +
    `<span style="font-size:8px;letter-spacing:.05em;text-transform:uppercase;color:var(--color-text-tertiary)">tempo</span>` +
    `<div style="flex:1;min-height:0;display:flex;align-items:center;gap:4px">` +
    `<input class="sift-tempo" type="range" min="-8" max="8" step="1" value="0" title="Tempo — double-click = reset" aria-label="Tempo" style="writing-mode:vertical-lr;direction:rtl;width:22px;height:100%;max-height:42px">` +
    `<span class="sift-tempo-out" style="font-family:var(--font-mono);font-size:8px;color:var(--color-text-secondary);width:22px">0%</span>` +
    `</div>` +
    `<button class="sift-key" title="Key-lock: tempo doesn't change pitch (off = varispeed)" style="border:0.5px solid var(--color-border-tertiary);border-radius:var(--border-radius-md);padding:2px 8px;font-size:8px;letter-spacing:.05em;text-transform:uppercase">key</button>` +
    `</div></div>`
  );
}

/** A verdict-panel chip: `success` = green-tinted (LOSSLESS), `neutral` = white@.06 (MATCH/UNIQUE),
 *  matching the Penpot `badge-*` shapes (see .interface-design/penpot-detail-spec.md). */
export function vchipHtml(label: string, tone: "success" | "neutral" | "danger" | "warning"): string {
  const css =
    tone === "success"
      ? "background:var(--color-background-success);color:var(--color-text-success)"
      : tone === "danger"
        ? "background:var(--color-background-danger);color:var(--color-text-danger)"
        : tone === "warning"
          ? "background:var(--color-background-warning);color:var(--color-text-warning)"
          : "background:rgba(255,255,255,.06);color:var(--color-text-secondary)";
  return `<span style="display:inline-flex;align-items:center;padding:3px 9px;border-radius:999px;font-size:10px;font-weight:600;letter-spacing:.03em;${css}">${esc(label)}</span>`;
}

/** ACTUAL verdict panel, faithful to the Penpot board: a verdict-tinted panel (`vb`) with an
 *  action headline ("Ready to file" etc.) over a chip row. The first chip (LOSSLESS / real
 *  quality) comes from the analysis; the `.sift-vchips` row is left open so filing.ts can append
 *  the MATCH% (identify) and UNIQUE/DUPLICATE (dedup) chips it owns the data for. */
function verdictCardHtml(r: AnalysisReport): string {
  const map = {
    ok: ["ti-circle-check", "Ready to file", "var(--color-text-success)", "rgba(91,192,140,.2)"],
    fake: ["ti-alert-triangle", "Over-encoded — re-source", "var(--color-text-danger)", "rgba(226,104,94,.16)"],
    grey: ["ti-help-circle", "Inspect first", "var(--color-text-warning)", "rgba(221,166,63,.16)"],
  } as const;
  const [icon, label, fg, panelBg] = map[r.verdict];
  const rq = realQuality(r);
  const qualityChip =
    r.verdict === "ok" && r.declared_rail === "lossless"
      ? vchipHtml("LOSSLESS", "success")
      : vchipHtml(rq.label, r.verdict === "fake" ? "danger" : r.verdict === "grey" ? "warning" : "neutral");
  return (
    `<div style="background:${panelBg};border-radius:var(--border-radius-lg);padding:12px 14px;margin-bottom:12px">` +
    `<div style="display:flex;align-items:center;gap:7px;margin-bottom:9px"><i class="ti ${icon}" style="font-size:16px;color:${fg}"></i><span style="font-size:16px;font-weight:600;color:${fg}">${label}</span></div>` +
    `<div class="sift-vchips" style="display:flex;align-items:center;gap:6px;flex-wrap:wrap">${qualityChip}</div>` +
    `</div>`
  );
}

function spectroAndTagsHtml(r: AnalysisReport): string {
  const yn = (b: boolean) => (b ? "yes" : "no");
  return (
    `<div style="margin-bottom:11px;border:0.5px solid var(--color-border-secondary);border-radius:var(--border-radius-md);overflow:hidden">` +
    `<button class="sift-sg-toggle" style="width:100%;display:flex;align-items:center;justify-content:space-between;gap:8px;padding:9px 11px;background:var(--color-background-secondary);border:none;color:var(--color-text-primary);cursor:pointer;font-size:11px;text-align:left">` +
    `<span style="display:flex;align-items:center;gap:8px"><span class="sift-sg-caret" style="display:inline-block;transition:transform .25s;color:var(--color-text-tertiary)">▸</span> Proof (spectrum)</span>` +
    `<span class="sift-sg-hint" style="font-size:11px;color:var(--color-text-info);flex:none">show</span>` +
    `</button>` +
    `<div class="sift-sg-body" style="max-height:0;overflow:hidden;transition:max-height .3s ease">` +
    `<div style="padding:8px 11px;font-size:10px;color:var(--color-text-tertiary);border-bottom:0.5px solid var(--color-border-tertiary);line-height:1.5">Declared <span class="pill">${esc(r.declared_format)}</span> ${r.declared_rail}${r.declared_bitrate ? " · " + r.declared_bitrate + " kbps" : ""} · cutoff ${fmt(r.cutoff_hz, 0)} Hz — ${spectroCaption(r.verdict)}</div>` +
    `<canvas class="sift-sg" width="720" height="180" style="width:100%;display:block;background:#000"></canvas>` +
    `<div style="padding:9px 11px;display:grid;grid-template-columns:1fr 1fr;gap:0 28px;font-size:12px">` +
    row("Verdict", r.verdict) +
    row("Cutoff", fmt(r.cutoff_hz, 0) + " Hz") +
    row("Duration", fmt(r.duration_sec, 1) + " s") +
    row("Channels", String(r.channels) + (r.dual_mono ? " (dual-mono)" : "")) +
    row("True-peak", fmt(r.true_peak_dbtp, 2) + " dBTP") +
    row("DC offset", fmt(r.dc_offset, 5)) +
    row("Clipping", r.clip_runs + " runs / " + fmt(r.clip_pct, 2) + "%") +
    row("Phase correlation", fmt(r.phase_correlation, 3)) +
    row("Silence head", r.silence_head_ms + " ms") +
    row("Silence tail", r.silence_tail_ms + " ms") +
    row("Truncated", yn(r.truncated)) +
    row("Container OK", yn(r.container_ok)) +
    row("Sample rate", r.sample_rate + " Hz") +
    row("Peaks (coverage)", peaksCoverage(r)) +
    `</div></div></div>` +
    `<div style="margin-bottom:4px">` +
    `<div style="font-size:9px;letter-spacing:.05em;text-transform:uppercase;color:var(--color-text-tertiary);margin-bottom:5px">Tags</div>` +
    `<div style="display:grid;grid-template-columns:1fr 1fr;gap:0 28px;font-size:12px">` +
    row("Tags CDJ OK", yn(r.tags_cdj_ok)) +
    row("Cover", yn(r.has_cover)) +
    row("ID3 version", r.id3_version || "—") +
    `</div></div>` +
    (r.codec_error ? `<div style="margin-top:12px;font-size:11px;color:#ff6b6b">codec error: ${esc(r.codec_error)}</div>` : "")
  );
}

/** Full report HTML (name + verdict chain + player row + spectrogram + tags). */
function reportHtml(r: AnalysisReport, closeBtn: boolean): string {
  const name = r.path.split(/[\\/]/).pop() || r.path;
  return (
    nameHeaderHtml(name, r.path, closeBtn) +
    verdictCardHtml(r) +
    playerRowHtml() +
    spectroAndTagsHtml(r)
  );
}

// One shared AudioContext for decoding formats the <audio> element can't play (AIFF).
let decodeCtx: AudioContext | null = null;

/** Wrap a decoded AudioBuffer as an in-memory 16-bit PCM WAV blob (lossless container swap;
 * AIFF and WAV are both PCM). Lets wavesurfer's media element play AIFF content the browser
 * decoded natively via Web Audio. */
export function audioBufferToWav(buf: AudioBuffer): Blob {
  const numCh = buf.numberOfChannels;
  const sr = buf.sampleRate;
  const len = buf.length;
  const blockAlign = numCh * 2;
  const dataLen = len * blockAlign;
  const ab = new ArrayBuffer(44 + dataLen);
  const view = new DataView(ab);
  const w = (off: number, s: string) => {
    for (let i = 0; i < s.length; i++) view.setUint8(off + i, s.charCodeAt(i));
  };
  w(0, "RIFF");
  view.setUint32(4, 36 + dataLen, true);
  w(8, "WAVE");
  w(12, "fmt ");
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, numCh, true);
  view.setUint32(24, sr, true);
  view.setUint32(28, sr * blockAlign, true);
  view.setUint16(32, blockAlign, true);
  view.setUint16(34, 16, true);
  w(36, "data");
  view.setUint32(40, dataLen, true);
  const chans: Float32Array[] = [];
  for (let c = 0; c < numCh; c++) chans.push(buf.getChannelData(c));
  let off = 44;
  for (let i = 0; i < len; i++) {
    for (let c = 0; c < numCh; c++) {
      const s = Math.max(-1, Math.min(1, chans[c][i]));
      view.setInt16(off, s < 0 ? s * 0x8000 : s * 0x7fff, true);
      off += 2;
    }
  }
  return new Blob([ab], { type: "audio/wav" });
}

/** Load a file the browser can't play natively (AIFF) by decoding it with Web Audio and
 * feeding the player a WAV blob. Falls back to the backend transcode if Web Audio refuses. */
async function loadDecoded(ws: WaveSurfer, path: string): Promise<void> {
  // Each await yields the event loop; the user may switch tracks meanwhile, which
  // destroys this ws and creates a new currentWs. Bail if we're no longer current,
  // so we never call loadBlob/load on a destroyed instance.
  try {
    const resp = await fetch(convertFileSrc(path));
    const arr = await resp.arrayBuffer();
    if (ws !== currentWs) return;
    if (!decodeCtx) decodeCtx = new AudioContext();
    const audioBuf = await decodeCtx.decodeAudioData(arr);
    if (ws !== currentWs) return;
    await ws.loadBlob(audioBufferToWav(audioBuf));
  } catch (e) {
    if (ws !== currentWs) return;
    console.error("web-audio decode failed, falling back to transcode", e);
    try {
      const src = await invoke<string>("playback_url", { path });
      if (ws !== currentWs) return;
      await ws.load(convertFileSrc(src));
    } catch (e2) {
      console.error("playback fallback failed", e2);
    }
  }
}

/** Mounts a wavesurfer player on the report's player row. Tempo uses the browser's native
 * time-stretch (`preservesPitch`) for key-lock — adequate for the ±8% DJ nudge; SoundTouch.js
 * was evaluated and skipped (would require re-architecting playback to Web Audio for marginal
 * gain at this range). See docs/ressources-externes.md.
 * `peaks` and `duration` are optional hints for the initial waveform display — audio
 * loads via the Web-Audio decode path regardless (direct asset-protocol load aborts). */
async function mountPlayer(root: HTMLElement, path: string, peaks?: number[], duration?: number) {
  const container = root.querySelector<HTMLElement>(".sift-wave");
  const playBtn = root.querySelector<HTMLButtonElement>(".sift-play");
  const timeEl = root.querySelector<HTMLElement>(".sift-time");
  const tempo = root.querySelector<HTMLInputElement>(".sift-tempo");
  const tempoOut = root.querySelector<HTMLElement>(".sift-tempo-out");
  if (!container) return;

  ensureStyles();
  destroyPlayer();
  const ws = WaveSurfer.create({
    container,
    height: 46,
    waveColor: "rgba(142,204,232,.45)",
    progressColor: "#8ecce8",
    cursorColor: "#FFdc82",
    normalize: true,
    peaks: peaks?.length ? [peaks] : undefined,
    duration: duration || undefined,
  });
  currentWs = ws;
  void loadDecoded(ws, path);

  const setIcon = (name: string) => {
    const i = playBtn?.querySelector("i");
    if (i) i.className = `ti ti-${name}`;
  };
  const keyEl = root.querySelector<HTMLButtonElement>(".sift-key");
  let keyLock = true; // DJ default: tempo doesn't move the pitch (browser time-stretch)
  const applyRate = () => ws.setPlaybackRate(1 + Number(tempo?.value || 0) / 100, keyLock);
  const refreshKey = () => {
    if (!keyEl) return;
    keyEl.style.background = keyLock ? "var(--color-background-info)" : "transparent";
    keyEl.style.color = keyLock ? "var(--color-text-info)" : "var(--color-text-tertiary)";
    keyEl.style.borderColor = keyLock ? "var(--color-border-info)" : "var(--color-border-tertiary)";
  };
  keyEl?.addEventListener("click", () => {
    keyLock = !keyLock;
    refreshKey();
    applyRate();
  });
  refreshKey();
  const timeVal = root.querySelector<HTMLElement>(".sift-time-val");
  let showRemaining = false;
  const updateTime = () => {
    if (!timeVal) return;
    const cur = ws.getCurrentTime(), dur = ws.getDuration();
    const left = showRemaining ? `-${mmss(dur - cur)}` : mmss(cur);
    timeVal.textContent = `${left} / ${mmss(dur)}`;
  };
  timeEl?.addEventListener("click", () => {
    showRemaining = !showRemaining;
    updateTime();
  });
  ws.on("ready", () => {
    applyRate();
    updateTime();
  });
  ws.on("timeupdate", updateTime);
  ws.on("play", () => setIcon("player-pause"));
  ws.on("pause", () => setIcon("player-play"));
  ws.on("finish", () => setIcon("player-play"));
  ws.on("error", (e) => {
    console.error("wavesurfer error", e);
    // route to the Rust log so it shows in the dev console (webview console isn't readable here)
    void invoke("report_smoke", { ok: false, detail: `wavesurfer ${path}: ${String(e)}` });
    // Audio always loads via loadDecoded, which already cascades Web Audio → backend transcode,
    // so there's nothing further to retry here — just surface the error.
  });
  playBtn?.addEventListener("click", () => void ws.playPause());
  const refreshTempo = () => {
    const v = Number(tempo!.value);
    if (tempoOut) tempoOut.textContent = `${v > 0 ? "+" : ""}${v}%`;
    // grey at neutral (0), coloured once nudged
    tempo!.classList.toggle("sift-active", v !== 0);
    applyRate();
  };
  tempo?.addEventListener("input", refreshTempo);
  refreshTempo();
  // double-click the fader → reset tempo/pitch to 0
  tempo?.addEventListener("dblclick", () => {
    tempo.value = "0";
    refreshTempo();
  });
}

/** Wires the spectrogram toggle inside `root` (extracted so it can be called
 * independently of player mounting — used after async analysis fill-in). */
function wireSpectrogram(root: HTMLElement, r: AnalysisReport) {
  const sg = root.querySelector<HTMLCanvasElement>(".sift-sg");
  const toggle = root.querySelector<HTMLButtonElement>(".sift-sg-toggle");
  const body = root.querySelector<HTMLElement>(".sift-sg-body");
  const caret = root.querySelector<HTMLElement>(".sift-sg-caret");
  const hint = root.querySelector<HTMLElement>(".sift-sg-hint");
  if (!sg || !toggle || !body || !caret || !hint) return;

  let open = false, loaded = false, busy = false;
  toggle.addEventListener("click", async () => {
    if (busy) return;
    if (open) {
      open = false;
      body.style.maxHeight = "0";
      caret.style.transform = "";
      hint.textContent = "show";
      return;
    }
    if (!loaded) {
      busy = true;
      hint.textContent = "computing…";
      try {
        const full = r.spectrogram.frames > 0 ? r : await analyzePath(r.path, true);
        drawSpectrogram(sg, full);
        loaded = true;
      } catch (e) {
        console.error("spectrogram analyze failed", e);
        hint.textContent = "failed — retry";
        busy = false;
        return;
      }
      busy = false;
    }
    open = true;
    caret.style.transform = "rotate(90deg)";
    hint.textContent = "hide";
    body.style.maxHeight = body.scrollHeight + "px";
  });
}

/** Wires the player + spectrogram toggle inside `root` (scoped — no global ids). */
function wireReport(root: HTMLElement, r: AnalysisReport) {
  mountPlayer(root, r.path, r.peaks, r.duration_sec);
  wireSpectrogram(root, r);
}

/** Renders the report INLINE into `container` (e.g. the Revue #mid pane). */
export function renderReportInto(container: HTMLElement, r: AnalysisReport) {
  container.innerHTML = `<div style="flex:1;overflow:auto;padding:2px 2px 8px">${reportHtml(r, false)}</div>`;
  wireReport(container, r);
}

// In-session report cache (path → report). Backend already caches in the DB; this skips even
// the IPC round-trip + loading spinner on revisits, so switching back to a track is instant.
const reportCache = new Map<string, AnalysisReport>();

/** Drops the in-session cache so the next open re-fetches from the backend (DB is the source
 *  of truth). Call when analysis results may have changed (e.g. the `analysis:changed` event)
 *  so a re-analysed or replaced file isn't served stale. */
export function clearReportCache(path?: string) {
  if (path) reportCache.delete(path);
  else reportCache.clear();
}

// Monotonic token: the latest openReportInto call wins. A slow analyse that resolves after the
// user already switched tracks must not overwrite the newer content in the shared container.
let openSeq = 0;

/** Loads (no spectrogram) and renders inline into `container`. Instant when cached.
 *
 * The player is mounted IMMEDIATELY from the path alone, before analysis completes.
 * This eliminates the "player never mounts" race: the old design awaited analyzePath
 * before mounting, and a background event bumping openSeq during that await caused the
 * seq-guard to abort the whole render (player included). Now the seq-guard only aborts
 * the analysis fill-in — the player is already running and stays untouched. */
export async function openReportInto(container: HTMLElement, path: string) {
  destroyPlayer();
  ensureStyles();
  const seq = ++openSeq;

  const cached = reportCache.get(path);
  if (cached) {
    renderReportInto(container, cached);
    return;
  }

  const name = path.split(/[\\/]/).pop() || path;

  // Fire analysis IPC immediately. For already-analyzed tracks the DB round-trip takes ~20ms.
  const analysisPromise = analyzePath(path, false);

  // Render the player shell. Son-first order: hero → audition band → verdict → proof. The
  // verdict-stub and analysis-body class hooks are filled in later (seq-guarded); their order
  // below the audition is what makes the detail "listen first, judge second".
  container.innerHTML =
    `<div style="flex:1;overflow:auto;padding:2px 2px 8px">` +
    heroHtml(name, path) +
    playerRowHtml() +
    `<div class="sift-verdict-stub" style="display:flex;align-items:center;gap:6px;margin:2px 0 12px;font-size:11px;color:var(--color-text-tertiary)">` +
    `<i class="ti ti-loader-2 sift-spin"></i>Analyzing…</div>` +
    `<div class="sift-analysis-body" hidden></div>` +
    keyboardHintsHtml() +
    `</div>`;

  // Race the analysis against a short timeout. For already-analyzed tracks (~20ms DB hit)
  // we win the race and can pass peaks to WaveSurfer.create() — which renders the waveform
  // instantly from the pre-computed data. For fresh tracks the timeout fires first and we
  // mount without peaks so audio starts loading while analysis runs in the background.
  const earlyResult = await Promise.race([
    analysisPromise.catch((): null => null),
    new Promise<null>((res) => setTimeout(() => res(null), 80)),
  ]) as AnalysisReport | null;

  if (seq !== openSeq) return;

  if (earlyResult) {
    reportCache.set(path, earlyResult);
    // Pass peaks to the constructor — the only path that renders the waveform immediately.
    void mountPlayer(container, path, earlyResult.peaks, earlyResult.duration_sec || undefined);
    const verdictEl = container.querySelector<HTMLElement>(".sift-verdict-stub");
    const bodyEl = container.querySelector<HTMLElement>(".sift-analysis-body");
    if (verdictEl) verdictEl.outerHTML = verdictCardHtml(earlyResult);
    if (bodyEl) {
      bodyEl.innerHTML = spectroAndTagsHtml(earlyResult);
      bodyEl.hidden = false;
      wireSpectrogram(container, earlyResult);
    }
    return;
  }

  // Timeout fired — mount player now so audio starts loading while analysis finishes.
  void mountPlayer(container, path);

  try {
    const r = await analysisPromise;
    reportCache.set(path, r);
    if (seq !== openSeq) return;
    const verdictEl = container.querySelector<HTMLElement>(".sift-verdict-stub");
    const bodyEl = container.querySelector<HTMLElement>(".sift-analysis-body");
    if (verdictEl) verdictEl.outerHTML = verdictCardHtml(r);
    if (bodyEl) {
      bodyEl.innerHTML = spectroAndTagsHtml(r);
      bodyEl.hidden = false;
      wireSpectrogram(container, r);
    }
  } catch (e) {
    console.error("analyze_path failed", e);
    if (seq !== openSeq) return;
    const verdictEl = container.querySelector<HTMLElement>(".sift-verdict-stub");
    if (verdictEl) {
      verdictEl.outerHTML =
        `<div style="margin-bottom:12px;font-size:11px;color:#ff6b6b">Analysis failed: ${esc(String(e))}</div>`;
    }
  }
}

const OVERLAY_ID = "sift-report-overlay";

/** Modal version, for the debug button (a file not in the queue). */
export async function openReportModal(path: string) {
  destroyPlayer();
  ensureStyles();
  document.getElementById(OVERLAY_ID)?.remove();
  const ov = document.createElement("div");
  ov.id = OVERLAY_ID;
  ov.style.cssText =
    "position:fixed;inset:0;z-index:9999;background:rgba(0,0,0,.6);display:flex;align-items:center;justify-content:center;padding:24px";
  ov.addEventListener("click", (e) => {
    if (e.target === ov) {
      destroyPlayer();
      ov.remove();
    }
  });
  document.body.appendChild(ov);
  const name = path.split(/[\\/]/).pop() || path;
  const cardCss =
    "background:var(--color-background-primary);color:var(--color-text-primary);border:0.5px solid var(--color-border-secondary);border-radius:var(--border-radius-lg,12px);box-shadow:0 12px 48px rgba(0,0,0,.5)";
  ov.innerHTML = `<div style="${cardCss};padding:22px 26px;font-size:13px;display:flex;align-items:center;gap:8px"><i class="ti ti-loader-2 sift-spin"></i>Analyzing <strong>${esc(name)}</strong>…</div>`;
  try {
    const r = await analyzePath(path, false);
    const card = document.createElement("div");
    card.style.cssText = `${cardCss};max-width:760px;width:100%;max-height:90vh;overflow:auto;padding:20px`;
    card.innerHTML = reportHtml(r, true);
    ov.innerHTML = "";
    ov.appendChild(card);
    card.querySelector(".sift-close")?.addEventListener("click", () => {
      destroyPlayer();
      ov.remove();
    });
    wireReport(card, r);
  } catch (e) {
    console.error("analyze_path failed", e);
    ov.innerHTML = `<div style="${cardCss};padding:22px 26px;font-size:13px;color:var(--color-text-danger)">Analysis failed: ${esc(String(e))}</div>`;
  }
}
