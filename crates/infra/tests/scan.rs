//! Scanning real folders: `picturesque_app::scan` over `StdFileSystem`.

use std::fs;
use std::path::Path;

use picturesque_app::scan::scan_sources;
use picturesque_infra::fs::StdFileSystem;

fn write(path: &Path, bytes: usize) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, vec![0u8; bytes]).unwrap();
}

#[test]
fn scans_a_real_tree() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(&root.join("2019/trip/IMG_1.CR2"), 30);
    write(&root.join("2019/trip/IMG_1.JPG"), 10);
    write(&root.join("2019/trip/IMG_1.xmp"), 2);
    write(&root.join("phone/IMG_2.HEIC"), 7);
    write(&root.join("phone/clip.MOV"), 1);
    write(&root.join(".cache/thumb.jpg"), 1);
    write(&root.join("Clean/2026/already.jpg"), 1);

    let report = scan_sources(&StdFileSystem, &[root.to_path_buf()], &[root.join("Clean")]);

    assert_eq!(report.photos.len(), 2, "{report:#?}");
    let pair = &report.photos[0];
    assert_eq!(
        pair.raw.as_ref().unwrap().path,
        root.join("2019/trip/IMG_1.CR2")
    );
    assert_eq!(pair.raw.as_ref().unwrap().size, 30);
    assert_eq!(pair.image.as_ref().unwrap().size, 10);
    assert_eq!(pair.sidecars.len(), 1);
    assert_eq!(
        report.photos[1].image.as_ref().unwrap().path,
        root.join("phone/IMG_2.HEIC")
    );
    assert_eq!(report.unsupported_files, 1);
    assert!(report.unreadable.is_empty());
}

#[test]
fn missing_root_is_reported_not_fatal() {
    let dir = tempfile::tempdir().unwrap();

    let report = scan_sources(&StdFileSystem, &[dir.path().join("nope")], &[]);

    assert_eq!(report.unreadable.len(), 1);
    assert_eq!(report.unreadable[0].1, "it does not exist");
}

#[cfg(unix)]
#[test]
fn follows_symlinked_files_but_not_symlinked_folders() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("src");
    write(&root.join("a.jpg"), 4);
    write(&dir.path().join("elsewhere/b.jpg"), 5);
    std::os::unix::fs::symlink(dir.path().join("elsewhere/b.jpg"), root.join("b.jpg")).unwrap();
    std::os::unix::fs::symlink(&root, root.join("loop")).unwrap();

    let report = scan_sources(&StdFileSystem, &[root.clone()], &[]);

    let sizes: Vec<u64> = report
        .photos
        .iter()
        .map(|p| p.image.as_ref().unwrap().size)
        .collect();
    assert_eq!(sizes, [4, 5]);
}
