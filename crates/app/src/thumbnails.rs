//! Cached thumbnails and previews of photos.
//!
//! Images are keyed by the content hash of the file they are made from,
//! which never changes, so cached images never go stale. Bumping
//! [`CACHE_VERSION`] discards every cached image (for example after
//! changing sizes or quality).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use picturesque_core::hash::ContentHash;
use picturesque_core::photo::{Photo, PhotoId};
use serde::Deserialize;
use ts_rs::TS;

use crate::ports::catalog::{CatalogError, PhotoCatalog};
use crate::ports::fs::FileSystem;
use crate::ports::images::ImageDecoder;
use crate::ports::settings::SettingsStore;

/// Version of the cache layout and rendering settings.
pub const CACHE_VERSION: u32 = 1;

/// The image sizes Picturesque caches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ImageSize {
    /// Grid tiles (longest edge 512 px: sharp at 256 px on high-DPI screens).
    Grid,
    /// Loupe and analysis (longest edge 2048 px).
    Preview,
    /// Full resolution, for 100% zoom; generated only when asked for.
    Full,
}

impl ImageSize {
    pub fn max_edge(self) -> u32 {
        match self {
            Self::Grid => 512,
            Self::Preview => 2048,
            // Rendering never enlarges, so this keeps the source size.
            Self::Full => 65_535,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Grid => "grid",
            Self::Preview => "preview",
            Self::Full => "full",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Grid, Self::Preview, Self::Full]
            .into_iter()
            .find(|s| s.name() == name)
    }
}

/// Why an image could not be provided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThumbnailError {
    NotFound(PhotoId),
    NoLibrary,
    /// The source could not be decoded; the message says why.
    Decode(String),
    Storage(CatalogError),
}

impl std::fmt::Display for ThumbnailError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "There is no photo {}.", id.0),
            Self::NoLibrary => f.write_str("No library folder is set."),
            Self::Decode(e) => write!(f, "Cannot decode the photo: {e}"),
            Self::Storage(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ThumbnailError {}

impl From<CatalogError> for ThumbnailError {
    fn from(e: CatalogError) -> Self {
        Self::Storage(e)
    }
}

/// Provides cached images, generating them on first use.
pub struct ThumbnailService {
    catalog: Arc<dyn PhotoCatalog>,
    settings: Arc<dyn SettingsStore>,
    fs: Arc<dyn FileSystem>,
    decoder: Arc<dyn ImageDecoder>,
    cache_dir: PathBuf,
    prefilling: AtomicBool,
    prefill_again: AtomicBool,
}

impl std::fmt::Debug for ThumbnailService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThumbnailService")
            .field("cache_dir", &self.cache_dir)
            .finish_non_exhaustive()
    }
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

impl ThumbnailService {
    pub fn new(
        catalog: Arc<dyn PhotoCatalog>,
        settings: Arc<dyn SettingsStore>,
        fs: Arc<dyn FileSystem>,
        decoder: Arc<dyn ImageDecoder>,
        cache_dir: PathBuf,
    ) -> Self {
        Self {
            catalog,
            settings,
            fs,
            decoder,
            cache_dir,
            prefilling: AtomicBool::new(false),
            prefill_again: AtomicBool::new(false),
        }
    }

    /// Where the image of `size` for content `hash` is cached.
    pub fn cache_path(&self, hash: &ContentHash, size: ImageSize) -> PathBuf {
        let hex = hash.to_hex();
        self.cache_dir
            .join(format!("v{CACHE_VERSION}"))
            .join(size.name())
            .join(&hex[..2])
            .join(format!("{hex}.jpg"))
    }

    /// The cached image of a photo, generated if missing.
    pub fn image(&self, id: PhotoId, size: ImageSize) -> Result<PathBuf, ThumbnailError> {
        let photo = self
            .catalog
            .photo(id)?
            .ok_or(ThumbnailError::NotFound(id))?;
        let root = self
            .settings
            .load_settings()?
            .library_root
            .ok_or(ThumbnailError::NoLibrary)?;
        self.ensure(&root, &photo, size)
    }

