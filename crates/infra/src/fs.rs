//! The real file system.

use std::path::Path;

use picturesque_app::ports::fs::{DirEntry, EntryKind, FileSystem};

/// [`FileSystem`] backed by `std::fs`.
#[derive(Debug, Default, Clone, Copy)]
pub struct StdFileSystem;

impl FileSystem for StdFileSystem {
    fn probe_writable_dir(&self, path: &Path) -> Result<(), String> {
        let meta = std::fs::metadata(path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => "it does not exist".to_owned(),
            std::io::ErrorKind::PermissionDenied => "permission denied".to_owned(),
            _ => e.to_string(),
        })?;
        if !meta.is_dir() {
            return Err("it is not a folder".into());
        }
        // Permission bits do not tell the whole story (ACLs, read-only
        // mounts), so try creating a file.
        tempfile::Builder::new()
            .prefix(".picturesque-write-test")
            .tempfile_in(path)
            .map(drop)
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::PermissionDenied => "it is not writable".to_owned(),
                _ => format!("it is not writable ({e})"),
            })
    }

    fn list_dir(&self, path: &Path) -> Result<Vec<DirEntry>, String> {
        let read = std::fs::read_dir(path).map_err(|e| io_reason(&e))?;
        let mut entries = Vec::new();
        for entry in read {
            let entry = entry.map_err(|e| io_reason(&e))?;
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let (kind, size) = if file_type.is_symlink() {
                // Follow links to files; never links to directories (loops).
                match std::fs::metadata(entry.path()) {
                    Ok(meta) if meta.is_file() => (EntryKind::File, meta.len()),
                    _ => (EntryKind::Other, 0),
                }
            } else if file_type.is_dir() {
                (EntryKind::Dir, 0)
            } else if file_type.is_file() {
                (
                    EntryKind::File,
                    entry.metadata().map(|m| m.len()).unwrap_or(0),
                )
            } else {
                (EntryKind::Other, 0)
            };
            entries.push(DirEntry { name, kind, size });
        }
        Ok(entries)
    }
}

fn io_reason(e: &std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::NotFound => "it does not exist".to_owned(),
        std::io::ErrorKind::PermissionDenied => "permission denied".to_owned(),
        _ => e.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_writable_directory_and_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();

        assert_eq!(StdFileSystem.probe_writable_dir(dir.path()), Ok(()));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn rejects_a_missing_path() {
        let dir = tempfile::tempdir().unwrap();

        assert_eq!(
            StdFileSystem.probe_writable_dir(&dir.path().join("nope")),
            Err("it does not exist".into())
        );
    }

    #[test]
    fn rejects_a_file() {
        let file = tempfile::NamedTempFile::new().unwrap();

        assert_eq!(
            StdFileSystem.probe_writable_dir(file.path()),
            Err("it is not a folder".into())
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_read_only_directory() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o555)).unwrap();

        let result = StdFileSystem.probe_writable_dir(dir.path());

        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        // Root ignores permission bits, so only assert when not root.
        if !is_root() {
            assert!(result.unwrap_err().contains("not writable"));
        }
    }

    #[cfg(unix)]
    fn is_root() -> bool {
        std::fs::metadata("/proc/self")
            .map(|m| std::os::unix::fs::MetadataExt::uid(&m) == 0)
            .unwrap_or(false)
    }
}
