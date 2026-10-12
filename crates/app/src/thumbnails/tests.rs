use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use picturesque_core::media::MediaFormat;
use picturesque_core::metadata::{CaptureMetadata, Orientation};
use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets, PhotoId};
use serde_json::json;

use super::{ImageSize, ThumbnailError, ThumbnailService};
use crate::api::Api;
use crate::ports::catalog::PhotoCatalog;
use crate::ports::settings::SettingsStore;
use crate::settings::Settings;
use crate::testing::{FakeDecoder, FakeMetadata, MemFs, TestWorld, fake_hash};

fn asset(path: &str, bytes: &[u8]) -> Asset {
    Asset {
        format: MediaFormat::from_file_name(path).unwrap(),
        path: PathBuf::from(path),
        size: bytes.len() as u64,
        hash: fake_hash(bytes),
    }
}

fn world(decoder: FakeDecoder) -> TestWorld {
    let fs = MemFs::new()
        .file("/lib/2026/IMG_1.CR3", b"raw")
        .file("/lib/2026/IMG_1.JPG", b"jpg")
        .file("/lib/2026/IMG_2.CR3", b"raw-2");
    let world = TestWorld::with_decoder(fs, FakeMetadata::default(), decoder);
    world
        .settings
        .save_settings(&Settings {
            library_root: Some("/lib".into()),
            ..Default::default()
        })
        .unwrap();
    world
}

fn add(world: &TestWorld, metadata: CaptureMetadata, assets: Vec<Asset>) -> PhotoId {
    world
        .catalog
        .insert_photo(&NewPhoto {
            metadata,
            assets: PhotoAssets::new(assets).unwrap(),
        })
        .unwrap()
}

fn service(world: &TestWorld) -> ThumbnailService {
    ThumbnailService::new(
        world.catalog.clone(),
        world.settings.clone(),
        world.fs.clone(),
        world.decoder.clone(),
        PathBuf::from("/cache"),
    )
}

#[test]
fn renders_the_display_file_once_and_reuses_the_cache() {
    let world = world(FakeDecoder::default());
    let id = add(
        &world,
        CaptureMetadata::default(),
        vec![
            asset("2026/IMG_1.CR3", b"raw"),
            asset("2026/IMG_1.JPG", b"jpg"),
        ],
    );
    let thumbs = service(&world);

    let first = thumbs.image(id, ImageSize::Grid).unwrap();
    let second = thumbs.image(id, ImageSize::Grid).unwrap();

    assert_eq!(first, second);
    let hex = fake_hash(b"jpg").to_hex();
    assert_eq!(
        first,
        Path::new("/cache/v1/grid")
            .join(&hex[..2])
            .join(format!("{hex}.jpg"))
    );
    assert_eq!(world.fs.read(&first).unwrap(), b"jpeg 512");
    // The JPEG of the pair is rendered, once.
    assert_eq!(
        *world.decoder.calls.lock().unwrap(),
        [(PathBuf::from("/lib/2026/IMG_1.JPG"), 512)]
    );
}

#[test]
fn sizes_are_cached_separately() {
    let world = world(FakeDecoder::default());
    let id = add(
        &world,
        CaptureMetadata::default(),
        vec![asset("2026/IMG_2.CR3", b"raw-2")],
    );
    let thumbs = service(&world);

    let grid = thumbs.image(id, ImageSize::Grid).unwrap();
    let preview = thumbs.image(id, ImageSize::Preview).unwrap();

    assert_ne!(grid, preview);
    assert_eq!(world.fs.read(&preview).unwrap(), b"jpeg 2048");
}

#[test]
fn fills_missing_dimensions_before_orientation() {
    let world = world(FakeDecoder {
        source_size: Some((4000, 6000)),
        ..Default::default()
    });
    let metadata = CaptureMetadata {
        orientation: Orientation::Rotate90,
        ..Default::default()
    };
    let id = add(&world, metadata, vec![asset("2026/IMG_2.CR3", b"raw-2")]);

    service(&world).image(id, ImageSize::Preview).unwrap();

    // Upright 4000×6000 from a photo rotated 90° is stored as 6000×4000.
    assert_eq!(
        world
            .catalog
            .photo(id)
            .unwrap()
            .unwrap()
            .metadata
            .dimensions,
        Some((6000, 4000))
    );
}

