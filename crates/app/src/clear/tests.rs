use std::path::{Path, PathBuf};

use picturesque_core::decision::{Decision, Flag};
use picturesque_core::media::MediaFormat;
use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets, PhotoId};

use super::*;
use crate::ports::catalog::{CopyRecord, PhotoCatalog};
use crate::ports::fs::FileSystem;
use crate::ports::settings::SettingsStore;
use crate::settings::{DeleteAfterCopy, Settings};
use crate::testing::{FakeMetadata, MemFs, TestWorld, fake_hash};

fn asset(path: &str, format: MediaFormat, bytes: &[u8]) -> Asset {
    Asset {
        format,
        path: PathBuf::from(path),
        size: bytes.len() as u64,
        hash: fake_hash(bytes),
    }
}

struct Card {
    world: TestWorld,
    service: ClearService,
    /// A RAW+JPEG pair with a sidecar, copied.
    pair: PhotoId,
    /// A JPEG copied, then its copy changed.
    tampered: PhotoId,
    /// A rejected JPEG, not copied.
    reject: PhotoId,
    /// An unflagged JPEG, not copied.
    other: PhotoId,
    /// A RAW+JPEG pair copied without its RAW.
    half: PhotoId,
}

fn card(mode: DeleteAfterCopy) -> Card {
    let fs = MemFs::new()
        .file("/card/A.CR3", b"raw a")
        .file("/card/A.JPG", b"jpg a")
        .file("/card/A.CR3.xmp", b"xmp a")
        .file("/card/B.JPG", b"jpg b")
        .file("/card/C.JPG", b"jpg c")
        .file("/card/D.JPG", b"jpg d")
        .file("/card/E.CR3", b"raw e")
        .file("/card/E.JPG", b"jpg e")
        .file("/out/A.CR3", b"raw a")
        .file("/out/A.JPG", b"jpg a")
        .file("/out/B.JPG", b"changed!")
        .file("/out/E.JPG", b"jpg e");
    let world = TestWorld::new(fs, FakeMetadata::default());
    world
        .settings
        .save_settings(&Settings {
            library_root: Some("/card".into()),
            open_folder: Some("/card".into()),
            delete_after_copy: mode,
            ..Default::default()
        })
        .unwrap();
    let add = |assets: Vec<Asset>| {
        world
            .catalog
            .insert_photo(&NewPhoto {
                metadata: Default::default(),
                assets: PhotoAssets::new(assets).unwrap(),
            })
            .unwrap()
    };
    let pair = add(vec![
        asset("A.CR3", MediaFormat::Raw, b"raw a"),
        asset("A.JPG", MediaFormat::Jpeg, b"jpg a"),
        asset("A.CR3.xmp", MediaFormat::Xmp, b"xmp a"),
    ]);
    let tampered = add(vec![asset("B.JPG", MediaFormat::Jpeg, b"jpg b")]);
    let reject = add(vec![asset("C.JPG", MediaFormat::Jpeg, b"jpg c")]);
    let other = add(vec![asset("D.JPG", MediaFormat::Jpeg, b"jpg d")]);
    let half = add(vec![
        asset("E.CR3", MediaFormat::Raw, b"raw e"),
        asset("E.JPG", MediaFormat::Jpeg, b"jpg e"),
    ]);
    world
        .catalog
        .set_decision(
            reject,
            Decision {
                flag: Flag::Reject,
                ..Default::default()
            },
        )
        .unwrap();
    let copy = |id, source: &str, bytes: &[u8]| CopyRecord {
        photo_id: id,
        source: source.into(),
        destination: Path::new("/out").join(source),
        hash: fake_hash(bytes),
        size: bytes.len() as u64,
    };
    world
        .catalog
        .record_copies(&[
            copy(pair, "A.CR3", b"raw a"),
            copy(pair, "A.JPG", b"jpg a"),
            copy(tampered, "B.JPG", b"jpg b"),
            copy(half, "E.JPG", b"jpg e"),
        ])
        .unwrap();
    let service = ClearService::new(
        world.catalog.clone(),
        world.fs.clone(),
        world.settings.clone(),
    );
    Card {
        world,
        service,
        pair,
        tampered,
        reject,
        other,
        half,
    }
}

