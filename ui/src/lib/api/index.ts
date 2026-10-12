import type { AppInfo } from "./generated/AppInfo";
import type { Settings } from "./generated/Settings";
import type { SettingsUpdate } from "./generated/SettingsUpdate";
import { selectTransport } from "./transport";

export { ApiError } from "./error";
export type { AppInfo } from "./generated/AppInfo";
export type { Settings } from "./generated/Settings";
export type { SettingsUpdate } from "./generated/SettingsUpdate";

const transport = selectTransport();

/** Returns the backend application's name and version. */
export function appInfo(): Promise<AppInfo> {
  return transport.call<AppInfo>("app_info");
}

/** Returns the saved settings (defaults if none were saved). */
export function getSettings(): Promise<Settings> {
  return transport.call<Settings>("get_settings");
}

/**
 * Validates and saves a change to some settings and returns the result.
 * Rejects with an `invalid_arguments` ApiError whose message explains the problem.
 */
export function updateSettings(update: SettingsUpdate): Promise<Settings> {
  return transport.call<Settings>("update_settings", update);
}
