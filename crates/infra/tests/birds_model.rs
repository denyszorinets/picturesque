//! Bird ID end to end with the real WingCLIP model: decode, crop,
//! preprocess, ONNX Runtime (with the weights in the `.data` file beside
//! the model), rank. Runs only when pointed at the downloaded files and a
//! photo of a bird:
//!
//!     PICTURESQUE_WINGCLIP_DIR=/path/to/WingCLIP-0.3 \
//!     PICTURESQUE_BIRD_PHOTO=/path/to/robin.jpg \
//!     PICTURESQUE_BIRD_EXPECT=amerob \
//!     cargo test -p picturesque-infra --test birds_model -- --ignored --nocapture
//!
//! `PICTURESQUE_BIRD_EXPECT` (an eBird code) is optional; without it the
//! suggestions are only printed.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use picturesque_app::api::{Api, AppContext};
use picturesque_app::models::{
    BIRD_DETECTOR, BIRD_DETECTOR_FILE, BIRDS, BIRDS_CLASSIFIER_FILE, BIRDS_LABELS_FILE,
    BIRDS_MODEL_FILE, BIRDS_WEIGHTS_FILE,
};
use picturesque_app::ports::catalog::PhotoCatalog;
use picturesque_app::ports::events::NoEvents;
use picturesque_app::ports::models::{ModelFile, ModelSpec};
use picturesque_app::ports::settings::SettingsStore;
use picturesque_app::settings::Settings;
use picturesque_core::hash::ContentHash;
use picturesque_core::media::MediaFormat;
use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets};
use picturesque_infra::catalog::SqliteCatalog;
use serde_json::json;

/// The model's files copied into a model store, as if downloaded.
fn installed(models: &Path, source: &Path) -> ModelSpec {
    let files = [
        BIRDS_MODEL_FILE,
        BIRDS_WEIGHTS_FILE,
        BIRDS_CLASSIFIER_FILE,
        BIRDS_LABELS_FILE,
    ]
    .iter()
    .map(|name| {
        let from = source.join(name);
        let dest = models.join(BIRDS).join("v1").join(name);
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::copy(&from, &dest).unwrap();
        ModelFile {
            name: (*name).into(),
            url: "http://127.0.0.1:9/unused".into(),
            sha256: String::new(),
            size: std::fs::metadata(&from).unwrap().len(),
        }
    })
    .collect();
    ModelSpec {
        id: BIRDS.into(),
        name: BIRDS.into(),
        purpose: String::new(),
        version: 1,
        license: "CC-BY-NC-4.0".into(),
        license_url: String::new(),
        homepage: String::new(),
        files,
    }
}

