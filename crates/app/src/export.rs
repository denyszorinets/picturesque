//! Exporting photos, by default the picks (`docs/features/export.rst`).
//!
//! Photos are copied (originals) or rendered (resized JPEGs) into a
//! folder, named with a path template. Every file is verified: copies are
//! hashed while read and compared with the catalog and with a re-read of
//! the written file; JPEGs are decoded again. Existing files are never
//! overwritten: names get " (2)", " (3)", … like imports.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use picturesque_core::decision::Flag;
use picturesque_core::photo::{Asset, Photo, PhotoId};
use picturesque_core::template::{PathTemplate, TemplateValues, unique_stem};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::library::PhotoQuery;
use crate::ports::catalog::PhotoCatalog;
use crate::ports::events::{AppEvent, EventSink};
use crate::ports::fs::FileSystem;
use crate::ports::images::ImageDecoder;
use crate::ports::pixels::PixelLoader;
use crate::ports::settings::SettingsStore;

/// File names when no template is given: the original names.
pub const DEFAULT_NAMING: &str = "{name}.{ext}";
const PROGRESS_INTERVAL: Duration = Duration::from_millis(200);

/// What to write for each photo.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ExportFormat {
    /// The original files, unchanged.
    #[default]
    Original,
    /// A JPEG rendered from the photo, optionally smaller.
    Jpeg,
}

/// Arguments of `start_export`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct ExportArgs {
    /// Absolute path of an existing folder to export into.
    #[ts(type = "string")]
    pub destination: PathBuf,
    /// Which photos; default: the picks.
    #[ts(optional)]
    pub query: Option<PhotoQuery>,
    #[serde(default)]
    #[ts(optional)]
    pub format: Option<ExportFormat>,
    /// Path template for the exported files (as for imports); default
    /// `{name}.{ext}`.
    #[ts(optional)]
    pub naming: Option<String>,
    /// Originals: also copy the RAW file of RAW+JPEG pairs (default true).
    #[ts(optional)]
    pub include_raw: Option<bool>,
    /// Originals: also copy XMP sidecars (default true).
    #[ts(optional)]
    pub include_sidecars: Option<bool>,
    /// JPEG: longest edge in pixels (never enlarged); default: full size.
    #[ts(optional)]
    pub max_edge: Option<u32>,
    /// JPEG quality 1–100, default 90.
    #[ts(optional)]
    pub quality: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ExportState {
    Idle,
    Running,
    Completed,
    Cancelled,
}

/// A photo that could not be exported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ExportFailure {
    #[ts(type = "number")]
    pub photo_id: i64,
    pub error: String,
}

/// Returned by `start_export` and `export_status`, and sent as
/// `export_progress`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ExportStatus {
    pub state: ExportState,
    #[ts(type = "string | null")]
    pub destination: Option<PathBuf>,
    /// Photos exported or failed so far.
    pub done: usize,
    pub total: usize,
    /// Files written and verified.
    pub files: usize,
    pub failed: Vec<ExportFailure>,
}

impl ExportStatus {
    fn idle() -> Self {
        Self {
            state: ExportState::Idle,
            destination: None,
            done: 0,
            total: 0,
            files: 0,
            failed: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportError {
    Invalid(String),
    Busy,
    Failed(String),
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(m) | Self::Failed(m) => f.write_str(m),
            Self::Busy => f.write_str("An export is already running."),
        }
    }
}

/// A validated export.
struct Job {
    destination: PathBuf,
    format: ExportFormat,
    template: PathTemplate,
    include_raw: bool,
    include_sidecars: bool,
    max_edge: u32,
    quality: u8,
}

/// Runs one export at a time in the background.
pub struct ExportService {
    catalog: Arc<dyn PhotoCatalog>,
    fs: Arc<dyn FileSystem>,
    decoder: Arc<dyn ImageDecoder>,
    pixels: Arc<dyn PixelLoader>,
    settings: Arc<dyn SettingsStore>,
    events: Arc<dyn EventSink>,
    status: Mutex<ExportStatus>,
    cancel: AtomicBool,
}

impl fmt::Debug for ExportService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExportService").finish_non_exhaustive()
    }
}

impl ExportService {
    pub fn new(
        catalog: Arc<dyn PhotoCatalog>,
        fs: Arc<dyn FileSystem>,
        decoder: Arc<dyn ImageDecoder>,
        pixels: Arc<dyn PixelLoader>,
        settings: Arc<dyn SettingsStore>,
        events: Arc<dyn EventSink>,
    ) -> Self {
        Self {
            catalog,
            fs,
            decoder,
            pixels,
            settings,
            events,
            status: Mutex::new(ExportStatus::idle()),
            cancel: AtomicBool::new(false),
        }
    }

    fn library_root(&self) -> Option<PathBuf> {
        self.settings.load_settings().ok()?.library_root
    }

    pub fn status(&self) -> ExportStatus {
        self.status.lock().expect("export lock").clone()
    }

