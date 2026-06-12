// Debug analysis tester — ACTIVE ONLY inside the Tauri app. Floating button → native file
// picker → report overlay (shared with the Revue queue via report-view).
import { open } from "@tauri-apps/plugin-dialog";
import { openReportFor } from "./report-view";

const AUDIO_EXTS = ["mp3", "flac", "wav", "aif", "aiff", "m4a", "aac", "ogg", "opus"];

async function pickAndAnalyze() {
  const sel = await open({
    multiple: false,
    filters: [{ name: "Audio", extensions: AUDIO_EXTS }],
  });
  if (typeof sel === "string") void openReportFor(sel);
}

export function installTestButton() {
  const btn = document.createElement("button");
  btn.id = "sift-test-btn";
  btn.textContent = "🔬 Tester l'analyse";
  btn.style.cssText =
    "position:fixed;right:16px;bottom:16px;z-index:9998;padding:9px 14px;font-size:13px;font-weight:600;border-radius:8px;background:#FFdc82;color:#1a1a1a;border:none;cursor:pointer;box-shadow:0 4px 16px rgba(0,0,0,.35)";
  btn.addEventListener("click", () => void pickAndAnalyze());
  document.body.appendChild(btn);
}