fn exists(card: &Card, path: &str) -> bool {
    card.world.fs.exists(Path::new(path))
}

#[test]
fn the_plan_counts_what_would_be_deleted() {
    let card = card(DeleteAfterCopy::Verified);

    let plan = card.service.plan().unwrap();

    assert_eq!(
        plan,
        ClearPlan {
            mode: DeleteAfterCopy::Verified,
            photos: 2,
            files: 4,
            bytes: 20,
            rejects: 0,
            partly_copied: 1,
        }
    );
    let with_rejects = self::card(DeleteAfterCopy::VerifiedAndRejects)
        .service
        .plan()
        .unwrap();
    assert_eq!((with_rejects.photos, with_rejects.rejects), (3, 1));
}

#[test]
fn deletes_only_photos_whose_copies_are_verified_again() {
    let card = card(DeleteAfterCopy::Verified);

    let result = card.service.clear().unwrap();

    assert_eq!((result.deleted, result.files), (1, 3));
    for gone in ["/card/A.CR3", "/card/A.JPG", "/card/A.CR3.xmp"] {
        assert!(!exists(&card, gone), "{gone}");
    }
    assert!(card.world.catalog.photo(card.pair).unwrap().is_none());
    // The copy changed after copying: kept, and said why.
    assert!(exists(&card, "/card/B.JPG"));
    assert_eq!(result.kept.len(), 1);
    assert_eq!(result.kept[0].photo_id, card.tampered.0);
    assert!(
        result.kept[0].reason.contains("changed"),
        "{:?}",
        result.kept
    );
    // Never copied, or copied without its RAW: untouched.
    for kept in ["/card/C.JPG", "/card/D.JPG", "/card/E.CR3", "/card/E.JPG"] {
        assert!(exists(&card, kept), "{kept}");
    }
    assert!(card.world.catalog.photo(card.other).unwrap().is_some());
    assert!(card.world.catalog.photo(card.half).unwrap().is_some());
    // The copies are untouched.
    assert!(exists(&card, "/out/A.CR3"));
}

#[test]
fn rejects_go_only_when_the_setting_says_so() {
    let card = card(DeleteAfterCopy::VerifiedAndRejects);

    let result = card.service.clear().unwrap();

    assert_eq!(result.deleted, 2);
    assert!(!exists(&card, "/card/C.JPG"));
    assert!(card.world.catalog.photo(card.reject).unwrap().is_none());
    assert!(exists(&card, "/card/D.JPG"));
}

#[test]
fn never_deletes_nothing() {
    let card = card(DeleteAfterCopy::Never);

    assert!(matches!(card.service.clear(), Err(ClearError::Invalid(_))));
    assert!(exists(&card, "/card/A.JPG"));
}

#[test]
fn the_library_is_never_cleared() {
    let card = card(DeleteAfterCopy::Verified);
    let mut settings = card.world.settings.load_settings().unwrap();
    settings.open_folder = None;
    card.world.settings.save_settings(&settings).unwrap();

    assert!(matches!(card.service.clear(), Err(ClearError::Invalid(_))));
    assert!(exists(&card, "/card/A.JPG"));
}

#[test]
fn a_missing_copy_keeps_the_original() {
    let card = card(DeleteAfterCopy::Verified);
    card.world.fs.delete(Path::new("/out/A.JPG"));

    let result = card.service.clear().unwrap();

    assert_eq!(result.deleted, 0);
    assert!(exists(&card, "/card/A.CR3"));
    assert!(exists(&card, "/card/A.JPG"));
    assert!(
        result.kept.iter().any(|k| k.photo_id == card.pair.0),
        "{:?}",
        result.kept
    );
}

#[test]
fn a_file_that_cannot_be_deleted_keeps_its_photo() {
    let card = card(DeleteAfterCopy::Verified);
    card.world.fs.lock_folder("/card");

    let result = card.service.clear().unwrap();

    assert_eq!(result.deleted, 0);
    assert!(exists(&card, "/card/A.CR3"));
    assert!(card.world.catalog.photo(card.pair).unwrap().is_some());
    let kept = result
        .kept
        .iter()
        .find(|k| k.photo_id == card.pair.0)
        .unwrap();
    assert!(kept.reason.contains("cannot delete"), "{}", kept.reason);
}
