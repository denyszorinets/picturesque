//! The continuous import pipeline (ADR-0012, `docs/features/import.rst`).
//!
//! Folders are added once as *sources*. Scanning a source records every
//! photo it holds as a *job*; a worker takes the jobs one at a time:
//!
//! 1. **read**: metadata, place, an existing sidecar decision;
//! 2. **duplicate check**: a photo whose RAW or image is already in the
//!    library is not copied;
//! 3. **copy**: every file is copied through a temporary file, verified by
//!    hash and renamed into place, all or nothing per photo (ADR-0008);
//! 4. **catalogued**: the photo appears in the library right away;
//!    thumbnails, analysis and series follow in the background.
//!
//! Jobs live in the catalog, so after a restart the queue continues where
//! it stopped. Unchanged photos are not imported again when a source is
//! rescanned. Every step is written to the pipeline log.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant, UNIX_EPOCH};

use picturesque_core::grouping::base_name;
use picturesque_core::hash::ContentHash;
use picturesque_core::media::{AssetRole, MediaFormat};
use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets, PhotoId};
use picturesque_core::template::{PathTemplate, TemplateValues, unique_stem};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ports::catalog::{CatalogError, PhotoCatalog};
use crate::ports::events::{AppEvent, EventSink};
use crate::ports::fs::FileSystem;
use crate::ports::geocoder::ReverseGeocoder;
use crate::ports::metadata::MetadataReader;
use crate::ports::pipeline::{
    ImportSource, Job, JobFile, JobStage, LogEntry, LogLevel, PipelineStore,
};
use crate::ports::settings::SettingsStore;
use crate::ports::sidecars::SidecarStore;
use crate::ports::watcher::FolderWatcher;
use crate::scan::{SourcePhoto, scan_sources};

mod copy;
mod read;
mod schedule;

use copy::{PlannedFile, copy_file};
use read::{Readers, read_one};
use schedule::Schedule;

/// Log entries kept.
const LOG_KEEP: usize = 20_000;
/// While paused (no usable library folder), how often to look again.
const PAUSE_RETRY: Duration = Duration::from_secs(10);
/// How often progress is sent while photos are being imported.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

/// What the pipeline needs from the outside world.
pub struct PipelineDeps {
    pub store: Arc<dyn PipelineStore>,
    pub catalog: Arc<dyn PhotoCatalog>,
    pub settings: Arc<dyn SettingsStore>,
    pub fs: Arc<dyn FileSystem>,
    pub metadata: Arc<dyn MetadataReader>,
    pub geocoder: Arc<dyn ReverseGeocoder>,
    pub sidecars: Arc<dyn SidecarStore>,
    pub events: Arc<dyn EventSink>,
    pub watcher: Arc<dyn FolderWatcher>,
}

impl fmt::Debug for PipelineDeps {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PipelineDeps").finish_non_exhaustive()
    }
}

/// The photo being worked on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ActiveJob {
    #[ts(type = "number")]
    pub job_id: i64,
    pub file_name: String,
    pub stage: JobStage,
}

/// Returned by `pipeline_status` and sent as `pipeline_progress`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct PipelineStatus {
    /// Photos waiting (including the one being worked on).
    pub queued: usize,
    pub imported: usize,
    pub duplicates: usize,
    pub failed: usize,
    pub active: Option<ActiveJob>,
    /// Whether a source is being scanned.
    pub scanning: bool,
    /// Why importing is paused (for example no library folder).
    pub paused: Option<String>,
}

/// Which jobs to list.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum JobFilter {
    #[default]
    All,
    /// Queued, reading or copying.
    Active,
    Imported,
    Duplicate,
    Failed,
}

impl JobFilter {
    fn stages(self) -> Option<Vec<JobStage>> {
        match self {
            Self::All => None,
            Self::Active => Some(vec![JobStage::Queued, JobStage::Reading, JobStage::Copying]),
            Self::Imported => Some(vec![JobStage::Imported]),
            Self::Duplicate => Some(vec![JobStage::Duplicate]),
            Self::Failed => Some(vec![JobStage::Failed]),
        }
    }
}

