//! Import runs as stored between planning, copying and resuming.

use std::path::PathBuf;

use picturesque_core::hash::ContentHash;
use picturesque_core::media::MediaFormat;
use picturesque_core::metadata::CaptureMetadata;
use picturesque_core::photo::PhotoId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Lifecycle of an import run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ImportState {
    /// Scanning sources and reading metadata.
    Planning,
    /// Plan ready; nothing copied yet (or resumable).
    Planned,
    Copying,
    Completed,
    Cancelled,
    Failed,
    /// The app stopped while planning or copying; can be started again.
    Interrupted,
}

impl ImportState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planning => "planning",
            Self::Planned => "planned",
            Self::Copying => "copying",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        [
            Self::Planning,
            Self::Planned,
            Self::Copying,
            Self::Completed,
            Self::Cancelled,
            Self::Failed,
            Self::Interrupted,
        ]
        .into_iter()
        .find(|state| state.as_str() == s)
    }

    /// Whether copying can be started (or resumed) from this state.
    pub fn can_start(self) -> bool {
        matches!(
            self,
            Self::Planned | Self::Cancelled | Self::Interrupted | Self::Failed
        )
    }
}

/// Outcome of one photo in a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PhotoStatus {
    Planned,
    /// Already in the library or earlier in this import; not copied.
    Duplicate,
    Imported,
    Failed,
}

impl PhotoStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Duplicate => "duplicate",
            Self::Imported => "imported",
            Self::Failed => "failed",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        [Self::Planned, Self::Duplicate, Self::Imported, Self::Failed]
            .into_iter()
            .find(|status| status.as_str() == s)
    }
}

/// Outcome of one file in a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum FileStatus {
    Planned,
    /// Copied and its copy hash-verified.
    Verified,
    Failed,
}

impl FileStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Verified => "verified",
            Self::Failed => "failed",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        [Self::Planned, Self::Verified, Self::Failed]
            .into_iter()
            .find(|status| status.as_str() == s)
    }
}

/// One file to copy.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedFile {
    /// Store row id, once saved.
    pub id: Option<i64>,
    /// Absolute source path.
    pub source: PathBuf,
    pub format: MediaFormat,
    pub size: u64,
    /// Source modification time (ns since the Unix epoch) at planning.
    pub modified_ns: Option<i64>,
    /// Known when it was needed for duplicate detection, or once copied.
    pub hash: Option<ContentHash>,
    /// Destination relative to the library root.
    pub target: PathBuf,
    pub status: FileStatus,
    pub error: Option<String>,
}

/// One photo (its RAW, image and sidecars) to import.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedPhoto {
    pub id: Option<i64>,
    pub metadata: CaptureMetadata,
    pub status: PhotoStatus,
    /// Why it is a duplicate or failed.
    pub reason: Option<String>,
    /// Catalog id once imported.
    pub photo_id: Option<PhotoId>,
    pub files: Vec<PlannedFile>,
    /// A decision found in the photo's existing sidecar, applied on import.
    pub decision: Option<picturesque_core::decision::Decision>,
}

/// A path that could not be imported, with the reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ImportProblem {
    #[ts(type = "string")]
    pub path: PathBuf,
    pub reason: String,
}

/// A stored import run.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportRun {
    pub id: i64,
    pub state: ImportState,
    pub library_root: PathBuf,
    pub template: String,
    pub sources: Vec<PathBuf>,
    pub photos: Vec<PlannedPhoto>,
    pub problems: Vec<ImportProblem>,
    pub error: Option<String>,
}