    fn ensure(
        &self,
        root: &Path,
        photo: &Photo,
        size: ImageSize,
    ) -> Result<PathBuf, ThumbnailError> {
        let asset = photo.assets.display();
        let path = self.cache_path(&asset.hash, size);
        if self.fs.exists(&path) {
            return Ok(path);
        }
        let dir = path.parent().expect("cache path has a folder");
        self.fs
            .create_dir_all(dir)
            .map_err(ThumbnailError::Decode)?;
        // Concurrent requests for the same image each write their own temp
        // file; the first rename wins and the others are discarded.
        let tmp = dir.join(format!(
            ".{}.{}.tmp",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let rendered = self
            .decoder
            .render(
                &root.join(&asset.path),
                asset.format,
                photo.metadata.orientation,
                size.max_edge(),
                &tmp,
            )
            .map_err(|e| {
                let _ = self.fs.remove_file(&tmp);
                ThumbnailError::Decode(e)
            })?;
        // Never remove an existing image: a reader may be about to open it.
        // If another thread got there first, its image is identical.
        if self.fs.rename_no_replace(&tmp, &path).is_err() {
            let _ = self.fs.remove_file(&tmp);
        }
        if photo.metadata.dimensions.is_none()
            && let Some((w, h)) = rendered.source_size
        {
            // Stored dimensions are pre-orientation.
            let (w, h) = if photo.metadata.orientation.swaps_dimensions() {
                (h, w)
            } else {
                (w, h)
            };
            self.catalog.set_dimensions(photo.id, w, h)?;
        }
        Ok(path)
    }

    /// Generates grid images for every photo that lacks one, on background
    /// threads. Does nothing if a prefill is already running.
    pub fn prefill_in_background(self: &Arc<Self>) {
        // A request while a pass runs makes it run once more, so photos
        // imported meanwhile are not missed.
        self.prefill_again.store(true, Ordering::Release);
        if self.prefilling.swap(true, Ordering::AcqRel) {
            return;
        }
        let service = Arc::clone(self);
        std::thread::spawn(move || {
            loop {
                while service.prefill_again.swap(false, Ordering::AcqRel) {
                    service.prefill();
                }
                service.prefilling.store(false, Ordering::Release);
                // A request that came in just now, after the last check.
                if !service.prefill_again.load(Ordering::Acquire)
                    || service.prefilling.swap(true, Ordering::AcqRel)
                {
                    break;
                }
            }
        });
    }

    /// Waits until a background prefill, if any, has finished (tests and
    /// tools).
    pub fn wait_for_prefill(&self) {
        while self.prefilling.load(Ordering::Acquire) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// Whether the photo's grid thumbnail exists.
    pub fn has_grid(&self, photo: &Photo) -> bool {
        self.fs
            .exists(&self.cache_path(&photo.assets.display().hash, ImageSize::Grid))
    }

    /// Generates missing grid images now, in parallel; returns how many
    /// were generated. Photos that fail to decode are skipped.
    pub fn prefill(&self) -> usize {
        let Ok(Some(root)) = self.settings.load_settings().map(|s| s.library_root) else {
            return 0;
        };
        let Ok(photos) = self.catalog.all_photos() else {
            return 0;
        };
        let missing: Vec<&Photo> = photos
            .iter()
            .filter(|p| {
                !self
                    .fs
                    .exists(&self.cache_path(&p.assets.display().hash, ImageSize::Grid))
            })
            .collect();
        let next = std::sync::atomic::AtomicUsize::new(0);
        let done = std::sync::atomic::AtomicUsize::new(0);
        let workers = std::thread::available_parallelism()
            .map_or(2, |n| n.get())
            .clamp(1, 4);
        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(photo) = missing.get(i) else { break };
                        if self.ensure(&root, photo, ImageSize::Grid).is_ok() {
                            done.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                });
            }
        });
        done.into_inner()
    }
}

#[cfg(test)]
mod tests;
