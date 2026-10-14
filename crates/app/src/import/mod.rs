//! Importing photos into the Clean library (ADR-0008).
//!
//! An import is a *run*: first a plan is built in the background (scan,
//! metadata, duplicates, targets) and stored; then, when the user starts
//! it, the plan is copied in the background. Both phases report progress
//! as events and can be cancelled; copying resumes where it stopped.

pub(crate) mod copy;
pub mod model;
pub(crate) mod plan;

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use picturesque_core::template::PathTemplate;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use self::copy::{copy_run, final_state};
use self::model::{FileStatus, ImportProblem, ImportRun, ImportState, PhotoStatus};
use self::plan::{PlanError, build_plan};
use crate::ports::catalog::{CatalogError, PhotoCatalog};
use crate::ports::events::{AppEvent, EventSink};
use crate::ports::fs::FileSystem;
use crate::ports::geocoder::ReverseGeocoder;
use crate::ports::imports::ImportStore;
use crate::ports::metadata::MetadataReader;
use crate::ports::settings::SettingsStore;

/// Infrastructure an import uses.
pub struct Deps {
    pub settings: Arc<dyn SettingsStore>,
    pub catalog: Arc<dyn PhotoCatalog>,
    pub store: Arc<dyn ImportStore>,
    pub fs: Arc<dyn FileSystem>,
    pub metadata: Arc<dyn MetadataReader>,
    pub geocoder: Arc<dyn ReverseGeocoder>,
    pub events: Arc<dyn EventSink>,
    pub sidecars: Arc<dyn crate::ports::sidecars::SidecarStore>,
}

impl fmt::Debug for Deps {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Deps").finish_non_exhaustive()
    }
}

/// Where a running phase is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    Scanning,
    Reading { done: usize, total: usize },
    Copying { done: usize, total: usize },
}

/// Which step a running import is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ImportPhase {
    Scanning,
    Reading,
    Copying,
}

/// Counts over a plan.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct PlanSummary {
    /// Photos found.
    pub photos: usize,
    /// Planned and not yet imported.
    pub to_import: usize,
    pub imported: usize,
    pub duplicates: usize,
    pub failed: usize,
    /// Files of photos to import or imported.
    pub files: usize,
    /// Bytes of those files.
    #[ts(type = "number")]
    pub bytes: u64,
}

/// Progress of a run, sent as an event and returned by `import_status`.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct ImportStatus {
    #[ts(type = "number")]
    pub run_id: i64,
    pub state: ImportState,
    /// Set while planning or copying.
    pub phase: Option<ImportPhase>,
    /// Progress within the phase (photos).
    pub done: usize,
    pub total: usize,
    /// Present once a plan exists.
    pub summary: Option<PlanSummary>,
    pub error: Option<String>,
}

/// One file of a planned photo, for display.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct PlannedFileView {
    pub source: String,
    /// Relative to the library folder, `/`-separated; `null` for photos
    /// that will not be copied (duplicates, failures found while planning).
    pub target: Option<String>,
    pub status: FileStatus,
    pub error: Option<String>,
}

/// One planned photo, for display.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct PlannedPhotoView {
    pub status: PhotoStatus,
    pub reason: Option<String>,
    /// Local capture time, ISO 8601.
    pub captured_at: Option<String>,
    /// "City, Country".
    pub place: Option<String>,
    pub camera: Option<String>,
    pub files: Vec<PlannedFileView>,
}

/// A run's plan, for review before and after copying.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct ImportPlanView {
    #[ts(type = "number")]
    pub run_id: i64,
    pub state: ImportState,
    pub library_root: String,
    pub template: String,
    pub summary: PlanSummary,
    pub photos: Vec<PlannedPhotoView>,
    pub problems: Vec<ImportProblem>,
    pub error: Option<String>,
}

/// Arguments of `plan_import`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct PlanImportArgs {
    /// Absolute source folders.
    #[ts(type = "string[]")]
    pub sources: Vec<PathBuf>,
}

/// Arguments of commands that act on one run.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct RunArgs {
    #[ts(type = "number")]
    pub run_id: i64,
}

/// Why an import command failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// The request cannot be carried out as asked; the message says why.
    Invalid(String),
    NotFound(i64),
    Storage(CatalogError),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(msg) => f.write_str(msg),
            Self::NotFound(id) => write!(f, "There is no import {id}."),
            Self::Storage(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ImportError {}

impl From<CatalogError> for ImportError {
    fn from(e: CatalogError) -> Self {
        Self::Storage(e)
    }
}

struct Running {
    run_id: i64,
    cancel: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

#[derive(Default)]
struct Jobs {
    running: Option<Running>,
    live: HashMap<i64, Progress>,
}

/// Plans, runs, cancels and reports imports. One import runs at a time.
pub struct ImportService {
    deps: Arc<Deps>,
    jobs: Arc<Mutex<Jobs>>,
}

impl fmt::Debug for ImportService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImportService").finish_non_exhaustive()
    }
}

