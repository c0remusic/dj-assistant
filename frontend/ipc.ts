import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AppInfo, DbHealth, Source, QueueItem } from "../shared/contracts";

export const appInfo = (): Promise<AppInfo> => invoke("app_info");
export const dbHealth = (): Promise<DbHealth> => invoke("db_health");
export const ffmpegVersion = (): Promise<string> => invoke("ffmpeg_version");

export const addSource = (path: string): Promise<Source> =>
  invoke("add_source", { path });
export const listSources = (): Promise<Source[]> => invoke("list_sources");
export const removeSource = (id: number): Promise<void> =>
  invoke("remove_source", { id });
export const listQueue = (): Promise<QueueItem[]> => invoke("list_queue");
export const rescanSource = (id: number): Promise<void> =>
  invoke("rescan_source", { id });
export const setSourceWatched = (id: number, watched: boolean): Promise<void> =>
  invoke("set_source_watched", { id, watched });

/** Subscribe to backend "queue:changed" pings. Returns an unlisten fn. */
export const onQueueChanged = (cb: () => void): Promise<UnlistenFn> =>
  listen("queue:changed", () => cb());
