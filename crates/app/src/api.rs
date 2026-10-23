//! The command API shared by every transport.
//!
//! The UI calls commands by name with JSON arguments. The Tauri shell and
//! the development HTTP server both forward those calls to [`Api::call`],
//! so behaviour is defined once (ADR-0010).

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use ts_rs::TS;

use crate::ai::AiService;
use crate::analysis::clip::{AestheticView, ClipAnalyzer};
use crate::analysis::faces::FaceAnalyzer;
use crate::analysis::technical::{QualityView, TechnicalAnalyzer};
use crate::analysis::{AnalysisService, Analyzer, PrioritizeArgs, ReanalyzeArgs, ReanalyzeResult};
use crate::embeddings::{DEFAULT_LIMIT, EmbeddingIndex, MAX_LIMIT, SimilarArgs};
use crate::explain::{self, SeriesPlace, Signals};
use crate::export::{ExportArgs, ExportError, ExportService};
use crate::hooks::EventHooks;
use crate::info;
use crate::library::{
    DecisionView, FacetsArgs, PhotoArgs, PhotoDetail, PhotoQuery, SetDecisionsArgs,
};
use crate::models::{ModelArgs, ModelError, ModelService};
use crate::pipeline::{
    AddSourceArgs, ImportPipeline, JobsArgs, LogArgs, PipelineDeps, PipelineError, RetryArgs,
    ScanArgs, SourceArgs, SourceWatchArgs,
};
use crate::ports::catalog::{AnalyzerId, PhotoCatalog};
use crate::ports::events::EventSink;
use crate::ports::fs::FileSystem;
use crate::ports::geocoder::ReverseGeocoder;
use crate::ports::images::ImageDecoder;
use crate::ports::inference::InferenceEngine;
use crate::ports::metadata::MetadataReader;
use crate::ports::models::{ModelSpec, ModelStore};
use crate::ports::pipeline::LogLevel;
use crate::ports::pipeline::PipelineStore;
use crate::ports::pixels::PixelLoader;
use crate::ports::settings::SettingsStore;
use crate::ports::sidecars::SidecarStore;
use crate::rejects::{RejectsError, RejectsService, UndoRejectMoveArgs};
use crate::search::{DEFAULT_LIMIT as SEARCH_LIMIT, SearchArgs, SearchError, TextSearch};
use crate::series::{ChooseBestArgs, EditSeriesArgs, SeriesService};
use crate::settings::{SettingsError, SettingsService, SettingsUpdate};
use crate::sidecars::SidecarService;
use crate::templates::{self, PreviewTemplateArgs};
use crate::thumbnails::{ImageSize, ThumbnailError, ThumbnailService};
use picturesque_core::photo::{PairView, PhotoId};
use picturesque_core::ranking::Candidate;

/// Category of an API failure. Transports map it to their own error form
/// (for example an HTTP status code).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ApiErrorCode {
    UnknownCommand,
    InvalidArguments,
    /// The thing asked for (for example a photo) does not exist.
    NotFound,
    Internal,
}

/// Error returned by a command, serialized as `{ "code": ..., "message": ... }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ApiError {
    pub code: ApiErrorCode,
    pub message: String,
}

impl ApiError {
    pub fn new(code: ApiErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ApiErrorCode::Internal, message)
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ApiError {}

/// Infrastructure the use cases run on, built by the entry point (desktop
/// shell or development server).
#[derive(Clone)]
pub struct AppContext {
    pub settings_store: Arc<dyn SettingsStore>,
    pub catalog: Arc<dyn PhotoCatalog>,
    pub fs: Arc<dyn FileSystem>,
    pub metadata: Arc<dyn MetadataReader>,
    pub geocoder: Arc<dyn ReverseGeocoder>,
    pub events: Arc<dyn EventSink>,
    pub decoder: Arc<dyn ImageDecoder>,
    /// Folder for cached thumbnails and previews.
    pub cache_dir: PathBuf,
    pub sidecars: Arc<dyn SidecarStore>,
    pub pixels: Arc<dyn PixelLoader>,
    /// Analyses run in the background over every photo.
    pub analyzers: Vec<Arc<dyn Analyzer>>,
    /// Runs local AI models.
    pub inference: Arc<dyn InferenceEngine>,
    /// Models the app offers for download (`models::catalog()`).
    pub models: Vec<ModelSpec>,
    /// Where downloaded models are kept.
    pub model_store: Arc<dyn ModelStore>,
    /// Import sources, jobs and the pipeline log (usually the catalog).
    pub pipeline_store: Arc<dyn PipelineStore>,
    /// Change notifications for watched import folders.
    pub watcher: Arc<dyn crate::ports::watcher::FolderWatcher>,
}

impl fmt::Debug for AppContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppContext").finish_non_exhaustive()
    }
}

/// What analysis says about one photo (`get_photo`).
struct Analyzed {
    quality: Option<QualityView>,
    aesthetic: Option<AestheticView>,
    faces: Option<Vec<crate::analysis::faces::FaceView>>,
    score: Option<f32>,
}

impl Analyzed {
    fn signals(&self) -> Signals<'_> {
        Signals {
            quality: self.quality.as_ref(),
            aesthetic: self.aesthetic.as_ref(),
            faces: self.faces.as_deref(),
        }
    }
}

