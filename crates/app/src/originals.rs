//! Deleting the original files of imported photos (ADR-0017).
//!
//! Importing only ever reads the originals (ADR-0008). When the user asks
//! for it, the originals of photos whose copy is verified can be deleted
//! afterwards, as a separate confirmed step: each photo is checked again
//! right before its files go, so a changed original or a lost or altered
//! library copy keeps the photo. The offer comes from the catalog's import
//! jobs and each deleted photo is recorded at once, so a cancelled or
//! interrupted run continues with what is left.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, UNIX_EPOCH};

use picturesque_core::media::AssetRole;
use picturesque_core::photo::Photo;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ports::catalog::{CatalogError, PhotoCatalog};
use crate::ports::events::{AppEvent, EventSink};
use crate::ports::fs::FileSystem;
use crate::ports::pipeline::{ImportSource, Job, JobFile, JobStage, PipelineStore};
use crate::ports::settings::SettingsStore;
use crate::settings::DeleteAfterImport;

/// How often progress is sent while originals are deleted.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

/// A folder's own choice about deleting originals.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum OriginalsChoice {
    /// Follow the `delete_after_import` setting.
    #[default]
    Default,
    Never,
    Ask,
}

impl OriginalsChoice {
    pub fn to_option(self) -> Option<bool> {
        match self {
            Self::Default => None,
            Self::Never => Some(false),
            Self::Ask => Some(true),
        }
    }
}

/// An import folder whose originals can be deleted (`delete_originals_offers`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct OriginalsOffer {
    #[ts(type = "number")]
    pub source_id: i64,
    #[ts(type = "string")]
    pub path: PathBuf,
    /// Imported photos whose originals have not been deleted.
    pub photos: usize,
}

/// What deleting would do (`delete_originals_plan`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct OriginalsPlan {
    #[ts(type = "number")]
    pub source_id: i64,
    /// Pass back to `delete_originals`: photos imported after this plan are
    /// not part of it.
    #[ts(type = "number")]
    pub through_job_id: i64,
    pub photos: usize,
    pub files: usize,
    #[ts(type = "number")]
    pub bytes: u64,
    /// Of those photos, the ones skipped as duplicates of library photos.
    pub duplicates: usize,
    /// Photos whose files changed since they were imported: kept.
    pub changed: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum OriginalsState {
    Idle,
    Running,
    Completed,
    Cancelled,
}

/// A photo whose originals were not deleted, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct OriginalsKept {
    #[ts(type = "number")]
    pub photo_id: i64,
    pub reason: String,
}

/// Returned by `delete_originals` and `delete_originals_status`, and sent
/// as `originals_progress`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct OriginalsStatus {
    pub state: OriginalsState,
    #[ts(type = "number | null")]
    pub source_id: Option<i64>,
    /// Photos looked at so far, deleted or kept.
    pub done: usize,
    pub total: usize,
    pub deleted: usize,
    /// Photos whose originals were gone already (deleted by hand).
    pub gone: usize,
    pub files: usize,
    #[ts(type = "number")]
    pub bytes: u64,
    pub kept: Vec<OriginalsKept>,
}

impl OriginalsStatus {
    fn idle() -> Self {
        Self {
            state: OriginalsState::Idle,
            source_id: None,
            done: 0,
            total: 0,
            deleted: 0,
            gone: 0,
            files: 0,
            bytes: 0,
            kept: Vec::new(),
        }
    }
}

/// What happened to one photo's originals.
enum Outcome {
    Deleted {
        files: usize,
        bytes: u64,
    },
    /// Every original was gone already.
    Gone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OriginalsError {
    /// Deleting is not possible now; the message says why.
    Invalid(String),
    /// Another run is going.
    Busy,
    Failed(String),
}

impl fmt::Display for OriginalsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(m) | Self::Failed(m) => f.write_str(m),
            Self::Busy => f.write_str("Originals are being deleted already."),
        }
    }
}

impl From<CatalogError> for OriginalsError {
    fn from(e: CatalogError) -> Self {
        Self::Failed(e.to_string())
    }
}

fn is_media(file: &JobFile) -> bool {
    file.format.role() != AssetRole::Sidecar
}

fn name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn modified_ns(fs: &dyn FileSystem, path: &Path) -> Option<(u64, Option<i64>)> {
    let stamp = fs.stamp(path).ok()?;
    let ns = stamp
        .modified
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| i64::try_from(d.as_nanos()).ok());
    Some((stamp.size, ns))
}