/// A job, as the activity view shows it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct JobView {
    #[ts(type = "number")]
    pub id: i64,
    pub file_name: String,
    /// The primary file's full path in the source.
    pub source_path: String,
    /// Names of every file of the photo.
    pub files: Vec<String>,
    pub stage: JobStage,
    pub error: Option<String>,
    #[ts(type = "number | null")]
    pub photo_id: Option<i64>,
    /// UTC, ISO 8601.
    pub updated_at: String,
    /// For a failed photo, the stage it failed in (`reading` or `copying`).
    pub failed_stage: Option<JobStage>,
    /// For imported photos: whether the grid thumbnail exists.
    pub thumbnail: Option<bool>,
    /// For imported photos: analyses done and to do.
    pub analysis: Option<[usize; 2]>,
}

/// Arguments of `add_import_source`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct AddSourceArgs {
    #[ts(type = "string")]
    pub path: PathBuf,
    /// Default true.
    #[ts(optional)]
    pub watch: Option<bool>,
}

/// Arguments of commands acting on one source.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct SourceArgs {
    #[ts(type = "number")]
    pub id: i64,
}

/// Arguments of `set_import_source_watch`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct SourceWatchArgs {
    #[ts(type = "number")]
    pub id: i64,
    pub watch: bool,
}

/// Arguments of `scan_import_sources`.
#[derive(Debug, Clone, Copy, Default, Deserialize, TS)]
#[serde(default, deny_unknown_fields)]
#[ts(export)]
pub struct ScanArgs {
    /// One source; default all of them.
    #[ts(optional, type = "number")]
    pub id: Option<i64>,
}

/// Arguments of `pipeline_jobs`.
#[derive(Debug, Clone, Copy, Default, Deserialize, TS)]
#[serde(default, deny_unknown_fields)]
#[ts(export)]
pub struct JobsArgs {
    #[ts(optional)]
    pub filter: Option<JobFilter>,
    /// 1–500, default 100.
    #[ts(optional)]
    pub limit: Option<usize>,
    /// Jobs older than this id (paging).
    #[ts(optional, type = "number")]
    pub before: Option<i64>,
}

/// Arguments of `pipeline_log`.
#[derive(Debug, Clone, Copy, Default, Deserialize, TS)]
#[serde(default, deny_unknown_fields)]
#[ts(export)]
pub struct LogArgs {
    /// Default `info` (everything).
    #[ts(optional)]
    pub min_level: Option<LogLevel>,
    /// 1–1000, default 200.
    #[ts(optional)]
    pub limit: Option<usize>,
    #[ts(optional, type = "number")]
    pub before: Option<i64>,
}

/// Arguments of `retry_failed_imports`.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(default, deny_unknown_fields)]
#[ts(export)]
pub struct RetryArgs {
    /// Some failed jobs; default all.
    #[ts(optional, type = "number[]")]
    pub ids: Option<Vec<i64>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipelineError {
    Invalid(String),
    Catalog(CatalogError),
}

impl fmt::Display for PipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(m) => f.write_str(m),
            Self::Catalog(e) => e.fmt(f),
        }
    }
}

impl From<CatalogError> for PipelineError {
    fn from(e: CatalogError) -> Self {
        Self::Catalog(e)
    }
}

/// What the background threads share.
#[derive(Default)]
struct Shared {
    /// Sources to scan (`None` in the set means all).
    scan_requests: HashSet<Option<i64>>,
    /// Set when new jobs may be waiting.
    work: bool,
    active: Option<ActiveJob>,
    paused: Option<String>,
    /// Watched sources by folder, for change notifications.
    watched: Vec<(PathBuf, i64)>,
    schedule: Option<Schedule>,
}

/// Runs the pipeline on two background threads: a scanner and a worker.
pub struct ImportPipeline {
    deps: PipelineDeps,
    shared: Mutex<Shared>,
    signal: Condvar,
    scanning: AtomicBool,
    started: AtomicBool,
    log_writes: AtomicUsize,
    last_progress: Mutex<Option<Instant>>,
    /// Sizes of catalogued media files: only files of these sizes can be
    /// duplicates and need hashing before they are copied.
    media_sizes: Mutex<Option<HashSet<u64>>>,
    /// Called after a photo was catalogued (thumbnails, analysis, series).
    on_imported: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
    /// Set once started, for the watcher's callback.
    me: OnceLock<Weak<Self>>,
}

impl fmt::Debug for ImportPipeline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImportPipeline").finish_non_exhaustive()
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn modified_ns(fs: &dyn FileSystem, path: &Path) -> Option<i64> {
    fs.stamp(path)
        .ok()?
        .modified
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| i64::try_from(d.as_nanos()).ok())
}

