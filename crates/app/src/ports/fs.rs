//! File-system capabilities used by use cases.

use std::path::Path;
use std::time::SystemTime;

use picturesque_core::hash::ContentHash;

/// Kind of a directory entry. Symbolic links are reported as what they
/// point to, except links to directories, which are [`EntryKind::Other`]
/// so scans cannot loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Dir,
    Other,
}

/// One entry of a directory listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub kind: EntryKind,
    /// Size in bytes for files; 0 otherwise.
    pub size: u64,
}

/// Size and modification time, used to notice a file changing between
/// planning and copying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStamp {
    pub size: u64,
    pub modified: Option<SystemTime>,
}

/// Why a no-replace rename failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameError {
    /// The destination already exists; nothing was changed.
    Exists,
    Other(String),
}

/// File-system checks and operations. Implemented on the real file system
/// in `picturesque-infra`; faked in tests. Error strings are
/// human-readable reasons.
pub trait FileSystem: Send + Sync {
    /// Succeeds if `path` is an existing directory that files can be
    /// created in.
    fn probe_writable_dir(&self, path: &Path) -> Result<(), String>;

    /// Lists a directory. Entries whose names are not valid UTF-8 are skipped.
    fn list_dir(&self, path: &Path) -> Result<Vec<DirEntry>, String>;

    fn exists(&self, path: &Path) -> bool;

    fn stamp(&self, path: &Path) -> Result<FileStamp, String>;

    /// BLAKE3 hash and size of a file's contents.
    fn hash_file(&self, path: &Path) -> Result<(ContentHash, u64), String>;

    fn create_dir_all(&self, path: &Path) -> Result<(), String>;

    /// Copies `src` to `dst` (replacing `dst` if present), flushes it to
    /// disk, and returns the hash and size of the bytes read from `src`.
    fn copy_hashing(&self, src: &Path, dst: &Path) -> Result<(ContentHash, u64), String>;

    /// Renames `from` to `to`, failing with [`RenameError::Exists`] rather
    /// than replacing an existing `to`.
    fn rename_no_replace(&self, from: &Path, to: &Path) -> Result<(), RenameError>;

    /// Removes a file; succeeds if it does not exist.
    fn remove_file(&self, path: &Path) -> Result<(), String>;

    /// Bytes free for this user on the disk holding `path`.
    fn available_space(&self, path: &Path) -> Result<u64, String>;
}

/// Whether `path` names a folder independently of the working directory:
/// absolute, or on Windows also from the root of the current drive
/// (`\Photos`), as Windows resolves it the same way everywhere in the app.
pub fn is_absolute(path: &Path) -> bool {
    path.is_absolute() || (cfg!(windows) && path.has_root())
}
