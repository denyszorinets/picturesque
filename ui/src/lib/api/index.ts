import type { AiStatus } from "./generated/AiStatus";
import type { AnalysisStatus } from "./generated/AnalysisStatus";
import type { ModelView } from "./generated/ModelView";
import type { SimilarPhoto } from "./generated/SimilarPhoto";
import type { AppEvent } from "./generated/AppEvent";
import type { AppInfo } from "./generated/AppInfo";
import type { ImportPlanView } from "./generated/ImportPlanView";
import type { ImportStatus } from "./generated/ImportStatus";
import type { DecisionChange } from "./generated/DecisionChange";
import type { DecisionView } from "./generated/DecisionView";
import type { PhotoDetail } from "./generated/PhotoDetail";
import type { PhotoFacets } from "./generated/PhotoFacets";
import type { PhotoPage } from "./generated/PhotoPage";
import type { PhotoQuery } from "./generated/PhotoQuery";
import type { SeriesEditView } from "./generated/SeriesEditView";
import type { SeriesView } from "./generated/SeriesView";
import type { Settings } from "./generated/Settings";
import type { SettingsUpdate } from "./generated/SettingsUpdate";
import type { TemplatePresetView } from "./generated/TemplatePresetView";
import type { TemplatePreview } from "./generated/TemplatePreview";
import { selectTransport, type ImageSize } from "./transport";

export { ApiError } from "./error";
export type { AiStatus } from "./generated/AiStatus";
export type { AnalysisStatus } from "./generated/AnalysisStatus";
export type { ModelView } from "./generated/ModelView";
export type { SimilarPhoto } from "./generated/SimilarPhoto";
export type { Provider } from "./generated/Provider";
export type { ProviderChoice } from "./generated/ProviderChoice";
export type { AppEvent } from "./generated/AppEvent";
export type { AppInfo } from "./generated/AppInfo";
export type { ImportPlanView } from "./generated/ImportPlanView";
export type { ImportStatus } from "./generated/ImportStatus";
export type { DecisionChange } from "./generated/DecisionChange";
export type { DecisionView } from "./generated/DecisionView";
export type { PhotoDetail } from "./generated/PhotoDetail";
export type { PhotoFacets } from "./generated/PhotoFacets";
export type { PhotoPage } from "./generated/PhotoPage";
export type { PhotoQuery } from "./generated/PhotoQuery";
export type { PhotoSummary } from "./generated/PhotoSummary";
export type { PhotoSort } from "./generated/PhotoSort";
export type { CameraKey } from "./generated/CameraKey";
export type { SeriesEditView } from "./generated/SeriesEditView";
export type { SeriesView } from "./generated/SeriesView";
export type { Settings } from "./generated/Settings";
export type { SettingsUpdate } from "./generated/SettingsUpdate";
export type { TemplatePresetView } from "./generated/TemplatePresetView";
export type { TemplatePreview } from "./generated/TemplatePreview";
export type { PlannedPhotoView } from "./generated/PlannedPhotoView";
export type { PhotoStatus } from "./generated/PhotoStatus";
export { canPickFolders, pickFolders } from "./dialogs";
export type { ImageSize } from "./transport";

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

/** Built-in path templates. */
export function templatePresets(): Promise<TemplatePresetView[]> {
  return transport.call<TemplatePresetView[]>("template_presets");
}

/** Validates a path template and renders it for sample photos. */
export function previewTemplate(template: string): Promise<TemplatePreview> {
  return transport.call<TemplatePreview>("preview_template", { template });
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

/** Everything known about a photo, formatted for display. */
export function getPhoto(photoId: number): Promise<PhotoDetail> {
  return transport.call<PhotoDetail>("get_photo", { photo_id: photoId });
}

/** URL of a photo's cached image (`grid`: 512 px, `preview`: 2048 px, `full`: native size). */
export function imageUrl(photoId: number, size: ImageSize): string {
  return transport.imageUrl(photoId, size);
}

/** A page of photos matching the filters (default: newest first, 200 per page). */
export function queryPhotos(query: PhotoQuery = {}): Promise<PhotoPage> {
  return transport.call<PhotoPage>("query_photos", query);
}

/**
 * Values present in the library, for building filters. With a filter,
 * `total` and `days` count only matching photos; option lists stay complete.
 */
export function photoFacets(filter?: PhotoQuery): Promise<PhotoFacets> {
  return transport.call<PhotoFacets>(
    "photo_facets",
    filter ? { filter } : undefined,
  );
}

/** Changes photos' flags and/or ratings; returns the resulting decisions. */
export function setDecisions(
  changes: DecisionChange[],
): Promise<DecisionView[]> {
  return transport.call<DecisionView[]>("set_decisions", { changes });
}

/** How many photos background analysis has finished. */
export function analysisStatus(): Promise<AnalysisStatus> {
  return transport.call<AnalysisStatus>("analysis_status");
}

/** Analyzes these photos next (for example the ones on screen). */
export function prioritizeAnalysis(
  photoIds: number[],
): Promise<AnalysisStatus> {
  return transport.call<AnalysisStatus>("prioritize_analysis", {
    photo_ids: photoIds,
  });
}

/** The series a photo belongs to, with its ranking. */
export function photoSeries(photoId: number): Promise<SeriesView> {
  return transport.call<SeriesView>("photo_series", { photo_id: photoId });
}

/** Splits or joins series before a photo (`null` removes the correction). */
export function editSeries(
  photoId: number,
  edit: SeriesEditView | null,
): Promise<SeriesView> {
  return transport.call<SeriesView>("edit_series", {
    photo_id: photoId,
    edit,
  });
}

/** Makes a photo the best of its series, or withdraws that choice. */
export function chooseBest(
  photoId: number,
  best: boolean,
): Promise<SeriesView> {
  return transport.call<SeriesView>("choose_best", {
    photo_id: photoId,
    best,
  });
}

/** Which hardware local AI models run on (probes it when the choice changed). */
export function aiStatus(): Promise<AiStatus> {
  return transport.call<AiStatus>("ai_status");
}

/** The AI models the app offers, with licenses and download state. */
export function listModels(): Promise<ModelView[]> {
  return transport.call<ModelView[]>("list_models");
}

/** Starts downloading a model; progress arrives as `model_progress`. */
export function installModel(id: string): Promise<ModelView> {
  return transport.call<ModelView>("install_model", { id });
}

export function cancelModel(id: string): Promise<ModelView> {
  return transport.call<ModelView>("cancel_model", { id });
}

/** Deletes a downloaded model. */
export function removeModel(id: string): Promise<ModelView> {
  return transport.call<ModelView>("remove_model", { id });
}

/** Photos matching a description, best first (needs the CLIP models). */
export function searchPhotos(text: string): Promise<SimilarPhoto[]> {
  return transport.call<SimilarPhoto[]>("search_photos", { text });
}
