//! The photo catalog (ADR-0004).

use std::collections::HashSet;
use std::fmt;
use std::path::PathBuf;

use picturesque_core::decision::Decision;
use picturesque_core::hash::ContentHash;
use picturesque_core::photo::{NewPhoto, Photo, PhotoId};
use picturesque_core::ranking::Ranked;
use picturesque_core::series::SeriesEdit;

use crate::library::{PhotoFacets, PhotoPage, PhotoQuery};

/// Failure of a catalog operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogError {
    /// A file with the same content (or library path) is already catalogued.
    Duplicate(String),
    /// The disk holding the catalog is full; retry once space is freed.
    Full,
    /// The underlying store failed.
    Storage(String),
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate(what) => write!(f, "already in the catalog: {what}"),
            Self::Full => f.write_str("the disk holding the catalog is full"),
            Self::Storage(msg) => write!(f, "catalog storage error: {msg}"),
        }
    }
}

impl std::error::Error for CatalogError {}

/// Which analyzer (and version) produced a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnalyzerId {
    pub name: &'static str,
    pub version: u32,
}

/// What the library keeps about a photo's analysis for sorting, filtering
/// and badges (derived by the series service).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhotoScore {
    pub id: PhotoId,
    /// Combined quality, 0–1.
    pub score: f32,
    /// Aesthetic score, 0–1, once the CLIP model ran.
    pub aesthetic: Option<f32>,
    /// Whether someone important has closed eyes; `None` when undecided.
    pub eyes_closed: Option<bool>,
    pub faces: u32,
    /// Suggested as a keeper (auto-pick; `core::ranking::is_keeper`).
    pub keeper: bool,
}

/// A series as stored: members in capture order, their ranking and the
/// best photo.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredSeries {
    pub members: Vec<PhotoId>,
    /// Best first.
    pub ranking: Vec<Ranked>,
    pub best: PhotoId,
    /// Whether the user chose `best`.
    pub best_chosen: bool,
}

/// A decision whose sidecar is out of date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirtyDecision {
    pub id: PhotoId,
    pub decision: Decision,
    pub token: String,
    /// Whether the photo gets the auto-pick label while unflagged.
    pub auto_labeled: bool,
}

/// A verified copy of one of a photo's files, made out of a folder
/// session (ADR-0014).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyRecord {
    pub photo_id: PhotoId,
    /// The original, relative to the folder.
    pub source: PathBuf,
    /// Where the copy is (absolute).
    pub destination: PathBuf,
    /// The copy's content when it was verified.
    pub hash: ContentHash,
    pub size: u64,
}

/// Stores and retrieves photos.
pub trait PhotoCatalog: Send + Sync {
    /// Stores a new photo with its assets in one transaction.
    fn insert_photo(&self, photo: &NewPhoto) -> Result<PhotoId, CatalogError>;

    fn photo(&self, id: PhotoId) -> Result<Option<Photo>, CatalogError>;

    /// The photo owning a RAW or image file with this content, if any.
    /// Sidecars are not considered.
    fn photo_with_media_hash(&self, hash: &ContentHash) -> Result<Option<PhotoId>, CatalogError>;

    fn photo_count(&self) -> Result<u64, CatalogError>;

    /// Every photo, oldest capture first.
    fn all_photos(&self) -> Result<Vec<Photo>, CatalogError>;

    /// Records a photo's stored pixel dimensions (before orientation), for
    /// formats whose size is only known after decoding.
    fn set_dimensions(&self, id: PhotoId, width: u32, height: u32) -> Result<(), CatalogError>;

    /// A page of photos matching `query` (already validated).
    fn query(&self, query: &PhotoQuery) -> Result<PhotoPage, CatalogError>;

    /// Values present in the library, for filters. With a filter, `total`
    /// and `days` count only matching photos (they drive the timeline
    /// layout); the option lists always cover the whole library.
    fn facets(&self, filter: Option<&PhotoQuery>) -> Result<PhotoFacets, CatalogError>;

    /// Photos per camera, lens and camera focal length, by decision.
    fn gear_usage(&self) -> Result<crate::stats::GearUsage, CatalogError>;

    /// A photo's decision; the default when none was made.
    fn decision(&self, id: PhotoId) -> Result<Decision, CatalogError>;

    fn set_decision(&self, id: PhotoId, decision: Decision) -> Result<(), CatalogError>;

    /// Records a decision read from the photo's sidecar: the sidecar
    /// already holds it, so it is not written back.
    fn set_decision_saved(&self, id: PhotoId, decision: Decision) -> Result<(), CatalogError>;

    /// Decisions changed since their sidecar was last written, with a token
    /// identifying the change.
    fn dirty_decisions(&self) -> Result<Vec<DirtyDecision>, CatalogError>;

    /// Removes a photo and everything known about it (its files are
    /// already gone).
    fn remove_photo(&self, id: PhotoId) -> Result<(), CatalogError>;

    /// Records verified copies, replacing earlier ones of the same files.
    fn record_copies(&self, copies: &[CopyRecord]) -> Result<(), CatalogError>;