/// Entry point for UI commands.
///
/// `call` is synchronous; transports run it on a blocking thread so use
/// cases can do file and database I/O directly.
pub struct Api {
    settings: SettingsService,
    thumbnails: Arc<ThumbnailService>,
    catalog: Arc<dyn PhotoCatalog>,
    sidecars: Arc<SidecarService>,
    analysis: Arc<AnalysisService>,
    series: Arc<SeriesService>,
    ai: AiService,
    models: Arc<ModelService>,
    embeddings: Arc<EmbeddingIndex>,
    search: TextSearch,
    rejects: RejectsService,
    export: Arc<ExportService>,
    pipeline: Arc<ImportPipeline>,
    finish: crate::finish::FinishService,
    clear: crate::clear::ClearService,
}

impl fmt::Debug for Api {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Api").finish_non_exhaustive()
    }
}

impl Api {
    /// Builds the use cases; nothing runs until
    /// [`start_background_work`](Self::start_background_work).
    pub fn new(ctx: AppContext) -> Result<Self, ApiError> {
        let decoder = Arc::clone(&ctx.decoder);
        let thumbnails = Arc::new(ThumbnailService::new(
            Arc::clone(&ctx.catalog),
            Arc::clone(&ctx.settings_store),
            Arc::clone(&ctx.fs),
            ctx.decoder,
            ctx.cache_dir,
        ));
        let events = Arc::new(EventHooks::new(ctx.events));
        let models = Arc::new(ModelService::new(
            ctx.models,
            ctx.model_store,
            Arc::clone(&events) as Arc<dyn EventSink>,
        ));
        // AI analyzers join once their models are downloaded.
        let mut analyzers = ctx.analyzers;
        analyzers.push(Arc::new(ClipAnalyzer::new(
            Arc::clone(&models),
            Arc::clone(&ctx.inference),
            Arc::clone(&ctx.settings_store),
        )));
        analyzers.push(Arc::new(FaceAnalyzer::new(
            Arc::clone(&models),
            Arc::clone(&ctx.inference),
            Arc::clone(&ctx.settings_store),
        )));
        let export = Arc::new(ExportService::new(
            Arc::clone(&ctx.catalog),
            Arc::clone(&ctx.fs),
            decoder,
            Arc::clone(&ctx.pixels),
            Arc::clone(&ctx.settings_store),
            Arc::clone(&events) as Arc<dyn EventSink>,
            Arc::clone(&ctx.sidecars),
        ));
        let analysis = Arc::new(AnalysisService::new(
            Arc::clone(&ctx.catalog),
            Arc::clone(&thumbnails),
            ctx.pixels,
            analyzers,
            Arc::clone(&events) as Arc<dyn EventSink>,
        ));
        let _ = events.analysis.set(Arc::clone(&analysis));
        let series = Arc::new(SeriesService::new(
            Arc::clone(&ctx.catalog),
            Arc::clone(&ctx.settings_store),
            Arc::clone(&events) as Arc<dyn EventSink>,
        ));
        let _ = events.series.set(Arc::clone(&series));
        let embeddings = Arc::new(EmbeddingIndex::new(Arc::clone(&ctx.catalog)));
        let _ = events.embeddings.set(Arc::clone(&embeddings));
        let pipeline = Arc::new(ImportPipeline::new(PipelineDeps {
            store: Arc::clone(&ctx.pipeline_store),
            catalog: Arc::clone(&ctx.catalog),
            settings: Arc::clone(&ctx.settings_store),
            fs: Arc::clone(&ctx.fs),
            metadata: Arc::clone(&ctx.metadata),
            geocoder: Arc::clone(&ctx.geocoder),
            sidecars: Arc::clone(&ctx.sidecars),
            events: Arc::clone(&events) as Arc<dyn EventSink>,
            watcher: Arc::clone(&ctx.watcher),
        }));
        {
            // Each imported photo gets its thumbnail, analysis and series.
            let (thumbnails, analysis, series) = (
                Arc::clone(&thumbnails),
                Arc::clone(&analysis),
                Arc::clone(&series),
            );
            pipeline.on_imported(move || {
                thumbnails.prefill_in_background();
                analysis.wake();
                series.notify();
            });
        }
        let catalog = Arc::clone(&ctx.catalog);
        let sidecars = Arc::new(SidecarService::new(
            Arc::clone(&ctx.catalog),
            Arc::clone(&ctx.settings_store),
            Arc::clone(&ctx.sidecars),
            Arc::clone(&events) as Arc<dyn EventSink>,
        ));
        let search = TextSearch::new(
            Arc::clone(&models),
            Arc::clone(&ctx.inference),
            Arc::clone(&ctx.settings_store),
            Arc::clone(&embeddings),
        );
        let rejects = RejectsService::new(
            Arc::clone(&catalog),
            Arc::clone(&ctx.fs),
            Arc::clone(&ctx.settings_store),
        );
        Ok(Self {
            clear: crate::clear::ClearService::new(
                Arc::clone(&catalog),
                Arc::clone(&ctx.fs),
                Arc::clone(&ctx.settings_store),
            ),
            finish: crate::finish::FinishService::new(Arc::clone(&catalog), Arc::clone(&sidecars)),
            pipeline,
            export,
            search,
            rejects,
            ai: AiService::new(ctx.inference, Arc::clone(&ctx.settings_store)),
            settings: SettingsService::new(ctx.settings_store, ctx.fs, Arc::clone(&catalog)),
            thumbnails,
            catalog,
            sidecars,
            analysis,
            series,
            models,
            embeddings,
        })
    }

