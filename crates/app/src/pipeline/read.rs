//! Reading a source photo before it is copied: metadata (with the JPEG
//! filling in for an unreadable RAW), place, file stamps and any decision
//! in an existing sidecar.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use picturesque_core::decision::Decision;
use picturesque_core::metadata::CaptureMetadata;

use super::copy::PlannedFile;
use crate::scan::SourcePhoto;

/// A source photo, read.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ReadPhoto {
    pub metadata: CaptureMetadata,
    /// Why it cannot be imported.
    pub reason: Option<String>,
    pub files: Vec<PlannedFile>,
    /// A decision found in the photo's existing sidecar, applied on import.
    pub decision: Option<Decision>,
}

/// What reading a source photo needs.
pub(super) struct Readers<'a> {
    pub fs: &'a dyn crate::ports::fs::FileSystem,
    pub metadata: &'a dyn crate::ports::metadata::MetadataReader,
    pub geocoder: &'a dyn crate::ports::geocoder::ReverseGeocoder,
    pub sidecars: &'a dyn crate::ports::sidecars::SidecarStore,
}

/// Reads a source photo's metadata, file stamps and existing sidecar
/// decision. A photo whose files cannot be read gets a `reason`.
pub(super) fn read_one(deps: &Readers<'_>, source: &SourcePhoto) -> ReadPhoto {
    let primary = source.primary();
    let mut reason = None;
    let mut metadata = match deps.metadata.read(&primary.path, primary.format) {
        Ok(m) => m,
        Err(e) => {
            reason = Some(format!("Cannot read {}: {e}", file_name(&primary.path)));
            CaptureMetadata::default()
        }
    };
    if let (Some(raw), Some(image)) = (&source.raw, &source.image)
        && raw.path == primary.path
        && let Ok(extra) = deps.metadata.read(&image.path, image.format)
    {
        if reason.is_some() {
            // The RAW is unreadable but its JPEG is fine: use the JPEG.
            metadata = extra;
            reason = None;
        } else {
            metadata.fill_missing_from(&extra);
        }
    }
    if metadata.place.is_none()
        && let Some(gps) = metadata.gps
    {
        metadata.place = deps.geocoder.place(gps);
    }

    let mut files = Vec::new();
    for file in source.files() {
        let stamp = deps.fs.stamp(&file.path);
        if let Err(e) = &stamp {
            reason.get_or_insert_with(|| format!("Cannot read {}: {e}", file_name(&file.path)));
        }
        let stamp = stamp.ok();
        files.push(PlannedFile {
            source: file.path.clone(),
            format: file.format,
            size: stamp.map_or(file.size, |s| s.size),
            modified_ns: stamp
                .and_then(|s| s.modified)
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .and_then(|d| i64::try_from(d.as_nanos()).ok()),
            hash: None,
            target: PathBuf::new(),
        });
    }

    // Decisions recorded by other tools (or earlier Picturesque use).
    let decision = source
        .sidecars
        .iter()
        .find_map(|s| deps.sidecars.read(&s.path).ok().flatten());

    ReadPhoto {
        decision,
        metadata,
        reason,
        files,
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}