    /// Every recorded copy.
    fn copies(&self) -> Result<Vec<CopyRecord>, CatalogError>;

    /// Photos marked to carry the auto-pick label.
    fn auto_labeled(&self) -> Result<HashSet<PhotoId>, CatalogError>;

    /// Marks exactly `ids` (photos without a decision get the default one)
    /// to carry the auto-pick label; photos whose mark changes get their
    /// sidecars rewritten. Returns how many changed.
    fn set_auto_labeled(&self, ids: &[PhotoId]) -> Result<usize, CatalogError>;

    /// Records that the sidecar reflects the change identified by `token`.
    /// Does nothing if the decision changed again meanwhile.
    fn mark_sidecar_written(&self, id: PhotoId, token: &str) -> Result<(), CatalogError>;

    /// Adds a sidecar file to a photo, or updates its size and hash.
    fn upsert_sidecar(
        &self,
        id: PhotoId,
        path: &std::path::Path,
        size: u64,
        hash: &ContentHash,
    ) -> Result<(), CatalogError>;

    /// Photos lacking a result from at least one of `analyzers` (at its
    /// current version), oldest capture first, at most `limit`.
    fn pending_analysis(
        &self,
        analyzers: &[AnalyzerId],
        limit: usize,
    ) -> Result<Vec<PhotoId>, CatalogError>;

    /// How many photos still need any of `analyzers`, and how many photos exist.
    fn analysis_progress(&self, analyzers: &[AnalyzerId]) -> Result<(usize, usize), CatalogError>;

    /// Which of `analyzers` already have a current result for `id`.
    fn analyzed(
        &self,
        id: PhotoId,
        analyzers: &[AnalyzerId],
    ) -> Result<Vec<AnalyzerId>, CatalogError>;

    /// Stores (or replaces) an analyzer's result for a photo.
    fn store_analysis(
        &self,
        id: PhotoId,
        analyzer: AnalyzerId,
        result: &serde_json::Value,
    ) -> Result<(), CatalogError>;

    /// Marks the results of these photos (every photo with `None`) for
    /// analysis again: they stay in use, but the photos count as pending
    /// until each analyzer stored a new result. Returns how many of the
    /// photos exist.
    fn mark_analysis_stale(&self, ids: Option<&[PhotoId]>) -> Result<usize, CatalogError>;

    /// A photo's current results, by analyzer name.
    fn analysis(&self, id: PhotoId) -> Result<Vec<(String, u32, serde_json::Value)>, CatalogError>;

    /// Every photo's id and local capture time.
    fn capture_times(&self) -> Result<Vec<(PhotoId, Option<chrono::NaiveDateTime>)>, CatalogError>;

    /// All current results of one analyzer (at its version).
    fn analysis_results(
        &self,
        analyzer: AnalyzerId,
    ) -> Result<Vec<(PhotoId, serde_json::Value)>, CatalogError>;

    /// The user's series corrections.
    fn series_edits(&self) -> Result<Vec<(PhotoId, SeriesEdit)>, CatalogError>;

    /// Sets (or with `None` removes) the correction before a photo.
    fn set_series_edit(&self, id: PhotoId, edit: Option<SeriesEdit>) -> Result<(), CatalogError>;

    /// Replaces all series. A series' id is its first member's id.
    fn replace_series(&self, series: &[StoredSeries]) -> Result<(), CatalogError>;

    /// The series a photo belongs to, with its id.
    fn series_of(&self, id: PhotoId) -> Result<Option<(PhotoId, StoredSeries)>, CatalogError>;

    /// Photos the user chose as best of their series, oldest choice first.
    fn best_choices(&self) -> Result<Vec<PhotoId>, CatalogError>;

    /// Records (as the latest choice) or removes a best-photo choice.
    fn set_best_choice(&self, id: PhotoId, chosen: bool) -> Result<(), CatalogError>;

    /// Photos flagged as rejected.
    fn rejected_photos(&self) -> Result<Vec<PhotoId>, CatalogError>;

    /// Changes the paths of a photo's assets (`old` → `new`, relative to
    /// the library), all or none.
    fn move_assets(&self, id: PhotoId, moves: &[(PathBuf, PathBuf)]) -> Result<(), CatalogError>;

    /// Records that `ids` were moved to the rejected folder in `batch`.
    fn record_reject_move(&self, batch: i64, ids: &[PhotoId]) -> Result<(), CatalogError>;

    /// The latest batch of moved rejects and its photos.
    fn last_reject_move(&self) -> Result<Option<(i64, Vec<PhotoId>)>, CatalogError>;

    /// Forgets that `ids` were moved (after moving them back).
    fn forget_reject_move(&self, ids: &[PhotoId]) -> Result<(), CatalogError>;

    /// Sets the combined scores and AI signals of photos; other photos'
    /// are removed.
    fn replace_scores(&self, scores: &[PhotoScore]) -> Result<(), CatalogError>;

    /// Sizes of all catalogued RAW and image files. A new file can only be
    /// a duplicate if its size is in this set, so only those are hashed.
    fn media_sizes(&self) -> Result<HashSet<u64>, CatalogError>;
}
