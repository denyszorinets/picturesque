//! Clearing a folder culled in place after its keepers were copied out
//! (ADR-0014): deleting originals whose copy is verified again first, and
//! rejects when the settings allow it. Nothing else is ever deleted.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use picturesque_core::decision::Flag;
use picturesque_core::media::MediaFormat;
use picturesque_core::photo::{Photo, PhotoId};
use serde::Serialize;
use ts_rs::TS;

use crate::ports::catalog::{CopyRecord, PhotoCatalog};
use crate::ports::fs::FileSystem;
use crate::ports::settings::SettingsStore;
use crate::settings::DeleteAfterCopy;

/// What clearing would delete (`clear_plan`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ClearPlan {
    pub mode: DeleteAfterCopy,
    /// Photos that would be deleted, rejects included.
    pub photos: usize,
    pub files: usize,
    #[ts(type = "number")]
    pub bytes: u64,
    /// Of those, rejects without a copy.
    pub rejects: usize,
    /// Photos copied only in part (a RAW without its JPEG): kept.
    pub partly_copied: usize,
}

/// A photo that was not deleted, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ClearKept {
    #[ts(type = "number")]
    pub photo_id: i64,
    pub reason: String,
}

/// What clearing did (`clear_folder`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ClearResult {
    pub deleted: usize,
    pub files: usize,
    #[ts(type = "number")]
    pub bytes: u64,
    pub kept: Vec<ClearKept>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClearError {
    /// Clearing is not possible here; the message says why.
    Invalid(String),
    Failed(String),
}

impl fmt::Display for ClearError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(m) | Self::Failed(m) => f.write_str(m),
        }
    }
}

fn failed(e: impl fmt::Display) -> ClearError {
    ClearError::Failed(e.to_string())
}

/// Why a photo may be deleted.
enum Reason {
    /// Every RAW and image file has a recorded copy.
    Copied(Vec<CopyRecord>),
    Rejected,
}

pub struct ClearService {
    catalog: Arc<dyn PhotoCatalog>,
    fs: Arc<dyn FileSystem>,
    settings: Arc<dyn SettingsStore>,
}

impl fmt::Debug for ClearService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClearService").finish_non_exhaustive()
    }
}

fn media(photo: &Photo) -> impl Iterator<Item = &picturesque_core::photo::Asset> {
    photo.assets.iter().filter(|a| a.format != MediaFormat::Xmp)
}

impl ClearService {
    pub fn new(
        catalog: Arc<dyn PhotoCatalog>,
        fs: Arc<dyn FileSystem>,
        settings: Arc<dyn SettingsStore>,
    ) -> Self {
        Self {
            catalog,
            fs,
            settings,
        }
    }

    /// The folder and the mode, if clearing is possible.
    fn folder(&self) -> Result<(PathBuf, DeleteAfterCopy), ClearError> {
        let settings = self.settings.load_settings().map_err(failed)?;
        let Some(folder) = settings.open_folder else {
            return Err(ClearError::Invalid(
                "Only a folder culled in place can be cleared; the library never is.".into(),
            ));
        };
        Ok((folder, settings.delete_after_copy))
    }

    /// Photos that may be deleted, and how many were copied only in part.
    fn candidates(
        &self,
        mode: DeleteAfterCopy,
    ) -> Result<(Vec<(Photo, Reason)>, usize), ClearError> {
        let mut copies: HashMap<PhotoId, Vec<CopyRecord>> = HashMap::new();
        for copy in self.catalog.copies().map_err(failed)? {
            copies.entry(copy.photo_id).or_default().push(copy);
        }
        let mut found = Vec::new();
        let mut partly = 0;
        for photo in self.catalog.all_photos().map_err(failed)? {
            let records = copies.remove(&photo.id).unwrap_or_default();
            let copied = media(&photo).all(|a| records.iter().any(|r| r.source == a.path));
            if copied && !records.is_empty() {
                found.push((photo, Reason::Copied(records)));
                continue;
            }
            if !records.is_empty() {
                partly += 1;
            }
            let rejected = self.catalog.decision(photo.id).map_err(failed)?.flag == Flag::Reject;
            if rejected && mode == DeleteAfterCopy::VerifiedAndRejects {
                found.push((photo, Reason::Rejected));
            }
        }
        Ok((found, partly))
    }