    /// A photo's current analysis results, read for display.
    fn analyzed(&self, id: PhotoId) -> Result<Analyzed, ApiError> {
        let results = self.catalog.analysis(id).map_err(catalog_error)?;
        let current = |analyzer: AnalyzerId| {
            results
                .iter()
                .find(|(name, version, _)| *name == analyzer.name && *version == analyzer.version)
                .map(|(_, _, result)| result)
        };
        let quality = current(TechnicalAnalyzer::ID).and_then(QualityView::from_result);
        let aesthetic = current(ClipAnalyzer::ID).and_then(AestheticView::from_result);
        let faces = current(FaceAnalyzer::ID).and_then(crate::analysis::faces::faces);
        let score = picturesque_core::ranking::photo_quality(&Candidate {
            id,
            scores: current(TechnicalAnalyzer::ID)
                .and_then(|r| serde_json::from_value(r.clone()).ok()),
            rejected: false,
            aesthetic: aesthetic.map(|a| a.score),
            face: faces.as_deref().and_then(crate::analysis::faces::signal),
        });
        Ok(Analyzed {
            quality,
            aesthetic,
            faces,
            score,
        })
    }

    /// Starts background work that should run while the app is open:
    /// generating missing thumbnails, writing sidecars and analyzing
    /// photos. Called by entry points, not tests.
    pub fn start_background_work(&self) {
        self.thumbnails.prefill_in_background();
        self.sidecars.start();
        self.analysis.start(analysis_workers());
        self.series.start();
        self.pipeline.start();
    }

    /// Writes pending sidecars and stops all background work, before the
    /// `Api` is replaced by another session's (ADR-0013). Work in progress
    /// finishes its current step; the catalog stays consistent.
    pub fn shutdown(&self) {
        self.sidecars.stop();
        self.pipeline.stop();
        self.analysis.stop();
        self.series.stop();
        self.thumbnails.stop();
    }

    /// Scans every import source and imports what it finds, on the calling
    /// thread (tests and tools). Returns the number of photos processed.
    pub fn run_imports(&self) -> usize {
        self.pipeline.resume();
        self.pipeline
            .scan_now(&std::collections::HashSet::from([None]));
        let imported = self.pipeline.run_until_idle();
        self.thumbnails.wait_for_prefill();
        imported
    }

    /// Waits for background thumbnail generation (tests and tools).
    pub fn wait_for_thumbnails(&self) {
        self.thumbnails.wait_for_prefill();
    }

    /// Recomputes series now (tests and tools).
    pub fn recompute_series(&self) -> Result<usize, ApiError> {
        self.series.recompute().map_err(catalog_error)
    }

    /// Runs all pending analysis on the calling thread (tests and tools).
    pub fn run_analysis(&self) -> usize {
        self.analysis.run_until_idle()
    }

    /// Writes pending XMP sidecars now (tests and shutdown).
    pub fn flush_sidecars(&self) -> crate::sidecars::FlushReport {
        self.sidecars.flush()
    }

    /// Resolves an image request path `{size}/{photo_id}[/{view}]` (as used
    /// by the `picturesque://` protocol and `/api/images/`) to the cached
    /// file. `view` is `image` or `raw`, the file of a RAW+JPEG pair to show;
    /// without it, the `pair_view` setting decides.
    pub fn image_request(&self, path: &str) -> Result<PathBuf, ApiError> {
        let invalid = || {
            ApiError::new(
                ApiErrorCode::InvalidArguments,
                format!("invalid image path `{path}`"),
            )
        };
        let mut parts = path.trim_matches('/').split('/');
        let (Some(size), Some(id), view, None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(invalid());
        };
        let size = ImageSize::from_name(size).ok_or_else(invalid)?;
        let id: i64 = id.parse().map_err(|_| invalid())?;
        let view = view
            .map(|v| PairView::from_name(v).ok_or_else(invalid))
            .transpose()?;
        self.image_path(id, size, view)
    }

    /// Path of a photo's cached image, generating it if needed. Images are
    /// served as files by each transport rather than through JSON. `view`
    /// defaults to the `pair_view` setting.
    pub fn image_path(
        &self,
        photo_id: i64,
        size: ImageSize,
        view: Option<PairView>,
    ) -> Result<PathBuf, ApiError> {
        let view = match view {
            Some(view) => Ok(view),
            None => self.thumbnails.pair_view(),
        };
        view.and_then(|view| self.thumbnails.image(PhotoId(photo_id), size, view))
            .map_err(|e| match e {
                ThumbnailError::NotFound(_) => ApiError::new(ApiErrorCode::NotFound, e.to_string()),
                ThumbnailError::NoLibrary => {
                    ApiError::new(ApiErrorCode::InvalidArguments, e.to_string())
                }
                ThumbnailError::Decode(_) | ThumbnailError::Storage(_) => {
                    ApiError::internal(e.to_string())
                }
            })
    }

