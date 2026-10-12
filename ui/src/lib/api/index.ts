import type { AppEvent } from "./generated/AppEvent";
import type { AppInfo } from "./generated/AppInfo";
import type { ImportPlanView } from "./generated/ImportPlanView";
import type { ImportStatus } from "./generated/ImportStatus";
import type { Settings } from "./generated/Settings";
import type { SettingsUpdate } from "./generated/SettingsUpdate";
import { selectTransport } from "./transport";

export { ApiError } from "./error";
export type { AppEvent } from "./generated/AppEvent";
export type { AppInfo } from "./generated/AppInfo";
export type { ImportPlanView } from "./generated/ImportPlanView";
export type { ImportStatus } from "./generated/ImportStatus";
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

/** Starts planning an import of the given absolute folders; progress arrives as events. */
export function planImport(sources: string[]): Promise<ImportStatus> {
  return transport.call<ImportStatus>("plan_import", { sources });
}

/** The plan (and, after copying, the outcome) of an import run. */
export function getImport(runId: number): Promise<ImportPlanView> {
  return transport.call<ImportPlanView>("get_import", { run_id: runId });
}

export function importStatus(runId: number): Promise<ImportStatus> {
  return transport.call<ImportStatus>("import_status", { run_id: runId });
}

/** Starts (or resumes) copying a planned import. */
export function startImport(runId: number): Promise<ImportStatus> {
  return transport.call<ImportStatus>("start_import", { run_id: runId });
}

export function cancelImport(runId: number): Promise<ImportStatus> {
  return transport.call<ImportStatus>("cancel_import", { run_id: runId });
}

/** Subscribes to backend events; returns an unsubscribe function. */
export function onAppEvent(handler: (event: AppEvent) => void): () => void {
  return transport.subscribe(handler);
}
