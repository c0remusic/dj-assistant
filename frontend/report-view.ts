// Shared analysis-report view (Tauri only): verdict, signals, waveform, on-demand
// spectrogram. Can render inline into a container (Revue #mid pane) or as a modal
// (debug button on an arbitrary picked file). Queries are scoped to a root element so
// inline + modal can't clash on ids.
import { analyzePath } from "./ipc";
import { convertFileSrc } from "@tauri-apps/api/core";
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
  s.replace(/[&<>"]/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : "&quot;",
  );
const fmt = (n: number, d = 1) => (Number.isFinite(n) ? n.toFixed(d) : String(n));

function verdictBadge(v: AnalysisReport["verdict"]): string {
  const map = {
    ok: ["ti-shield-check", "Authentique", "var(--color-background-success)", "var(--color-text-success)"],
    fake: ["ti-alert-triangle", "Fake / sur-encodé", "var(--color-background-danger)", "var(--color-text-danger)"],
    grey: ["ti-help-circle", "Zone grise", "var(--color-background-warning)", "var(--color-text-warning)"],
  } as const;
  const [icon, label, bg, fg] = map[v];
  return `<span style="display:inline-flex;align-items:center;gap:5px;padding:3px 10px;border-radius:var(--border-radius-md);font-weight:600;font-size:12px;color:${fg};background:${bg}"><i class="ti ${icon}" style="font-size:13px"></i>${label}</span>`;
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

/** The report's inner HTML (no positioning chrome). `closeBtn` adds a "fermer" button. */
function reportHtml(r: AnalysisReport, closeBtn: boolean): string {
  const yn = (b: boolean) => (b ? "oui" : "non");
  const name = r.path.split(/[\\/]/).pop() || r.path;
  return `
    <div style="display:flex;justify-content:space-between;align-items:flex-start;gap:12px;margin-bottom:12px">
      <div style="min-width:0"><div style="font-size:13px;font-weight:600;word-break:break-all;color:var(--color-text-primary)">${esc(name)}</div>
        <div style="font-size:10px;color:var(--color-text-tertiary);font-family:var(--font-mono);word-break:break-all;margin-top:2px">${esc(r.path)}</div></div>
      ${closeBtn ? '<button class="sift-close" style="flex:none;font-size:13px;padding:4px 10px">fermer</button>' : ""}
    </div>
    <div style="display:flex;align-items:center;gap:9px;margin-bottom:12px;flex-wrap:wrap">${verdictBadge(r.verdict)}
      <span style="font-size:11px;color:var(--color-text-tertiary)">déclaré <span class="pill">${esc(r.declared_format)}</span> ${r.declared_rail}${r.declared_bitrate ? " · " + r.declared_bitrate + " kbps" : ""}</span></div>

    <div style="display:flex;align-items:center;gap:12px;margin-bottom:11px;padding:8px 11px;min-height:80px;background:var(--color-background-secondary);border-radius:var(--border-radius-md)">
      <div style="flex:none;align-self:stretch;width:62px;position:relative;display:flex;flex-direction:column;align-items:center;justify-content:center">
        <button class="sift-play" title="Lecture / pause" style="flex:none;width:30px;height:30px;border-radius:50%;display:inline-flex;align-items:center;justify-content:center;padding:0"><i class="ti ti-player-play" style="font-size:13px"></i></button>
        <span class="sift-time" title="Cliquer : écoulé ⇄ restant" style="position:absolute;bottom:0;left:50%;transform:translateX(-50%);white-space:nowrap;font-family:var(--font-mono);font-size:9px;color:var(--color-text-secondary);cursor:pointer;transition:color .15s;display:inline-flex;align-items:center;justify-content:center;gap:3px"><span class="sift-time-val">0:00 / 0:00</span></span>
      </div>
      <div class="sift-wave" style="flex:1;min-width:0;align-self:center;cursor:pointer"></div>
      <div style="flex:none;align-self:stretch;width:62px;position:relative;display:flex;align-items:center;justify-content:center">
        <span style="position:absolute;top:0;left:0;right:0;text-align:center;font-size:8px;letter-spacing:.05em;text-transform:uppercase;color:var(--color-text-tertiary)">tempo</span>
        <input class="sift-tempo" type="range" min="-8" max="8" step="1" value="0" title="Tempo — double-clic = reset" aria-label="Tempo" style="writing-mode:vertical-lr;direction:rtl;width:22px;height:38px">
        <span class="sift-tempo-out" style="position:absolute;right:2px;top:50%;transform:translateY(-50%);font-family:var(--font-mono);font-size:8px;color:var(--color-text-secondary)">0%</span>
        <button class="sift-key" title="Key-lock : le tempo ne change pas le pitch (off = varispeed)" style="position:absolute;bottom:0;border:0.5px solid var(--color-border-tertiary);border-radius:var(--border-radius-md);padding:1px 8px;font-size:8px;letter-spacing:.05em;text-transform:uppercase">key</button>
      </div>
    </div>
    <div style="margin-bottom:11px;border:0.5px solid var(--color-border-secondary);border-radius:var(--border-radius-md);overflow:hidden">
      <button class="sift-sg-toggle" style="width:100%;display:flex;align-items:center;justify-content:space-between;gap:8px;padding:9px 11px;background:var(--color-background-secondary);border:none;color:var(--color-text-primary);cursor:pointer;font-size:11px;text-align:left">
        <span style="display:flex;align-items:center;gap:8px"><span class="sift-sg-caret" style="display:inline-block;transition:transform .25s;color:var(--color-text-tertiary)">▸</span> Spectrogramme <span style="color:var(--color-text-tertiary)">— preuve visuelle de la coupure</span></span>
        <span class="sift-sg-hint" style="font-size:11px;color:var(--color-text-info);flex:none">afficher</span>
      </button>
      <div class="sift-sg-body" style="max-height:0;overflow:hidden;transition:max-height .3s ease">
        <canvas class="sift-sg" width="720" height="180" style="width:100%;display:block;background:#000"></canvas>
      </div>
    </div>

    <div style="display:grid;grid-template-columns:1fr 1fr;gap:0 28px;font-size:12px">
      ${row("Verdict", r.verdict)}
      ${row("Coupure", fmt(r.cutoff_hz, 0) + " Hz")}
      ${row("Durée", fmt(r.duration_sec, 1) + " s")}
      ${row("Canaux", String(r.channels) + (r.dual_mono ? " (dual-mono)" : ""))}
      ${row("True-peak", fmt(r.true_peak_dbtp, 2) + " dBTP")}
      ${row("DC offset", fmt(r.dc_offset, 5))}
      ${row("Écrêtage", r.clip_runs + " runs / " + fmt(r.clip_pct, 2) + "%")}
      ${row("Corrélation phase", fmt(r.phase_correlation, 3))}
      ${row("Silence tête", r.silence_head_ms + " ms")}
      ${row("Silence queue", r.silence_tail_ms + " ms")}
      ${row("Tronqué", yn(r.truncated))}
      ${row("Conteneur OK", yn(r.container_ok))}
      ${row("Tags CDJ OK", yn(r.tags_cdj_ok))}
      ${row("Pochette", yn(r.has_cover))}
      ${row("Version ID3", r.id3_version || "—")}
      ${row("Sample rate", r.sample_rate + " Hz")}
      ${row("Peaks (couverture)", peaksCoverage(r))}
    </div>
    ${r.codec_error ? `<div style="margin-top:12px;font-size:11px;color:#ff6b6b">codec error: ${esc(r.codec_error)}</div>` : ""}`;
}

/** Mounts a wavesurfer player on the report's player row (varispeed tempo for now). */
function mountPlayer(root: HTMLElement, r: AnalysisReport) {
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
    url: convertFileSrc(r.path),
    peaks: r.peaks.length ? [r.peaks] : undefined,
    duration: r.duration_sec || undefined,
  });
  currentWs = ws;

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
  ws.on("error", (e) => console.error("wavesurfer error", e));
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

/** Wires the player + spectrogram toggle inside `root` (scoped — no global ids). */
function wireReport(root: HTMLElement, r: AnalysisReport) {
  mountPlayer(root, r);

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
      hint.textContent = "afficher";
      return;
    }
    if (!loaded) {
      busy = true;
      hint.textContent = "calcul…";
      try {
        const full = r.spectrogram.frames > 0 ? r : await analyzePath(r.path, true);
        drawSpectrogram(sg, full);
        loaded = true;
      } catch (e) {
        console.error("spectrogram analyze failed", e);
        hint.textContent = "échec — réessayer";
        busy = false;
        return;
      }
      busy = false;
    }
    open = true;
    caret.style.transform = "rotate(90deg)";
    hint.textContent = "masquer";
    body.style.maxHeight = body.scrollHeight + "px";
  });
}

