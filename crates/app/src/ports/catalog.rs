//! The photo catalog (ADR-0004).

use std::fmt;

use picturesque_core::hash::ContentHash;
use picturesque_core::photo::{NewPhoto, Photo, PhotoId};

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
}