    pub fn cancel(&self) -> ExportStatus {
        self.cancel.store(true, Ordering::Release);
        self.status()
    }

    fn validate(&self, args: &ExportArgs) -> Result<Job, ExportError> {
        if !args.destination.is_absolute() {
            return Err(ExportError::Invalid(format!(
                "The export folder must be an absolute path, got `{}`.",
                args.destination.display()
            )));
        }
        self.fs
            .probe_writable_dir(&args.destination)
            .map_err(|reason| {
                ExportError::Invalid(format!(
                    "Cannot export to `{}`: {reason}.",
                    args.destination.display()
                ))
            })?;
        if let Some(root) = self.library_root()
            && args.destination.starts_with(&root)
        {
            return Err(ExportError::Invalid(
                "Choose an export folder outside the library.".into(),
            ));
        }
        let template = PathTemplate::parse(args.naming.as_deref().unwrap_or(DEFAULT_NAMING))
            .map_err(|e| ExportError::Invalid(e.to_string()))?;
        if args.max_edge == Some(0) || args.quality.is_some_and(|q| !(1..=100).contains(&q)) {
            return Err(ExportError::Invalid(
                "The size must be above 0 and the quality 1-100.".into(),
            ));
        }
        Ok(Job {
            destination: args.destination.clone(),
            format: args.format.unwrap_or_default(),
            template,
            include_raw: args.include_raw.unwrap_or(true),
            include_sidecars: args.include_sidecars.unwrap_or(true),
            max_edge: args.max_edge.unwrap_or(u32::MAX >> 1),
            quality: args.quality.unwrap_or(90),
        })
    }

    /// The photos of `query` (default: picks), oldest first unless the
    /// query sorts otherwise.
    fn photos(&self, query: Option<&PhotoQuery>) -> Result<Vec<PhotoId>, ExportError> {
        let mut query = query.cloned().unwrap_or_else(|| PhotoQuery {
            flags: Some(vec![Flag::Pick]),
            ..Default::default()
        });
        // Sequence numbers follow capture order.
        query
            .sort
            .get_or_insert(crate::library::PhotoSort::CapturedAsc);
        query.limit = Some(crate::library::MAX_PAGE);
        query.cursor = None;
        query.validate().map_err(ExportError::Invalid)?;
        let mut ids = Vec::new();
        loop {
            let page = self
                .catalog
                .query(&query)
                .map_err(|e| ExportError::Failed(e.to_string()))?;
            ids.extend(page.photos.iter().map(|p| PhotoId(p.id)));
            match page.next_cursor {
                Some(next) => query.cursor = Some(next),
                None => return Ok(ids),
            }
        }
    }

    /// Validates and starts an export in the background.
    pub fn start(self: &Arc<Self>, args: ExportArgs) -> Result<ExportStatus, ExportError> {
        let job = self.validate(&args)?;
        let ids = self.photos(args.query.as_ref())?;
        {
            let mut status = self.status.lock().expect("export lock");
            if status.state == ExportState::Running {
                return Err(ExportError::Busy);
            }
            *status = ExportStatus {
                state: ExportState::Running,
                destination: Some(job.destination.clone()),
                done: 0,
                total: ids.len(),
                files: 0,
                failed: Vec::new(),
            };
        }
        self.cancel.store(false, Ordering::Release);
        let service = Arc::clone(self);
        std::thread::spawn(move || service.run(&job, &ids));
        Ok(self.status())
    }

    /// Runs an export on the calling thread (tests and tools).
    pub fn run_now(self: &Arc<Self>, args: ExportArgs) -> Result<ExportStatus, ExportError> {
        let job = self.validate(&args)?;
        let ids = self.photos(args.query.as_ref())?;
        *self.status.lock().expect("export lock") = ExportStatus {
            state: ExportState::Running,
            destination: Some(job.destination.clone()),
            total: ids.len(),
            ..ExportStatus::idle()
        };
        self.cancel.store(false, Ordering::Release);
        self.run(&job, &ids);
        Ok(self.status())
    }

    fn run(&self, job: &Job, ids: &[PhotoId]) {
        let mut last_sent = Instant::now();
        for (i, id) in ids.iter().enumerate() {
            if self.cancel.load(Ordering::Acquire) {
                self.status.lock().expect("export lock").state = ExportState::Cancelled;
                break;
            }
            let outcome = match self.catalog.photo(*id) {
                Ok(Some(photo)) => self.export_photo(job, &photo, i as u32 + 1),
                Ok(None) => Err("the photo was removed".into()),
                Err(e) => Err(e.to_string()),
            };
            {
                let mut status = self.status.lock().expect("export lock");
                status.done += 1;
                match outcome {
                    Ok(files) => status.files += files,
                    Err(error) => status.failed.push(ExportFailure {
                        photo_id: id.0,
                        error,
                    }),
                }
            }
            if last_sent.elapsed() >= PROGRESS_INTERVAL {
                last_sent = Instant::now();
                self.events.emit(&AppEvent::ExportProgress(self.status()));
            }
        }
        {
            let mut status = self.status.lock().expect("export lock");
            if status.state == ExportState::Running {
                status.state = ExportState::Completed;
            }
        }
        self.events.emit(&AppEvent::ExportProgress(self.status()));
    }

