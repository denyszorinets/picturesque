//! Reading capture metadata from photo files.

use std::path::Path;

use picturesque_core::media::MediaFormat;
use picturesque_core::metadata::CaptureMetadata;

/// Extracts [`CaptureMetadata`] from a file. Missing fields are `None`;
/// an error means the file could not be read at all. The place is never
/// filled in here (see reverse geocoding).
pub trait MetadataReader: Send + Sync {
    fn read(&self, path: &Path, format: MediaFormat) -> Result<CaptureMetadata, String>;
}