/// RAW if present, else the image, else the first file.
fn primary(files: &[JobFile]) -> &JobFile {
    files
        .iter()
        .find(|f| f.format == MediaFormat::Raw)
        .or_else(|| files.iter().find(|f| f.format.role() != AssetRole::Sidecar))
        .unwrap_or(&files[0])
}

fn stem_key(name: &str) -> String {
    base_name(name)
        .unwrap_or_else(|| name.rsplit_once('.').map_or(name, |(s, _)| s))
        .to_lowercase()
}

impl ImportPipeline {
    pub fn new(deps: PipelineDeps) -> Self {
        Self {
            deps,
            shared: Mutex::new(Shared::default()),
            signal: Condvar::new(),
            scanning: AtomicBool::new(false),
            started: AtomicBool::new(false),
            log_writes: AtomicUsize::new(0),
            last_progress: Mutex::new(None),
            media_sizes: Mutex::new(None),
            on_imported: Mutex::new(None),
            me: OnceLock::new(),
        }
    }

    /// Sets what runs after each catalogued photo.
    pub fn on_imported(&self, f: impl Fn() + Send + Sync + 'static) {
        *self.on_imported.lock().expect("hook lock") = Some(Box::new(f));
    }

    fn log(&self, level: LogLevel, job_id: Option<i64>, photo_id: Option<PhotoId>, message: &str) {
        let _ = self.deps.store.append_log(level, job_id, photo_id, message);
        if self.log_writes.fetch_add(1, Ordering::Relaxed) % 500 == 499 {
            let _ = self.deps.store.prune_log(LOG_KEEP);
        }
    }

    // ---- sources ----

    pub fn sources(&self) -> Result<Vec<ImportSource>, PipelineError> {
        Ok(self.deps.store.sources()?)
    }

    /// Adds a folder to import from and scans it.
    pub fn add_source(&self, path: &Path, watch: bool) -> Result<ImportSource, PipelineError> {
        if !path.is_absolute() {
            return Err(PipelineError::Invalid(format!(
                "The folder must be an absolute path, got `{}`.",
                path.display()
            )));
        }
        self.deps.fs.list_dir(path).map_err(|reason| {
            PipelineError::Invalid(format!("Cannot read `{}`: {reason}.", path.display()))
        })?;
        if let Ok(Some(root)) = self.deps.settings.load_settings().map(|s| s.library_root)
            && (root.starts_with(path) || path.starts_with(&root))
        {
            return Err(PipelineError::Invalid(
                "Import from a folder outside the library folder.".into(),
            ));
        }
        let source = self.deps.store.add_source(path, watch)?;
        self.sync_watches();
        self.log(
            LogLevel::Info,
            None,
            None,
            &format!("Added import folder {}", path.display()),
        );
        self.request_scan(Some(source.id));
        Ok(source)
    }

    pub fn remove_source(&self, id: i64) -> Result<(), PipelineError> {
        let source = self.source(id)?;
        self.deps.store.remove_source(id)?;
        self.sync_watches();
        self.log(
            LogLevel::Info,
            None,
            None,
            &format!("Removed import folder {}", source.path.display()),
        );
        self.emit_progress(true);
        Ok(())
    }

    pub fn set_watch(&self, id: i64, watch: bool) -> Result<ImportSource, PipelineError> {
        self.source(id)?;
        self.deps.store.set_source_watch(id, watch)?;
        self.sync_watches();
        if watch {
            self.request_scan(Some(id));
        }
        self.source(id)
    }

