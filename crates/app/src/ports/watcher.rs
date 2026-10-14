//! File-system change notifications for watched import folders (ADR-0012).

use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Called with the watched folder in which something changed.
pub type ChangeHandler = Arc<dyn Fn(&Path) + Send + Sync>;

pub trait FolderWatcher: Send + Sync {
    /// Watches exactly `folders`, recursively, replacing any earlier set.
    /// Returns the folders that cannot be watched, with the reason.
    fn watch(&self, folders: &[PathBuf], changed: ChangeHandler) -> Vec<(PathBuf, String)>;
}

/// Watches nothing; watched folders are then only rescanned periodically.
#[derive(Debug, Default)]
pub struct NoWatcher;

impl FolderWatcher for NoWatcher {
    fn watch(&self, _: &[PathBuf], _: ChangeHandler) -> Vec<(PathBuf, String)> {
        Vec::new()
    }
}