/// Whether a file is as it was when the pipeline found it.
fn unchanged(fs: &dyn FileSystem, file: &JobFile) -> bool {
    modified_ns(fs, &file.path) == Some((file.size, file.modified_ns))
}

/// What the checks before deleting a photo found.
enum Check {
    /// Delete these files (those that still exist), each as stamped when it
    /// was checked.
    Delete(Vec<(JobFile, (u64, Option<i64>))>),
    /// Every file is gone already.
    Gone,
    Keep(String),
}

/// Deletes originals of imported photos, one run at a time in the
/// background.
pub struct OriginalsService {
    store: Arc<dyn PipelineStore>,
    catalog: Arc<dyn PhotoCatalog>,
    fs: Arc<dyn FileSystem>,
    settings: Arc<dyn SettingsStore>,
    events: Arc<dyn EventSink>,
    status: Mutex<OriginalsStatus>,
    cancel: AtomicBool,
}

impl fmt::Debug for OriginalsService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OriginalsService").finish_non_exhaustive()
    }
}

impl OriginalsService {
    pub fn new(
        store: Arc<dyn PipelineStore>,
        catalog: Arc<dyn PhotoCatalog>,
        fs: Arc<dyn FileSystem>,
        settings: Arc<dyn SettingsStore>,
        events: Arc<dyn EventSink>,
    ) -> Self {
        Self {
            store,
            catalog,
            fs,
            settings,
            events,
            status: Mutex::new(OriginalsStatus::idle()),
            cancel: AtomicBool::new(false),
        }
    }

    pub fn status(&self) -> OriginalsStatus {
        self.status.lock().expect("originals lock").clone()
    }

    pub fn cancel(&self) -> OriginalsStatus {
        self.cancel.store(true, Ordering::Release);
        self.status()
    }

