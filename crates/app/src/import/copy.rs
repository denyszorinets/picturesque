//! Executing a plan: verified, all-or-nothing copying per photo
//! (ADR-0008).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;

use picturesque_core::hash::ContentHash;
use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets};

use super::model::{FileStatus, ImportRun, ImportState, PhotoStatus, PlannedFile, PlannedPhoto};
use super::{Deps, Progress};
use crate::ports::catalog::CatalogError;
use crate::ports::fs::RenameError;

/// How copying ended.
#[derive(Debug, PartialEq, Eq)]
pub enum CopyOutcome {
    Completed,
    Cancelled,
    /// The run could not continue (library folder gone, catalog error).
    Failed(String),
}

/// Copies every planned photo of `run`, resuming where a previous attempt
/// stopped. Photos already imported, duplicates and failures are skipped.
pub fn copy_run(
    deps: &Deps,
    run: &ImportRun,
    cancel: &AtomicBool,
    progress: &(dyn Fn(Progress) + Sync),
) -> CopyOutcome {
    if let Err(reason) = deps.fs.probe_writable_dir(&run.library_root) {
        return CopyOutcome::Failed(format!(
            "The library folder `{}` is not available: {reason}.",
            run.library_root.display()
        ));
    }
    let pending: Vec<&PlannedPhoto> = run
        .photos
        .iter()
        .filter(|p| p.status == PhotoStatus::Planned)
        .collect();
    let total = pending.len();
    progress(Progress::Copying { done: 0, total });

    for (i, photo) in pending.into_iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return CopyOutcome::Cancelled;
        }
        if let Err(e) = import_photo(deps, &run.library_root, photo) {
            return CopyOutcome::Failed(e.to_string());
        }
        progress(Progress::Copying { done: i + 1, total });
    }
    CopyOutcome::Completed
}

/// Copies one photo's files and catalogues it. A file failure fails only
/// this photo (its copies are removed again); a catalog failure stops the run.
fn import_photo(deps: &Deps, root: &Path, photo: &PlannedPhoto) -> Result<(), CatalogError> {
    let photo_row = photo.id.expect("saved plan has ids");
    let mut copied: Vec<(&PlannedFile, PathBuf, ContentHash)> = Vec::new();
    let mut failure = None;

    for file in &photo.files {
        let file_row = file.id.expect("saved plan has ids");
        match copy_file(deps, root, file) {
            Ok(hash) => {
                deps.store
                    .update_file(file_row, FileStatus::Verified, Some(&hash), None)?;
                copied.push((file, root.join(&file.target), hash));
            }
            Err(reason) => {
                deps.store
                    .update_file(file_row, FileStatus::Failed, None, Some(&reason))?;
                failure = Some(format!("{}: {reason}", file.source.display()));
                break;
            }
        }
    }

    if let Some(reason) = failure {
        // All or nothing: remove this photo's copies, so a RAW is never
        // left in the library without its JPEG.
        for (file, path, _) in &copied {
            let _ = deps.fs.remove_file(path);
            deps.store
                .update_file(file.id.expect("saved"), FileStatus::Planned, None, None)?;
        }
        deps.store
            .update_photo(photo_row, PhotoStatus::Failed, None, Some(&reason))?;
        return Ok(());
    }

    let assets = copied
        .iter()
        .map(|(file, _, hash)| Asset {
            format: file.format,
            path: file.target.clone(),
            size: file.size,
            hash: *hash,
        })
        .collect();
    let assets = PhotoAssets::new(assets).map_err(|e| CatalogError::Storage(e.to_string()))?;
    let new = NewPhoto {
        metadata: photo.metadata.clone(),
        assets,
    };
    let photo_id = match deps.catalog.insert_photo(&new) {
        Ok(id) => id,
        // Catalogued by an earlier attempt that stopped before recording it.
        Err(CatalogError::Duplicate(_)) => deps
            .catalog
            .photo_with_media_hash(&new.assets.primary().hash)?
            .ok_or_else(|| CatalogError::Storage("duplicate photo not found".into()))?,
        Err(e) => return Err(e),
    };
    if let Some(decision) = photo.decision {
        deps.catalog.set_decision(photo_id, decision)?;
    }
    deps.store
        .update_photo(photo_row, PhotoStatus::Imported, Some(photo_id), None)
}

/// Copies one file to its target via a temporary file, verifying the copy
/// by hash. Returns the content hash. A target that already holds the same
/// content (from an interrupted attempt) is accepted as is.
fn copy_file(deps: &Deps, root: &Path, file: &PlannedFile) -> Result<ContentHash, String> {
    let fs = deps.fs.as_ref();
    let dst = root.join(&file.target);

    if fs.exists(&dst) {
        let (existing, _) = fs.hash_file(&dst)?;
        let expected = match file.hash {
            Some(h) => h,
            None => fs.hash_file(&file.source)?.0,
        };
        return if existing == expected {
            Ok(existing)
        } else {
            Err("a different file already exists at the destination".into())
        };
    }

    ensure_unchanged(deps, file)?;
    let parent = dst.parent().expect("target has a folder");
    fs.create_dir_all(parent)?;
    let tmp = parent.join(format!(
        ".{}.picturesque-part",
        dst.file_name().expect("file name").to_string_lossy()
    ));

    let mut verified = None;
    for _attempt in 0..2 {
        let (source_hash, size) = fs.copy_hashing(&file.source, &tmp).inspect_err(|_| {
            let _ = fs.remove_file(&tmp);
        })?;
        if size != file.size || file.hash.is_some_and(|h| h != source_hash) {
            let _ = fs.remove_file(&tmp);
            return Err(changed_message());
        }
        let (copy_hash, _) = fs.hash_file(&tmp)?;
        if copy_hash == source_hash {
            verified = Some(source_hash);
            break;
        }
        let _ = fs.remove_file(&tmp);
    }
    let Some(hash) = verified else {
        return Err(
            "the copy did not match the original twice; the destination disk may be failing".into(),
        );
    };

    if let Err(e) = ensure_unchanged(deps, file) {
        let _ = fs.remove_file(&tmp);
        return Err(e);
    }
    match fs.rename_no_replace(&tmp, &dst) {
        Ok(()) => Ok(hash),
        Err(RenameError::Exists) => {
            let _ = fs.remove_file(&tmp);
            Err("another file appeared at the destination during the import".into())
        }
        Err(RenameError::Other(e)) => {
            let _ = fs.remove_file(&tmp);
            Err(e)
        }
    }
}

fn ensure_unchanged(deps: &Deps, file: &PlannedFile) -> Result<(), String> {
    let stamp = deps.fs.stamp(&file.source)?;
    let modified_ns = stamp
        .modified
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| i64::try_from(d.as_nanos()).ok());
    if stamp.size != file.size || (file.modified_ns.is_some() && modified_ns != file.modified_ns) {
        return Err(changed_message());
    }
    Ok(())
}

fn changed_message() -> String {
    "the file changed after the import was planned; plan the import again".into()
}

/// The state a run ends in for a copy outcome.
pub fn final_state(outcome: &CopyOutcome) -> (ImportState, Option<String>) {
    match outcome {
        CopyOutcome::Completed => (ImportState::Completed, None),
        CopyOutcome::Cancelled => (ImportState::Cancelled, None),
        CopyOutcome::Failed(e) => (ImportState::Failed, Some(e.clone())),
    }
}