    fn source(&self, id: i64) -> Result<ImportSource, PipelineError> {
        self.deps
            .store
            .sources()?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| PipelineError::Invalid(format!("There is no import folder {id}.")))
    }

    /// Asks the scanner to scan one source, or all.
    pub fn request_scan(&self, id: Option<i64>) {
        let mut shared = self.shared.lock().expect("pipeline lock");
        shared.scan_requests.insert(id);
        self.signal.notify_all();
    }

    // ---- status and history ----

    pub fn status(&self) -> PipelineStatus {
        let counts: HashMap<JobStage, usize> = self
            .deps
            .store
            .job_counts()
            .unwrap_or_default()
            .into_iter()
            .collect();
        let count = |s: JobStage| counts.get(&s).copied().unwrap_or(0);
        let shared = self.shared.lock().expect("pipeline lock");
        PipelineStatus {
            queued: count(JobStage::Queued) + count(JobStage::Reading) + count(JobStage::Copying),
            imported: count(JobStage::Imported),
            duplicates: count(JobStage::Duplicate),
            failed: count(JobStage::Failed),
            active: shared.active.clone(),
            scanning: self.scanning.load(Ordering::Acquire),
            paused: shared.paused.clone(),
        }
    }

    /// Jobs newest first; `details` adds thumbnail and analysis state for
    /// imported photos.
    pub fn jobs(
        &self,
        filter: JobFilter,
        limit: usize,
        before: Option<i64>,
        details: &dyn Fn(PhotoId) -> (bool, [usize; 2]),
    ) -> Result<Vec<JobView>, PipelineError> {
        let stages = filter.stages();
        let jobs = self
            .deps
            .store
            .jobs(stages.as_deref(), limit.clamp(1, 500), before)?;
        Ok(jobs
            .into_iter()
            .map(|job| {
                let first = primary(&job.files);
                let (thumbnail, analysis) = match (job.stage, job.photo_id) {
                    (JobStage::Imported, Some(id)) => {
                        let (t, a) = details(id);
                        (Some(t), Some(a))
                    }
                    _ => (None, None),
                };
                JobView {
                    id: job.id,
                    file_name: file_name(&first.path),
                    source_path: first.path.display().to_string(),
                    files: job.files.iter().map(|f| file_name(&f.path)).collect(),
                    stage: job.stage,
                    error: job.error.clone(),
                    photo_id: job.photo_id.map(|p| p.0),
                    updated_at: job.updated_at.clone(),
                    failed_stage: job.failed_stage,
                    thumbnail,
                    analysis,
                }
            })
            .collect())
    }

    pub fn log_entries(
        &self,
        min_level: LogLevel,
        limit: usize,
        before: Option<i64>,
    ) -> Result<Vec<LogEntry>, PipelineError> {
        Ok(self
            .deps
            .store
            .log(min_level, limit.clamp(1, 1000), before)?)
    }

    pub fn retry(&self, ids: Option<&[i64]>) -> Result<usize, PipelineError> {
        let n = self.deps.store.retry_failed(ids)?;
        if n > 0 {
            self.log(
                LogLevel::Info,
                None,
                None,
                &format!("Retrying {n} failed photos"),
            );
            self.wake_worker();
        }
        self.emit_progress(true);
        Ok(n)
    }

    fn wake_worker(&self) {
        self.shared.lock().expect("pipeline lock").work = true;
        self.signal.notify_all();
    }

    fn emit_progress(&self, force: bool) {
        let mut last = self.last_progress.lock().expect("progress lock");
        if !force && last.is_some_and(|t| t.elapsed() < PROGRESS_INTERVAL) {
            return;
        }
        *last = Some(Instant::now());
        drop(last);
        self.deps
            .events
            .emit(&AppEvent::PipelineProgress(self.status()));
    }

    // ---- background threads ----

    /// Resumes unfinished jobs, scans watched sources, and starts the
    /// scanner and worker threads (once).
    pub fn start(self: &Arc<Self>) {
        if self.started.swap(true, Ordering::AcqRel) {
            return;
        }
        let _ = self.me.set(Arc::downgrade(self));
        self.resume();
        self.sync_watches();
        self.scan_watched();
        self.wake_worker();
        let scanner = Arc::clone(self);
        std::thread::spawn(move || scanner.scanner());
        let worker = Arc::clone(self);
        std::thread::spawn(move || worker.worker());
    }

    /// Puts photos interrupted by a stop back in the queue.
    pub fn resume(&self) {
        if let Ok(n) = self.deps.store.requeue_unfinished()
            && n > 0
        {
            self.log(
                LogLevel::Info,
                None,
                None,
                &format!("Resuming {n} photos interrupted when the app stopped"),
            );
        }
    }

    /// Asks for a scan of every watched source.
    pub fn scan_watched(&self) {
        for source in self.deps.store.sources().unwrap_or_default() {
            if source.watch {
                self.request_scan(Some(source.id));
            }
        }
    }

    /// Watches the watched sources' folders (once started).
    fn sync_watches(&self) {
        let Some(me) = self.me.get().cloned() else {
            return;
        };
        let watched: Vec<(PathBuf, i64)> = self
            .deps
            .store
            .sources()
            .unwrap_or_default()
            .into_iter()
            .filter(|s| s.watch)
            .map(|s| (s.path, s.id))
            .collect();
        let folders: Vec<PathBuf> = watched.iter().map(|(p, _)| p.clone()).collect();
        self.shared.lock().expect("pipeline lock").watched = watched;
        let failed = self.deps.watcher.watch(
            &folders,
            Arc::new(move |folder: &Path| {
                if let Some(pipeline) = me.upgrade() {
                    pipeline.folder_changed(folder);
                }
            }),
        );
        for (folder, reason) in failed {
            self.log(
                LogLevel::Warn,
                None,
                None,
                &format!(
                    "Cannot watch {} for new photos ({reason}); it is still scanned periodically",
                    folder.display()
                ),
            );
        }
    }

    /// Something changed in a watched folder: scan it once it is quiet.
    pub fn folder_changed(&self, folder: &Path) {
        let mut shared = self.shared.lock().expect("pipeline lock");
        let Some(id) = shared
            .watched
            .iter()
            .find(|(p, _)| p == folder)
            .map(|&(_, id)| id)
        else {
            return;
        };
        let now = Instant::now();
        shared
            .schedule
            .get_or_insert_with(|| Schedule::new(now))
            .changed(id, now);
        self.signal.notify_all();
    }

    /// The periodic rescan interval, if any.
    fn rescan_interval(&self) -> Option<Duration> {
        let minutes = self
            .deps
            .settings
            .load_settings()
            .map(|s| s.import.rescan_minutes)
            .unwrap_or(5);
        (minutes > 0).then(|| Duration::from_secs(60 * u64::from(minutes)))
    }

    fn scanner(&self) {
        // Settings changes take effect within this long.
        const MAX_WAIT: Duration = Duration::from_secs(60);
        loop {
            let rescan = self.rescan_interval();
            let requests = {
                let mut shared = self.shared.lock().expect("pipeline lock");
                loop {
                    let now = Instant::now();
                    let due = shared
                        .schedule
                        .get_or_insert_with(|| Schedule::new(now))
                        .due(now, rescan);
                    let mut requests = std::mem::take(&mut shared.scan_requests);
                    requests.extend(due.changed.into_iter().map(Some));
                    if due.rescan {
                        requests.extend(shared.watched.iter().map(|&(_, id)| Some(id)));
                    }
                    if !requests.is_empty() {
                        break requests;
                    }
                    let wait = due.wait.unwrap_or(MAX_WAIT).min(MAX_WAIT);
                    let (guard, timeout) = self
                        .signal
                        .wait_timeout(shared, wait)
                        .expect("pipeline lock");
                    shared = guard;
                    if timeout.timed_out() && due.wait.is_none_or(|w| w >= MAX_WAIT) {
                        // Pick up a changed rescan interval.
                        break HashSet::new();
                    }
                }
            };
            self.scan_now(&requests);
        }
    }

    /// Scans the requested sources (`None` = all) on the calling thread.
    pub fn scan_now(&self, requests: &HashSet<Option<i64>>) {
        let sources: Vec<ImportSource> = self
            .deps
            .store
            .sources()
            .unwrap_or_default()
            .into_iter()
            .filter(|s| requests.contains(&None) || requests.contains(&Some(s.id)))
            .collect();
        if sources.is_empty() {
            return;
        }
        self.scanning.store(true, Ordering::Release);
        self.emit_progress(true);
        for source in sources {
            self.scan_source(&source);
        }
        self.scanning.store(false, Ordering::Release);
        self.emit_progress(true);
    }

    fn scan_source(&self, source: &ImportSource) {
        let fs = self.deps.fs.as_ref();
        if let Err(reason) = fs.list_dir(&source.path) {
            let message = format!(
                "Cannot read import folder {}: {reason}",
                source.path.display()
            );
            if source.scan_error.as_deref() != Some(message.as_str()) {
                self.log(LogLevel::Error, None, None, &message);
            }
            let _ = self.deps.store.record_scan(source.id, Some(&message));
            return;
        }
        let skip: Vec<PathBuf> = self
            .deps
            .settings
            .load_settings()
            .ok()
            .and_then(|s| s.library_root)
            .into_iter()
            .collect();
        let report = scan_sources(fs, std::slice::from_ref(&source.path), &skip);
        for (path, reason) in &report.unreadable {
            self.log(
                LogLevel::Warn,
                None,
                None,
                &format!("Cannot read folder {}: {reason}", path.display()),
            );
        }
        let mut queued = 0;
        for photo in &report.photos {
            let files: Vec<JobFile> = photo
                .files()
                .map(|f| JobFile {
                    path: f.path.clone(),
                    format: f.format,
                    size: f.size,
                    modified_ns: modified_ns(fs, &f.path),
                })
                .collect();
            let key = photo.primary().path.clone();
            match self.deps.store.offer_job(source.id, &key, &files) {
                Ok(Some(_)) => {
                    queued += 1;
                    if queued % 50 == 1 {
                        self.wake_worker();
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    self.log(
                        LogLevel::Error,
                        None,
                        None,
                        &format!("Cannot queue {}: {e}", key.display()),
                    );
                }
            }
        }
        if queued > 0 {
            self.log(
                LogLevel::Info,
                None,
                None,
                &format!("Found {queued} new photos in {}", source.path.display()),
            );
            self.wake_worker();
        }
        let _ = self.deps.store.record_scan(source.id, None);
    }

    fn worker(&self) {
        loop {
            if !self.run_next() {
                let mut shared = self.shared.lock().expect("pipeline lock");
                let wait = if shared.paused.is_some() {
                    PAUSE_RETRY
                } else {
                    Duration::from_secs(60)
                };
                if !shared.work {
                    shared = self
                        .signal
                        .wait_timeout(shared, wait)
                        .expect("pipeline lock")
                        .0;
                }
                shared.work = false;
            }
        }
    }

    /// Imports every queued photo on the calling thread (tests and tools);
    /// returns how many jobs were processed.
    pub fn run_until_idle(&self) -> usize {
        let mut n = 0;
        while self.run_next() {
            n += 1;
        }
        n
    }

    /// Processes the next queued job; `false` when there is nothing to do
    /// (or importing is paused).
    fn run_next(&self) -> bool {
        let job = match self.deps.store.next_job() {
            Ok(Some(job)) => job,
            Ok(None) => {
                // Nothing waits, so nothing is paused.
                if self
                    .shared
                    .lock()
                    .expect("pipeline lock")
                    .paused
                    .take()
                    .is_some()
                {
                    self.emit_progress(true);
                }
                return false;
            }
            Err(e) => {
                self.log(
                    LogLevel::Error,
                    None,
                    None,
                    &format!("Cannot read the import queue: {e}"),
                );
                return false;
            }
        };
        let root = match self.usable_library() {
            Ok(root) => {
                let was_paused = self.shared.lock().expect("pipeline lock").paused.take();
                if was_paused.is_some() {
                    self.log(LogLevel::Info, None, None, "Importing resumed");
                    self.emit_progress(true);
                }
                root
            }
            Err(reason) => {
                let mut shared = self.shared.lock().expect("pipeline lock");
                if shared.paused.as_deref() != Some(reason.as_str()) {
                    shared.paused = Some(reason.clone());
                    drop(shared);
                    self.log(
                        LogLevel::Warn,
                        None,
                        None,
                        &format!("Importing paused: {reason}"),
                    );
                    self.emit_progress(true);
                }
                return false;
            }
        };
        self.process(&root, &job);
        true
    }

    fn usable_library(&self) -> Result<PathBuf, String> {
        let settings = self
            .deps
            .settings
            .load_settings()
            .map_err(|e| e.to_string())?;
        let root = settings
            .library_root
            .ok_or("choose a library folder in Settings")?;
        self.deps.fs.probe_writable_dir(&root).map_err(|reason| {
            format!(
                "the library folder {} is not available ({reason})",
                root.display()
            )
        })?;
        Ok(root)
    }

    fn set_stage(&self, job: &Job, stage: JobStage, photo: Option<PhotoId>, error: Option<&str>) {
        let _ = self.deps.store.set_job_stage(job.id, stage, photo, error);
        let mut shared = self.shared.lock().expect("pipeline lock");
        shared.active = (!stage.is_final()).then(|| ActiveJob {
            job_id: job.id,
            file_name: file_name(&primary(&job.files).path),
            stage,
        });
        drop(shared);
        self.emit_progress(stage.is_final() && stage != JobStage::Imported);
    }

    fn process(&self, root: &Path, job: &Job) {
        let name = file_name(&primary(&job.files).path);
        let fail = |message: String| {
            self.set_stage(job, JobStage::Failed, None, Some(&message));
            self.log(
                LogLevel::Error,
                Some(job.id),
                None,
                &format!("{name}: {message}"),
            );
        };

        // 1. Read.
        self.set_stage(job, JobStage::Reading, None, None);
        let fs = self.deps.fs.as_ref();
        if let Some(changed) = job.files.iter().find(|f| {
            fs.stamp(&f.path).map(|s| s.size).ok() != Some(f.size)
                || modified_ns(fs, &f.path) != f.modified_ns
        }) {
            // The next scan queues it again with its files as they are then.
            return fail(format!(
                "{} changed since it was found; it will be imported again",
                file_name(&changed.path)
            ));
        }
        let source = SourcePhoto {
            raw: job
                .files
                .iter()
                .find(|f| f.format == MediaFormat::Raw)
                .map(to_source),
            image: job
                .files
                .iter()
                .find(|f| f.format != MediaFormat::Raw && f.format.role() != AssetRole::Sidecar)
                .map(to_source),
            sidecars: job
                .files
                .iter()
                .filter(|f| f.format.role() == AssetRole::Sidecar)
                .map(to_source)
                .collect(),
        };
        if source.raw.is_none() && source.image.is_none() {
            return fail("no RAW or image file".into());
        }
        let readers = Readers {
            fs: self.deps.fs.as_ref(),
            metadata: self.deps.metadata.as_ref(),
            geocoder: self.deps.geocoder.as_ref(),
            sidecars: self.deps.sidecars.as_ref(),
        };
        let mut photo = read_one(&readers, &source);
        if let Some(reason) = &photo.reason {
            return fail(reason.clone());
        }

        // 2. Duplicate check: hash media files whose size is already
        //    catalogued.
        let sizes = {
            let mut cached = self.media_sizes.lock().expect("sizes lock");
            if cached.is_none() {
                *cached = self.deps.catalog.media_sizes().ok();
            }
            cached.clone().unwrap_or_default()
        };
        for file in photo
            .files
            .iter_mut()
            .filter(|f| f.format.role() != AssetRole::Sidecar && sizes.contains(&f.size))
        {
            match self.deps.fs.hash_file(&file.source) {
                Ok((hash, _)) => {
                    file.hash = Some(hash);
                    match self.deps.catalog.photo_with_media_hash(&hash) {
                        Ok(Some(existing)) => {
                            let message =
                                format!("{} is already in the library", file_name(&file.source));
                            self.set_stage(
                                job,
                                JobStage::Duplicate,
                                Some(existing),
                                Some(&message),
                            );
                            self.log(
                                LogLevel::Warn,
                                Some(job.id),
                                Some(existing),
                                &format!("{name}: skipped, {message}"),
                            );
                            return;
                        }
                        Ok(None) => {}
                        Err(e) => return fail(e.to_string()),
                    }
                }
                Err(e) => return fail(format!("cannot read {}: {e}", file_name(&file.source))),
            }
        }

        // 3. Copy and verify.
        self.set_stage(job, JobStage::Copying, None, None);
        let template = match self
            .deps
            .settings
            .load_settings()
            .map_err(|e| e.to_string())
            .and_then(|s| PathTemplate::parse(&s.path_template).map_err(|e| e.to_string()))
        {
            Ok(t) => t,
            Err(e) => return fail(format!("invalid folder structure: {e}")),
        };
        assign_target(
            self.deps.fs.as_ref(),
            root,
            &template,
            &mut photo.files,
            &photo.metadata,
            job.id,
        );
        let mut copied: Vec<(PlannedFile, ContentHash)> = Vec::new();
        for file in &photo.files {
            match copy_file(self.deps.fs.as_ref(), root, file) {
                Ok(hash) => copied.push((file.clone(), hash)),
                Err(reason) => {
                    // All or nothing: no RAW without its JPEG.
                    for (done, _) in &copied {
                        let _ = self.deps.fs.remove_file(&root.join(&done.target));
                    }
                    return fail(format!("{}: {reason}", file_name(&file.source)));
                }
            }
        }

        // 4. Catalogue.
        let assets = copied
            .iter()
            .map(|(file, hash)| Asset {
                format: file.format,
                path: file.target.clone(),
                size: file.size,
                hash: *hash,
            })
            .collect();
        let new = match PhotoAssets::new(assets) {
            Ok(assets) => NewPhoto {
                metadata: photo.metadata.clone(),
                assets,
            },
            Err(e) => return fail(e.to_string()),
        };
        let photo_id = match self.deps.catalog.insert_photo(&new) {
            Ok(id) => id,
            // Catalogued by an attempt that stopped before recording it.
            Err(CatalogError::Duplicate(_)) => {
                match self
                    .deps
                    .catalog
                    .photo_with_media_hash(&new.assets.primary().hash)
                {
                    Ok(Some(id)) => id,
                    _ => return fail("the photo is in the catalog but cannot be found".into()),
                }
            }
            Err(e) => return fail(e.to_string()),
        };
        if let Some(decision) = photo.decision {
            let _ = self.deps.catalog.set_decision(photo_id, decision);
        }
        if let Some(sizes) = self.media_sizes.lock().expect("sizes lock").as_mut() {
            sizes.extend(copied.iter().map(|(f, _)| f.size));
        }
        self.set_stage(job, JobStage::Imported, Some(photo_id), None);
        let target = &new.assets.primary().path;
        self.log(
            LogLevel::Info,
            Some(job.id),
            Some(photo_id),
            &format!("{name}: imported to {}", target.display()),
        );
        if let Some(hook) = self.on_imported.lock().expect("hook lock").as_ref() {
            hook();
        }
    }
}

