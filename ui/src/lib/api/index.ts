import type { AiStatus } from "./generated/AiStatus";
import type { TasteStatus } from "./generated/TasteStatus";
import type { AnalysisStatus } from "./generated/AnalysisStatus";
import type { SidecarStatus } from "./generated/SidecarStatus";
import type { FinishResult } from "./generated/FinishResult";
import type { ClearPlan } from "./generated/ClearPlan";
import type { ClearResult } from "./generated/ClearResult";
import type { FinishStatus } from "./generated/FinishStatus";
import type { ModelView } from "./generated/ModelView";
import type { ExportArgs } from "./generated/ExportArgs";
import type { ExportStatus } from "./generated/ExportStatus";
import type { RejectMovePlan } from "./generated/RejectMovePlan";
import type { RejectMoveResult } from "./generated/RejectMoveResult";
import type { SimilarPhoto } from "./generated/SimilarPhoto";
import type { AppEvent } from "./generated/AppEvent";
import type { AppInfo } from "./generated/AppInfo";
import type { ImportSource } from "./generated/ImportSource";
import type { JobsArgs } from "./generated/JobsArgs";
import type { JobView } from "./generated/JobView";
import type { LogArgs } from "./generated/LogArgs";
import type { LogEntry } from "./generated/LogEntry";
import type { PipelineStatus } from "./generated/PipelineStatus";
import type { DecisionChange } from "./generated/DecisionChange";
import type { DecisionView } from "./generated/DecisionView";
import type { PhotoDetail } from "./generated/PhotoDetail";
import type { LibraryStats } from "./generated/LibraryStats";
import type { PhotoFacets } from "./generated/PhotoFacets";
import type { ReanalyzeResult } from "./generated/ReanalyzeResult";
import type { PhotoPage } from "./generated/PhotoPage";
import type { PhotoQuery } from "./generated/PhotoQuery";
import type { SeriesEditView } from "./generated/SeriesEditView";
import type { SeriesView } from "./generated/SeriesView";
import type { Settings } from "./generated/Settings";
import type { SettingsUpdate } from "./generated/SettingsUpdate";
import type { TemplatePresetView } from "./generated/TemplatePresetView";
import type { TemplatePreview } from "./generated/TemplatePreview";
import { selectTransport, type ImageSize, type PairView } from "./transport";

export { ApiError } from "./error";
export type { AiStatus } from "./generated/AiStatus";
export type { TasteStatus } from "./generated/TasteStatus";
export type { TasteSettings } from "./generated/TasteSettings";
export type { ReanalyzeResult } from "./generated/ReanalyzeResult";
export type { GearStats } from "./generated/GearStats";
export type { KeeperCounts } from "./generated/KeeperCounts";
export type { LibraryStats } from "./generated/LibraryStats";
export type { AnalysisStatus } from "./generated/AnalysisStatus";
export type { ModelView } from "./generated/ModelView";
export type { ExportArgs } from "./generated/ExportArgs";
export type { ExportStatus } from "./generated/ExportStatus";
export type { RejectMovePlan } from "./generated/RejectMovePlan";
export type { RejectMoveResult } from "./generated/RejectMoveResult";
export type { SimilarPhoto } from "./generated/SimilarPhoto";
export type { Provider } from "./generated/Provider";
export type { ProviderChoice } from "./generated/ProviderChoice";
export type { AppEvent } from "./generated/AppEvent";
export type { AppInfo } from "./generated/AppInfo";
export type { ImportSource } from "./generated/ImportSource";
export type { JobFilter } from "./generated/JobFilter";
export type { JobStage } from "./generated/JobStage";
export type { JobView } from "./generated/JobView";
export type { LogEntry } from "./generated/LogEntry";
export type { LogLevel } from "./generated/LogLevel";
export type { PipelineStatus } from "./generated/PipelineStatus";
export type { ImportSettings } from "./generated/ImportSettings";
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
export type { RecentFolder } from "./generated/RecentFolder";
export type { ColorLabel } from "./generated/ColorLabel";
export type { FinishResult } from "./generated/FinishResult";
export type { ClearPlan } from "./generated/ClearPlan";
export type { ClearResult } from "./generated/ClearResult";
export type { DeleteAfterCopy } from "./generated/DeleteAfterCopy";
export type { FinishStatus } from "./generated/FinishStatus";
export type { LabelSettings } from "./generated/LabelSettings";
export type { SidecarStatus } from "./generated/SidecarStatus";
export type { SettingsUpdate } from "./generated/SettingsUpdate";
export type { TemplatePresetView } from "./generated/TemplatePresetView";
export type { TemplatePreview } from "./generated/TemplatePreview";
export {
  canPickFolders,
  canReveal,
  pickFolders,
  revealInFolder,
} from "./dialogs";
export type { ImageSize, PairView } from "./transport";

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

/**
 * Culls `path` in place instead of the library or the folder open now
 * (ADR-0013). Returns the settings as the folder sees them.
 */
export function openFolder(path: string): Promise<Settings> {
  return transport.call<Settings>("open_folder", { path });
}

/** What clearing the folder after copying would delete (ADR-0014). */
export function clearPlan(): Promise<ClearPlan> {
  return transport.call<ClearPlan>("clear_plan");
}

/**
 * Deletes originals whose copy is verified again, and rejects when the
 * settings allow it. Permanent.
 */
export function clearFolder(): Promise<ClearResult> {
  return transport.call<ClearResult>("clear_folder");
}

/** What finishing the folder would do. */
export function finishStatus(): Promise<FinishStatus> {
  return transport.call<FinishStatus>("finish_status");
}

