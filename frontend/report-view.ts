// Shared analysis-report view (Tauri only): verdict, signals, waveform, on-demand
// spectrogram. Can render inline into a container (Revue #mid pane) or as a modal
// (debug button on an arbitrary picked file). Queries are scoped to a root element so
// inline + modal can't clash on ids.
import { analyzePath } from "./ipc";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import WaveSurfer from "wavesurfer.js";
import type { AnalysisReport } from "../shared/contracts";
import { requireEl } from "./dom";

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
    ".sift-spin{display:inline-block;animation:sift-spin 1s linear infinite}";
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
    return { label: `MP3 ≈ ${estKbps(r.cutoff_hz)} kbps — à vérifier`, bg: "var(--color-background-warning)", fg: "var(--color-text-warning)" };
  // genuine: describe the actual quality, not a yes/no
  const real =
    r.declared_rail === "lossless"
      ? "lossless · pleine bande"
      : r.declared_bitrate
        ? `${r.declared_bitrate} kbps réels`
        : "qualité authentique";
  return { label: real, bg: "var(--color-background-success)", fg: "var(--color-text-success)" };
}

function spectroCaption(v: AnalysisReport["verdict"]): string {
  if (v === "fake") return "coupure nette = transcodage probable";
  if (v === "grey") return "à vérifier visuellement";
  return "énergie pleine bande = encodage conforme";
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

export function row(label: string, value: string): string {
  return `<div class="sift-row"><span class="sift-row-label">${label}</span><span class="sift-row-value">${value}</span></div>`;
}

// ── HTML helpers ────────────────────────────────────────────────────────────

/** Keyboard-hint row for the bottom action rail (filing.ts), matching the board's `kbd` line —
 *  the maquette anchors these to the rail, not the scrollable detail content. */
export function keyboardHintsHtml(): string {
  const k = (key: string, what: string) => `<span><b>${key}</b> ${what}</span>`;
  return (
    `<div class="sift-kbd-hints">` +
    k("SPACE", "écouter") + k("ENTER", "ranger") + k("BKSP", "jeter") + k("HAUT/BAS", "naviguer") +
    `</div>`
  );
}

/** Single header, folded into the player card itself (2026-07-02: the standalone Hero above the
 *  player was pure duplication — same title/artist/path, twice). Cover (real art once identified,
 *  a minimalist vinyl placeholder via `.sift-cover-frame`'s CSS until then) + title + artist ·
 *  version + raw path, optionally a close button (`openReportModal`'s popup only). Keeps the
 *  shared `.sift-report-cover`/`.sift-report-name`/`.sift-report-sub` hooks that filing.ts writes
 *  into (cover src on identify, clean displayName on reconcile). */
function playerHeaderHtml(name: string, path: string, closeBtn: boolean): string {
  return (
    `<div class="sift-player-header">` +
    `<div class="sift-cover-frame">` +
    `<img class="sift-report-cover sift-player-cover" hidden alt="">` +
    `</div>` +
    `<div class="sift-player-header-body">` +
    `<div class="sift-report-name sift-player-name">${esc(name)}</div>` +
    `<div class="sift-report-sub sift-player-sub"></div>` +
    `<div class="sift-player-path">${esc(path)}</div>` +
    `</div>` +
    (closeBtn ? `<button class="sift-close sift-report-close">fermer</button>` : "") +
    `</div>`
  );
}

function playerRowHtml(name: string, path: string, closeBtn = false): string {
  return (
    `<div class="sift-player-row">` +
    playerHeaderHtml(name, path, closeBtn) +
    `<div class="sift-player-audition">` +
    `<button class="sift-play sift-play-btn" title="Lecture / pause (espace)"><i class="ti ti-player-play"></i></button>` +
    `<div class="sift-wave-wrap is-paused">` +
    `<div class="sift-wave sift-player-wave"></div>` +
    `<div class="sift-wave-hover"></div>` +
    `<span class="sift-time-elapsed">0:00</span>` +
    `<span class="sift-time-total">0:00</span>` +
    `</div>` +
    `</div>` +
    `<div class="sift-player-controls">` +
    `<div class="sift-slider-block">` +
    `<span class="sift-slider-label">Volume</span>` +
    `<div class="sift-slider-track sift-volume-track">` +
    `<div class="sift-slider-rail"></div>` +
    `<div class="sift-slider-fill sift-volume-fill"></div>` +
    `<div class="sift-slider-thumb sift-volume-thumb"></div>` +
    `</div></div>` +
    `<div class="sift-player-spacer"></div>` +
    `<div class="sift-key-block" title="Key-lock : le tempo ne change pas la tonalité (off = varispeed)">` +
    `<span class="sift-slider-label">Key-lock</span>` +
    `<button class="sift-key sift-key-btn">ON</button>` +
    `</div>` +
    `<div class="sift-slider-block">` +
    `<span class="sift-slider-label">Tempo<span class="sift-tempo-out">0%</span></span>` +
    `<div class="sift-slider-track sift-tempo-track" title="Tempo — double-clic = réinitialiser">` +
    `<div class="sift-slider-rail"></div>` +
    `<div class="sift-slider-fill sift-tempo-fill"></div>` +
    `<div class="sift-slider-thumb sift-tempo-thumb"></div>` +
    `</div></div>` +
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
          : "background:var(--overlay-selected);color:var(--color-text-secondary)";
  return `<span class="sift-vchip" style="${css}">${esc(label)}</span>`;
}

/** ACTUAL verdict panel, faithful to the Penpot board: a verdict-tinted panel (`vb`) with an
 *  action headline ("Ready to file" etc.) over a chip row. The first chip (LOSSLESS / real
 *  quality) comes from the analysis; the `.sift-vchips` row is left open so filing.ts can append
 *  the MATCH% (identify) and UNIQUE/DUPLICATE (dedup) chips it owns the data for. */
export function verdictCardHtml(r: AnalysisReport): string {
  const map = {
    ok: ["ti-circle-check", "Prêt à ranger", "var(--color-text-success)", "rgba(91,192,140,.2)"],
    fake: ["ti-alert-triangle", "Sur-encodé — à re-sourcer", "var(--color-text-danger)", "rgba(226,104,94,.16)"],
    grey: ["ti-help-circle", "À vérifier d'abord", "var(--color-text-warning)", "rgba(221,166,63,.16)"],
  } as const;
  const [icon, label, fg, panelBg] = map[r.verdict];
  const rq = realQuality(r);
  const qualityChip =
    r.verdict === "ok" && r.declared_rail === "lossless"
      ? vchipHtml("LOSSLESS", "success")
      : vchipHtml(rq.label, r.verdict === "fake" ? "danger" : r.verdict === "grey" ? "warning" : "neutral");
  return (
    `<div class="sift-verdict-card" style="background:${panelBg}">` +
    `<div class="sift-verdict-main">` +
    `<div class="sift-verdict-head"><i class="ti ${icon}" style="color:${fg}"></i><span class="sift-verdict-label" style="color:${fg}">${label}</span></div>` +
    `<div class="sift-vchips sift-vchips-row">${qualityChip}</div>` +
    `</div>` +
    `<div class="sift-verdict-finalname-col">` +
    `<div class="sift-verdict-finalname-label">Nom final</div>` +
    `<div class="sift-verdict-finalname" style="color:${fg}"></div>` +
    `</div>` +
    `</div>`
  );
}

function spectroAndTagsHtml(r: AnalysisReport): string {
  const yn = (b: boolean) => (b ? "oui" : "non");
  return (
    `<div class="sift-spectro-box">` +
    `<button class="sift-sg-toggle sift-spectro-toggle">` +
    `<span class="sift-spectro-toggle-label"><span class="sift-sg-caret sift-spectro-caret">▸</span> Preuve (spectre)</span>` +
    `<span class="sift-sg-hint sift-spectro-hint">afficher</span>` +
    `</button>` +
    `<div class="sift-sg-body sift-spectro-body">` +
    `<div class="sift-spectro-declared">Déclaré <span class="pill">${esc(r.declared_format)}</span> ${r.declared_rail}${r.declared_bitrate ? " · " + r.declared_bitrate + " kbps" : ""} · coupure ${fmt(r.cutoff_hz, 0)} Hz — ${spectroCaption(r.verdict)}</div>` +
    `<canvas class="sift-sg sift-spectro-canvas" width="720" height="180"></canvas>` +
    `<div class="sift-spectro-rows">` +
    row("Verdict", r.verdict) +
    row("Coupure", fmt(r.cutoff_hz, 0) + " Hz") +
    row("Durée", fmt(r.duration_sec, 1) + " s") +
    row("Canaux", String(r.channels) + (r.dual_mono ? " (dual-mono)" : "")) +
    row("True-peak", fmt(r.true_peak_dbtp, 2) + " dBTP") +
    row("DC offset", fmt(r.dc_offset, 5)) +
    row("Écrêtage", r.clip_runs + " runs / " + fmt(r.clip_pct, 2) + "%") +
    row("Corrélation de phase", fmt(r.phase_correlation, 3)) +
    row("Silence début", r.silence_head_ms + " ms") +
    row("Silence fin", r.silence_tail_ms + " ms") +
    row("Tronqué", yn(r.truncated)) +
    row("Conteneur OK", yn(r.container_ok)) +
    row("Fréquence d'échantillonnage", r.sample_rate + " Hz") +
    row("Pics (couverture)", peaksCoverage(r)) +
    `</div></div></div>` +
    // Tags CDJ OK / Version ID3 moved to the Identification card (filing.ts, alongside Label/
    // Année/Genre) — Pochette dropped entirely (redondant avec la pochette déjà visible dans le
    // hero). Nothing meaningful was left in the old "Tags" box, so it's gone too; codec_error is
    // its own standalone diagnostic, not tied to those three fields.
    (r.codec_error ? `<div class="sift-codec-error">erreur codec : ${esc(r.codec_error)}</div>` : "")
  );
}

/** Report HTML minus the verdict conclusion (name + player row + spectrogram/tags). The verdict
 *  is rendered separately, after Identification, by the caller (see `verdictContainer` on
 *  `openReportInto`/`renderReportInto`) — it's the CONCLUSION and must come last, right above
 *  the action rail, matching the maquette. `openReportModal` (no Identification card) appends
 *  `verdictCardHtml` itself, right after this. */
function reportHtml(r: AnalysisReport, closeBtn: boolean): string {
  const name = r.path.split(/[\\/]/).pop() || r.path;
  return (
    playerRowHtml(name, r.path, closeBtn) +
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

// Session cache of already-decoded audio (path → WAV blob), so switching back to a track
// already opened this session skips the fetch + full decodeAudioData entirely — that decode
// (not analysis) is the real cost of a queue switch. Capped small: an 8-min stereo 16-bit
// WAV is ~80MB, so this is a short "recently played" window, not a full-library cache.
const MAX_DECODED_CACHE = 4;
const decodedCache = new Map<string, Blob>();

function cacheDecoded(path: string, blob: Blob): void {
  decodedCache.delete(path);
  decodedCache.set(path, blob);
  if (decodedCache.size > MAX_DECODED_CACHE) {
    const oldest = decodedCache.keys().next().value;
    if (oldest !== undefined) decodedCache.delete(oldest);
  }
}

/** Drops cached decoded audio (call alongside clearReportCache when a file's content
 *  may have changed, e.g. re-analysed/replaced) so a stale decode is never replayed. */
export function clearDecodedCache(path?: string): void {
  if (path) decodedCache.delete(path);
  else decodedCache.clear();
}

/** Load a file the browser can't play natively (AIFF) by decoding it with Web Audio and
 * feeding the player a WAV blob. Falls back to the backend transcode if Web Audio refuses. */
async function loadDecoded(ws: WaveSurfer, path: string): Promise<void> {
  const cached = decodedCache.get(path);
  if (cached) {
    cacheDecoded(path, cached); // bump recency
    if (ws !== currentWs) return;
    await ws.loadBlob(cached);
    return;
  }
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
    const wav = audioBufferToWav(audioBuf);
    cacheDecoded(path, wav);
    await ws.loadBlob(wav);
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
  const container = requireEl<HTMLElement>(".sift-wave", "mountPlayer", root);
  const playBtn = root.querySelector<HTMLButtonElement>(".sift-play");
  const tempoOut = root.querySelector<HTMLElement>(".sift-tempo-out");
  const volumeTrack = root.querySelector<HTMLElement>(".sift-volume-track");
  const volumeFill = root.querySelector<HTMLElement>(".sift-volume-fill");
  const volumeThumb = root.querySelector<HTMLElement>(".sift-volume-thumb");
  const tempoTrack = root.querySelector<HTMLElement>(".sift-tempo-track");
  const tempoFill = root.querySelector<HTMLElement>(".sift-tempo-fill");
  const tempoThumb = root.querySelector<HTMLElement>(".sift-tempo-thumb");

  ensureStyles();
  destroyPlayer();
  const ws = WaveSurfer.create({
    container,
    height: 46,
    barWidth: 2,
    barGap: 1,
    barRadius: 1,
    cursorWidth: 0,
    waveColor: "rgba(255,255,255,.35)",
    progressColor: "#ff5500",
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
  let tempoValue = 0; // -8..8, drives both playback rate and the custom slider visuals
  const applyRate = () => ws.setPlaybackRate(1 + tempoValue / 100, keyLock);
  const refreshKey = () => {
    if (!keyEl) return;
    keyEl.textContent = keyLock ? "ON" : "OFF";
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

  // Custom sliders (never native <input type=range> — see DESIGN.md): drag anywhere on the
  // track, thumb/fill follow the mouse until release. Volume fills from the left; tempo fills
  // from the centre (0 = neutral), matching the pitch-fader convention.
  const dragSlider = (track: HTMLElement, onMove: (pct: number) => void) => {
    const update = (clientX: number) => {
      const rect = track.getBoundingClientRect();
      onMove(Math.max(0, Math.min(1, (clientX - rect.left) / Math.max(1, rect.width))));
    };
    track.addEventListener("mousedown", (e) => {
      update(e.clientX);
      const onMouseMove = (ev: MouseEvent) => update(ev.clientX);
      const onMouseUp = () => {
        window.removeEventListener("mousemove", onMouseMove);
        window.removeEventListener("mouseup", onMouseUp);
      };
      window.addEventListener("mousemove", onMouseMove);
      window.addEventListener("mouseup", onMouseUp);
    });
  };

  const renderVolume = (pct: number) => {
    if (volumeFill) volumeFill.style.width = `${pct * 100}%`;
    if (volumeThumb) volumeThumb.style.left = `${pct * 100}%`;
  };
  renderVolume(1); // WaveSurfer's own default (full volume)
  if (volumeTrack) {
    dragSlider(volumeTrack, (pct) => {
      ws.setVolume(pct);
      renderVolume(pct);
    });
  }

  const renderTempo = () => {
    const pct = ((tempoValue + 8) / 16) * 100;
    if (tempoFill) {
      const left = Math.min(pct, 50);
      tempoFill.style.left = `${left}%`;
      tempoFill.style.width = `${Math.abs(pct - 50)}%`;
    }
    if (tempoThumb) tempoThumb.style.left = `${pct}%`;
    if (tempoOut) tempoOut.textContent = `${tempoValue > 0 ? "+" : ""}${tempoValue}%`;
    applyRate();
  };
  renderTempo();
  if (tempoTrack) {
    dragSlider(tempoTrack, (pct) => {
      tempoValue = Math.max(-8, Math.min(8, Math.round(-8 + pct * 16)));
      renderTempo();
    });
    tempoTrack.addEventListener("dblclick", () => {
      tempoValue = 0;
      renderTempo();
    });
  }
  // SoundCloud-style: elapsed + total shown at once, overlaid on the waveform itself — no
  // elapsed/remaining toggle needed since both numbers are always visible together.
  const timeElapsedEl = root.querySelector<HTMLElement>(".sift-time-elapsed");
  const timeTotalEl = root.querySelector<HTMLElement>(".sift-time-total");
  const updateTime = () => {
    if (timeElapsedEl) timeElapsedEl.textContent = mmss(ws.getCurrentTime());
    if (timeTotalEl) timeTotalEl.textContent = mmss(ws.getDuration());
  };
  ws.on("ready", () => {
    applyRate();
    updateTime();
  });
  ws.on("timeupdate", updateTime);

  // Waveform dims a touch while paused (and re-lights on hover, so scrubbing/seeking a paused
  // track still reads clearly) — `.is-paused` starts set in the HTML (nothing is playing yet).
  const waveWrapEl = root.querySelector<HTMLElement>(".sift-wave-wrap");
  ws.on("play", () => {
    setIcon("player-pause");
    waveWrapEl?.classList.remove("is-paused");
  });
  ws.on("pause", () => {
    setIcon("player-play");
    waveWrapEl?.classList.add("is-paused");
  });
  ws.on("finish", () => {
    setIcon("player-play");
    waveWrapEl?.classList.add("is-paused");
  });

  // Hover-scrub preview: lighten the waveform from the start up to the cursor, so hovering
  // previews where a click would seek to — dimmer than the actual orange playhead fill.
  const waveHoverEl = root.querySelector<HTMLElement>(".sift-wave-hover");
  if (waveHoverEl) {
    container.addEventListener("mousemove", (e) => {
      const rect = container.getBoundingClientRect();
      const pct = Math.max(0, Math.min(1, (e.clientX - rect.left) / Math.max(1, rect.width)));
      waveHoverEl.style.width = `${pct * 100}%`;
    });
    container.addEventListener("mouseleave", () => {
      waveHoverEl.style.width = "0";
    });
  }
  ws.on("error", (e) => {
    console.error("wavesurfer error", e);
    // route to the Rust log so it shows in the dev console (webview console isn't readable here)
    void invoke("report_smoke", { ok: false, detail: `wavesurfer ${path}: ${String(e)}` });
    // Audio always loads via loadDecoded, which already cascades Web Audio → backend transcode,
    // so there's nothing further to retry here — just surface the error.
  });
  playBtn?.addEventListener("click", () => void ws.playPause());
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

/** Renders the report INLINE into `container` (e.g. the Revue #mid pane). `verdictContainer`,
 *  when given, gets the verdict conclusion card instead of `container` — see `openReportInto`. */
export function renderReportInto(
  container: HTMLElement,
  r: AnalysisReport,
  verdictContainer?: HTMLElement,
) {
  container.innerHTML = `<div class="sift-report-scroll">${reportHtml(r, false)}</div>`;
  if (verdictContainer) verdictContainer.innerHTML = verdictCardHtml(r);
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
  clearDecodedCache(path);
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
export async function openReportInto(
  container: HTMLElement,
  path: string,
  verdictContainer?: HTMLElement,
): Promise<AnalysisReport | null> {
  destroyPlayer();
  ensureStyles();
  const seq = ++openSeq;

  const cached = reportCache.get(path);
  if (cached) {
    renderReportInto(container, cached, verdictContainer);
    return cached;
  }

  const name = path.split(/[\\/]/).pop() || path;

  // Fire analysis IPC immediately. For already-analyzed tracks the DB round-trip takes ~20ms.
  const analysisPromise = analyzePath(path, false);

  // Render the player shell. Son-first order: player (header+audition) → proof (Preuves). The
  // verdict conclusion goes LAST, above the action rail — in `verdictContainer` when the caller
  // supplies one (filing.ts/library-detail.ts, both of which insert Identification between here
  // and their own verdict slot), else in a `.sift-verdict-stub` kept inside this same scroll
  // (openReportModal, which has no Identification card of its own). Filled in later (seq-guarded).
  const verdictHost = () => verdictContainer ?? container.querySelector<HTMLElement>(".sift-verdict-stub");
  container.innerHTML =
    `<div class="sift-report-scroll">` +
    playerRowHtml(name, path) +
    `<div class="sift-analysis-body" hidden></div>` +
    (verdictContainer ? "" : `<div class="sift-verdict-stub"></div>`) +
    `</div>`;

  // Race the analysis against a short timeout. For already-analyzed tracks (DB cache hit)
  // we win the race and can pass peaks to WaveSurfer.create() — which renders the waveform
  // instantly from the pre-computed data. For fresh tracks the timeout fires first and we
  // mount without peaks so audio starts loading while analysis runs in the background.
  // 300ms (not 20-80ms): the DB hit itself is fast, but the full invoke round-trip (IPC
  // dispatch + JSON (de)serialization of the report incl. the peaks array) regularly exceeds
  // 80ms in a `tauri dev` debug build, which was tripping the timeout — and showing the
  // "Analyse en cours…" stub — for tracks that were in fact already analyzed.
  const earlyResult = await Promise.race([
    analysisPromise.catch((): null => null),
    new Promise<null>((res) => setTimeout(() => res(null), 300)),
  ]) as AnalysisReport | null;

  if (seq !== openSeq) return null;

  if (earlyResult) {
    reportCache.set(path, earlyResult);
    // Pass peaks to the constructor — the only path that renders the waveform immediately.
    void mountPlayer(container, path, earlyResult.peaks, earlyResult.duration_sec || undefined);
    const verdictEl = verdictHost();
    const bodyEl = container.querySelector<HTMLElement>(".sift-analysis-body");
    if (verdictEl) verdictEl.innerHTML = verdictCardHtml(earlyResult);
    if (bodyEl) {
      bodyEl.innerHTML = spectroAndTagsHtml(earlyResult);
      bodyEl.hidden = false;
      wireSpectrogram(container, earlyResult);
    }
    return earlyResult;
  }

  // Timeout fired — this is a genuinely fresh track (no DB cache to hit), so the wait is
  // real. Only now does the loader text get shown.
  const pendingEl = verdictHost();
  if (pendingEl) {
    pendingEl.innerHTML = `<i class="ti ti-loader-2 sift-spin"></i>Analyse en cours…`;
  }
  void mountPlayer(container, path);

  try {
    const r = await analysisPromise;
    reportCache.set(path, r);
    if (seq !== openSeq) return null;
    const verdictEl = verdictHost();
    const bodyEl = container.querySelector<HTMLElement>(".sift-analysis-body");
    if (verdictEl) verdictEl.innerHTML = verdictCardHtml(r);
    if (bodyEl) {
      bodyEl.innerHTML = spectroAndTagsHtml(r);
      bodyEl.hidden = false;
      wireSpectrogram(container, r);
    }
    return r;
  } catch (e) {
    console.error("analyze_path failed", e);
    if (seq !== openSeq) return null;
    const verdictEl = verdictHost();
    if (verdictEl) {
      verdictEl.innerHTML =
        `<div class="sift-analysis-fail">Échec de l'analyse : ${esc(String(e))}</div>`;
    }
    return null;
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
  ov.className = "sift-report-overlay";
  ov.addEventListener("click", (e) => {
    if (e.target === ov) {
      destroyPlayer();
      ov.remove();
    }
  });
  document.body.appendChild(ov);
  const name = path.split(/[\\/]/).pop() || path;
  ov.innerHTML = `<div class="sift-report-overlay-card sift-report-overlay-loading"><i class="ti ti-loader-2 sift-spin"></i>Analyse de <strong>${esc(name)}</strong>…</div>`;
  try {
    const r = await analyzePath(path, false);
    const card = document.createElement("div");
    card.className = "sift-report-overlay-card sift-report-overlay-modal";
    card.innerHTML = reportHtml(r, true) + verdictCardHtml(r);
    ov.innerHTML = "";
    ov.appendChild(card);
    card.querySelector(".sift-close")?.addEventListener("click", () => {
      destroyPlayer();
      ov.remove();
    });
    wireReport(card, r);
  } catch (e) {
    console.error("analyze_path failed", e);
    ov.innerHTML = `<div class="sift-report-overlay-card sift-report-overlay-error">Échec de l'analyse : ${esc(String(e))}</div>`;
  }
}
