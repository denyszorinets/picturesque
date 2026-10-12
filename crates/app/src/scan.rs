//! Finding importable photos in source folders.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use picturesque_core::grouping::group_files;
use picturesque_core::media::MediaFormat;

use crate::ports::fs::{EntryKind, FileSystem};

/// Folder names never descended into: system, trash and NAS metadata
/// folders. Hidden folders (starting with `.`) are skipped as well.
const SKIPPED_DIRS: &[&str] = &[
    "@eadir",
    "#recycle",
    "$recycle.bin",
    "system volume information",
    "__macosx",
    "lost+found",
];

/// A file found in a source folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub path: PathBuf,
    pub format: MediaFormat,
    pub size: u64,
}

/// The files of one photo found in a source folder.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourcePhoto {
    pub raw: Option<SourceFile>,
    pub image: Option<SourceFile>,
    pub sidecars: Vec<SourceFile>,
}

impl SourcePhoto {
    pub fn files(&self) -> impl Iterator<Item = &SourceFile> {
        self.raw
            .iter()
            .chain(self.image.iter())
            .chain(self.sidecars.iter())
    }

    /// The file metadata should be read from: the RAW when present.
    pub fn primary(&self) -> &SourceFile {
        self.raw
            .as_ref()
            .or(self.image.as_ref())
            .expect("a source photo has a RAW or image")
    }
}

/// Outcome of scanning source folders.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScanReport {
    /// Photos found, in path order.
    pub photos: Vec<SourcePhoto>,
    /// Sidecars that match no photo; not imported.
    pub orphan_sidecars: Vec<PathBuf>,
    /// Paths that could not be read, with the reason. The scan continues.
    pub unreadable: Vec<(PathBuf, String)>,
    /// Files skipped because their format is not supported.
    pub unsupported_files: usize,
}

/// Recursively scans `roots` for photos, skipping hidden and system
/// folders and anything under `exclude` (typically the Clean library, so
/// it is never imported into itself). A root nested inside another root
/// is scanned once.
pub fn scan_sources(fs: &dyn FileSystem, roots: &[PathBuf], exclude: &[PathBuf]) -> ScanReport {
    let mut roots: Vec<&PathBuf> = roots.iter().collect();
    roots.sort();
    roots.dedup();
    let roots: Vec<&PathBuf> = roots
        .iter()
        .filter(|r| {
            !roots
                .iter()
                .any(|other| other != *r && r.starts_with(other))
        })
        .copied()
        .collect();

    let mut report = ScanReport::default();
    for root in roots {
        if is_excluded(root, exclude) {
            continue;
        }
        scan_dir(fs, root, exclude, &mut report);
    }
    report
}

fn is_excluded(path: &Path, exclude: &[PathBuf]) -> bool {
    exclude.iter().any(|e| path.starts_with(e))
}

fn is_skipped_dir(name: &str) -> bool {
    name.starts_with('.') || SKIPPED_DIRS.contains(&name.to_lowercase().as_str())
}

