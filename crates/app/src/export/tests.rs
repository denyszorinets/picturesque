use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::NaiveDate;
use picturesque_core::decision::{Decision, Flag};
use picturesque_core::media::MediaFormat;
use picturesque_core::metadata::{CaptureMetadata, CaptureTime};
use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets, PhotoId};

use super::*;
use crate::settings::Settings;
use crate::testing::{FakeMetadata, MemFs, TestWorld, fake_hash};

fn asset(path: &str, format: MediaFormat, bytes: &[u8]) -> Asset {
    Asset {
        format,
        path: PathBuf::from(path),
        size: bytes.len() as u64,
        hash: fake_hash(bytes),
    }
}

/// A picked RAW+JPEG+XMP photo, a picked JPEG and an unpicked JPEG.
fn world() -> (TestWorld, Arc<ExportService>, Vec<PhotoId>) {
    let fs = MemFs::new()
        .file("/lib/2026/a.CR3", b"raw a")
        .file("/lib/2026/a.JPG", b"jpg a")
        .file("/lib/2026/a.CR3.xmp", b"xmp a")
        .file("/lib/2026/b.JPG", b"jpg b")
        .file("/lib/2026/c.JPG", b"jpg c")
        .dir("/out");
    let world = TestWorld::new(fs, FakeMetadata::default());
    world
        .settings
        .save_settings(&Settings {
            library_root: Some("/lib".into()),
            ..Default::default()
        })
        .unwrap();
    let day = NaiveDate::from_ymd_opt(2026, 10, 12).unwrap();
    let add = |assets: Vec<Asset>, hour: u32| {
        world
            .catalog
            .insert_photo(&NewPhoto {
                metadata: CaptureMetadata {
                    captured_at: Some(CaptureTime::new(day.and_hms_opt(hour, 0, 0).unwrap(), None)),
                    ..Default::default()
                },
                assets: PhotoAssets::new(assets).unwrap(),
            })
            .unwrap()
    };
    let a = add(
        vec![
            asset("2026/a.CR3", MediaFormat::Raw, b"raw a"),
            asset("2026/a.JPG", MediaFormat::Jpeg, b"jpg a"),
            asset("2026/a.CR3.xmp", MediaFormat::Xmp, b"xmp a"),
        ],
        9,
    );
    let b = add(vec![asset("2026/b.JPG", MediaFormat::Jpeg, b"jpg b")], 10);
    let c = add(vec![asset("2026/c.JPG", MediaFormat::Jpeg, b"jpg c")], 11);
    for id in [a, b] {
        world
            .catalog
            .set_decision(
                id,
                Decision {
                    flag: Flag::Pick,
                    ..Default::default()
                },
            )
            .unwrap();
    }
    let service = Arc::new(ExportService::new(
        world.catalog.clone(),
        world.fs.clone(),
        world.decoder.clone(),
        world.pixels.clone(),
        world.settings.clone(),
        world.events.clone(),
    ));
    (world, service, vec![a, b, c])
}

fn args(format: ExportFormat) -> ExportArgs {
    ExportArgs {
        destination: "/out".into(),
        query: None,
        format: Some(format),
        naming: None,
        include_raw: None,
        include_sidecars: None,
        max_edge: None,
        quality: None,
    }
}

fn read(world: &TestWorld, path: &str) -> Option<Vec<u8>> {
    world.fs.read(Path::new(path))
}

#[test]
fn exports_the_picks_originals_with_raw_and_sidecars() {
    let (world, service, _) = world();
    let status = service.run_now(args(ExportFormat::Original)).unwrap();
    assert_eq!(status.state, ExportState::Completed);
    assert_eq!((status.done, status.total, status.files), (2, 2, 4));
    assert!(status.failed.is_empty());
    assert_eq!(read(&world, "/out/a.JPG").unwrap(), b"jpg a");
    assert_eq!(read(&world, "/out/a.CR3").unwrap(), b"raw a");
    assert_eq!(read(&world, "/out/a.CR3.xmp").unwrap(), b"xmp a");
    assert_eq!(read(&world, "/out/b.JPG").unwrap(), b"jpg b");
    assert!(read(&world, "/out/c.JPG").is_none(), "not a pick");
}

#[test]
fn names_follow_the_template_and_never_overwrite() {
    let (world, service, _) = world();
    world
        .fs
        .write(Path::new("/out/2026-10-12/0001-a.JPG"), b"already here");
    let status = service
        .run_now(ExportArgs {
            naming: Some("{date}/{seq}-{name}.{ext}".into()),
            include_raw: Some(false),
            include_sidecars: Some(false),
            ..args(ExportFormat::Original)
        })
        .unwrap();
    assert_eq!(status.files, 2, "{status:?}");
    assert_eq!(
        read(&world, "/out/2026-10-12/0001-a.JPG").unwrap(),
        b"already here"
    );
    assert_eq!(
        read(&world, "/out/2026-10-12/0001-a (2).JPG").unwrap(),
        b"jpg a"
    );
    assert_eq!(
        read(&world, "/out/2026-10-12/0002-b.JPG").unwrap(),
        b"jpg b"
    );
    assert!(read(&world, "/out/2026-10-12/0001-a (2).CR3").is_none());
}

#[test]
fn a_changed_original_fails_and_leaves_nothing_behind() {
    let (world, service, ids) = world();
    world.fs.write(Path::new("/lib/2026/a.CR3"), b"tampered!");
    let status = service.run_now(args(ExportFormat::Original)).unwrap();
    assert_eq!(status.files, 1);
    assert_eq!(status.failed.len(), 1);
    assert_eq!(status.failed[0].photo_id, ids[0].0);
    assert!(status.failed[0].error.contains("changed"));
    for f in ["/out/a.JPG", "/out/a.CR3", "/out/a.CR3.xmp"] {
        assert!(read(&world, f).is_none(), "{f}");
    }
}

#[test]
fn exports_resized_jpegs() {
    let (world, service, _) = world();
    let status = service
        .run_now(ExportArgs {
            max_edge: Some(1600),
            query: Some(PhotoQuery::default()),
            ..args(ExportFormat::Jpeg)
        })
        .unwrap();
    assert_eq!((status.done, status.files), (3, 3));
    assert_eq!(read(&world, "/out/a.jpg").unwrap(), b"jpeg 1600");
    assert!(read(&world, "/out/c.jpg").is_some());
    assert!(read(&world, "/out/a.CR3").is_none());
    let calls = world.decoder.calls.lock().unwrap().clone();
    assert!(calls.contains(&(PathBuf::from("/lib/2026/a.JPG"), 1600)));
}

#[test]
fn validates_the_destination_and_options() {
    let (world, service, _) = world();
    let invalid = |a: ExportArgs| matches!(service.run_now(a), Err(ExportError::Invalid(_)));
    assert!(invalid(ExportArgs {
        destination: "out".into(),
        ..args(ExportFormat::Original)
    }));
    assert!(invalid(ExportArgs {
        destination: "/missing".into(),
        ..args(ExportFormat::Original)
    }));
    world.fs.create_dir_all(Path::new("/lib/export")).unwrap();
    assert!(invalid(ExportArgs {
        destination: "/lib/export".into(),
        ..args(ExportFormat::Original)
    }));
    assert!(invalid(ExportArgs {
        naming: Some("no-extension".into()),
        ..args(ExportFormat::Original)
    }));
    assert!(invalid(ExportArgs {
        quality: Some(0),
        ..args(ExportFormat::Jpeg)
    }));
}
