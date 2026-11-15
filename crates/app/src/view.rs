//! Clearing photos from view (`docs/features/clear-view.rst`): photos leave
//! the catalog, never the disk. **Clear view…** removes the photos in view;
//! a folder culled in place also drops photos whose files are gone (a
//! memory card that was wiped), each time it is scanned.
//!
//! Import jobs are kept, so an unchanged file that is still in an import
//! folder does not come back on the next scan; a new or changed file is
//! imported as usual.

use std::path::Path;
use std::sync::Arc;

use picturesque_core::photo::{Photo, PhotoId};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::library::{MAX_PAGE, PhotoQuery};
use crate::ports::catalog::{CatalogError, PhotoCatalog};
use crate::ports::events::{AppEvent, EventSink};
use crate::ports::fs::FileSystem;

/// Arguments of `clear_view`.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(default, deny_unknown_fields)]
#[ts(export)]
pub struct ClearViewArgs {
    /// The photos to remove: those matching these filters (every photo
    /// when empty). Stacked series count whole; paging is ignored.
    pub query: PhotoQuery,
}

/// Result of `clear_view`, and the `photos_removed` event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct PhotosRemoved {
    /// Photos removed from the catalog.
    pub removed: usize,
}

/// Removes photos from the catalog without touching their files.
pub struct ViewService {
    catalog: Arc<dyn PhotoCatalog>,
    events: Arc<dyn EventSink>,
}

impl std::fmt::Debug for ViewService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ViewService").finish_non_exhaustive()
    }
}

impl ViewService {
    pub fn new(catalog: Arc<dyn PhotoCatalog>, events: Arc<dyn EventSink>) -> Self {
        Self { catalog, events }
    }

    /// Removes the photos matching `query` from the catalog. Returns how
    /// many were removed.
    pub fn clear(&self, query: &PhotoQuery) -> Result<PhotosRemoved, CatalogError> {
        let ids = matching(self.catalog.as_ref(), query)?;
        let removed = remove(self.catalog.as_ref(), &ids)?;
        if removed > 0 {
            self.events
                .emit(&AppEvent::PhotosRemoved(PhotosRemoved { removed }));
        }
        Ok(PhotosRemoved { removed })
    }
}

/// Every photo matching `query`, with series not stacked (a stack in view
/// stands for all its photos).
fn matching(catalog: &dyn PhotoCatalog, query: &PhotoQuery) -> Result<Vec<PhotoId>, CatalogError> {
    let mut query = PhotoQuery {
        collapse_series: None,
        expanded_series: None,
        sort: None,
        limit: Some(MAX_PAGE),
        cursor: None,
        ..query.clone()
    };
    let mut ids = Vec::new();
    loop {
        let page = catalog.query(&query)?;
        ids.extend(page.photos.iter().map(|p| PhotoId(p.id)));
        match page.next_cursor {
            Some(next) => query.cursor = Some(next),
            None => return Ok(ids),
        }
    }
}

fn remove(catalog: &dyn PhotoCatalog, ids: &[PhotoId]) -> Result<usize, CatalogError> {
    for id in ids {
        catalog.remove_photo(*id)?;
    }
    Ok(ids.len())
}

/// Whether every RAW and image file of the photo is gone from `root`
/// (sidecars do not count: a photo without its pixels cannot be shown).
fn files_gone(fs: &dyn FileSystem, root: &Path, photo: &Photo) -> bool {
    let mut media = photo
        .assets
        .raw()
        .into_iter()
        .chain(photo.assets.image())
        .peekable();
    media.peek().is_some() && media.all(|a| !fs.exists(&root.join(&a.path)))
}

/// In a folder culled in place, removes the photos whose files are no
/// longer in the folder from the catalog. Only call it after the folder
/// was read successfully: a card that is not inserted must not empty the
/// view. Returns how many photos were removed.
pub fn prune_missing(
    catalog: &dyn PhotoCatalog,
    fs: &dyn FileSystem,
    folder: &Path,
) -> Result<usize, CatalogError> {
    let gone: Vec<PhotoId> = catalog
        .all_photos()?
        .iter()
        .filter(|p| files_gone(fs, folder, p))
        .map(|p| p.id)
        .collect();
    remove(catalog, &gone)
}

#[cfg(test)]
mod tests;
