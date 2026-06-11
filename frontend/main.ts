import "./app.js";
import { invoke } from "@tauri-apps/api/core";
import { appInfo, dbHealth, ffmpegVersion } from "./ipc";

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
