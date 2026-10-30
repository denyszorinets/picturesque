//! The real file system.

use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::Path;

use picturesque_app::ports::fs::{DirEntry, EntryKind, FileStamp, FileSystem, RenameError};
use picturesque_core::hash::ContentHash;

const BUFFER: usize = 1 << 20;

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

    fn exists(&self, path: &Path) -> bool {
        std::fs::symlink_metadata(path).is_ok()
    }

    fn stamp(&self, path: &Path) -> Result<FileStamp, String> {
        let meta = std::fs::metadata(path).map_err(|e| io_reason(&e))?;
        Ok(FileStamp {
            size: meta.len(),
            modified: meta.modified().ok(),
        })
    }

    fn hash_file(&self, path: &Path) -> Result<(ContentHash, u64), String> {
        let file = File::open(path).map_err(|e| io_reason(&e))?;
        let mut hasher = blake3::Hasher::new();
        let size = std::io::copy(&mut BufReader::with_capacity(BUFFER, file), &mut hasher)
            .map_err(|e| io_reason(&e))?;
        Ok((ContentHash::from_bytes(*hasher.finalize().as_bytes()), size))
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), String> {
        std::fs::create_dir_all(path).map_err(|e| io_reason(&e))
    }

    fn copy_hashing(&self, src: &Path, dst: &Path) -> Result<(ContentHash, u64), String> {
        let mut reader =
            BufReader::with_capacity(BUFFER, File::open(src).map_err(|e| io_reason(&e))?);
        let mut out = File::create(dst).map_err(|e| io_reason(&e))?;
        let mut hasher = blake3::Hasher::new();
        let mut buf = vec![0u8; BUFFER];
        let mut size = 0u64;
        loop {
            let n = reader.read(&mut buf).map_err(|e| io_reason(&e))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            out.write_all(&buf[..n]).map_err(|e| io_reason(&e))?;
            size += n as u64;
        }
        out.sync_all().map_err(|e| io_reason(&e))?;
        Ok((ContentHash::from_bytes(*hasher.finalize().as_bytes()), size))
    }

    fn rename_no_replace(&self, from: &Path, to: &Path) -> Result<(), RenameError> {
        // A hard link fails atomically if `to` exists. File systems without
        // hard links (FAT, exFAT) fall back to check-then-rename, which is
        // safe here because Picturesque is the only writer during an import.
        match std::fs::hard_link(from, to) {
            Ok(()) => std::fs::remove_file(from).map_err(|e| RenameError::Other(io_reason(&e))),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(RenameError::Exists),
            Err(_) => {
                if std::fs::symlink_metadata(to).is_ok() {
                    return Err(RenameError::Exists);
                }
                std::fs::rename(from, to).map_err(|e| RenameError::Other(io_reason(&e)))
            }
        }
    }

    fn sync_durable(&self, path: &Path, up_to: &Path) -> Result<(), String> {
        // Windows needs write access to flush a file; it has no way to
        // flush a directory, where the rename is journaled by NTFS.
        std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .and_then(|f| f.sync_all())
            .map_err(|e| io_reason(&e))?;
        #[cfg(unix)]
        for dir in path.ancestors().skip(1) {
            File::open(dir)
                .and_then(|d| d.sync_all())
                .map_err(|e| io_reason(&e))?;
            if dir == up_to {
                break;
            }
        }
        #[cfg(not(unix))]
        let _ = up_to;
        Ok(())
    }

    fn remove_file(&self, path: &Path) -> Result<(), String> {
        match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(io_reason(&e)),
            _ => Ok(()),
        }
    }

    fn available_space(&self, path: &Path) -> Result<u64, String> {
        fs4::available_space(path).map_err(|e| io_reason(&e))
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

    #[test]
    fn copy_hashing_matches_hash_file_and_blake3() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.bin");
        let data: Vec<u8> = (0..3_000_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&src, &data).unwrap();

        let (copied_hash, size) = StdFileSystem
            .copy_hashing(&src, &dir.path().join("dst.bin"))
            .unwrap();

        assert_eq!(size, data.len() as u64);
        assert_eq!(copied_hash.as_bytes(), blake3::hash(&data).as_bytes());
        assert_eq!(
            StdFileSystem
                .hash_file(&dir.path().join("dst.bin"))
                .unwrap(),
            (copied_hash, size)
        );
    }

    #[test]
    fn rename_no_replace_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b, c) = (
            dir.path().join("a"),
            dir.path().join("b"),
            dir.path().join("c"),
        );
        std::fs::write(&a, b"a").unwrap();
        std::fs::write(&b, b"b").unwrap();

        assert_eq!(
            StdFileSystem.rename_no_replace(&a, &b),
            Err(RenameError::Exists)
        );
        assert_eq!(std::fs::read(&b).unwrap(), b"b");
        assert_eq!(StdFileSystem.rename_no_replace(&a, &c), Ok(()));
        assert!(!a.exists());
        assert_eq!(std::fs::read(&c).unwrap(), b"a");
    }

    #[test]
    fn reports_free_space() {
        let dir = tempfile::tempdir().unwrap();
        assert!(StdFileSystem.available_space(dir.path()).unwrap() > 0);
        // Windows answers for the drive even when the folder is missing.
        #[cfg(unix)]
        assert!(
            StdFileSystem
                .available_space(&dir.path().join("nope"))
                .is_err()
        );
    }

    #[test]
    fn remove_file_tolerates_missing_files() {
        let dir = tempfile::tempdir().unwrap();

        assert_eq!(StdFileSystem.remove_file(&dir.path().join("nope")), Ok(()));
    }

    #[cfg(unix)]
    fn is_root() -> bool {
        std::fs::metadata("/proc/self")
            .map(|m| std::os::unix::fs::MetadataExt::uid(&m) == 0)
            .unwrap_or(false)
    }
}
