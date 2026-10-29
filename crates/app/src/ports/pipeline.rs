//! Storage for the continuous import pipeline (ADR-0012): import sources,
//! one job per source photo, and the pipeline log.

use std::path::PathBuf;

use picturesque_core::media::MediaFormat;
use picturesque_core::photo::PhotoId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::catalog::CatalogError;

/// A folder photos are imported from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ImportSource {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "string")]
    pub path: PathBuf,
    /// Imported automatically when photos appear or change.
    pub watch: bool,
    /// When it was last scanned (UTC, ISO 8601).
    pub scanned_at: Option<String>,
    /// Why the last scan failed.
    pub scan_error: Option<String>,
    /// Whether to offer deleting the originals once imported; `None`
    /// follows the `delete_after_import` setting (ADR-0017).
    pub delete_originals: Option<bool>,
}

/// Where a photo is in the pipeline. `Queued`, `Reading` and `Copying`
/// are in progress; the others are final.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum JobStage {
    /// Found in a source, waiting.
    Queued,
    /// Reading metadata and checking for duplicates.
    Reading,
    /// Copying and verifying the files.
    Copying,
    /// Copied, verified and catalogued.
    Imported,
    /// Already in the library; not copied.
    Duplicate,
    Failed,
}

impl JobStage {
    pub const ALL: [Self; 6] = [
        Self::Queued,
        Self::Reading,
        Self::Copying,
        Self::Imported,
        Self::Duplicate,
        Self::Failed,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Reading => "reading",
            Self::Copying => "copying",
            Self::Imported => "imported",
            Self::Duplicate => "duplicate",
            Self::Failed => "failed",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|stage| stage.as_str() == s)
    }

    pub fn is_final(self) -> bool {
        matches!(self, Self::Imported | Self::Duplicate | Self::Failed)
    }
}

/// One file of a source photo, as found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobFile {
    pub path: PathBuf,
    pub format: MediaFormat,
    pub size: u64,
    /// Modification time, ns since the Unix epoch.
    pub modified_ns: Option<i64>,
}

/// A source photo moving through the pipeline.
#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub id: i64,
    pub source_id: Option<i64>,
    pub files: Vec<JobFile>,
    pub stage: JobStage,
    pub photo_id: Option<PhotoId>,
    pub error: Option<String>,
    pub updated_at: String,
    /// For a failed job, the stage it failed in.
    pub failed_stage: Option<JobStage>,
}

/// Severity of a log entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        [Self::Info, Self::Warn, Self::Error]
            .into_iter()
            .find(|l| l.as_str() == s)
    }
}

/// One line of the pipeline log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct LogEntry {
    #[ts(type = "number")]
    pub id: i64,
    /// UTC, ISO 8601.
    pub at: String,
    pub level: LogLevel,
    #[ts(type = "number | null")]
    pub job_id: Option<i64>,
    #[ts(type = "number | null")]
    pub photo_id: Option<i64>,
    pub message: String,
}

pub trait PipelineStore: Send + Sync {
    /// Adds a source, or returns the existing one with that path.
    fn add_source(&self, path: &std::path::Path, watch: bool)
    -> Result<ImportSource, CatalogError>;
    fn sources(&self) -> Result<Vec<ImportSource>, CatalogError>;
    fn set_source_watch(&self, id: i64, watch: bool) -> Result<(), CatalogError>;
    /// Sets the folder's own choice about deleting originals; `None`
    /// follows the setting.
    fn set_source_delete_originals(
        &self,
        id: i64,
        choice: Option<bool>,
    ) -> Result<(), CatalogError>;
    /// Removes a source and its waiting jobs; finished jobs stay as
    /// history.
    fn remove_source(&self, id: i64) -> Result<(), CatalogError>;
    fn record_scan(&self, id: i64, error: Option<&str>) -> Result<(), CatalogError>;

    /// Records a photo found in a source, keyed by its primary file. A new
    /// photo, or a finished one whose files changed, is queued; returns
    /// the job id if it was.
    fn offer_job(
        &self,
        source_id: i64,
        key: &std::path::Path,
        files: &[JobFile],
    ) -> Result<Option<i64>, CatalogError>;
    /// The oldest queued job.
    fn next_job(&self) -> Result<Option<Job>, CatalogError>;
    fn set_job_stage(
        &self,
        id: i64,
        stage: JobStage,
        photo_id: Option<PhotoId>,
        error: Option<&str>,
    ) -> Result<(), CatalogError>;
    /// Puts jobs left in progress (the app stopped) back in the queue;
    /// returns how many.
    fn requeue_unfinished(&self) -> Result<usize, CatalogError>;
    /// Queues failed jobs again (all, or those listed).
    fn retry_failed(&self, ids: Option<&[i64]>) -> Result<usize, CatalogError>;
    fn job(&self, id: i64) -> Result<Option<Job>, CatalogError>;
    /// Jobs newest first, optionally only some stages, before `before_id`.
    fn jobs(
        &self,
        stages: Option<&[JobStage]>,
        limit: usize,
        before_id: Option<i64>,
    ) -> Result<Vec<Job>, CatalogError>;
    /// Jobs of a source whose originals may still be on disk: imported or
    /// skipped as a duplicate, with a library photo, and not yet deleted
    /// from the source. Oldest first; only jobs up to `through_job_id`
    /// when given (ADR-0017).
    fn cleanup_candidates(
        &self,
        source_id: i64,
        through_job_id: Option<i64>,
    ) -> Result<Vec<Job>, CatalogError>;
    /// Jobs of a source that are queued or being worked on.
    fn unfinished_jobs(&self, source_id: i64) -> Result<usize, CatalogError>;
    /// Records that a job's source files were deleted.
    fn mark_source_removed(&self, job_id: i64) -> Result<(), CatalogError>;
    /// Number of jobs per stage.
    fn job_counts(&self) -> Result<Vec<(JobStage, usize)>, CatalogError>;

    fn append_log(
        &self,
        level: LogLevel,
        job_id: Option<i64>,
        photo_id: Option<PhotoId>,
        message: &str,
    ) -> Result<(), CatalogError>;
    /// Log entries newest first, at `min_level` or above, before `before_id`.
    fn log(
        &self,
        min_level: LogLevel,
        limit: usize,
        before_id: Option<i64>,
    ) -> Result<Vec<LogEntry>, CatalogError>;
    /// Keeps only the newest `keep` log entries.
    fn prune_log(&self, keep: usize) -> Result<(), CatalogError>;
}