    /// Applies decision changes. Everything is validated first, so an
    /// invalid change (bad rating, unknown photo) changes nothing.
    fn set_decisions(
        &self,
        changes: &[crate::library::DecisionChange],
    ) -> Result<Vec<DecisionView>, ApiError> {
        use picturesque_core::decision::{Decision, Rating};
        let mut planned = Vec::with_capacity(changes.len());
        for change in changes {
            let id = PhotoId(change.photo_id);
            if self.catalog.photo(id).map_err(catalog_error)?.is_none() {
                return Err(ApiError::new(
                    ApiErrorCode::NotFound,
                    format!("There is no photo {}.", change.photo_id),
                ));
            }
            let current = self.catalog.decision(id).map_err(catalog_error)?;
            let rating = match change.rating {
                Some(stars) => Rating::new(stars).ok_or_else(|| {
                    ApiError::new(
                        ApiErrorCode::InvalidArguments,
                        format!("A rating must be 0-5, got {stars}."),
                    )
                })?,
                None => current.rating,
            };
            planned.push((
                id,
                Decision {
                    flag: change.flag.unwrap_or(current.flag),
                    rating,
                },
            ));
        }
        planned
            .into_iter()
            .map(|(id, decision)| {
                self.catalog
                    .set_decision(id, decision)
                    .map_err(catalog_error)?;
                Ok(DecisionView {
                    photo_id: id.0,
                    flag: decision.flag,
                    rating: decision.rating.stars(),
                })
            })
            .collect()
    }

    fn require_photo(&self, photo_id: i64) -> Result<(), ApiError> {
        match self
            .catalog
            .photo(PhotoId(photo_id))
            .map_err(catalog_error)?
        {
            Some(_) => Ok(()),
            None => Err(ApiError::new(
                ApiErrorCode::NotFound,
                format!("There is no photo {photo_id}."),
            )),
        }
    }