/**
 * Gives auto-picked photos the auto-pick colour label and writes every
 * pending sidecar.
 */
export function finishFolder(): Promise<FinishResult> {
  return transport.call<FinishResult>("finish_folder");
}

/** Goes back to the library from a folder culled in place. */
export function openLibrary(): Promise<Settings> {
  return transport.call<Settings>("open_library");
}

/** Built-in path templates. */
export function templatePresets(): Promise<TemplatePresetView[]> {
  return transport.call<TemplatePresetView[]>("template_presets");
}

/** Validates a path template and renders it for sample photos. */
export function previewTemplate(template: string): Promise<TemplatePreview> {
  return transport.call<TemplatePreview>("preview_template", { template });
}

/** The folders photos are imported from. */
export function listImportSources(): Promise<ImportSource[]> {
  return transport.call<ImportSource[]>("list_import_sources");
}

/** Adds a folder to import from and starts importing it. */
export function addImportSource(
  path: string,
  watch = true,
): Promise<ImportSource> {
  return transport.call<ImportSource>("add_import_source", { path, watch });
}

/** Forgets a folder (imported photos stay); returns the remaining folders. */
export function removeImportSource(id: number): Promise<ImportSource[]> {
  return transport.call<ImportSource[]>("remove_import_source", { id });
}

/** Turns automatic import of new photos in a folder on or off. */
export function setImportSourceWatch(
  id: number,
  watch: boolean,
): Promise<ImportSource> {
  return transport.call<ImportSource>("set_import_source_watch", {
    id,
    watch,
  });
}

/** Scans one folder (or all) for new photos now. */
export function scanImportSources(id?: number): Promise<PipelineStatus> {
  return transport.call<PipelineStatus>(
    "scan_import_sources",
    id === undefined ? {} : { id },
  );
}

/** Counts and current work of the import pipeline. */
export function pipelineStatus(): Promise<PipelineStatus> {
  return transport.call<PipelineStatus>("pipeline_status");
}

/** Photos in the import pipeline, newest first. */
export function pipelineJobs(args: JobsArgs = {}): Promise<JobView[]> {
  return transport.call<JobView[]>("pipeline_jobs", args);
}

/** The import pipeline log, newest first. */
export function pipelineLog(args: LogArgs = {}): Promise<LogEntry[]> {
  return transport.call<LogEntry[]>("pipeline_log", args);
}

/** Queues failed photos again (all, or those listed). */
export function retryFailedImports(ids?: number[]): Promise<PipelineStatus> {
  return transport.call<PipelineStatus>(
    "retry_failed_imports",
    ids === undefined ? {} : { ids },
  );
}

/** Subscribes to backend events; returns an unsubscribe function. */
export function onAppEvent(handler: (event: AppEvent) => void): () => void {
  return transport.subscribe(handler);
}

/** Everything known about a photo, formatted for display. */
export function getPhoto(photoId: number): Promise<PhotoDetail> {
  return transport.call<PhotoDetail>("get_photo", { photo_id: photoId });
}

/**
 * URL of a photo's cached image (`grid`: 512 px, `preview`: 2048 px, `full`:
 * native size). `view` picks the file of a RAW+JPEG pair; without it, the
 * `pair_view` setting does.
 */
export function imageUrl(
  photoId: number,
  size: ImageSize,
  view?: PairView,
): string {
  return transport.imageUrl(photoId, size, view);
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

/** Keepers per camera and lens, and focal lengths per camera. */
export function libraryStats(): Promise<LibraryStats> {
  return transport.call<LibraryStats>("library_stats");
}

/** Changes photos' flags and/or ratings; returns the resulting decisions. */
export function setDecisions(
  changes: DecisionChange[],
): Promise<DecisionView[]> {
  return transport.call<DecisionView[]>("set_decisions", { changes });
}

/**
 * Analyzes photos again (every photo without `photoIds`); earlier results
 * stay in use until replaced.
 */
export function reanalyzePhotos(photoIds?: number[]): Promise<ReanalyzeResult> {
  return transport.call<ReanalyzeResult>(
    "reanalyze_photos",
    photoIds ? { photo_ids: photoIds } : undefined,
  );
}

/** Whether decisions have been written to their XMP sidecars. */
export function sidecarStatus(): Promise<SidecarStatus> {
  return transport.call<SidecarStatus>("sidecar_status");
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

/** What the personal taste model has learned from picks and rejects. */
export function tasteStatus(): Promise<TasteStatus> {
  return transport.call<TasteStatus>("taste_status");
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

/** How many rejected photos `moveRejects` would move, and what can be undone. */
export function planRejectMove(): Promise<RejectMovePlan> {
  return transport.call<RejectMovePlan>("plan_reject_move");
}

/** Moves rejected photos into the library's `_Rejected` folder. */
export function moveRejects(): Promise<RejectMoveResult> {
  return transport.call<RejectMoveResult>("move_rejects");
}

/** Moves the latest batch of moved rejects back. */
export function undoRejectMove(batch: number): Promise<RejectMoveResult> {
  return transport.call<RejectMoveResult>("undo_reject_move", { batch });
}

/** Starts exporting photos (by default the picks); progress arrives as
 * `export_progress`. */
export function startExport(args: ExportArgs): Promise<ExportStatus> {
  return transport.call<ExportStatus>("start_export", args);
}

export function exportStatus(): Promise<ExportStatus> {
  return transport.call<ExportStatus>("export_status");
}

export function cancelExport(): Promise<ExportStatus> {
  return transport.call<ExportStatus>("cancel_export");
}
