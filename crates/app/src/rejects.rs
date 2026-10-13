//! Moving rejected photos out of the way (`docs/features/rejects.rst`).
//!
//! Rejected photos are moved, with all their files (RAW, image, sidecars),
//! into the `_Rejected` folder of the library, keeping their paths inside
//! it: `2026/2026-10-12/IMG_1.CR3` becomes
//! `_Rejected/2026/2026-10-12/IMG_1.CR3`. Files are only renamed, never
//! copied or deleted, so nothing can be lost:
//!
//! - a photo's files move together: if one cannot be moved, the ones
//!   already moved go back and the photo stays where it was;
//! - the catalog is updated after the files moved, and if that fails the
//!   files go back too;
//! - should the app stop between the two, the next move or undo finds the
//!   files at the other location and updates the catalog (`repair`).
//!
//! Each move is a batch; the latest one can be undone.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use picturesque_core::photo::Photo;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ports::catalog::{CatalogError, PhotoCatalog};
use crate::ports::fs::FileSystem;
use crate::ports::settings::SettingsStore;

/// Folder inside the library that rejected photos are moved to.
pub const REJECTED_DIR: &str = "_Rejected";

/// What `plan_reject_move` reports before moving.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct RejectMovePlan {
    /// Rejected photos not in `_Rejected` yet.
    pub photos: usize,
    pub files: usize,
    #[ts(type = "number")]
    pub bytes: u64,
    /// The batch `undo_reject_move` would move back, if any.
    #[ts(type = "number | null")]
    pub undo_batch: Option<i64>,
    /// Photos in that batch.
    pub undo_photos: usize,
}

/// A photo that could not be moved; it stayed where it was.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct MoveFailure {
    #[ts(type = "number")]
    pub photo_id: i64,
    pub error: String,
}

/// Outcome of `move_rejects` and `undo_reject_move`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct RejectMoveResult {
    /// The batch moved (or moved back); `null` when nothing moved.
    #[ts(type = "number | null")]
    pub batch: Option<i64>,
    pub moved: usize,
    pub failed: Vec<MoveFailure>,
}

/// Arguments of `undo_reject_move`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct UndoRejectMoveArgs {
    /// Must be the latest batch (from `plan_reject_move`).
    #[ts(type = "number")]
    pub batch: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RejectsError {
    /// No library folder is set, or the batch is not the latest.
    Invalid(String),
    Catalog(CatalogError),
}

impl fmt::Display for RejectsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(m) => f.write_str(m),
            Self::Catalog(e) => e.fmt(f),
        }
    }
}

impl From<CatalogError> for RejectsError {
    fn from(e: CatalogError) -> Self {
        Self::Catalog(e)
    }
}

/// Where a photo's file goes: into `_Rejected` (`to_rejected`) or back.
fn moved_path(path: &Path, to_rejected: bool) -> Option<PathBuf> {
    if to_rejected {
        (!in_rejected(path)).then(|| Path::new(REJECTED_DIR).join(path))
    } else {
        path.strip_prefix(REJECTED_DIR).ok().map(Path::to_path_buf)
    }
}

fn in_rejected(path: &Path) -> bool {
    path.starts_with(REJECTED_DIR)
}

fn files(photo: &Photo) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = photo
        .assets
        .raw()
        .into_iter()
        .chain(photo.assets.image())
        .chain(photo.assets.sidecars())
        .map(|a| a.path.clone())
        .collect();
    files.dedup();
    files
}

/// Moves rejected photos to `_Rejected` and back.
pub struct RejectsService {
    catalog: Arc<dyn PhotoCatalog>,
    fs: Arc<dyn FileSystem>,
    settings: Arc<dyn SettingsStore>,
    /// One move or undo at a time.
    running: Mutex<()>,
}

impl fmt::Debug for RejectsService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RejectsService").finish_non_exhaustive()
    }
}

impl RejectsService {
    pub fn new(
        catalog: Arc<dyn PhotoCatalog>,
        fs: Arc<dyn FileSystem>,
        settings: Arc<dyn SettingsStore>,
    ) -> Self {
        Self {
            catalog,
            fs,
            settings,
            running: Mutex::new(()),
        }
    }

    fn root(&self) -> Result<PathBuf, RejectsError> {
        self.settings
            .load_settings()?
            .library_root
            .ok_or_else(|| RejectsError::Invalid("Choose a library folder first.".into()))
    }

    /// Rejected photos still outside `_Rejected`.
    fn to_move(&self) -> Result<Vec<Photo>, RejectsError> {
        let mut photos = Vec::new();
        for id in self.catalog.rejected_photos()? {
            if let Some(photo) = self.catalog.photo(id)?
                && files(&photo).iter().any(|f| !in_rejected(f))
            {
                photos.push(photo);
            }
        }
        Ok(photos)
    }

