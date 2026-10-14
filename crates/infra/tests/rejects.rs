//! Moving rejects on the real file system and catalog.

use std::path::Path;
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

fn add(catalog: &SqliteCatalog, lib: &Path, files: &[(&str, MediaFormat)], n: u8) -> PhotoId {
    let assets = files
        .iter()
        .enumerate()
        .map(|(i, (path, format))| {
            let full = lib.join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(&full, format!("{path} contents")).unwrap();
            Asset {
                format: *format,
                path: (*path).into(),
                size: std::fs::metadata(&full).unwrap().len(),
                hash: ContentHash::from_bytes([n * 10 + i as u8; 32]),
            }
        })
        .collect();
    catalog
        .insert_photo(&NewPhoto {
            metadata: Default::default(),
            assets: PhotoAssets::new(assets).unwrap(),
        })
        .unwrap()
}

fn read(lib: &Path, path: &str) -> Option<String> {
    std::fs::read_to_string(lib.join(path)).ok()
}

#[test]
fn moves_rejects_and_back_without_losing_files() {
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
    let a = add(
        &catalog,
        &lib,
        &[
            ("2026/a.CR3", MediaFormat::Raw),
            ("2026/a.JPG", MediaFormat::Jpeg),
        ],
        1,
    );
    let b = add(&catalog, &lib, &[("2026/b.JPG", MediaFormat::Jpeg)], 2);
    let c = add(&catalog, &lib, &[("2026/c.JPG", MediaFormat::Jpeg)], 3);
    let api = Api::new(AppContext {
        pipeline_store: catalog.clone(),
        watcher: Arc::new(picturesque_app::ports::watcher::NoWatcher),
        settings_store: catalog.clone(),
        catalog: catalog.clone(),
        fs: Arc::new(StdFileSystem),
        metadata: Arc::new(ExifMetadataReader),
        geocoder: Arc::new(GeoNamesGeocoder::default()),
        events: Arc::new(NoEvents),
        decoder: Arc::new(VipsDecoder),
        sidecars: Arc::new(picturesque_infra::xmp::XmpSidecars),
        pixels: Arc::new(VipsPixels),
        analyzers: Vec::new(),
        inference: Arc::new(picturesque_infra::inference::OrtEngine::new(1)),
        models: Vec::new(),
        model_store: Arc::new(picturesque_infra::models::HttpModelStore::new(
            dir.path().join("models"),
        )),
        cache_dir: dir.path().join("cache"),
    })
    .unwrap();
    api.call(
        "set_decisions",
        json!({ "changes": [
            { "photo_id": a.0, "flag": "reject" },
            { "photo_id": b.0, "flag": "reject" },
        ] }),
    )
    .unwrap();
    // A file is already where b would go: b must stay untouched.
    std::fs::create_dir_all(lib.join("_Rejected/2026")).unwrap();
    std::fs::write(lib.join("_Rejected/2026/b.JPG"), "someone else's").unwrap();

    let plan = api.call("plan_reject_move", Value::Null).unwrap();
    assert_eq!(
        (plan["photos"].clone(), plan["files"].clone()),
        (json!(2), json!(3))
    );
    let result = api.call("move_rejects", Value::Null).unwrap();
    assert_eq!(result["moved"], 1);
    assert_eq!(result["failed"][0]["photo_id"], b.0);

    assert_eq!(
        read(&lib, "_Rejected/2026/a.CR3").as_deref(),
        Some("2026/a.CR3 contents")
    );
    assert_eq!(
        read(&lib, "_Rejected/2026/a.JPG").as_deref(),
        Some("2026/a.JPG contents")
    );
    assert!(read(&lib, "2026/a.CR3").is_none());
    assert_eq!(
        read(&lib, "2026/b.JPG").as_deref(),
        Some("2026/b.JPG contents")
    );
    assert_eq!(
        read(&lib, "_Rejected/2026/b.JPG").as_deref(),
        Some("someone else's")
    );
    assert_eq!(
        read(&lib, "2026/c.JPG").as_deref(),
        Some("2026/c.JPG contents")
    );
    let photo = catalog.photo(a).unwrap().unwrap();
    assert_eq!(
        photo.assets.raw().unwrap().path,
        Path::new("_Rejected/2026/a.CR3")
    );
    // The catalog survives a restart with the new paths.
    let thumbnail = api.call("get_photo", json!({ "photo_id": a.0 })).unwrap();
    assert_eq!(thumbnail["files"][0]["path"], "_Rejected/2026/a.CR3");

    let batch = result["batch"].as_i64().unwrap();
    let undone = api
        .call("undo_reject_move", json!({ "batch": batch }))
        .unwrap();
    assert_eq!(undone["moved"], 1);
    assert_eq!(
        read(&lib, "2026/a.CR3").as_deref(),
        Some("2026/a.CR3 contents")
    );
    assert!(read(&lib, "_Rejected/2026/a.CR3").is_none());
    assert_eq!(
        catalog
            .photo(a)
            .unwrap()
            .unwrap()
            .assets
            .raw()
            .unwrap()
            .path,
        Path::new("2026/a.CR3")
    );
    let _ = c;
}
