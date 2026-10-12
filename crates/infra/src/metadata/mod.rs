//! Reading capture metadata from photo files.

mod container;
mod fields;
mod raw;

use std::path::Path;
use std::time::SystemTime;

use chrono::{DateTime, Local};
use picturesque_app::ports::metadata::MetadataReader;
use picturesque_core::media::MediaFormat;
use picturesque_core::metadata::{CaptureMetadata, CaptureTime};

pub use fields::{ExifFields, parse_datetime, parse_offset};

/// [`MetadataReader`] for RAW files (`rawler`) and image containers
/// (`kamadak-exif`). When a file records no capture time at all, its
/// modification time is used, in the computer's time zone.
#[derive(Debug, Default, Clone, Copy)]
pub struct ExifMetadataReader;

impl MetadataReader for ExifMetadataReader {
    fn read(&self, path: &Path, format: MediaFormat) -> Result<CaptureMetadata, String> {
        let fields = match format {
            MediaFormat::Raw => raw::read(path)?,
            MediaFormat::Xmp => return Err("XMP sidecars carry no capture metadata".into()),
            _ => container::read(path)?,
        };
        let mut metadata = fields.to_metadata();

        if format != MediaFormat::Raw {
            // Header sizes are wrong for HEIF: coded size ignores the
            // clean-aperture crop. Its dimensions come from the decoder.
            let size = if format == MediaFormat::Heif {
                None
            } else {
                imagesize::size(path).ok()
            };
            if metadata.dimensions.is_none() {
                metadata.dimensions = size.map(|s| (s.width as u32, s.height as u32));
            }
            if size.is_none() && format != MediaFormat::Heif && fields == ExifFields::default() {
                return Err("not a readable image".into());
            }
        }

        if metadata.captured_at.is_none() {
            metadata.captured_at = file_time(path);
        }
        Ok(metadata)
    }
}

fn file_time(path: &Path) -> Option<CaptureTime> {
    let modified: SystemTime = std::fs::metadata(path).ok()?.modified().ok()?;
    let local: DateTime<Local> = modified.into();
    Some(CaptureTime::new(local.naive_local(), Some(*local.offset())))
}