fn scan_dir(fs: &dyn FileSystem, dir: &Path, exclude: &[PathBuf], report: &mut ScanReport) {
    let entries = match fs.list_dir(dir) {
        Ok(entries) => entries,
        Err(reason) => {
            report.unreadable.push((dir.to_path_buf(), reason));
            return;
        }
    };

    let mut files: BTreeMap<String, u64> = BTreeMap::new();
    let mut subdirs = Vec::new();
    for entry in entries {
        match entry.kind {
            EntryKind::Dir if !is_skipped_dir(&entry.name) => subdirs.push(dir.join(&entry.name)),
            EntryKind::File if !entry.name.starts_with('.') => {
                if MediaFormat::from_file_name(&entry.name).is_some() {
                    files.insert(entry.name, entry.size);
                } else {
                    report.unsupported_files += 1;
                }
            }
            _ => {}
        }
    }

    let names: Vec<&String> = files.keys().collect();
    let grouping = group_files(&names);
    let file = |name: &String| SourceFile {
        path: dir.join(name),
        format: MediaFormat::from_file_name(name).expect("grouped files are supported"),
        size: files[name],
    };
    for group in grouping.groups {
        report.photos.push(SourcePhoto {
            raw: group.raw.as_ref().map(file),
            image: group.image.as_ref().map(file),
            sidecars: group.sidecars.iter().map(file).collect(),
        });
    }
    report
        .orphan_sidecars
        .extend(grouping.orphan_sidecars.iter().map(|n| dir.join(n)));

    subdirs.sort();
    for sub in subdirs {
        if !is_excluded(&sub, exclude) {
            scan_dir(fs, &sub, exclude, report);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::ports::fs::DirEntry;

    /// In-memory directory tree. Keys are absolute paths.
    #[derive(Default)]
    pub struct TreeFs {
        pub dirs: BTreeMap<PathBuf, Vec<DirEntry>>,
        pub unreadable: Vec<PathBuf>,
    }

    impl TreeFs {
        pub fn file(mut self, path: &str, size: u64) -> Self {
            let path = PathBuf::from(path);
            let (parent, name) = (
                path.parent().unwrap().to_path_buf(),
                path.file_name().unwrap(),
            );
            self.ensure_dir(&parent);
            self.dirs.get_mut(&parent).unwrap().push(DirEntry {
                name: name.to_string_lossy().into_owned(),
                kind: EntryKind::File,
                size,
            });
            self
        }

        pub fn dir(mut self, path: &str) -> Self {
            self.ensure_dir(Path::new(path));
            self
        }

        pub fn symlinked_dir(mut self, path: &str) -> Self {
            let path = PathBuf::from(path);
            let parent = path.parent().unwrap().to_path_buf();
            self.ensure_dir(&parent);
            self.dirs.get_mut(&parent).unwrap().push(DirEntry {
                name: path.file_name().unwrap().to_string_lossy().into_owned(),
                kind: EntryKind::Other,
                size: 0,
            });
            self
        }

        fn ensure_dir(&mut self, path: &Path) {
            if self.dirs.contains_key(path) {
                return;
            }
            self.dirs.insert(path.to_path_buf(), Vec::new());
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                self.ensure_dir(parent);
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                self.dirs.get_mut(parent).unwrap().push(DirEntry {
                    name,
                    kind: EntryKind::Dir,
                    size: 0,
                });
            }
        }
    }

    impl FileSystem for TreeFs {
        fn probe_writable_dir(&self, _: &Path) -> Result<(), String> {
            Ok(())
        }

        fn list_dir(&self, path: &Path) -> Result<Vec<DirEntry>, String> {
            if self.unreadable.iter().any(|u| u == path) {
                return Err("permission denied".into());
            }
            self.dirs
                .get(path)
                .cloned()
                .ok_or_else(|| "it does not exist".into())
        }
    }

    /// Paths compare by component, so this also holds on Windows, where
    /// `join` inserts backslashes.
    fn names(photo: &SourcePhoto) -> Vec<PathBuf> {
        photo.files().map(|f| f.path.clone()).collect()
    }

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn finds_photos_recursively_with_pairs_and_sizes() {
        let fs = TreeFs::default()
            .file("/src/a/IMG_1.CR3", 30)
            .file("/src/a/IMG_1.JPG", 5)
            .file("/src/b/c/IMG_2.HEIC", 3);

        let report = scan_sources(&fs, &[p("/src")], &[]);

        assert_eq!(report.photos.len(), 2);
        assert_eq!(
            names(&report.photos[0]),
            [p("/src/a/IMG_1.CR3"), p("/src/a/IMG_1.JPG")]
        );
        assert_eq!(report.photos[0].raw.as_ref().unwrap().size, 30);
        assert_eq!(report.photos[0].primary().format, MediaFormat::Raw);
        assert_eq!(names(&report.photos[1]), [p("/src/b/c/IMG_2.HEIC")]);
    }

    #[test]
    fn skips_hidden_system_and_nas_folders_and_hidden_files() {
        let fs = TreeFs::default()
            .file("/src/keep.jpg", 1)
            .file("/src/._keep.jpg", 1)
            .file("/src/.thumbnails/t.jpg", 1)
            .file("/src/@eaDir/keep.jpg/SYNOPHOTO_THUMB.jpg", 1)
            .file("/src/$RECYCLE.BIN/old.jpg", 1)
            .file("/src/__MACOSX/x.jpg", 1);

        let report = scan_sources(&fs, &[p("/src")], &[]);

        assert_eq!(report.photos.len(), 1);
        assert_eq!(names(&report.photos[0]), [p("/src/keep.jpg")]);
    }

    #[test]
    fn counts_unsupported_files_and_reports_orphan_sidecars() {
        let fs = TreeFs::default()
            .file("/src/clip.MOV", 1)
            .file("/src/notes.txt", 1)
            .file("/src/lost.xmp", 1)
            .file("/src/a.jpg", 1);

        let report = scan_sources(&fs, &[p("/src")], &[]);

        assert_eq!(report.unsupported_files, 2);
        assert_eq!(report.orphan_sidecars, [p("/src/lost.xmp")]);
    }

    #[test]
    fn reports_unreadable_folders_and_continues() {
        let mut fs = TreeFs::default()
            .file("/src/locked/a.jpg", 1)
            .file("/src/open/b.jpg", 1);
        fs.unreadable.push(p("/src/locked"));

        let report = scan_sources(&fs, &[p("/src"), p("/missing")], &[]);

        assert_eq!(report.photos.len(), 1);
        assert_eq!(
            report.unreadable,
            [
                (p("/missing"), "it does not exist".into()),
                (p("/src/locked"), "permission denied".into())
            ]
        );
    }

    #[test]
    fn never_scans_the_excluded_library() {
        let fs = TreeFs::default()
            .file("/photos/old/a.jpg", 1)
            .file("/photos/Clean/2026/b.jpg", 1);

        let report = scan_sources(&fs, &[p("/photos")], &[p("/photos/Clean")]);

        assert_eq!(report.photos.len(), 1);
        assert_eq!(names(&report.photos[0]), [p("/photos/old/a.jpg")]);
        assert!(
            scan_sources(&fs, &[p("/photos/Clean")], &[p("/photos/Clean")])
                .photos
                .is_empty()
        );
    }

    #[test]
    fn nested_and_repeated_roots_are_scanned_once() {
        let fs = TreeFs::default()
            .file("/src/a.jpg", 1)
            .file("/src/sub/b.jpg", 1);

        let report = scan_sources(&fs, &[p("/src/sub"), p("/src"), p("/src")], &[]);

        assert_eq!(report.photos.len(), 2);
    }

    #[test]
    fn does_not_follow_symlinked_folders() {
        let fs = TreeFs::default()
            .file("/src/a.jpg", 1)
            .symlinked_dir("/src/loop");

        let report = scan_sources(&fs, &[p("/src")], &[]);

        assert_eq!(report.photos.len(), 1);
        assert!(report.unreadable.is_empty());
    }

    #[test]
    fn empty_folder_yields_empty_report() {
        let fs = TreeFs::default().dir("/src");

        assert_eq!(scan_sources(&fs, &[p("/src")], &[]), ScanReport::default());
    }
}
