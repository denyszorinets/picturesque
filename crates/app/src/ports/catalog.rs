//! The photo catalog (ADR-0004).

use std::collections::HashSet;
use std::fmt;

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
    /// The underlying store failed.
    Storage(String),
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate(what) => write!(f, "already in the catalog: {what}"),
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

    /// A photo's decision; the default when none was made.
    fn decision(&self, id: PhotoId) -> Result<Decision, CatalogError>;

    fn set_decision(&self, id: PhotoId, decision: Decision) -> Result<(), CatalogError>;

    /// Decisions changed since their sidecar was last written, with a token
    /// identifying the change.
    fn dirty_decisions(&self) -> Result<Vec<DirtyDecision>, CatalogError>;

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

    /// Sets the combined quality scores (0–1) of photos; other photos'
    /// scores are removed.
    fn replace_scores(&self, scores: &[(PhotoId, f32)]) -> Result<(), CatalogError>;

    /// Sizes of all catalogued RAW and image files. A new file can only be
    /// a duplicate if its size is in this set, so only those are hashed.
    fn media_sizes(&self) -> Result<HashSet<u64>, CatalogError>;
}
