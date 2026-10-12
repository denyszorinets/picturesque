//! File-system capabilities used by use cases.

use std::path::Path;

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

/// File-system checks and operations. Implemented on the real file system
/// in `picturesque-infra`; faked in tests.
pub trait FileSystem: Send + Sync {
    /// Succeeds if `path` is an existing directory that files can be
    /// created in. The error is a human-readable reason.
    fn probe_writable_dir(&self, path: &Path) -> Result<(), String>;

    /// Lists a directory. Entries whose names are not valid UTF-8 are
    /// skipped. The error is a human-readable reason.
    fn list_dir(&self, path: &Path) -> Result<Vec<DirEntry>, String>;
}
