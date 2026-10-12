//! The photo catalog (ADR-0004).

use std::collections::HashSet;
use std::fmt;

use picturesque_core::decision::Decision;
use picturesque_core::hash::ContentHash;
use picturesque_core::photo::{NewPhoto, Photo, PhotoId};

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

    /// Sizes of all catalogued RAW and image files. A new file can only be
    /// a duplicate if its size is in this set, so only those are hashed.
    fn media_sizes(&self) -> Result<HashSet<u64>, CatalogError>;
}
