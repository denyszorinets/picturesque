//! Background analysis with real decoding: technical scores separate a
//! sharp photo from a blurred copy.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use picturesque_app::api::{Api, AppContext};
use picturesque_app::ports::catalog::PhotoCatalog;
use picturesque_app::ports::events::NoEvents;
use picturesque_app::ports::settings::SettingsStore;
use picturesque_app::settings::Settings;
use picturesque_core::hash::ContentHash;
use picturesque_core::media::MediaFormat;
use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets, PhotoId};
use picturesque_infra::catalog::SqliteCatalog;
use picturesque_infra::fs::StdFileSystem;
use picturesque_infra::geocode::GeoNamesGeocoder;
use picturesque_infra::images::{VipsDecoder, VipsPixels};
use picturesque_infra::metadata::ExifMetadataReader;
use serde_json::{Value, json};

fn api(catalog: &Arc<SqliteCatalog>, cache_dir: PathBuf) -> Api {
    Api::new(AppContext {
        settings_store: catalog.clone(),
        catalog: catalog.clone(),
        imports: catalog.clone(),
        fs: Arc::new(StdFileSystem),
        metadata: Arc::new(ExifMetadataReader),
        geocoder: Arc::new(GeoNamesGeocoder::default()),
        events: Arc::new(NoEvents),
        decoder: Arc::new(VipsDecoder),
        sidecars: Arc::new(picturesque_infra::xmp::XmpSidecars),
        pixels: Arc::new(VipsPixels),
        analyzers: picturesque_app::analysis::default_analyzers(),
        cache_dir,
    })
    .unwrap()
}

/// A detailed scene (fine stripes and rings), like a sharp photo.
fn scene() -> image::RgbImage {
    image::RgbImage::from_fn(1200, 800, |x, y| {
        let stripes = if (x / 3 + y / 5) % 2 == 0 { 60 } else { 0 };
        let (dx, dy) = (x as f32 - 600.0, y as f32 - 400.0);
        let rings = if ((dx * dx + dy * dy).sqrt() as u32 / 6).is_multiple_of(2) {
            50
        } else {
            0
        };
        let v = (40 + x / 16 + stripes + rings).min(255) as u8;
        image::Rgb([v, v, v / 2 + 40])
    })
}

fn add(catalog: &SqliteCatalog, lib: &Path, name: &str, image: &image::RgbImage, n: u8) -> PhotoId {
    image.save(lib.join(name)).unwrap();
    let size = std::fs::metadata(lib.join(name)).unwrap().len();
    catalog
        .insert_photo(&NewPhoto {
            metadata: Default::default(),
            assets: PhotoAssets::new(vec![Asset {
                format: MediaFormat::Jpeg,
                path: name.into(),
                size,
                hash: ContentHash::from_bytes([n; 32]),
            }])
            .unwrap(),
        })
        .unwrap()
}

fn quality(api: &Api, id: PhotoId) -> Value {
    api.call("get_photo", json!({ "photo_id": id.0 })).unwrap()["quality"].clone()
}

#[test]
fn scores_separate_sharp_from_blurred_photos() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("Clean");
    std::fs::create_dir_all(&lib).unwrap();
    let catalog = Arc::new(SqliteCatalog::open_in_memory().unwrap());
    catalog
        .save_settings(&Settings {
            library_root: Some(lib.clone()),
            ..Default::default()
        })
        .unwrap();
    let sharp_image = scene();
    let sharp = add(&catalog, &lib, "sharp.jpg", &sharp_image, 1);
    let blurred = add(
        &catalog,
        &lib,
        "blurred.jpg",
        &image::imageops::blur(&sharp_image, 4.0),
        2,
    );
    let api = api(&catalog, dir.path().join("cache"));
    assert_eq!(quality(&api, sharp), Value::Null, "not analyzed yet");

    assert_eq!(api.run_analysis(), 2);

    let (a, b) = (quality(&api, sharp), quality(&api, blurred));
    let score = |q: &Value, k: &str| q[k].as_f64().unwrap();
    assert!(score(&a, "sharpness") > 0.9, "{a}");
    assert!(score(&b, "sharpness") < 0.3, "{b}");
    assert!(score(&a, "overall") > score(&b, "overall"));
    assert!(score(&a, "exposure") > 0.9, "{a}");
    assert_eq!(
        api.call("analysis_status", Value::Null).unwrap(),
        json!({ "analyzed": 2, "total": 2, "running": false })
    );
}