    pub fn plan(&self) -> Result<RejectMovePlan, RejectsError> {
        let photos = self.to_move()?;
        let (files, bytes) = photos.iter().fold((0, 0), |(n, b), p| {
            let assets: Vec<_> = p
                .assets
                .raw()
                .into_iter()
                .chain(p.assets.image())
                .chain(p.assets.sidecars())
                .collect();
            (
                n + assets.len(),
                b + assets.iter().map(|a| a.size).sum::<u64>(),
            )
        });
        let undo = self.catalog.last_reject_move()?;
        Ok(RejectMovePlan {
            photos: photos.len(),
            files,
            bytes,
            undo_batch: undo.as_ref().map(|(b, _)| *b),
            undo_photos: undo.map_or(0, |(_, ids)| ids.len()),
        })
    }

    /// Moves every rejected photo to `_Rejected`, as a new batch.
    pub fn move_rejects(&self) -> Result<RejectMoveResult, RejectsError> {
        let _running = self.running.lock().expect("rejects lock");
        let root = self.root()?;
        let photos = self.to_move()?;
        for photo in &photos {
            self.repair(&root, photo)?;
        }
        let photos = self.to_move()?;
        let batch = self.catalog.last_reject_move()?.map_or(1, |(b, _)| b + 1);
        let mut moved = Vec::new();
        let mut failed = Vec::new();
        for photo in &photos {
            match self.move_photo(&root, photo, true) {
                Ok(()) => moved.push(photo.id),
                Err(error) => failed.push(MoveFailure {
                    photo_id: photo.id.0,
                    error,
                }),
            }
        }
        if !moved.is_empty() {
            self.catalog.record_reject_move(batch, &moved)?;
        }
        Ok(RejectMoveResult {
            batch: (!moved.is_empty()).then_some(batch),
            moved: moved.len(),
            failed,
        })
    }

    /// Moves the photos of the latest batch back where they were.
    pub fn undo(&self, batch: i64) -> Result<RejectMoveResult, RejectsError> {
        let _running = self.running.lock().expect("rejects lock");
        let root = self.root()?;
        let Some((latest, ids)) = self.catalog.last_reject_move()? else {
            return Err(RejectsError::Invalid("There is no move to undo.".into()));
        };
        if latest != batch {
            return Err(RejectsError::Invalid(format!(
                "Only the latest move ({latest}) can be undone."
            )));
        }
        let mut back = Vec::new();
        let mut failed = Vec::new();
        for id in ids {
            let Some(photo) = self.catalog.photo(id)? else {
                continue;
            };
            self.repair(&root, &photo)?;
            let photo = self.catalog.photo(id)?.unwrap_or(photo);
            match self.move_photo(&root, &photo, false) {
                Ok(()) => back.push(id),
                Err(error) => failed.push(MoveFailure {
                    photo_id: id.0,
                    error,
                }),
            }
        }
        self.catalog.forget_reject_move(&back)?;
        Ok(RejectMoveResult {
            batch: Some(batch),
            moved: back.len(),
            failed,
        })
    }

    /// Fixes the catalog for files found at the other location (after an
    /// interrupted move).
    fn repair(&self, root: &Path, photo: &Photo) -> Result<(), RejectsError> {
        let mut fixes = Vec::new();
        for file in files(photo) {
            if self.fs.exists(&root.join(&file)) {
                continue;
            }
            let other = moved_path(&file, true).or_else(|| moved_path(&file, false));
            if let Some(other) = other
                && self.fs.exists(&root.join(&other))
            {
                fixes.push((file, other));
            }
        }
        if !fixes.is_empty() {
            self.catalog.move_assets(photo.id, &fixes)?;
        }
        Ok(())
    }

    /// Moves one photo's files; on any failure puts back what moved.
    fn move_photo(&self, root: &Path, photo: &Photo, to_rejected: bool) -> Result<(), String> {
        let moves: Vec<(PathBuf, PathBuf)> = files(photo)
            .into_iter()
            .filter_map(|f| moved_path(&f, to_rejected).map(|to| (f, to)))
            .collect();
        let mut done: Vec<&(PathBuf, PathBuf)> = Vec::new();
        let undo = |done: &[&(PathBuf, PathBuf)]| {
            for (from, to) in done.iter().rev() {
                let _ = self.fs.rename_no_replace(&root.join(to), &root.join(from));
            }
        };
        for m in &moves {
            let (from, to) = (root.join(&m.0), root.join(&m.1));
            if !self.fs.exists(&from) {
                undo(&done);
                return Err(format!("{} is missing", m.0.display()));
            }
            let result = to
                .parent()
                .map_or(Ok(()), |dir| self.fs.create_dir_all(dir))
                .and_then(|()| {
                    self.fs
                        .rename_no_replace(&from, &to)
                        .map_err(|e| format!("cannot move {}: {e:?}", m.0.display()))
                });
            if let Err(e) = result {
                undo(&done);
                return Err(e);
            }
            done.push(m);
        }
        if let Err(e) = self.catalog.move_assets(photo.id, &moves) {
            undo(&done);
            return Err(e.to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
