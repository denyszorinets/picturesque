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
mod tests {
    use super::*;
    use crate::testing::MemFs;

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
        let fs = MemFs::new()
            .file("/src/a/IMG_1.CR3", &[0; 30])
            .file("/src/a/IMG_1.JPG", &[0; 5])
            .file("/src/b/c/IMG_2.HEIC", &[0; 3]);

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
        let fs = MemFs::new()
            .file("/src/keep.jpg", &[0; 1])
            .file("/src/._keep.jpg", &[0; 1])
            .file("/src/.thumbnails/t.jpg", &[0; 1])
            .file("/src/@eaDir/keep.jpg/SYNOPHOTO_THUMB.jpg", &[0; 1])
            .file("/src/$RECYCLE.BIN/old.jpg", &[0; 1])
            .file("/src/__MACOSX/x.jpg", &[0; 1]);

        let report = scan_sources(&fs, &[p("/src")], &[]);

        assert_eq!(report.photos.len(), 1);
        assert_eq!(names(&report.photos[0]), [p("/src/keep.jpg")]);
    }

    #[test]
    fn counts_unsupported_files_and_reports_orphan_sidecars() {
        let fs = MemFs::new()
            .file("/src/clip.MOV", &[0; 1])
            .file("/src/notes.txt", &[0; 1])
            .file("/src/lost.xmp", &[0; 1])
            .file("/src/a.jpg", &[0; 1]);

        let report = scan_sources(&fs, &[p("/src")], &[]);

        assert_eq!(report.unsupported_files, 2);
        assert_eq!(report.orphan_sidecars, [p("/src/lost.xmp")]);
    }

    #[test]
    fn reports_unreadable_folders_and_continues() {
        let fs = MemFs::new()
            .file("/src/locked/a.jpg", &[0; 1])
            .file("/src/open/b.jpg", &[0; 1])
            .unreadable("/src/locked");

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
        let fs = MemFs::new()
            .file("/photos/old/a.jpg", &[0; 1])
            .file("/photos/Clean/2026/b.jpg", &[0; 1]);

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
        let fs = MemFs::new()
            .file("/src/a.jpg", &[0; 1])
            .file("/src/sub/b.jpg", &[0; 1]);

        let report = scan_sources(&fs, &[p("/src/sub"), p("/src"), p("/src")], &[]);

        assert_eq!(report.photos.len(), 2);
    }

    #[test]
    fn does_not_follow_symlinked_folders() {
        let fs = MemFs::new()
            .file("/src/a.jpg", &[0; 1])
            .symlinked_dir("/src/loop");

        let report = scan_sources(&fs, &[p("/src")], &[]);

        assert_eq!(report.photos.len(), 1);
        assert!(report.unreadable.is_empty());
    }

    #[test]
    fn empty_folder_yields_empty_report() {
        let fs = MemFs::new().dir("/src");

        assert_eq!(scan_sources(&fs, &[p("/src")], &[]), ScanReport::default());
    }
}