#[test]
#[ignore = "needs WingCLIP (PICTURESQUE_WINGCLIP_DIR) and a bird photo (PICTURESQUE_BIRD_PHOTO)"]
fn real_model_identifies_a_bird() {
    let (Some(model_dir), Some(photo)) = (
        std::env::var_os("PICTURESQUE_WINGCLIP_DIR"),
        std::env::var_os("PICTURESQUE_BIRD_PHOTO"),
    ) else {
        panic!("set PICTURESQUE_WINGCLIP_DIR and PICTURESQUE_BIRD_PHOTO");
    };
    let photo = PathBuf::from(photo);
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::copy(&photo, lib.join("bird.jpg")).unwrap();
    let spec = installed(&dir.path().join("models"), Path::new(&model_dir));
    let catalog = Arc::new(SqliteCatalog::open_in_memory().unwrap());
    catalog
        .save_settings(&Settings {
            library_root: Some(lib.clone()),
            ..Default::default()
        })
        .unwrap();
    let id = catalog
        .insert_photo(&NewPhoto {
            metadata: Default::default(),
            assets: PhotoAssets::new(vec![Asset {
                format: MediaFormat::Jpeg,
                path: "bird.jpg".into(),
                size: std::fs::metadata(lib.join("bird.jpg")).unwrap().len(),
                hash: ContentHash::from_bytes([1; 32]),
            }])
            .unwrap(),
        })
        .unwrap();
    let api = Api::new(AppContext {
        pipeline_store: catalog.clone(),
        watcher: Arc::new(picturesque_app::ports::watcher::NoWatcher),
        critic: Arc::new(picturesque_infra::critic::CriticAdapter::new()),
        power: Arc::new(picturesque_infra::power::SystemPower),
        species_lists: Arc::new(picturesque_infra::birds::EbirdSpeciesLists::default()),
        settings_store: catalog.clone(),
        catalog: catalog.clone(),
        fs: Arc::new(picturesque_infra::fs::StdFileSystem),
        metadata: Arc::new(picturesque_infra::metadata::ExifMetadataReader),
        geocoder: Arc::new(picturesque_infra::geocode::GeoNamesGeocoder::default()),
        events: Arc::new(NoEvents),
        decoder: Arc::new(picturesque_infra::images::VipsDecoder),
        sidecars: Arc::new(picturesque_infra::xmp::XmpSidecars),
        pixels: Arc::new(picturesque_infra::images::VipsPixels),
        analyzers: Vec::new(),
        inference: Arc::new(picturesque_infra::inference::OrtEngine::new(2)),
        models: vec![spec],
        model_store: Arc::new(picturesque_infra::models::HttpModelStore::new(
            dir.path().join("models"),
        )),
        cache_dir: dir.path().join("cache"),
    })
    .unwrap();

    let started = std::time::Instant::now();
    let result = api
        .call(
            "identify_bird",
            json!({ "photo_id": id.0, "box": { "x": 0.0, "y": 0.0, "width": 1.0, "height": 1.0 } }),
        )
        .unwrap();
    println!("identified in {:?}", started.elapsed());
    for s in result["suggestions"].as_array().unwrap() {
        println!(
            "{:>6.1}%  {} ({}, {})",
            s["confidence"].as_f64().unwrap() * 100.0,
            s["species"]["common"],
            s["species"]["scientific"],
            s["species"]["code"]
        );
    }
    assert_eq!(result["suggestions"].as_array().unwrap().len(), 5);
    if let Ok(expected) = std::env::var("PICTURESQUE_BIRD_EXPECT") {
        assert_eq!(
            result["suggestions"][0]["species"]["code"],
            expected.as_str()
        );
    }

    // With the RT-DETR bird finder (PICTURESQUE_BIRD_DETECTOR, its
    // model.onnx): every bird found, each identified.
    let Some(detector) = std::env::var_os("PICTURESQUE_BIRD_DETECTOR") else {
        return;
    };
    let models = dir.path().join("models");
    let dest = models
        .join(BIRD_DETECTOR)
        .join("v1")
        .join(BIRD_DETECTOR_FILE);
    std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
    std::fs::copy(&detector, &dest).unwrap();
    let finder = ModelSpec {
        id: BIRD_DETECTOR.into(),
        name: BIRD_DETECTOR.into(),
        purpose: String::new(),
        version: 1,
        license: "Apache-2.0".into(),
        license_url: String::new(),
        homepage: String::new(),
        files: vec![ModelFile {
            name: BIRD_DETECTOR_FILE.into(),
            url: "http://127.0.0.1:9/unused".into(),
            sha256: String::new(),
            size: std::fs::metadata(&dest).unwrap().len(),
        }],
    };
    let api = Api::new(AppContext {
        models: vec![installed(&models, Path::new(&model_dir)), finder],
        model_store: Arc::new(picturesque_infra::models::HttpModelStore::new(models)),
        ..context(&catalog, dir.path())
    })
    .unwrap();
    let started = std::time::Instant::now();
    let found = api.call("find_birds", json!({ "photo_id": id.0 })).unwrap();
    println!("found in {:?}", started.elapsed());
    let birds = found["birds"].as_array().unwrap();
    for (n, b) in birds.iter().enumerate() {
        println!(
            "bird {} ({:.0}% sure) at {:.2},{:.2} {:.2}x{:.2}: {} {:.0}%",
            n + 1,
            b["score"].as_f64().unwrap() * 100.0,
            b["box"]["x"].as_f64().unwrap(),
            b["box"]["y"].as_f64().unwrap(),
            b["box"]["width"].as_f64().unwrap(),
            b["box"]["height"].as_f64().unwrap(),
            b["suggestions"][0]["species"]["common"],
            b["suggestions"][0]["confidence"].as_f64().unwrap() * 100.0
        );
    }
    let min: usize = std::env::var("PICTURESQUE_BIRD_COUNT")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(1);
    assert!(birds.len() >= min, "found {} birds", birds.len());
}

/// Infrastructure for an `Api` over `catalog`, without models.
fn context(catalog: &Arc<SqliteCatalog>, dir: &Path) -> AppContext {
    AppContext {
        pipeline_store: catalog.clone(),
        watcher: Arc::new(picturesque_app::ports::watcher::NoWatcher),
        critic: Arc::new(picturesque_infra::critic::CriticAdapter::new()),
        power: Arc::new(picturesque_infra::power::SystemPower),
        species_lists: Arc::new(picturesque_infra::birds::EbirdSpeciesLists::default()),
        settings_store: catalog.clone(),
        catalog: catalog.clone(),
        fs: Arc::new(picturesque_infra::fs::StdFileSystem),
        metadata: Arc::new(picturesque_infra::metadata::ExifMetadataReader),
        geocoder: Arc::new(picturesque_infra::geocode::GeoNamesGeocoder::default()),
        events: Arc::new(NoEvents),
        decoder: Arc::new(picturesque_infra::images::VipsDecoder),
        sidecars: Arc::new(picturesque_infra::xmp::XmpSidecars),
        pixels: Arc::new(picturesque_infra::images::VipsPixels),
        analyzers: Vec::new(),
        inference: Arc::new(picturesque_infra::inference::OrtEngine::new(2)),
        models: Vec::new(),
        model_store: Arc::new(picturesque_infra::models::HttpModelStore::new(
            dir.join("models"),
        )),
        cache_dir: dir.join("cache"),
    }
}
