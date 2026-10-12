use std::path::PathBuf;

use chrono::{FixedOffset, NaiveDate};
use picturesque_app::ports::catalog::{CatalogError, PhotoCatalog};
use picturesque_core::hash::ContentHash;
use picturesque_core::media::MediaFormat;
use picturesque_core::metadata::{
    Camera, CaptureMetadata, CaptureTime, Exposure, ExposureTime, GeoPoint, Orientation, Place,
};
use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets, PhotoId};
use picturesque_infra::catalog::{SCHEMA_VERSION, SqliteCatalog};

fn hash(n: u8) -> ContentHash {
    ContentHash::from_bytes([n; 32])
}

fn asset(path: &str, n: u8) -> Asset {
    Asset {
        format: MediaFormat::from_file_name(path).unwrap(),
        path: path.split('/').collect(),
        size: 1000 + u64::from(n),
        hash: hash(n),
    }
}

fn full_metadata() -> CaptureMetadata {
    CaptureMetadata {
        captured_at: Some(CaptureTime::new(
            NaiveDate::from_ymd_opt(2026, 10, 12)
                .unwrap()
                .and_hms_milli_opt(14, 30, 5, 250)
                .unwrap(),
            FixedOffset::east_opt(3600),
        )),
        camera: Some(Camera::new("Canon", "Canon EOS R5")),
        lens: Some("RF24-70mm F2.8 L IS USM".into()),
        exposure: Exposure {
            iso: Some(400),
            f_number: Some(2.8),
            exposure_time: Some(ExposureTime {
                numerator: 1,
                denominator: 250,
            }),
            focal_length_mm: Some(35.0),
        },
        gps: GeoPoint::new(38.7223, -9.1393),
        place: Some(Place {
            city: "Lisbon".into(),
            region: Some("Lisbon".into()),
            country: "Portugal".into(),
            country_code: "PT".into(),
        }),
        orientation: Orientation::Rotate90,
        dimensions: Some((8192, 5464)),
    }
}

fn photo(metadata: CaptureMetadata, assets: Vec<Asset>) -> NewPhoto {
    NewPhoto {
        metadata,
        assets: PhotoAssets::new(assets).unwrap(),
    }
}

#[test]
fn new_catalog_is_migrated_to_the_latest_schema() {
    let catalog = SqliteCatalog::open_in_memory().unwrap();

    assert_eq!(catalog.schema_version().unwrap(), SCHEMA_VERSION);
    assert_eq!(catalog.photo_count().unwrap(), 0);
}

#[test]
fn photo_round_trips_with_full_metadata_and_assets() {
    let catalog = SqliteCatalog::open_in_memory().unwrap();
    let new = photo(
        full_metadata(),
        vec![
            asset("2026/2026-10-12 Lisbon/IMG_1.CR3", 1),
            asset("2026/2026-10-12 Lisbon/IMG_1.JPG", 2),
            asset("2026/2026-10-12 Lisbon/IMG_1.CR3.xmp", 3),
        ],
    );

    let id = catalog.insert_photo(&new).unwrap();
    let stored = catalog.photo(id).unwrap().expect("photo exists");

    assert_eq!(stored.id, id);
    assert_eq!(stored.metadata, new.metadata);
    assert_eq!(stored.assets, new.assets);
    assert_eq!(
        stored.assets.raw().unwrap().path,
        PathBuf::from_iter(["2026", "2026-10-12 Lisbon", "IMG_1.CR3"])
    );
}

#[test]
fn photo_with_empty_metadata_round_trips() {
    let catalog = SqliteCatalog::open_in_memory().unwrap();
    let new = photo(CaptureMetadata::default(), vec![asset("unsorted/a.png", 9)]);

    let id = catalog.insert_photo(&new).unwrap();

    assert_eq!(
        catalog.photo(id).unwrap().unwrap().metadata,
        CaptureMetadata::default()
    );
}

#[test]
fn missing_photo_is_none() {
    let catalog = SqliteCatalog::open_in_memory().unwrap();

    assert_eq!(catalog.photo(PhotoId(42)).unwrap(), None);
}

#[test]
fn finds_photos_by_media_hash_but_not_by_sidecar_hash() {
    let catalog = SqliteCatalog::open_in_memory().unwrap();
    let id = catalog
        .insert_photo(&photo(
            CaptureMetadata::default(),
            vec![asset("a.NEF", 1), asset("a.JPG", 2), asset("a.xmp", 3)],
        ))
        .unwrap();

    assert_eq!(catalog.photo_with_media_hash(&hash(1)).unwrap(), Some(id));
    assert_eq!(catalog.photo_with_media_hash(&hash(2)).unwrap(), Some(id));
    assert_eq!(catalog.photo_with_media_hash(&hash(3)).unwrap(), None);
    assert_eq!(catalog.photo_with_media_hash(&hash(4)).unwrap(), None);
}

#[test]
fn duplicate_media_content_is_rejected_and_nothing_is_stored() {
    let catalog = SqliteCatalog::open_in_memory().unwrap();
    catalog
        .insert_photo(&photo(CaptureMetadata::default(), vec![asset("a.JPG", 1)]))
        .unwrap();

    let err = catalog
        .insert_photo(&photo(
            CaptureMetadata::default(),
            vec![asset("b.CR3", 2), asset("copy-of-a.JPG", 1)],
        ))
        .unwrap_err();

    assert!(matches!(err, CatalogError::Duplicate(_)), "{err:?}");
    assert_eq!(catalog.photo_count().unwrap(), 1);
    assert_eq!(catalog.photo_with_media_hash(&hash(2)).unwrap(), None);
}

#[test]
fn identical_sidecars_may_belong_to_different_photos() {
    let catalog = SqliteCatalog::open_in_memory().unwrap();
    catalog
        .insert_photo(&photo(
            CaptureMetadata::default(),
            vec![asset("a.JPG", 1), asset("a.xmp", 9)],
        ))
        .unwrap();

    catalog
        .insert_photo(&photo(
            CaptureMetadata::default(),
            vec![asset("b.JPG", 2), asset("b.xmp", 9)],
        ))
        .unwrap();

    assert_eq!(catalog.photo_count().unwrap(), 2);
}

#[test]
fn file_catalog_persists_and_reopens_without_remigrating() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("catalog.sqlite");
    let id = {
        let catalog = SqliteCatalog::open(&path).unwrap();
        catalog
            .insert_photo(&photo(full_metadata(), vec![asset("a.CR3", 1)]))
            .unwrap()
    };

    let reopened = SqliteCatalog::open(&path).unwrap();

    assert_eq!(reopened.schema_version().unwrap(), SCHEMA_VERSION);
    assert_eq!(
        reopened.photo(id).unwrap().unwrap().metadata,
        full_metadata()
    );
}

#[test]
fn refuses_a_catalog_from_a_newer_version() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("catalog.sqlite");
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
    }

    let err = SqliteCatalog::open(&path).unwrap_err();

    assert!(err.to_string().contains("newer"), "{err}");
}