impl ImportService {
    /// Runs left unfinished by a previous session are marked interrupted.
    pub fn new(deps: Deps) -> Result<Self, ImportError> {
        deps.store.mark_unfinished_interrupted()?;
        Ok(Self {
            deps: Arc::new(deps),
            jobs: Arc::default(),
        })
    }

    /// Starts planning an import of `sources` in the background and
    /// returns the run id.
    pub fn plan(&self, sources: Vec<PathBuf>) -> Result<i64, ImportError> {
        let settings = self.deps.settings.load_settings()?;
        let library_root = settings.library_root.ok_or_else(|| {
            ImportError::Invalid("Choose a library folder before importing.".into())
        })?;
        let template = PathTemplate::parse(&settings.path_template)
            .map_err(|e| ImportError::Invalid(e.to_string()))?;
        if sources.is_empty() {
            return Err(ImportError::Invalid(
                "Choose at least one folder to import from.".into(),
            ));
        }
        if let Some(relative) = sources.iter().find(|s| !s.is_absolute()) {
            return Err(ImportError::Invalid(format!(
                "Source folders must be absolute paths, got `{}`.",
                relative.display()
            )));
        }
        if let Some(inside) = sources.iter().find(|s| s.starts_with(&library_root)) {
            return Err(ImportError::Invalid(format!(
                "`{}` is inside the library folder; it cannot be imported into itself.",
                inside.display()
            )));
        }

        let mut jobs = self.lock_idle()?;
        let run_id = self
            .deps
            .store
            .create_run(&library_root, template.as_str(), &sources)?;
        let cancel = Arc::new(AtomicBool::new(false));
        let (deps, shared, flag) = (
            Arc::clone(&self.deps),
            Arc::clone(&self.jobs),
            Arc::clone(&cancel),
        );
        let handle = std::thread::spawn(move || {
            let progress = reporter(&deps, &shared, run_id, ImportState::Planning);
            let result = build_plan(&deps, &sources, &library_root, &template, &flag, &progress);
            let (state, error) = match result {
                Ok((mut photos, problems)) => {
                    match deps.store.save_plan(run_id, &mut photos, &problems) {
                        Ok(()) => (ImportState::Planned, None),
                        Err(e) => (ImportState::Failed, Some(e.to_string())),
                    }
                }
                Err(PlanError::Cancelled) => (ImportState::Cancelled, None),
                Err(PlanError::Catalog(e)) => (ImportState::Failed, Some(e.to_string())),
            };
            finish(&deps, &shared, run_id, state, error);
        });
        jobs.running = Some(Running {
            run_id,
            cancel,
            handle,
        });
        Ok(run_id)
    }

    /// Starts (or resumes) copying a planned run in the background.
    pub fn start(&self, run_id: i64) -> Result<(), ImportError> {
        let mut jobs = self.lock_idle()?;
        let run = self.load(run_id)?;
        if !run.state.can_start() {
            return Err(ImportError::Invalid(format!(
                "Import {run_id} cannot be started while it is {}.",
                run.state.as_str()
            )));
        }
        self.deps
            .store
            .set_state(run_id, ImportState::Copying, None)?;
        let cancel = Arc::new(AtomicBool::new(false));
        let (deps, shared, flag) = (
            Arc::clone(&self.deps),
            Arc::clone(&self.jobs),
            Arc::clone(&cancel),
        );
        let handle = std::thread::spawn(move || {
            let progress = reporter(&deps, &shared, run_id, ImportState::Copying);
            let (state, error) = match deps.store.load_run(run_id) {
                Ok(Some(run)) => final_state(&copy_run(&deps, &run, &flag, &progress)),
                Ok(None) => (ImportState::Failed, Some("the import disappeared".into())),
                Err(e) => (ImportState::Failed, Some(e.to_string())),
            };
            finish(&deps, &shared, run_id, state, error);
        });
        jobs.running = Some(Running {
            run_id,
            cancel,
            handle,
        });
        Ok(())
    }

    /// Asks the running planning or copying of `run_id` to stop. Copying
    /// stops between photos, so no photo is left half-copied.
    pub fn cancel(&self, run_id: i64) -> Result<(), ImportError> {
        let jobs = self.jobs.lock().expect("jobs lock");
        match &jobs.running {
            Some(r) if r.run_id == run_id && !r.handle.is_finished() => {
                r.cancel.store(true, Ordering::Relaxed);
                Ok(())
            }
            _ => Err(ImportError::Invalid(format!(
                "Import {run_id} is not running."
            ))),
        }
    }

    pub fn status(&self, run_id: i64) -> Result<ImportStatus, ImportError> {
        let run = self.load(run_id)?;
        let live = self
            .jobs
            .lock()
            .expect("jobs lock")
            .live
            .get(&run_id)
            .copied();
        Ok(status_of(&run, live))
    }

    pub fn plan_view(&self, run_id: i64) -> Result<ImportPlanView, ImportError> {
        Ok(plan_view(&self.load(run_id)?))
    }

    /// Waits for the running job, if any, to finish.
    pub fn wait(&self) {
        let running = self.jobs.lock().expect("jobs lock").running.take();
        if let Some(r) = running {
            let _ = r.handle.join();
        }
    }

