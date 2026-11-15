use std::path::{Path, PathBuf};

use picturesque_core::decision::{Decision, Flag};
use picturesque_core::media::MediaFormat;
use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets};
use serde_json::{Value, json};

use super::*;
use crate::api::Api;
use crate::ports::settings::SettingsStore;
use crate::settings::Settings;
use crate::testing::{FakeMetadata, MemFs, TestWorld, fake_hash};

/// A folder culled in place with a RAW+JPEG pair, a lone JPEG and a lone
/// RAW, catalogued.
fn world() -> (TestWorld, [PhotoId; 3]) {
    let fs = MemFs::new()
        .file("/card/IMG_1.CR3", b"raw one")
        .file("/card/IMG_1.JPG", b"jpg one")
        .file("/card/IMG_2.JPG", b"jpg two")
        .file("/card/IMG_3.CR3", b"raw three");
    let world = TestWorld::new(fs, FakeMetadata::default());
    world
        .settings
        .save_settings(&Settings {
            library_root: Some("/card".into()),
            open_folder: Some("/card".into()),
            ..Default::default()
        })
        .unwrap();
    let asset = |format, path: &str, bytes: &[u8]| Asset {
        format,
        path: PathBuf::from(path),
        size: bytes.len() as u64,
        hash: fake_hash(bytes),
    };
    let insert = |assets: Vec<Asset>| {
        world
            .catalog
            .insert_photo(&NewPhoto {
                metadata: Default::default(),
                assets: PhotoAssets::new(assets).unwrap(),
            })
            .unwrap()
    };
    let ids = [
        insert(vec![
            asset(MediaFormat::Raw, "IMG_1.CR3", b"raw one"),
            asset(MediaFormat::Jpeg, "IMG_1.JPG", b"jpg one"),
        ]),
        insert(vec![asset(MediaFormat::Jpeg, "IMG_2.JPG", b"jpg two")]),
        insert(vec![asset(MediaFormat::Raw, "IMG_3.CR3", b"raw three")]),
    ];
    (world, ids)
}

fn ids(world: &TestWorld) -> Vec<PhotoId> {
    let mut ids: Vec<PhotoId> = world
        .catalog
        .all_photos()
        .unwrap()
        .iter()
        .map(|p| p.id)
        .collect();
    ids.sort();
    ids
}

fn files(world: &TestWorld) -> usize {
    ["IMG_1.CR3", "IMG_1.JPG", "IMG_2.JPG", "IMG_3.CR3"]
        .iter()
        .filter(|f| world.fs.exists(&Path::new("/card").join(f)))
        .count()
}

#[test]
fn clearing_the_view_removes_photos_but_never_files() {
    let (world, _) = world();
    let api = Api::new(world.context()).unwrap();

    let result = api.call("clear_view", Value::Null).unwrap();

    assert_eq!(result, json!({ "removed": 3 }));
    assert!(ids(&world).is_empty());
    assert_eq!(files(&world), 4, "every file is still there");
    assert!(
        world
            .events
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|e| matches!(e, AppEvent::PhotosRemoved(PhotosRemoved { removed: 3 })))
    );
}

#[test]
fn clearing_the_view_removes_only_the_photos_in_view() {
    let (world, [first, second, third]) = world();
    world
        .catalog
        .set_decision(
            second,
            Decision {
                flag: Flag::Reject,
                ..Default::default()
            },
        )
        .unwrap();
    let api = Api::new(world.context()).unwrap();

    let result = api
        .call(
            "clear_view",
            // Stacking and paging do not narrow what is cleared.
            json!({ "query": { "flags": ["reject"], "collapse_series": true, "limit": 1 } }),
        )
        .unwrap();

    assert_eq!(result, json!({ "removed": 1 }));
    assert_eq!(ids(&world), [first, third]);
}

#[test]
fn reloading_works_only_in_a_folder_and_saves_decisions_first() {
    let (world, [first, ..]) = world();
    world
        .catalog
        .set_decision(
            first,
            Decision {
                flag: Flag::Pick,
                ..Default::default()
            },
        )
        .unwrap();
    let api = Api::new(world.context()).unwrap();

    let result = api.call("reload_folder", Value::Null).unwrap();

    assert_eq!(result, json!({ "removed": 3 }));
    assert!(ids(&world).is_empty());
    assert_eq!(files(&world), 4);
    // The pick reached its sidecar before the photo left the catalog.
    let written = world.sidecars.0.lock().unwrap();
    assert!(
        written
            .iter()
            .any(|(path, d)| path.to_string_lossy().contains("IMG_1") && d.flag == Flag::Pick),
        "{written:?}"
    );
    drop(written);

    // In the library, nothing is reloaded.
    let mut settings = world.settings.load_settings().unwrap();
    settings.open_folder = None;
    world.settings.save_settings(&settings).unwrap();
    let err = api.call("reload_folder", Value::Null).unwrap_err();
    assert!(err.message.contains("Only a folder"), "{}", err.message);
}

#[test]
fn clearing_an_empty_view_says_nothing() {
    let (world, _) = world();
    let view = ViewService::new(world.catalog.clone(), world.events.clone());

    let removed = view
        .clear(&PhotoQuery {
            photo_ids: Some(vec![999]),
            ..Default::default()
        })
        .unwrap();

    assert_eq!(removed.removed, 0);
    assert_eq!(ids(&world).len(), 3);
    assert!(world.events.0.lock().unwrap().is_empty());
}

#[test]
fn photos_whose_pixels_are_all_gone_are_pruned() {
    let (world, [first, ..]) = world();
    // The JPEG of the pair is gone, but its RAW is still there.
    world.fs.delete(Path::new("/card/IMG_1.JPG"));
    world.fs.delete(Path::new("/card/IMG_2.JPG"));
    world.fs.delete(Path::new("/card/IMG_3.CR3"));

    let removed = prune_missing(
        world.catalog.as_ref(),
        world.fs.as_ref(),
        Path::new("/card"),
    )
    .unwrap();

    assert_eq!(removed, 2);
    assert_eq!(ids(&world), [first]);
}
