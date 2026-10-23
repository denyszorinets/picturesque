use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use picturesque_core::media::MediaFormat;
use picturesque_core::metadata::{CaptureMetadata, Orientation};
use picturesque_core::photo::{Asset, NewPhoto, PairView, PhotoAssets, PhotoId};
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

    let first = thumbs.image(id, ImageSize::Grid, PairView::Image).unwrap();
    let second = thumbs.image(id, ImageSize::Grid, PairView::Image).unwrap();

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

fn pair(world: &TestWorld) -> PhotoId {
    add(
        world,
        CaptureMetadata::default(),
        vec![
            asset("2026/IMG_1.CR3", b"raw"),
            asset("2026/IMG_1.JPG", b"jpg"),
        ],
    )
}

fn show_pairs_as(world: &TestWorld, view: PairView) {
    let mut settings = world.settings.load_settings().unwrap();
    settings.pair_view = view;
    world.settings.save_settings(&settings).unwrap();
}

#[test]
fn either_file_of_a_pair_is_rendered_and_cached_on_its_own() {
    let world = world(FakeDecoder::default());
    let id = pair(&world);
    let thumbs = service(&world);

    let jpeg = thumbs
        .image(id, ImageSize::Preview, PairView::Image)
        .unwrap();
    let raw = thumbs.image(id, ImageSize::Preview, PairView::Raw).unwrap();

    assert_ne!(jpeg, raw);
    assert_eq!(
        raw,
        thumbs.cache_path(&fake_hash(b"raw"), ImageSize::Preview)
    );
    assert_eq!(
        *world.decoder.calls.lock().unwrap(),
        [
            (PathBuf::from("/lib/2026/IMG_1.JPG"), 2048),
            (PathBuf::from("/lib/2026/IMG_1.CR3"), 2048)
        ]
    );
}

#[test]
fn prefill_and_has_grid_follow_the_pair_setting() {
    let world = world(FakeDecoder::default());
    let id = pair(&world);
    show_pairs_as(&world, PairView::Raw);
    let thumbs = service(&world);

    assert_eq!(thumbs.prefill(), 1);

    assert_eq!(
        *world.decoder.calls.lock().unwrap(),
        [(PathBuf::from("/lib/2026/IMG_1.CR3"), 512)]
    );
    let photo = world.catalog.photo(id).unwrap().unwrap();
    assert!(thumbs.has_grid(&photo, PairView::Raw));
    assert!(!thumbs.has_grid(&photo, PairView::Image));
}

#[test]
fn image_requests_choose_the_file_of_a_pair() {
    let world = world(FakeDecoder::default());
    let id = pair(&world);
    let api = Api::new(world.context()).unwrap();
    let thumbs = service(&world);
    let jpeg = thumbs.cache_path(&fake_hash(b"jpg"), ImageSize::Grid);
    let raw = thumbs.cache_path(&fake_hash(b"raw"), ImageSize::Grid);

    assert_eq!(api.image_request(&format!("grid/{}", id.0)).unwrap(), jpeg);
    assert_eq!(
        api.image_request(&format!("grid/{}/raw", id.0)).unwrap(),
        raw
    );
    assert_eq!(
        api.image_request(&format!("grid/{}/image", id.0)).unwrap(),
        jpeg
    );

    // Without a view, the setting decides.
    show_pairs_as(&world, PairView::Raw);
    assert_eq!(api.image_request(&format!("grid/{}", id.0)).unwrap(), raw);

    use crate::api::ApiErrorCode::*;
    for bad in ["grid/1/jpeg", "grid/1/raw/extra"] {
        assert_eq!(
            api.image_request(bad).unwrap_err().code,
            InvalidArguments,
            "{bad}"
        );
    }
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

    let grid = thumbs.image(id, ImageSize::Grid, PairView::Image).unwrap();
    let preview = thumbs
        .image(id, ImageSize::Preview, PairView::Image)
        .unwrap();

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

    service(&world)
        .image(id, ImageSize::Preview, PairView::Image)
        .unwrap();

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

    service(&world)
        .image(id, ImageSize::Grid, PairView::Image)
        .unwrap();

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

    let err = service(&world)
        .image(id, ImageSize::Grid, PairView::Image)
        .unwrap_err();

    assert!(matches!(err, ThumbnailError::Decode(_)), "{err:?}");
    assert!(world.fs.files_under(Path::new("/cache")).is_empty());
}

#[test]
fn reports_missing_photos_and_library() {
    let world = world(FakeDecoder::default());
    assert_eq!(
        service(&world).image(PhotoId(9), ImageSize::Grid, PairView::Image),
        Err(ThumbnailError::NotFound(PhotoId(9)))
    );

    let id = add(
        &world,
        CaptureMetadata::default(),
        vec![asset("2026/IMG_2.CR3", b"raw-2")],
    );
    world.settings.save_settings(&Settings::default()).unwrap();
    assert_eq!(
        service(&world).image(id, ImageSize::Grid, PairView::Image),
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
    thumbs.image(a, ImageSize::Grid, PairView::Image).unwrap();

    let generated = thumbs.prefill();

    // One already cached, one generated, one whose file is missing.
    assert_eq!(generated, 1);
    assert_eq!(world.decoder.calls.lock().unwrap().len(), 3);
}

#[test]
fn an_imported_photo_starts_a_prefill() {
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
    api.call("add_import_source", json!({ "path": "/src" }))
        .unwrap();
    assert_eq!(api.run_imports(), 1);

    let deadline = Instant::now() + Duration::from_secs(5);
    while world.fs.files_under(Path::new("/cache/v1/grid")).is_empty() {
        assert!(Instant::now() < deadline, "prefill did not run");
        std::thread::sleep(Duration::from_millis(10));
    }
    let _ = Arc::clone(&world.decoder);
}

#[test]
fn image_requests_are_parsed_and_validated() {
    let world = world(FakeDecoder::default());
    let id = add(
        &world,
        CaptureMetadata::default(),
        vec![asset("2026/IMG_2.CR3", b"raw-2")],
    );
    let api = Api::new(world.context()).unwrap();

    assert!(
        api.image_request(&format!("grid/{}", id.0))
            .unwrap()
            .starts_with("/cache/v1/grid")
    );
    assert!(
        api.image_request(&format!("/preview/{}/", id.0))
            .unwrap()
            .starts_with("/cache/v1/preview")
    );
    use crate::api::ApiErrorCode::*;
    assert_eq!(
        api.image_request("huge/1").unwrap_err().code,
        InvalidArguments
    );
    assert_eq!(
        api.image_request("grid/abc").unwrap_err().code,
        InvalidArguments
    );
    assert_eq!(
        api.image_request("grid").unwrap_err().code,
        InvalidArguments
    );
    assert_eq!(api.image_request("grid/999").unwrap_err().code, NotFound);
}

#[test]
fn a_concurrently_created_image_is_kept_not_replaced() {
    let world = world(FakeDecoder::default());
    let id = add(
        &world,
        CaptureMetadata::default(),
        vec![asset("2026/IMG_2.CR3", b"raw-2")],
    );
    let thumbs = service(&world);
    let path = thumbs.cache_path(&fake_hash(b"raw-2"), ImageSize::Grid);
    // Another thread finished first.
    world.fs.write(&path, b"from the other thread");

    // A rendering that started before it existed must not replace it.
    let photo = world.catalog.photo(id).unwrap().unwrap();
    let got = thumbs
        .ensure(Path::new("/lib"), &photo, ImageSize::Grid, PairView::Image)
        .unwrap();

    assert_eq!(got, path);
    assert_eq!(world.fs.read(&path).unwrap(), b"from the other thread");
}
