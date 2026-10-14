//! Verified, never-overwriting copies of source files into the library
//! (ADR-0008).

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use picturesque_core::hash::ContentHash;
use picturesque_core::media::MediaFormat;

use crate::ports::fs::{FileSystem, RenameError};

/// One file of a photo, and where it goes.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PlannedFile {
    /// Absolute source path.
    pub source: PathBuf,
    pub format: MediaFormat,
    pub size: u64,
    /// Source modification time (ns since the Unix epoch) when read.
    pub modified_ns: Option<i64>,
    /// Known when it was needed for duplicate detection.
    pub hash: Option<ContentHash>,
    /// Destination relative to the library root.
    pub target: PathBuf,
}

/// Copies one file to its target via a temporary file, verifying the copy
/// by hash. Returns the content hash. A target that already holds the same
/// content (from an interrupted attempt) is accepted as is.
pub(super) fn copy_file(
    fs: &dyn FileSystem,
    root: &Path,
    file: &PlannedFile,
) -> Result<ContentHash, String> {
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

    ensure_unchanged(fs, file)?;
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

    if let Err(e) = ensure_unchanged(fs, file) {
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

fn ensure_unchanged(fs: &dyn FileSystem, file: &PlannedFile) -> Result<(), String> {
    let stamp = fs.stamp(&file.source)?;
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
    "the file changed while it was being imported; it will be imported again".into()
}