    /// Exports one photo; returns the number of files written.
    fn export_photo(&self, job: &Job, photo: &Photo, seq: u32) -> Result<usize, String> {
        let root = self.library_root().ok_or("no library folder is set")?;
        let primary = photo
            .assets
            .image()
            .or(photo.assets.raw())
            .ok_or("the photo has no image file")?;
        let stem_of = |a: &Asset| {
            a.path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        let original_stem = stem_of(primary);
        let m = &photo.metadata;
        let rendered = job.template.render(&TemplateValues {
            captured_at: m.captured_at.map(|t| t.local),
            place: m.place.as_ref(),
            camera: m.camera.as_ref(),
            lens: m.lens.as_deref(),
            original_name: &original_stem,
            seq: Some(seq),
        });
        let dir = job.destination.join(&rendered.dir);
        self.fs.create_dir_all(&dir)?;

        // Files to write: (source asset or None for the rendered JPEG, name
        // after the stem, e.g. ".CR3" or ".CR3.xmp").
        let mut outputs: Vec<(Option<&Asset>, String)> = Vec::new();
        let suffix = |a: &Asset| {
            let name = a
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let stem = stem_of(primary);
            match name.strip_prefix(&stem) {
                Some(rest) if !rest.is_empty() => rest.to_owned(),
                _ => format!(
                    ".{}",
                    a.path
                        .extension()
                        .map(|e| e.to_string_lossy().into_owned())
                        .unwrap_or_default()
                ),
            }
        };
        match job.format {
            ExportFormat::Original => {
                outputs.push((Some(primary), suffix(primary)));
                if job.include_raw
                    && let Some(raw) = photo.assets.raw()
                    && raw.path != primary.path
                {
                    outputs.push((Some(raw), suffix(raw)));
                }
                if job.include_sidecars {
                    for sidecar in photo.assets.sidecars() {
                        outputs.push((Some(sidecar), suffix(sidecar)));
                    }
                }
            }
            ExportFormat::Jpeg => outputs.push((None, ".jpg".into())),
        }
        let stem = unique_stem(&rendered.stem, |candidate| {
            outputs
                .iter()
                .any(|(_, rest)| self.fs.exists(&dir.join(format!("{candidate}{rest}"))))
        });

        let mut written: Vec<PathBuf> = Vec::new();
        let result = (|| {
            for (asset, rest) in &outputs {
                let dest = dir.join(format!("{stem}{rest}"));
                match asset {
                    Some(asset) => self.copy_verified(&root.join(&asset.path), &dest, asset)?,
                    None => self.render_verified(&root, photo, job, &dest)?,
                }
                written.push(dest);
            }
            Ok::<_, String>(())
        })();
        if let Err(e) = result {
            for file in &written {
                let _ = self.fs.remove_file(file);
            }
            return Err(e);
        }
        Ok(written.len())
    }

    fn copy_verified(&self, src: &Path, dest: &Path, asset: &Asset) -> Result<(), String> {
        let (hash, size) = self.fs.copy_hashing(src, dest).inspect_err(|_| {
            let _ = self.fs.remove_file(dest);
        })?;
        let check = || -> Result<(), String> {
            // Sidecars are rewritten as decisions change; only media files
            // must match the catalog.
            if asset.format != picturesque_core::media::MediaFormat::Xmp
                && (hash != asset.hash || size != asset.size)
            {
                return Err(format!(
                    "{} changed since it was imported",
                    asset.path.display()
                ));
            }
            let (written, written_size) = self.fs.hash_file(dest)?;
            if written != hash || written_size != size {
                return Err(format!("{} was not written correctly", dest.display()));
            }
            Ok(())
        };
        check().inspect_err(|_| {
            let _ = self.fs.remove_file(dest);
        })
    }

    fn render_verified(
        &self,
        root: &Path,
        photo: &Photo,
        job: &Job,
        dest: &Path,
    ) -> Result<(), String> {
        let source = photo
            .assets
            .image()
            .or(photo.assets.raw())
            .ok_or("the photo has no image file")?;
        let result = self
            .decoder
            .export_jpeg(
                &root.join(&source.path),
                source.format,
                photo.metadata.orientation,
                job.max_edge,
                job.quality,
                dest,
            )
            .and_then(|_| self.pixels.load(dest, 64).map(|_| ()))
            .map_err(|e| format!("cannot export {}: {e}", source.path.display()));
        if result.is_err() {
            let _ = self.fs.remove_file(dest);
        }
        result
    }
}

#[cfg(test)]
mod tests;
