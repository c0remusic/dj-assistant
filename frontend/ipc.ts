import { invoke } from "@tauri-apps/api/core";
import type { AppInfo, DbHealth } from "../shared/contracts";

export const appInfo = (): Promise<AppInfo> => invoke("app_info");
export const dbHealth = (): Promise<DbHealth> => invoke("db_health");
export const ffmpegVersion = (): Promise<string> => invoke("ffmpeg_version");
