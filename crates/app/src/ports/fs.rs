//! File-system capabilities used by use cases.

use std::path::Path;

/// File-system checks and operations. Implemented on the real file system
/// in `picturesque-infra`; faked in tests.
pub trait FileSystem: Send + Sync {
    /// Succeeds if `path` is an existing directory that files can be
    /// created in. The error is a human-readable reason.
    fn probe_writable_dir(&self, path: &Path) -> Result<(), String>;
}
