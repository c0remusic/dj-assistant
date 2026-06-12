// Debug analysis tester — ACTIVE ONLY inside the Tauri app. Floating button → native file
// picker → analyze_path → full report overlay with waveform + spectrogram (cutoff line).
// Throwaway-ish dev tool to judge verdict quality before M2c builds the real Revue UI.
import { analyzePath } from "./ipc";
import { open } from "@tauri-apps/plugin-dialog";
import type { AnalysisReport } from "../shared/contracts";

const AUDIO_EXTS = ["mp3", "flac", "wav", "aif", "aiff", "m4a", "aac", "ogg", "opus"];
const PEAKS_WINDOW = 512; // must match analysis::PEAKS_WINDOW (mono samples per peak)

const esc = (s: string) =>
  s.replace(/[&<>"]/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : "&quot;",
  );

const fmt = (n: number, d = 1) =>
  Number.isFinite(n) ? n.toFixed(d) : String(n);

function verdictBadge(v: AnalysisReport["verdict"]): string {
  const map = {
    ok: ["✓ Authentique", "#5cc97a", "#1f3a24"],
    fake: ["✗ Fake (transcodé)", "#ff6b6b", "#3a1f1f"],
    grey: ["? Zone grise", "#f0c060", "#3a331f"],
  } as const;
  const [label, fg, bg] = map[v];
  return `<span style="display:inline-block;padding:4px 12px;border-radius:6px;font-weight:600;font-size:14px;color:${fg};background:${bg};border:1px solid ${fg}">${label}</span>`;
}

/** Draws the peaks envelope as a mirrored waveform — MAX over each pixel's bucket so no
 * transient between sample points is skipped (avoids the "sparse/excerpt" look). */
function drawWaveform(canvas: HTMLCanvasElement, peaks: number[]) {
  const ctx = canvas.getContext("2d");
  if (!ctx || peaks.length === 0) return;
  const w = canvas.width, h = canvas.height, mid = h / 2;
  ctx.clearRect(0, 0, w, h);
  ctx.fillStyle = "#8ecce8";
  const per = peaks.length / w; // peaks per pixel column
  for (let x = 0; x < w; x++) {
    const start = Math.floor(x * per);
    const end = Math.max(start + 1, Math.floor((x + 1) * per));
    let m = 0;
    for (let i = start; i < end && i < peaks.length; i++) {
      if (peaks[i] > m) m = peaks[i];
    }
    const bar = m * mid;
    ctx.fillRect(x, mid - bar, 1, Math.max(1, bar * 2));
  }
}

/** Renders the spectrogram (mag_db, row-major by frame) + a red cutoff line. */
function drawSpectrogram(canvas: HTMLCanvasElement, r: AnalysisReport) {
  const ctx = canvas.getContext("2d");
  const sg = r.spectrogram;
  if (!ctx || sg.frames === 0 || sg.bins === 0) return;
  const w = canvas.width, h = canvas.height;
  const img = ctx.createImageData(w, h);
  for (let x = 0; x < w; x++) {
    const f = Math.min(sg.frames - 1, Math.floor((x / w) * sg.frames));
    for (let y = 0; y < h; y++) {
      // bottom = low freq, top = high freq
      const b = Math.min(sg.bins - 1, Math.floor(((h - 1 - y) / h) * sg.bins));
      const val = sg.mag_db[f * sg.bins + b] || 0; // 0..255
      const i = (y * w + x) * 4;
      // simple blue→white heat
      img.data[i] = val;
      img.data[i + 1] = val;
      img.data[i + 2] = Math.min(255, 60 + val);
      img.data[i + 3] = 255;
    }
  }
  ctx.putImageData(img, 0, 0);
  // cutoff line
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

/** Diagnostic: how much of the track the peaks actually cover, vs the declared duration.
 * If "couvert" << "durée", the decode stopped early (not just a rendering issue). */
function peaksCoverage(r: AnalysisReport): string {
  const sr = r.sample_rate || 44100;
  const covered = (r.peaks.length * PEAKS_WINDOW) / sr;
  const pct = r.duration_sec > 0 ? (covered / r.duration_sec) * 100 : 0;
  return `${r.peaks.length} pts ≈ ${covered.toFixed(1)}s / ${r.duration_sec.toFixed(1)}s (${pct.toFixed(0)}%)`;
}

function row(label: string, value: string): string {
  return `<div style="display:flex;justify-content:space-between;gap:16px;padding:3px 0;border-bottom:1px solid rgba(237,233,224,.08)"><span style="color:var(--color-text-tertiary)">${label}</span><span style="font-family:monospace;text-align:right">${value}</span></div>`;
}

function showReport(r: AnalysisReport) {
  document.getElementById("sift-test-overlay")?.remove();
  const ov = document.createElement("div");
  ov.id = "sift-test-overlay";
  ov.style.cssText =
    "position:fixed;inset:0;z-index:9999;background:rgba(0,0,0,.6);display:flex;align-items:center;justify-content:center;padding:24px";
  const yn = (b: boolean) => (b ? "oui" : "non");
  const name = r.path.split(/[\\/]/).pop() || r.path;

  ov.innerHTML = `
    <div style="background:#1a1a1a;color:#ede9e0;border:1px solid rgba(237,233,224,.15);border-radius:12px;max-width:760px;width:100%;max-height:90vh;overflow:auto;padding:20px;box-shadow:0 12px 48px rgba(0,0,0,.5)">
      <div style="display:flex;justify-content:space-between;align-items:flex-start;gap:12px;margin-bottom:14px">
        <div><div style="font-size:15px;font-weight:600;word-break:break-all">${esc(name)}</div>
          <div style="font-size:11px;color:var(--color-text-tertiary);word-break:break-all;margin-top:2px">${esc(r.path)}</div></div>
        <button id="sift-test-close" style="flex:none;font-size:13px;padding:4px 10px">fermer</button>
      </div>
      <div style="margin-bottom:14px">${verdictBadge(r.verdict)}
        <span style="margin-left:10px;font-size:12px;color:var(--color-text-tertiary)">déclaré ${esc(r.declared_format)} · ${r.declared_rail}${r.declared_bitrate ? " · " + r.declared_bitrate + " kbps" : ""}</span></div>

      <div id="sift-sg-section" style="margin-bottom:12px;border:1px solid rgba(237,233,224,.12);border-radius:8px;overflow:hidden">
        <button id="sift-sg-toggle" style="width:100%;display:flex;align-items:center;justify-content:space-between;gap:8px;padding:10px 12px;background:rgba(237,233,224,.04);border:none;color:#ede9e0;cursor:pointer;font-size:12px;text-align:left">
          <span style="display:flex;align-items:center;gap:8px"><span id="sift-sg-caret" style="display:inline-block;transition:transform .25s;color:var(--color-text-tertiary)">▸</span> Spectrogramme <span style="color:var(--color-text-tertiary)">— preuve visuelle de la coupure</span></span>
          <span id="sift-sg-hint" style="font-size:11px;color:#FFdc82;flex:none">afficher</span>
        </button>
        <div id="sift-sg-body" style="max-height:0;overflow:hidden;transition:max-height .3s ease">
          <canvas id="sift-sg" width="720" height="180" style="width:100%;display:block;background:#000"></canvas>
        </div>
      </div>
      <div style="font-size:11px;color:var(--color-text-tertiary);margin:0 0 4px">Waveform</div>
      <canvas id="sift-wf" width="720" height="70" style="width:100%;border-radius:6px;background:#101418;margin-bottom:14px"></canvas>

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
      ${r.codec_error ? `<div style="margin-top:12px;font-size:11px;color:#ff6b6b">codec error: ${esc(r.codec_error)}</div>` : ""}
    </div>`;

  document.body.appendChild(ov);
  ov.addEventListener("click", (e) => {
    if (e.target === ov) ov.remove();
  });
  document.getElementById("sift-test-close")?.addEventListener("click", () => ov.remove());
  drawWaveform(document.getElementById("sift-wf") as HTMLCanvasElement, r.peaks);

  const sgCanvas = document.getElementById("sift-sg") as HTMLCanvasElement;
  const toggle = document.getElementById("sift-sg-toggle") as HTMLButtonElement;
  const body = document.getElementById("sift-sg-body") as HTMLElement;
  const caret = document.getElementById("sift-sg-caret") as HTMLElement;
  const hint = document.getElementById("sift-sg-hint") as HTMLElement;

  let open = false;
  let loaded = false;
  let busy = false;

  toggle.addEventListener("click", async () => {
    if (busy) return;
    if (open) {
      open = false;
      body.style.maxHeight = "0";
      caret.style.transform = "";
      hint.textContent = "afficher";
      return;
    }
    // opening: load the heavy grid on first open only
    if (!loaded) {
      busy = true;
      hint.textContent = "calcul…";
      try {
        const full = r.spectrogram.frames > 0 ? r : await analyzePath(r.path, true);
        drawSpectrogram(sgCanvas, full);
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

async function pickAndAnalyze(btn: HTMLButtonElement) {
  const sel = await open({
    multiple: false,
    filters: [{ name: "Audio", extensions: AUDIO_EXTS }],
  });
  if (typeof sel !== "string") return;
  const prev = btn.textContent;
  btn.textContent = "⏳ analyse…";
  btn.disabled = true;
  try {
    const report = await analyzePath(sel);
    showReport(report);
  } catch (e) {
    console.error("analyze_path failed", e);
    alert("Analyse échouée : " + String(e));
  } finally {
    btn.textContent = prev;
    btn.disabled = false;
  }
}

export function installTestButton() {
  const btn = document.createElement("button");
  btn.id = "sift-test-btn";
  btn.textContent = "🔬 Tester l'analyse";
  btn.style.cssText =
    "position:fixed;right:16px;bottom:16px;z-index:9998;padding:9px 14px;font-size:13px;font-weight:600;border-radius:8px;background:#FFdc82;color:#1a1a1a;border:none;cursor:pointer;box-shadow:0 4px 16px rgba(0,0,0,.35)";
  btn.addEventListener("click", () => void pickAndAnalyze(btn));
  document.body.appendChild(btn);
}
