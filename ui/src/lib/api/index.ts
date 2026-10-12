import { invoke } from "@tauri-apps/api/core";
import type { AppInfo } from "./types";

export type { AppInfo } from "./types";

/** Returns the backend application's name and version. */
export function appInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}