/** Renders the report INLINE into `container` (e.g. the Revue #mid pane). */
export function renderReportInto(container: HTMLElement, r: AnalysisReport) {
  container.innerHTML = `<div style="flex:1;overflow:auto;padding:2px 2px 8px">${reportHtml(r, false)}</div>`;
  wireReport(container, r);
}

/** Loads (no spectrogram) and renders inline into `container`, with a loading state. */
export async function openReportInto(container: HTMLElement, path: string) {
  destroyPlayer();
  ensureStyles();
  const name = path.split(/[\\/]/).pop() || path;
  container.innerHTML = `<div style="flex:1;display:flex;align-items:center;justify-content:center;gap:8px;color:var(--color-text-tertiary);font-size:13px"><i class="ti ti-loader-2 sift-spin"></i>Analyse de ${esc(name)}…</div>`;
  try {
    const r = await analyzePath(path, false);
    renderReportInto(container, r);
  } catch (e) {
    console.error("analyze_path failed", e);
    container.innerHTML = `<div style="flex:1;display:flex;align-items:center;justify-content:center;color:#ff6b6b;font-size:13px">Analyse échouée : ${esc(String(e))}</div>`;
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
  ov.innerHTML = `<div style="${cardCss};padding:22px 26px;font-size:13px;display:flex;align-items:center;gap:8px"><i class="ti ti-loader-2 sift-spin"></i>Analyse de <strong>${esc(name)}</strong>…</div>`;
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
    ov.innerHTML = `<div style="${cardCss};padding:22px 26px;font-size:13px;color:var(--color-text-danger)">Analyse échouée : ${esc(String(e))}</div>`;
  }
}