fn to_source(f: &JobFile) -> crate::scan::SourceFile {
    crate::scan::SourceFile {
        path: f.path.clone(),
        format: f.format,
        size: f.size,
    }
}

/// Renders the photo's place in the library and picks a file name that
/// is free on disk for all its files.
fn assign_target(
    fs: &dyn FileSystem,
    root: &Path,
    template: &PathTemplate,
    files: &mut [PlannedFile],
    metadata: &picturesque_core::metadata::CaptureMetadata,
    seq: i64,
) {
    let primary = files
        .iter()
        .find(|f| f.format == MediaFormat::Raw)
        .or_else(|| files.iter().find(|f| f.format.role() != AssetRole::Sidecar))
        .unwrap_or(&files[0]);
    let primary_name = file_name(&primary.source);
    let original_stem = base_name(&primary_name).unwrap_or(&primary_name).to_owned();
    let rendered = template.render(&TemplateValues {
        captured_at: metadata.captured_at.map(|t| t.local),
        place: metadata.place.as_ref(),
        camera: metadata.camera.as_ref(),
        lens: metadata.lens.as_deref(),
        original_name: &original_stem,
        seq: u32::try_from(seq).ok(),
    });
    let suffixes: Vec<String> = files
        .iter()
        .map(|file| {
            let name = file_name(&file.source);
            match file.format.role() {
                // `IMG_1.CR3.xmp` keeps `.CR3.xmp`; `IMG_1.xmp` keeps `.xmp`.
                AssetRole::Sidecar => name.get(original_stem.len()..).unwrap_or(".xmp").to_owned(),
                _ => format!(".{}", name.rsplit_once('.').map_or("", |(_, e)| e)),
            }
        })
        .collect();
    let dir = root.join(&rendered.dir);
    let mut on_disk: HashMap<String, Vec<String>> = HashMap::new();
    for entry in fs.list_dir(&dir).unwrap_or_default() {
        on_disk
            .entry(stem_key(&entry.name))
            .or_default()
            .push(entry.name);
    }
    // A name is free if nothing uses it, or if what uses it is this
    // photo's own copy left by an interrupted attempt (same content).
    let leftover = |candidate: &str, existing: &[String]| {
        existing.iter().all(|name| {
            files.iter().zip(&suffixes).any(|(file, suffix)| {
                if *name != format!("{candidate}{suffix}") {
                    return false;
                }
                let theirs = fs.hash_file(&dir.join(name)).ok().map(|(h, _)| h);
                let ours = file
                    .hash
                    .or_else(|| fs.hash_file(&file.source).ok().map(|(h, _)| h));
                theirs.is_some() && theirs == ours
            })
        })
    };
    let stem = unique_stem(&rendered.stem, |candidate| {
        on_disk
            .get(&candidate.to_lowercase())
            .is_some_and(|existing| !leftover(candidate, existing))
    });
    for (file, suffix) in files.iter_mut().zip(&suffixes) {
        file.target = rendered.dir.join(format!("{stem}{suffix}"));
    }
}

#[cfg(test)]
mod tests;