    fn source(&self, id: i64) -> Result<ImportSource, OriginalsError> {
        self.store
            .sources()?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| OriginalsError::Invalid(format!("There is no import folder {id}.")))
    }

    /// Whether this folder's originals are offered for deletion.
    fn enabled(&self, source: &ImportSource) -> Result<bool, OriginalsError> {
        let settings = self
            .settings
            .load_settings()
            .map_err(|e| OriginalsError::Failed(e.to_string()))?;
        if settings.open_folder.is_some() {
            // In a folder culled in place the source is the library.
            return Ok(false);
        }
        Ok(source
            .delete_originals
            .unwrap_or(settings.delete_after_import == DeleteAfterImport::Ask))
    }

    /// Folders with imported photos whose originals can be deleted. Reads
    /// the catalog only, so an unplugged card costs nothing.
    pub fn offers(&self) -> Result<Vec<OriginalsOffer>, OriginalsError> {
        let mut offers = Vec::new();
        for source in self.store.sources()? {
            if !self.enabled(&source)? || self.store.unfinished_jobs(source.id)? > 0 {
                continue;
            }
            let photos = self.store.cleanup_candidates(source.id, None)?.len();
            if photos > 0 {
                offers.push(OriginalsOffer {
                    source_id: source.id,
                    path: source.path,
                    photos,
                });
            }
        }
        Ok(offers)
    }

    /// Why deleting from this folder is not possible now, if so.
    fn ready(&self, source: &ImportSource) -> Result<(), OriginalsError> {
        if !self.enabled(source)? {
            return Err(OriginalsError::Invalid(
                "Deleting originals is turned off for this folder.".into(),
            ));
        }
        if self.store.unfinished_jobs(source.id)? > 0 {
            return Err(OriginalsError::Invalid(
                "Photos from this folder are still being imported.".into(),
            ));
        }
        // An original must never be the library's own copy.
        let root = self
            .settings
            .load_settings()
            .map_err(|e| OriginalsError::Failed(e.to_string()))?
            .library_root
            .ok_or_else(|| OriginalsError::Invalid("No library folder is set.".into()))?;
        if root.starts_with(&source.path) || source.path.starts_with(&root) {
            return Err(OriginalsError::Invalid(
                "The library folder and this import folder overlap; nothing is deleted.".into(),
            ));
        }
        self.fs.probe_writable_dir(&source.path).map_err(|why| {
            OriginalsError::Invalid(format!(
                "Cannot delete from {}: {why}. A locked memory card cannot be cleared.",
                source.path.display()
            ))
        })
    }

    /// What deleting this folder's originals would do. Photos whose
    /// originals are all gone already are recorded as done.
    pub fn plan(&self, source_id: i64) -> Result<OriginalsPlan, OriginalsError> {
        let source = self.source(source_id)?;
        self.ready(&source)?;
        let mut plan = OriginalsPlan {
            source_id,
            through_job_id: 0,
            photos: 0,
            files: 0,
            bytes: 0,
            duplicates: 0,
            changed: 0,
        };
        for job in self.store.cleanup_candidates(source_id, None)? {
            plan.through_job_id = plan.through_job_id.max(job.id);
            let present: Vec<&JobFile> = job
                .files
                .iter()
                .filter(|f| self.fs.exists(&f.path))
                .collect();
            if present.is_empty() {
                self.store.mark_source_removed(job.id)?;
                continue;
            }
            if present.iter().any(|f| !unchanged(self.fs.as_ref(), f)) {
                plan.changed += 1;
                continue;
            }
            plan.photos += 1;
            plan.duplicates += usize::from(job.stage == JobStage::Duplicate);
            plan.files += present.len();
            plan.bytes += present.iter().map(|f| f.size).sum::<u64>();
        }
        Ok(plan)
    }

    /// Starts deleting in the background the originals of jobs up to
    /// `through_job_id` (from the plan the user confirmed).
    pub fn start(
        self: &Arc<Self>,
        source_id: i64,
        through_job_id: i64,
    ) -> Result<OriginalsStatus, OriginalsError> {
        let (source, jobs) = self.begin(source_id, through_job_id)?;
        let service = Arc::clone(self);
        std::thread::spawn(move || service.run(&source, &jobs));
        Ok(self.status())
    }

    /// Deletes on the calling thread (tests and tools).
    pub fn run_now(
        self: &Arc<Self>,
        source_id: i64,
        through_job_id: i64,
    ) -> Result<OriginalsStatus, OriginalsError> {
        let (source, jobs) = self.begin(source_id, through_job_id)?;
        self.run(&source, &jobs);
        Ok(self.status())
    }

    fn begin(
        &self,
        source_id: i64,
        through_job_id: i64,
    ) -> Result<(ImportSource, Vec<Job>), OriginalsError> {
        let source = self.source(source_id)?;
        self.ready(&source)?;
        let jobs = self
            .store
            .cleanup_candidates(source_id, Some(through_job_id))?;
        let mut status = self.status.lock().expect("originals lock");
        if status.state == OriginalsState::Running {
            return Err(OriginalsError::Busy);
        }
        *status = OriginalsStatus {
            state: OriginalsState::Running,
            source_id: Some(source_id),
            total: jobs.len(),
            ..OriginalsStatus::idle()
        };
        self.cancel.store(false, Ordering::Release);
        Ok((source, jobs))
    }

    fn run(&self, source: &ImportSource, jobs: &[Job]) {
        let mut last_sent = Instant::now();
        for job in jobs {
            // A folder removed from the app is no longer ours to delete from.
            if self.cancel.load(Ordering::Acquire) || self.source(source.id).is_err() {
                self.status.lock().expect("originals lock").state = OriginalsState::Cancelled;
                break;
            }
            let outcome = self.remove_originals(source, job);
            {
                let mut status = self.status.lock().expect("originals lock");
                status.done += 1;
                if let Some(photo_id) = job.photo_id {
                    match outcome {
                        Ok(Outcome::Deleted { files, bytes }) => {
                            status.deleted += 1;
                            status.files += files;
                            status.bytes += bytes;
                        }
                        Ok(Outcome::Gone) => status.gone += 1,
                        Err(reason) => status.kept.push(OriginalsKept {
                            photo_id: photo_id.0,
                            reason,
                        }),
                    }
                }
            }
            if last_sent.elapsed() >= PROGRESS_INTERVAL {
                last_sent = Instant::now();
                self.events
                    .emit(&AppEvent::OriginalsProgress(self.status()));
            }
        }
        {
            let mut status = self.status.lock().expect("originals lock");
            if status.state == OriginalsState::Running {
                status.state = OriginalsState::Completed;
            }
        }
        self.events
            .emit(&AppEvent::OriginalsProgress(self.status()));
    }

    /// Checks one photo against the library and deletes its originals;
    /// returns the files and bytes deleted, or why they were kept.
    fn remove_originals(&self, source: &ImportSource, job: &Job) -> Result<Outcome, String> {
        let fail = |e: CatalogError| e.to_string();
        // The job must still be what the plan saw (not re-queued since).
        let current = self.store.job(job.id).map_err(fail)?;
        if current.as_ref().is_none_or(|c| {
            c.stage != job.stage || c.photo_id != job.photo_id || c.files != job.files
        }) {
            return Err("it was imported again since the plan".into());
        }
        let photo = match job.photo_id {
            Some(id) => self.catalog.photo(id).map_err(fail)?,
            None => None,
        }
        .ok_or("the photo is no longer in the library")?;
        let files = match self.check(source, job, &photo)? {
            Check::Gone => {
                self.store.mark_source_removed(job.id).map_err(fail)?;
                return Ok(Outcome::Gone);
            }
            Check::Keep(reason) => return Err(reason),
            Check::Delete(files) => files,
        };
        // Media files first: the photo is gone from the folder once they
        // are; sidecars follow.
        let (mut deleted, mut bytes) = (0, 0);
        let ordered = files
            .iter()
            .filter(|(f, _)| is_media(f))
            .chain(files.iter().filter(|(f, _)| !is_media(f)));
        for (file, stamp) in ordered {
            let kept_note = |mut reason: String| {
                if deleted > 0 {
                    reason.push_str(&format!("; {deleted} of its files were deleted already"));
                }
                reason
            };
            // Hashing a RAW takes a while: look again right before deleting.
            if modified_ns(self.fs.as_ref(), &file.path) != Some(*stamp) {
                return Err(kept_note(format!(
                    "{} changed while it was being checked",
                    name(&file.path)
                )));
            }
            if let Err(e) = self.fs.remove_file(&file.path) {
                return Err(kept_note(format!(
                    "cannot delete {}: {e}",
                    name(&file.path)
                )));
            }
            deleted += 1;
            bytes += file.size;
        }
        self.store.mark_source_removed(job.id).map_err(fail)?;
        Ok(Outcome::Deleted {
            files: deleted,
            bytes,
        })
    }

    /// Whether each original that still exists is unchanged since the
    /// import and, for media files, in the library byte for byte with the
    /// library copy intact.
    fn check(&self, source: &ImportSource, job: &Job, photo: &Photo) -> Result<Check, String> {
        let root = self
            .settings
            .load_settings()
            .map_err(|e| e.to_string())?
            .library_root
            .ok_or("no library folder is set")?;
        let present: Vec<&JobFile> = job
            .files
            .iter()
            .filter(|f| self.fs.exists(&f.path))
            .collect();
        if present.is_empty() {
            return Ok(Check::Gone);
        }
        let mut checked = Vec::new();
        for file in present {
            let inside = file.path.starts_with(&source.path)
                && !file
                    .path
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir));
            if !inside {
                return Ok(Check::Keep(format!(
                    "{} is outside the folder",
                    name(&file.path)
                )));
            }
            // What the plan counts as changed is kept here as well.
            let stamp = (file.size, file.modified_ns);
            match modified_ns(self.fs.as_ref(), &file.path) {
                Some(now) if now == stamp => {}
                Some(_) => {
                    return Ok(Check::Keep(format!(
                        "{} changed since it was imported",
                        name(&file.path)
                    )));
                }
                None => return Ok(Check::Keep(format!("cannot read {}", name(&file.path)))),
            }
            if is_media(file) {
                let (hash, size) = self
                    .fs
                    .hash_file(&file.path)
                    .map_err(|e| format!("cannot read {}: {e}", name(&file.path)))?;
                let Some(asset) = photo
                    .assets
                    .iter()
                    .filter(|a| a.role() != AssetRole::Sidecar)
                    .find(|a| a.hash == hash && a.size == size)
                else {
                    return Ok(Check::Keep(format!(
                        "{} is not in the library",
                        name(&file.path)
                    )));
                };
                let copy = root.join(&asset.path);
                if copy == file.path {
                    return Ok(Check::Keep(format!(
                        "{} is the library's own copy",
                        name(&file.path)
                    )));
                }
                match self.fs.hash_file(&copy) {
                    Ok((hash, size)) if hash == asset.hash && size == asset.size => {}
                    Ok(_) => {
                        return Ok(Check::Keep(format!(
                            "the library copy of {} changed",
                            name(&file.path)
                        )));
                    }
                    Err(_) => {
                        return Ok(Check::Keep(format!(
                            "the library copy of {} is missing",
                            name(&file.path)
                        )));
                    }
                }
            }
            // A sidecar has no library hash (the app rewrites its own), so
            // the stamp checked above is all that tells it is as imported.
            checked.push((file.clone(), stamp));
        }
        Ok(Check::Delete(checked))
    }
}

#[cfg(test)]
mod tests;
