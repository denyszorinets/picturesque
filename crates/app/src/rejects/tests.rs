use std::path::{Path, PathBuf};

use picturesque_core::decision::{Decision, Flag};
use picturesque_core::media::MediaFormat;
use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets, PhotoId};

use super::*;
use crate::ports::fs::FileSystem;
use crate::settings::Settings;
use crate::testing::{FakeMetadata, MemFs, TestWorld, fake_hash};

/// A library with a rejected RAW+JPEG+XMP photo, a rejected JPEG and a
/// kept JPEG.
fn world() -> (TestWorld, RejectsService, Vec<PhotoId>) {
    let fs = MemFs::new()
        .file("/lib/2026/a.CR3", b"raw a")
        .file("/lib/2026/a.JPG", b"jpg a")
        .file("/lib/2026/a.CR3.xmp", b"xmp a")
        .file("/lib/2026/b.JPG", b"jpg b")
        .file("/lib/2026/c.JPG", b"jpg c");
    let world = TestWorld::new(fs, FakeMetadata::default());
    world
        .settings
        .save_settings(&Settings {
            library_root: Some("/lib".into()),
            ..Default::default()
        })
        .unwrap();
    let asset = |path: &str, format| Asset {
        format,
        path: PathBuf::from(path),
        size: 5,
        hash: fake_hash(path.as_bytes()),
    };
    let add = |assets: Vec<Asset>| {
        world
            .catalog
            .insert_photo(&NewPhoto {
                metadata: Default::default(),
                assets: PhotoAssets::new(assets).unwrap(),
            })
            .unwrap()
    };
    let a = add(vec![
        asset("2026/a.CR3", MediaFormat::Raw),
        asset("2026/a.JPG", MediaFormat::Jpeg),
        asset("2026/a.CR3.xmp", MediaFormat::Xmp),
    ]);
    let b = add(vec![asset("2026/b.JPG", MediaFormat::Jpeg)]);
    let c = add(vec![asset("2026/c.JPG", MediaFormat::Jpeg)]);
    for id in [a, b] {
        world
            .catalog
            .set_decision(
                id,
                Decision {
                    flag: Flag::Reject,
                    ..Default::default()
                },
            )
            .unwrap();
    }
    let service = RejectsService::new(
        world.catalog.clone(),
        world.fs.clone(),
        world.settings.clone(),
    );
    (world, service, vec![a, b, c])
}

fn paths(world: &TestWorld, id: PhotoId) -> Vec<String> {
    let photo = world.catalog.photo(id).unwrap().unwrap();
    let mut paths: Vec<String> = files(&photo)
        .iter()
        // `/`-separated on every platform.
        .map(|p| {
            p.iter()
                .map(|c| c.to_string_lossy())
                .collect::<Vec<_>>()
                .join("/")
        })
        .collect();
    paths.sort();
    paths
}

fn exists(world: &TestWorld, path: &str) -> bool {
    world.fs.exists(Path::new(path))
}

#[test]
fn moves_rejected_photos_with_all_their_files_and_undoes() {
    let (world, service, ids) = world();
    let plan = service.plan().unwrap();
    assert_eq!((plan.photos, plan.files, plan.bytes), (2, 4, 20));
    assert_eq!(plan.undo_batch, None);

    let result = service.move_rejects().unwrap();
    assert_eq!((result.batch, result.moved), (Some(1), 2));
    assert!(result.failed.is_empty());
    assert_eq!(
        paths(&world, ids[0]),
        [
            "_Rejected/2026/a.CR3",
            "_Rejected/2026/a.CR3.xmp",
            "_Rejected/2026/a.JPG"
        ]
    );
    assert!(exists(&world, "/lib/_Rejected/2026/a.CR3.xmp"));
    assert!(!exists(&world, "/lib/2026/a.CR3"));
    assert_eq!(paths(&world, ids[2]), ["2026/c.JPG"]);
    // Nothing left to move; the move can be undone.
    let plan = service.plan().unwrap();
    assert_eq!(
        (plan.photos, plan.undo_batch, plan.undo_photos),
        (0, Some(1), 2)
    );
    assert_eq!(service.move_rejects().unwrap().batch, None);

    let undone = service.undo(1).unwrap();
    assert_eq!(undone.moved, 2);
    assert_eq!(paths(&world, ids[1]), ["2026/b.JPG"]);
    assert!(exists(&world, "/lib/2026/a.CR3"));
    assert!(!exists(&world, "/lib/_Rejected/2026/a.CR3"));
    assert_eq!(service.plan().unwrap().undo_batch, None);
    assert!(matches!(service.undo(1), Err(RejectsError::Invalid(_))));
}

#[test]
fn a_photo_that_cannot_move_completely_stays_where_it_was() {
    let (world, service, ids) = world();
    // Something is already where a.JPG would go.
    world
        .fs
        .create_dir_all(Path::new("/lib/_Rejected/2026"))
        .unwrap();
    world
        .fs
        .rename_no_replace(
            Path::new("/lib/2026/c.JPG"),
            Path::new("/lib/_Rejected/2026/a.JPG"),
        )
        .unwrap();

    let result = service.move_rejects().unwrap();
    assert_eq!(result.moved, 1);
    assert_eq!(result.failed.len(), 1);
    assert_eq!(result.failed[0].photo_id, ids[0].0);
    // a's RAW, moved first, came back; the catalog still has every file
    // where it is.
    assert_eq!(
        paths(&world, ids[0]),
        ["2026/a.CR3", "2026/a.CR3.xmp", "2026/a.JPG"]
    );
    for file in ["/lib/2026/a.CR3", "/lib/2026/a.JPG", "/lib/2026/a.CR3.xmp"] {
        assert!(exists(&world, file), "{file}");
    }
    assert!(!exists(&world, "/lib/_Rejected/2026/a.CR3"));
    assert_eq!(paths(&world, ids[1]), ["_Rejected/2026/b.JPG"]);
}

#[test]
fn an_interrupted_move_is_repaired() {
    let (world, service, ids) = world();
    // The app stopped after moving b's file, before updating the catalog.
    world
        .fs
        .create_dir_all(Path::new("/lib/_Rejected/2026"))
        .unwrap();
    world
        .fs
        .rename_no_replace(
            Path::new("/lib/2026/b.JPG"),
            Path::new("/lib/_Rejected/2026/b.JPG"),
        )
        .unwrap();

    let result = service.move_rejects().unwrap();
    // b is already in place (repaired); a moves now.
    assert_eq!(result.moved, 1);
    assert!(result.failed.is_empty());
    assert_eq!(paths(&world, ids[1]), ["_Rejected/2026/b.JPG"]);
    assert_eq!(paths(&world, ids[0])[0], "_Rejected/2026/a.CR3");
}

#[test]
fn needs_a_library_folder() {
    let (world, service, _) = world();
    world.settings.save_settings(&Settings::default()).unwrap();
    assert!(matches!(
        service.move_rejects(),
        Err(RejectsError::Invalid(_))
    ));
}
