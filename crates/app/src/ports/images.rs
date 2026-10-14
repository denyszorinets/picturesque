//! Decoding photos into small upright JPEGs (ADR-0005).

use std::path::Path;

use picturesque_core::media::MediaFormat;
use picturesque_core::metadata::Orientation;

/// Result of rendering an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rendered {
    /// Size of the written image, upright.
    pub width: u32,
    pub height: u32,
    /// Full size of the source as displayed (upright), when known. For RAW
    /// files this is the size of the embedded preview.
    pub source_size: Option<(u32, u32)>,
}

pub trait ImageDecoder: Send + Sync {
    /// Writes an upright JPEG of `source` to `dest`, its longest edge at
    /// most `max_edge` pixels (never enlarged). `orientation` is the
    /// photo's EXIF orientation; decoders apply it where the decoded
    /// pixels are not already upright (RAW previews).
    fn render(
        &self,
        source: &Path,
        format: MediaFormat,
        orientation: Orientation,
        max_edge: u32,
        dest: &Path,
    ) -> Result<Rendered, String>;

    /// Like `render`, for exporting: JPEG `quality` (1–100) and the
    /// source's metadata (EXIF, XMP, colour profile) kept where possible.
    fn export_jpeg(
        &self,
        source: &Path,
        format: MediaFormat,
        orientation: Orientation,
        max_edge: u32,
        quality: u8,
        dest: &Path,
    ) -> Result<Rendered, String> {
        let _ = quality;
        self.render(source, format, orientation, max_edge, dest)
    }
}