    pub fn plan(&self) -> Result<ClearPlan, ClearError> {
        let (_, mode) = self.folder()?;
        let (candidates, partly_copied) = if mode == DeleteAfterCopy::Never {
            (Vec::new(), 0)
        } else {
            self.candidates(mode)?
        };
        Ok(ClearPlan {
            mode,
            photos: candidates.len(),
            files: candidates
                .iter()
                .map(|(p, _)| p.assets.iter().count())
                .sum(),
            bytes: candidates
                .iter()
                .flat_map(|(p, _)| p.assets.iter().map(|a| a.size))
                .sum(),
            rejects: candidates
                .iter()
                .filter(|(_, r)| matches!(r, Reason::Rejected))
                .count(),
            partly_copied,
        })
    }

    /// Why a copied photo's copies cannot be trusted now, if so.
    fn check_copies(
        &self,
        folder: &std::path::Path,
        photo: &Photo,
        copies: &[CopyRecord],
    ) -> Option<String> {
        for copy in copies {
            match self.fs.hash_file(&copy.destination) {
                Ok((hash, size)) if hash == copy.hash && size == copy.size => {}
                Ok(_) => {
                    return Some(format!(
                        "its copy {} changed since it was copied",
                        copy.destination.display()
                    ));
                }
                Err(_) => {
                    return Some(format!(
                        "its copy {} is missing",
                        copy.destination.display()
                    ));
                }
            }
        }
        // The originals must still be what was copied.
        for asset in media(photo) {
            match self.fs.stamp(&folder.join(&asset.path)) {
                Ok(stamp) if stamp.size == asset.size => {}
                _ => {
                    return Some(format!(
                        "{} changed since it was copied",
                        asset.path.display()
                    ));
                }
            }
        }
        None
    }

    /// Deletes what the plan says, re-verifying every copy first.
    pub fn clear(&self) -> Result<ClearResult, ClearError> {
        let (folder, mode) = self.folder()?;
        if mode == DeleteAfterCopy::Never {
            return Err(ClearError::Invalid(
                "Deleting originals is turned off in Settings.".into(),
            ));
        }
        let (candidates, _) = self.candidates(mode)?;
        let mut result = ClearResult {
            deleted: 0,
            files: 0,
            bytes: 0,
            kept: Vec::new(),
        };
        for (photo, reason) in candidates {
            if let Reason::Copied(copies) = &reason
                && let Some(why) = self.check_copies(&folder, &photo, copies)
            {
                result.kept.push(ClearKept {
                    photo_id: photo.id.0,
                    reason: why,
                });
                continue;
            }
            // Media files first: a photo is gone once they are.
            let mut files: Vec<&picturesque_core::photo::Asset> = media(&photo).collect();
            files.extend(photo.assets.sidecars());
            let mut deleted = 0;
            let mut problem = None;
            for asset in &files {
                match self.fs.remove_file(&folder.join(&asset.path)) {
                    Ok(()) => {
                        deleted += 1;
                        result.files += 1;
                        result.bytes += asset.size;
                    }
                    Err(e) => {
                        problem = Some(format!("cannot delete {}: {e}", asset.path.display()));
                        break;
                    }
                }
            }
            match problem {
                None => {
                    self.catalog.remove_photo(photo.id).map_err(failed)?;
                    result.deleted += 1;
                }
                Some(reason) => {
                    let reason = if deleted > 0 {
                        format!("{reason}; {deleted} of its files were deleted already")
                    } else {
                        reason
                    };
                    result.kept.push(ClearKept {
                        photo_id: photo.id.0,
                        reason,
                    });
                }
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests;