#[test]
fn keeps_known_dimensions() {
    let world = world(FakeDecoder {
        source_size: Some((1, 1)),
        ..Default::default()
    });
    let metadata = CaptureMetadata {
        dimensions: Some((8000, 6000)),
        ..Default::default()
    };
    let id = add(&world, metadata, vec![asset("2026/IMG_2.CR3", b"raw-2")]);

    service(&world).image(id, ImageSize::Grid).unwrap();

    assert_eq!(
        world
            .catalog
            .photo(id)
            .unwrap()
            .unwrap()
            .metadata
            .dimensions,
        Some((8000, 6000))
    );
}

#[test]
fn decode_failures_leave_nothing_in_the_cache() {
    let world = world(FakeDecoder {
        fail: true,
        ..Default::default()
    });
    let id = add(
        &world,
        CaptureMetadata::default(),
        vec![asset("2026/IMG_2.CR3", b"raw-2")],
    );

    let err = service(&world).image(id, ImageSize::Grid).unwrap_err();

    assert!(matches!(err, ThumbnailError::Decode(_)), "{err:?}");
    assert!(world.fs.files_under(Path::new("/cache")).is_empty());
}

#[test]
fn reports_missing_photos_and_library() {
    let world = world(FakeDecoder::default());
    assert_eq!(
        service(&world).image(PhotoId(9), ImageSize::Grid),
        Err(ThumbnailError::NotFound(PhotoId(9)))
    );

    let id = add(
        &world,
        CaptureMetadata::default(),
        vec![asset("2026/IMG_2.CR3", b"raw-2")],
    );
    world.settings.save_settings(&Settings::default()).unwrap();
    assert_eq!(
        service(&world).image(id, ImageSize::Grid),
        Err(ThumbnailError::NoLibrary)
    );
}

#[test]
fn prefill_generates_only_missing_grid_images() {
    let world = world(FakeDecoder::default());
    let a = add(
        &world,
        CaptureMetadata::default(),
        vec![asset("2026/IMG_1.JPG", b"jpg")],
    );
    add(
        &world,
        CaptureMetadata::default(),
        vec![asset("2026/IMG_2.CR3", b"raw-2")],
    );
    add(
        &world,
        CaptureMetadata::default(),
        vec![asset("2026/missing.CR3", b"gone")],
    );
    let thumbs = service(&world);
    thumbs.image(a, ImageSize::Grid).unwrap();

    let generated = thumbs.prefill();

    // One already cached, one generated, one whose file is missing.
    assert_eq!(generated, 1);
    assert_eq!(world.decoder.calls.lock().unwrap().len(), 3);
}

#[test]
fn a_completed_import_starts_a_prefill() {
    let fs = MemFs::new().file("/src/a.jpg", b"a").dir("/lib");
    let world = TestWorld::new(fs, FakeMetadata::default());
    world
        .settings
        .save_settings(&Settings {
            library_root: Some("/lib".into()),
            ..Default::default()
        })
        .unwrap();
    let api = Api::new(world.context()).unwrap();
    let run = api
        .call("plan_import", json!({ "sources": ["/src"] }))
        .unwrap()["run_id"]
        .clone();
    api.wait_for_imports();

    api.call("start_import", json!({ "run_id": run })).unwrap();
    api.wait_for_imports();

    let deadline = Instant::now() + Duration::from_secs(5);
    while world.fs.files_under(Path::new("/cache/v1/grid")).is_empty() {
        assert!(Instant::now() < deadline, "prefill did not run");
        std::thread::sleep(Duration::from_millis(10));
    }
    let _ = Arc::clone(&world.decoder);
}