    /// Runs `command` with JSON `args` and returns its JSON result.
    pub fn call(&self, command: &str, args: Value) -> Result<Value, ApiError> {
        match command {
            "app_info" => {
                no_args(command, &args)?;
                respond(info::app_info())
            }
            "get_settings" => {
                no_args(command, &args)?;
                respond(self.settings.get().map_err(settings_error)?)
            }
            "update_settings" => {
                let update: SettingsUpdate = parse_args(command, args)?;
                let series_changed = update.series.is_some()
                    || update.ranking.is_some()
                    || update.auto_pick.is_some();
                let settings = self.settings.update(update).map_err(settings_error)?;
                if series_changed {
                    self.series.recompute().map_err(catalog_error)?;
                }
                respond(settings)
            }
            "template_presets" => {
                no_args(command, &args)?;
                respond(templates::presets())
            }
            "preview_template" => {
                let args: PreviewTemplateArgs = parse_args(command, args)?;
                respond(templates::preview(&args.template))
            }
            "query_photos" => {
                let query: PhotoQuery = parse_args(
                    command,
                    if args.is_null() {
                        Value::Object(Default::default())
                    } else {
                        args
                    },
                )?;
                query
                    .validate()
                    .map_err(|m| ApiError::new(ApiErrorCode::InvalidArguments, m))?;
                respond(self.catalog.query(&query).map_err(catalog_error)?)
            }
            "get_photo" => {
                let PhotoArgs { photo_id } = parse_args(command, args)?;
                let id = PhotoId(photo_id);
                let photo = self
                    .catalog
                    .photo(id)
                    .map_err(catalog_error)?
                    .ok_or_else(|| {
                        ApiError::new(
                            ApiErrorCode::NotFound,
                            format!("There is no photo {photo_id}."),
                        )
                    })?;
                let decision = self.catalog.decision(id).map_err(catalog_error)?;
                let mut detail = PhotoDetail::new(&photo, decision);
                let analyzed = self.analyzed(id)?;
                detail.quality = analyzed.quality;
                detail.aesthetic = analyzed.aesthetic;
                detail.faces = analyzed.faces.clone();
                detail.score = analyzed.score;
                let series = self.series.view(id).map_err(catalog_error)?;
                let best = match series.best_photo_id {
                    Some(best) if best != photo_id => Some(self.analyzed(PhotoId(best))?),
                    _ => None,
                };
                let place = series.best_photo_id.map(|best_id| SeriesPlace {
                    rank: series
                        .ranking
                        .iter()
                        .position(|r| r.photo_id == photo_id)
                        .map_or(series.photo_ids.len(), |i| i + 1),
                    size: series.photo_ids.len(),
                    is_best: best_id == photo_id,
                    best_chosen: series.best_chosen,
                    best: best.as_ref().map(Analyzed::signals).unwrap_or_default(),
                });
                detail.reasons = explain::reasons(analyzed.signals(), place);
                respond(detail)
            }
            "set_decisions" => {
                let SetDecisionsArgs { changes } = parse_args(command, args)?;
                let decisions = self.set_decisions(&changes)?;
                self.sidecars.notify();
                // Rejected photos are never chosen as best.
                self.series.notify();
                respond(decisions)
            }
            "photo_facets" => {
                let FacetsArgs { filter } = if args.is_null() {
                    FacetsArgs::default()
                } else {
                    parse_args(command, args)?
                };
                if let Some(f) = &filter {
                    f.validate()
                        .map_err(|m| ApiError::new(ApiErrorCode::InvalidArguments, m))?;
                }
                respond(
                    self.catalog
                        .facets(filter.as_ref())
                        .map_err(catalog_error)?,
                )
            }
            "library_stats" => {
                no_args(command, &args)?;
                respond(crate::stats::summarize(
                    self.catalog.gear_usage().map_err(catalog_error)?,
                ))
            }
            "similar_photos" => {
                let SimilarArgs { photo_id, limit } = parse_args(command, args)?;
                let limit = limit.unwrap_or(DEFAULT_LIMIT);
                if !(1..=MAX_LIMIT).contains(&limit) {
                    return Err(ApiError::new(
                        ApiErrorCode::InvalidArguments,
                        format!("`limit` must be 1-{MAX_LIMIT}."),
                    ));
                }
                self.require_photo(photo_id)?;
                respond(
                    self.embeddings
                        .similar(PhotoId(photo_id), limit)
                        .map_err(catalog_error)?,
                )
            }
            "list_import_sources" => {
                no_args(command, &args)?;
                respond(self.pipeline.sources().map_err(pipeline_error)?)
            }
            "add_import_source" => {
                let AddSourceArgs { path, watch } = parse_args(command, args)?;
                respond(
                    self.pipeline
                        .add_source(&path, watch.unwrap_or(true))
                        .map_err(pipeline_error)?,
                )
            }
            "remove_import_source" => {
                let SourceArgs { id } = parse_args(command, args)?;
                self.pipeline.remove_source(id).map_err(pipeline_error)?;
                respond(self.pipeline.sources().map_err(pipeline_error)?)
            }
            "set_import_source_watch" => {
                let SourceWatchArgs { id, watch } = parse_args(command, args)?;
                respond(self.pipeline.set_watch(id, watch).map_err(pipeline_error)?)
            }
            "scan_import_sources" => {
                let ScanArgs { id } = if args.is_null() {
                    ScanArgs::default()
                } else {
                    parse_args(command, args)?
                };
                self.pipeline.request_scan(id);
                respond(self.pipeline.status())
            }
            "pipeline_status" => {
                no_args(command, &args)?;
                respond(self.pipeline.status())
            }
            "pipeline_jobs" => {
                let JobsArgs {
                    filter,
                    limit,
                    before,
                } = if args.is_null() {
                    JobsArgs::default()
                } else {
                    parse_args(command, args)?
                };
                let view = self.thumbnails.pair_view().unwrap_or_default();
                let details = |id: PhotoId| {
                    let thumbnail = self
                        .catalog
                        .photo(id)
                        .ok()
                        .flatten()
                        .is_some_and(|p| self.thumbnails.has_grid(&p, view));
                    (thumbnail, self.analysis.progress_of(id))
                };
                respond(
                    self.pipeline
                        .jobs(
                            filter.unwrap_or_default(),
                            limit.unwrap_or(100),
                            before,
                            &details,
                        )
                        .map_err(pipeline_error)?,
                )
            }
            "pipeline_log" => {
                let LogArgs {
                    min_level,
                    limit,
                    before,
                } = if args.is_null() {
                    LogArgs::default()
                } else {
                    parse_args(command, args)?
                };
                respond(
                    self.pipeline
                        .log_entries(
                            min_level.unwrap_or(LogLevel::Info),
                            limit.unwrap_or(200),
                            before,
                        )
                        .map_err(pipeline_error)?,
                )
            }
            "retry_failed_imports" => {
                let RetryArgs { ids } = if args.is_null() {
                    RetryArgs::default()
                } else {
                    parse_args(command, args)?
                };
                self.pipeline
                    .retry(ids.as_deref())
                    .map_err(pipeline_error)?;
                respond(self.pipeline.status())
            }
            "start_export" => {
                let export: ExportArgs = parse_args(command, args)?;
                respond(self.export.start(export).map_err(export_error)?)
            }
            "export_status" => {
                no_args(command, &args)?;
                respond(self.export.status())
            }
            "cancel_export" => {
                no_args(command, &args)?;
                respond(self.export.cancel())
            }
            "plan_reject_move" => {
                no_args(command, &args)?;
                respond(self.rejects.plan().map_err(rejects_error)?)
            }
            "move_rejects" => {
                no_args(command, &args)?;
                respond(self.rejects.move_rejects().map_err(rejects_error)?)
            }
            "undo_reject_move" => {
                let UndoRejectMoveArgs { batch } = parse_args(command, args)?;
                respond(self.rejects.undo(batch).map_err(rejects_error)?)
            }
            "search_photos" => {
                let SearchArgs { text, limit } = parse_args(command, args)?;
                respond(
                    self.search
                        .search(&text, limit.unwrap_or(SEARCH_LIMIT))
                        .map_err(search_error)?,
                )
            }
            "list_models" => {
                no_args(command, &args)?;
                respond(self.models.list())
            }
            "install_model" => {
                let ModelArgs { id } = parse_args(command, args)?;
                respond(self.models.install(&id).map_err(model_error)?)
            }
            "cancel_model" => {
                let ModelArgs { id } = parse_args(command, args)?;
                respond(self.models.cancel(&id).map_err(model_error)?)
            }
            "remove_model" => {
                let ModelArgs { id } = parse_args(command, args)?;
                respond(self.models.remove(&id).map_err(model_error)?)
            }
            "ai_status" => {
                no_args(command, &args)?;
                respond(self.ai.status().map_err(catalog_error)?)
            }
            "clear_plan" => {
                no_args(command, &args)?;
                respond(self.clear.plan().map_err(clear_error)?)
            }
            "clear_folder" => {
                no_args(command, &args)?;
                let result = self.clear.clear().map_err(clear_error)?;
                // Deleted photos leave their series and the search index.
                self.series.notify();
                self.embeddings.invalidate();
                respond(result)
            }
            "finish_status" => {
                no_args(command, &args)?;
                respond(self.finish.status().map_err(catalog_error)?)
            }
            "finish_folder" => {
                no_args(command, &args)?;
                respond(self.finish.finish().map_err(catalog_error)?)
            }
            "sidecar_status" => {
                no_args(command, &args)?;
                respond(self.sidecars.status())
            }
            "analysis_status" => {
                no_args(command, &args)?;
                respond(self.analysis.status())
            }
            "prioritize_analysis" => {
                let PrioritizeArgs { photo_ids } = parse_args(command, args)?;
                let ids: Vec<PhotoId> = photo_ids.into_iter().map(PhotoId).collect();
                self.analysis.prioritize(&ids);
                respond(self.analysis.status())
            }
            "reanalyze_photos" => {
                let ReanalyzeArgs { photo_ids } = if args.is_null() {
                    ReanalyzeArgs::default()
                } else {
                    parse_args(command, args)?
                };
                let ids: Option<Vec<PhotoId>> =
                    photo_ids.map(|ids| ids.into_iter().map(PhotoId).collect());
                let photos = self
                    .analysis
                    .reanalyze(ids.as_deref())
                    .map_err(catalog_error)?;
                respond(ReanalyzeResult {
                    photos,
                    waiting_for_models: self.analysis.waiting_for_models(),
                })
            }
            "photo_series" => {
                let PhotoArgs { photo_id } = parse_args(command, args)?;
                self.require_photo(photo_id)?;
                respond(self.series.view(PhotoId(photo_id)).map_err(catalog_error)?)
            }
            "edit_series" => {
                let EditSeriesArgs { photo_id, edit } = parse_args(command, args)?;
                self.require_photo(photo_id)?;
                respond(
                    self.series
                        .edit(PhotoId(photo_id), edit.map(Into::into))
                        .map_err(catalog_error)?,
                )
            }
            "choose_best" => {
                let ChooseBestArgs { photo_id, best } = parse_args(command, args)?;
                self.require_photo(photo_id)?;
                respond(
                    self.series
                        .choose_best(PhotoId(photo_id), best)
                        .map_err(catalog_error)?,
                )
            }
            _ => Err(ApiError::new(
                ApiErrorCode::UnknownCommand,
                format!("unknown command `{command}`"),
            )),
        }
    }
}