    fn load(&self, run_id: i64) -> Result<ImportRun, ImportError> {
        self.deps
            .store
            .load_run(run_id)?
            .ok_or(ImportError::NotFound(run_id))
    }

    fn lock_idle(&self) -> Result<std::sync::MutexGuard<'_, Jobs>, ImportError> {
        let mut jobs = self.jobs.lock().expect("jobs lock");
        if let Some(r) = &jobs.running {
            if !r.handle.is_finished() {
                return Err(ImportError::Invalid(format!(
                    "Import {} is still running; wait for it or cancel it first.",
                    r.run_id
                )));
            }
            if let Some(done) = jobs.running.take() {
                let _ = done.handle.join();
            }
        }
        Ok(jobs)
    }
}

/// A progress callback that records live progress and emits events,
/// at most once per percent so large imports do not flood the UI.
fn reporter(
    deps: &Arc<Deps>,
    jobs: &Arc<Mutex<Jobs>>,
    run_id: i64,
    state: ImportState,
) -> impl Fn(Progress) + Send + Sync + use<> {
    let (deps, jobs) = (Arc::clone(deps), Arc::clone(jobs));
    let last = Mutex::new(None::<(Option<ImportPhase>, usize)>);
    move |progress| {
        jobs.lock()
            .expect("jobs lock")
            .live
            .insert(run_id, progress);
        let (phase, done, total) = phase_of(progress);
        let step = (total / 100).max(1);
        let mut last = last.lock().expect("progress lock");
        let changed = match *last {
            Some((p, d)) => p != phase || done == total || done >= d + step,
            None => true,
        };
        if changed {
            *last = Some((phase, done));
            let status = ImportStatus {
                run_id,
                state,
                phase,
                done,
                total,
                summary: None,
                error: None,
            };
            deps.events.emit(&AppEvent::ImportProgress(status));
        }
    }
}

fn finish(deps: &Deps, jobs: &Mutex<Jobs>, run_id: i64, state: ImportState, error: Option<String>) {
    let _ = deps.store.set_state(run_id, state, error.as_deref());
    jobs.lock().expect("jobs lock").live.remove(&run_id);
    if let Ok(Some(run)) = deps.store.load_run(run_id) {
        deps.events
            .emit(&AppEvent::ImportProgress(status_of(&run, None)));
    }
}

fn phase_of(progress: Progress) -> (Option<ImportPhase>, usize, usize) {
    match progress {
        Progress::Scanning => (Some(ImportPhase::Scanning), 0, 0),
        Progress::Reading { done, total } => (Some(ImportPhase::Reading), done, total),
        Progress::Copying { done, total } => (Some(ImportPhase::Copying), done, total),
    }
}

fn summarize(run: &ImportRun) -> PlanSummary {
    let mut s = PlanSummary {
        photos: run.photos.len(),
        ..Default::default()
    };
    for photo in &run.photos {
        match photo.status {
            PhotoStatus::Planned => s.to_import += 1,
            PhotoStatus::Imported => s.imported += 1,
            PhotoStatus::Duplicate => s.duplicates += 1,
            PhotoStatus::Failed => s.failed += 1,
        }
        if matches!(photo.status, PhotoStatus::Planned | PhotoStatus::Imported) {
            s.files += photo.files.len();
            s.bytes += photo.files.iter().map(|f| f.size).sum::<u64>();
        }
    }
    s
}

fn status_of(run: &ImportRun, live: Option<Progress>) -> ImportStatus {
    let (phase, done, total) = live.map_or((None, 0, 0), phase_of);
    let summary = (run.state != ImportState::Planning).then(|| summarize(run));
    ImportStatus {
        run_id: run.id,
        state: run.state,
        phase,
        done,
        total,
        summary,
        error: run.error.clone(),
    }
}

fn plan_view(run: &ImportRun) -> ImportPlanView {
    let photos = run
        .photos
        .iter()
        .map(|p| PlannedPhotoView {
            status: p.status,
            reason: p.reason.clone(),
            captured_at: p
                .metadata
                .captured_at
                .map(|t| t.local.format("%Y-%m-%dT%H:%M:%S").to_string()),
            place: p
                .metadata
                .place
                .as_ref()
                .map(|pl| format!("{}, {}", pl.city, pl.country)),
            camera: p.metadata.camera.as_ref().map(|c| c.display_name()),
            files: p
                .files
                .iter()
                .map(|f| PlannedFileView {
                    source: f.source.display().to_string(),
                    target: (!f.target.as_os_str().is_empty()).then(|| {
                        f.target
                            .iter()
                            .map(|c| c.to_string_lossy())
                            .collect::<Vec<_>>()
                            .join("/")
                    }),
                    status: f.status,
                    error: f.error.clone(),
                })
                .collect(),
        })
        .collect();
    ImportPlanView {
        run_id: run.id,
        state: run.state,
        library_root: run.library_root.display().to_string(),
        template: run.template.clone(),
        summary: summarize(run),
        photos,
        problems: run.problems.clone(),
        error: run.error.clone(),
    }
}

#[cfg(test)]
mod tests;