/// Half the cores (at least one), leaving the rest for the UI and imports.
fn analysis_workers() -> usize {
    std::thread::available_parallelism().map_or(1, |n| (n.get() / 2).max(1))
}

fn pipeline_error(e: PipelineError) -> ApiError {
    match e {
        PipelineError::Invalid(_) => ApiError::new(ApiErrorCode::InvalidArguments, e.to_string()),
        PipelineError::Catalog(e) => catalog_error(e),
    }
}

fn export_error(e: ExportError) -> ApiError {
    match e {
        ExportError::Invalid(_) | ExportError::Busy => {
            ApiError::new(ApiErrorCode::InvalidArguments, e.to_string())
        }
        ExportError::Failed(_) => ApiError::internal(e.to_string()),
    }
}

fn rejects_error(e: RejectsError) -> ApiError {
    match e {
        RejectsError::Invalid(_) => ApiError::new(ApiErrorCode::InvalidArguments, e.to_string()),
        RejectsError::Catalog(e) => catalog_error(e),
    }
}

fn search_error(e: SearchError) -> ApiError {
    match e {
        SearchError::Invalid(_) | SearchError::ModelMissing(_) => {
            ApiError::new(ApiErrorCode::InvalidArguments, e.to_string())
        }
        SearchError::Failed(_) => ApiError::internal(e.to_string()),
    }
}

fn model_error(e: ModelError) -> ApiError {
    match e {
        ModelError::Unknown(_) => ApiError::new(ApiErrorCode::NotFound, e.to_string()),
        ModelError::Busy(_) => ApiError::new(ApiErrorCode::InvalidArguments, e.to_string()),
        ModelError::Storage(_) => ApiError::internal(e.to_string()),
    }
}

fn catalog_error(e: crate::ports::catalog::CatalogError) -> ApiError {
    ApiError::internal(e.to_string())
}

fn clear_error(e: crate::clear::ClearError) -> ApiError {
    match e {
        crate::clear::ClearError::Invalid(msg) => {
            ApiError::new(ApiErrorCode::InvalidArguments, msg)
        }
        crate::clear::ClearError::Failed(msg) => ApiError::internal(msg),
    }
}

fn settings_error(e: SettingsError) -> ApiError {
    match e {
        SettingsError::Invalid(msg) => ApiError::new(ApiErrorCode::InvalidArguments, msg),
        SettingsError::Storage(e) => ApiError::internal(e.to_string()),
    }
}

/// Accepts a missing argument (`null`) or an empty object.
fn no_args(command: &str, args: &Value) -> Result<(), ApiError> {
    match args {
        Value::Null => Ok(()),
        Value::Object(map) if map.is_empty() => Ok(()),
        _ => Err(ApiError::new(
            ApiErrorCode::InvalidArguments,
            format!("`{command}` takes no arguments"),
        )),
    }
}

fn parse_args<T: DeserializeOwned>(command: &str, args: Value) -> Result<T, ApiError> {
    serde_json::from_value(args).map_err(|e| {
        ApiError::new(
            ApiErrorCode::InvalidArguments,
            format!("invalid arguments for `{command}`: {e}"),
        )
    })
}

fn respond<T: Serialize>(result: T) -> Result<Value, ApiError> {
    serde_json::to_value(result).map_err(|e| ApiError::internal(format!("serialize result: {e}")))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::testing::{FakeMetadata, MemFs, TestWorld};

    fn api() -> Api {
        Api::new(TestWorld::new(MemFs::new(), FakeMetadata::default()).context()).unwrap()
    }

    #[test]
    fn app_info_returns_name_and_version() {
        let result = api()
            .call("app_info", Value::Null)
            .expect("app_info succeeds");

        assert_eq!(
            result,
            json!({ "name": "Picturesque", "version": env!("CARGO_PKG_VERSION") })
        );
    }

    #[test]
    fn app_info_accepts_empty_object_arguments() {
        assert!(api().call("app_info", json!({})).is_ok());
    }

    #[test]
    fn app_info_rejects_arguments() {
        let err = api()
            .call("app_info", json!({ "unexpected": 1 }))
            .expect_err("arguments are rejected");

        assert_eq!(err.code, ApiErrorCode::InvalidArguments);
    }

    #[test]
    fn unknown_command_is_reported() {
        let err = api()
            .call("does_not_exist", Value::Null)
            .expect_err("unknown command fails");

        assert_eq!(err.code, ApiErrorCode::UnknownCommand);
        assert!(err.message.contains("does_not_exist"));
    }

    #[test]
    fn errors_serialize_with_snake_case_codes() {
        let err = ApiError::new(ApiErrorCode::InvalidArguments, "bad");

        assert_eq!(
            serde_json::to_value(err).expect("serializes"),
            json!({ "code": "invalid_arguments", "message": "bad" })
        );
    }

    #[test]
    fn get_settings_returns_defaults() {
        let result = api().call("get_settings", Value::Null).unwrap();

        assert_eq!(
            result,
            json!({
                "library_root": null,
                "path_template": crate::settings::DEFAULT_PATH_TEMPLATE,
                "sidecar_naming": "file_extension",
                "series": { "max_gap_seconds": 30, "max_distance": 16 },
                "ranking": { "sharpness": 0.6, "exposure": 0.3, "noise": 0.1, "aesthetic": 0.4 },
                "ai": { "provider": "auto" },
                "import": { "rescan_minutes": 5 },
                "auto_pick": { "enabled": true, "min_quality": 0.5 },
                "onboarding": { "tour_done": false },
                "open_folder": null,
                "recent_folders": [],
                "labels": { "pick": "green", "auto_pick": "yellow" },
                "delete_after_copy": "verified",
                "pair_view": "image"
            })
        );
    }

    #[test]
    fn update_settings_applies_and_returns_settings() {
        let api = api();

        let result = api
            .call(
                "update_settings",
                json!({ "path_template": "{year}/{name}.{ext}" }),
            )
            .unwrap();

        assert_eq!(result["path_template"], "{year}/{name}.{ext}");
        assert_eq!(api.call("get_settings", Value::Null).unwrap(), result);
    }

    #[test]
    fn invalid_settings_are_invalid_arguments() {
        let err = api()
            .call(
                "update_settings",
                json!({ "library_root": "relative/path" }),
            )
            .unwrap_err();

        assert_eq!(err.code, ApiErrorCode::InvalidArguments);
        assert!(err.message.contains("absolute"));
    }

    #[test]
    fn ai_status_follows_the_provider_setting() {
        let api = api();
        let status = api.call("ai_status", Value::Null).unwrap();
        assert_eq!(status["choice"], "auto");
        assert_eq!(status["provider"], "cpu");
        assert_eq!(status["available"], json!(["cpu"]));

        let settings = api
            .call("update_settings", json!({ "ai": { "provider": "webgpu" } }))
            .unwrap();
        assert_eq!(settings["ai"]["provider"], "webgpu");
        let status = api.call("ai_status", Value::Null).unwrap();
        assert_eq!(status["choice"], "webgpu");
        assert_eq!(status["provider"], "cpu");
        assert!(status["problem"].as_str().unwrap().contains("WebGPU"));

        let err = api
            .call("update_settings", json!({ "ai": { "provider": "tpu" } }))
            .unwrap_err();
        assert_eq!(err.code, ApiErrorCode::InvalidArguments);
    }

    #[test]
    fn unknown_settings_fields_are_rejected() {
        let err = api()
            .call("update_settings", json!({ "colour": "blue" }))
            .unwrap_err();

        assert_eq!(err.code, ApiErrorCode::InvalidArguments);
    }

    mod decisions {
        use picturesque_core::hash::ContentHash;
        use picturesque_core::media::MediaFormat;
        use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets};
        use serde_json::json;

        use std::path::Path;

        use serde_json::Value;

        use crate::api::{Api, ApiErrorCode};
        use crate::ports::catalog::PhotoCatalog;
        use crate::ports::settings::SettingsStore;
        use crate::testing::{FakeMetadata, MemFs, TestWorld};

        fn api_with_photos(n: u8) -> (Api, TestWorld) {
            let world = TestWorld::new(MemFs::new(), FakeMetadata::default());
            for i in 0..n {
                world
                    .catalog
                    .insert_photo(&NewPhoto {
                        metadata: Default::default(),
                        assets: PhotoAssets::new(vec![Asset {
                            format: MediaFormat::Jpeg,
                            path: format!("{i}.jpg").into(),
                            size: 1,
                            hash: ContentHash::from_bytes([i; 32]),
                        }])
                        .unwrap(),
                    })
                    .unwrap();
            }
            (Api::new(world.context()).unwrap(), world)
        }

        #[test]
        fn sets_flags_and_ratings_independently() {
            let (api, _w) = api_with_photos(2);

            api.call(
                "set_decisions",
                json!({ "changes": [{ "photo_id": 1, "rating": 4 }] }),
            )
            .unwrap();
            let result = api
                .call("set_decisions", json!({ "changes": [{ "photo_id": 1, "flag": "pick" }, { "photo_id": 2, "flag": "reject" }] }))
                .unwrap();

            assert_eq!(
                result,
                json!([{ "photo_id": 1, "flag": "pick", "rating": 4 }, { "photo_id": 2, "flag": "reject", "rating": 0 }])
            );
        }

        /// Photos 1 and 2 are suggested keepers; 2 is rejected.
        fn finishing_world() -> (Api, TestWorld) {
            let (api, world) = api_with_photos(3);
            world
                .settings
                .save_settings(&crate::settings::Settings {
                    library_root: Some("/lib".into()),
                    ..Default::default()
                })
                .unwrap();
            let keeper = |id: i64| crate::ports::catalog::PhotoScore {
                id: picturesque_core::photo::PhotoId(id),
                score: 0.9,
                aesthetic: None,
                eyes_closed: None,
                faces: 0,
                keeper: true,
            };
            world
                .catalog
                .replace_scores(&[keeper(1), keeper(2)])
                .unwrap();
            api.call(
                "set_decisions",
                json!({ "changes": [{ "photo_id": 2, "flag": "reject" }] }),
            )
            .unwrap();
            (api, world)
        }

        fn label(world: &TestWorld, file: &str) -> Option<crate::ports::sidecars::ColorLabel> {
            world
                .sidecars
                .1
                .lock()
                .unwrap()
                .get(Path::new(file))
                .copied()
                .flatten()
        }

        #[test]
        fn finishing_labels_auto_picked_photos_and_writes_their_sidecars() {
            let (api, world) = finishing_world();
            assert_eq!(
                api.call("finish_status", Value::Null).unwrap(),
                json!({ "auto_picked": 1, "labels_to_write": 1, "picks": 0, "rejects": 1 })
            );

            let result = api.call("finish_folder", Value::Null).unwrap();

            assert_eq!(result["auto_labeled"], 1);
            assert_eq!(result["failed"], 0);
            assert_eq!(
                label(&world, "/lib/0.jpg.xmp"),
                Some(crate::ports::sidecars::ColorLabel::Yellow)
            );
            assert_eq!(label(&world, "/lib/1.jpg.xmp"), None, "rejected");
            assert_eq!(
                api.call("finish_status", Value::Null).unwrap()["labels_to_write"],
                0
            );
        }

        #[test]
        fn a_decision_after_finishing_replaces_the_auto_label() {
            let (api, world) = finishing_world();
            api.call("finish_folder", Value::Null).unwrap();

            api.call(
                "set_decisions",
                json!({ "changes": [{ "photo_id": 1, "flag": "pick" }] }),
            )
            .unwrap();
            api.flush_sidecars();
            assert_eq!(
                label(&world, "/lib/0.jpg.xmp"),
                Some(crate::ports::sidecars::ColorLabel::Green)
            );

            api.call(
                "set_decisions",
                json!({ "changes": [{ "photo_id": 1, "flag": "reject" }] }),
            )
            .unwrap();
            api.flush_sidecars();
            assert_eq!(label(&world, "/lib/0.jpg.xmp"), None);
        }

        #[test]
        fn a_photo_no_longer_auto_picked_loses_its_label_when_finishing_again() {
            let (api, world) = finishing_world();
            api.call("finish_folder", Value::Null).unwrap();
            world.catalog.replace_scores(&[]).unwrap();

            assert_eq!(
                api.call("finish_status", Value::Null).unwrap()["labels_to_write"],
                1
            );
            api.call("finish_folder", Value::Null).unwrap();

            assert_eq!(label(&world, "/lib/0.jpg.xmp"), None);
        }

        #[test]
        fn deciding_wakes_the_sidecar_writer() {
            let (api, world) = api_with_photos(1);

            api.call(
                "set_decisions",
                json!({ "changes": [{ "photo_id": 1, "flag": "pick" }] }),
            )
            .unwrap();

            let events = world.events.0.lock().unwrap();
            assert!(
                events.iter().any(|e| matches!(
                    e,
                    crate::ports::events::AppEvent::SidecarProgress(s) if s.pending == 1
                )),
                "a decision reports a pending sidecar"
            );
        }

        #[test]
        fn an_invalid_change_changes_nothing() {
            let (api, world) = api_with_photos(2);

            let bad_rating = api
                .call("set_decisions", json!({ "changes": [{ "photo_id": 1, "flag": "pick" }, { "photo_id": 2, "rating": 9 }] }))
                .unwrap_err();
            let missing = api
                .call("set_decisions", json!({ "changes": [{ "photo_id": 1, "flag": "pick" }, { "photo_id": 99, "flag": "pick" }] }))
                .unwrap_err();

            assert_eq!(bad_rating.code, ApiErrorCode::InvalidArguments);
            assert_eq!(missing.code, ApiErrorCode::NotFound);
            assert_eq!(
                world
                    .catalog
                    .decision(picturesque_core::photo::PhotoId(1))
                    .unwrap(),
                Default::default()
            );
        }
    }
}
