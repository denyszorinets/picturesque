//! Faces and closed eyes on real photos with the real models.
//!
//! Needs the YuNet detector and the face mesh (see `models::catalog`):
//!
//!     PICTURESQUE_FACE_DETECTOR=/path/face_detection_yunet_2023mar.onnx \
//!     PICTURESQUE_FACE_MESH=/path/mesh.onnx \
//!     cargo test -p picturesque-infra --test faces -- --ignored

use std::path::{Path, PathBuf};
use std::sync::Arc;

use picturesque_app::api::{Api, AppContext};
use picturesque_app::models::{FACE_DETECTOR_FILE, FACE_MESH_FILE, FACES};
use picturesque_app::ports::catalog::PhotoCatalog;
use picturesque_app::ports::events::NoEvents;
use picturesque_app::ports::models::{ModelFile, ModelSpec};
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

fn spec(models: &Path, files: &[(&str, PathBuf)]) -> ModelSpec {
    ModelSpec {
        id: FACES.into(),
        name: FACES.into(),
        purpose: String::new(),
        version: 1,
        license: "MIT".into(),
        license_url: String::new(),
        homepage: String::new(),
        files: files
            .iter()
            .map(|(name, source)| {
                let dest = models.join(FACES).join("v1").join(name);
                std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
                std::fs::copy(source, &dest).unwrap();
                ModelFile {
                    name: (*name).into(),
                    url: "http://127.0.0.1:9/unused".into(),
                    sha256: String::new(),
                    size: std::fs::metadata(source).unwrap().len(),
                }
            })
            .collect(),
    }
}

#[test]
#[ignore = "needs the face models (PICTURESQUE_FACE_DETECTOR, PICTURESQUE_FACE_MESH)"]
fn closed_eye_fixtures_are_detected() {
    let var = |name: &str| PathBuf::from(std::env::var_os(name).expect(name));
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("Clean");
    std::fs::create_dir_all(&lib).unwrap();
    let models = dir.path().join("models");
    let spec = spec(
        &models,
        &[
            (FACE_DETECTOR_FILE, var("PICTURESQUE_FACE_DETECTOR")),
            (FACE_MESH_FILE, var("PICTURESQUE_FACE_MESH")),
        ],
    );
    let catalog = Arc::new(SqliteCatalog::open_in_memory().unwrap());
    catalog
        .save_settings(&Settings {
            library_root: Some(lib.clone()),
            ..Default::default()
        })
        .unwrap();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/faces");
    let mut photos: Vec<(String, PhotoId)> = Vec::new();
    for (n, name) in [
        "closed_stdenis",
        "closed_veronez",
        "open_bennekom",
        "open_bruhl",
    ]
    .iter()
    .enumerate()
    {
        let file = format!("{name}.jpg");
        std::fs::copy(fixtures.join(&file), lib.join(&file)).unwrap();
        let id = catalog
            .insert_photo(&NewPhoto {
                metadata: Default::default(),
                assets: PhotoAssets::new(vec![Asset {
                    format: MediaFormat::Jpeg,
                    path: file.clone().into(),
                    size: std::fs::metadata(lib.join(&file)).unwrap().len(),
                    hash: ContentHash::from_bytes([n as u8 + 1; 32]),
                }])
                .unwrap(),
            })
            .unwrap();
        photos.push(((*name).to_owned(), id));
    }
    let api = Api::new(AppContext {
        pipeline_store: catalog.clone(),
        watcher: Arc::new(picturesque_app::ports::watcher::NoWatcher),
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
        analyzers: Vec::new(),
        inference: Arc::new(picturesque_infra::inference::OrtEngine::new(4)),
        models: vec![spec],
        model_store: Arc::new(picturesque_infra::models::HttpModelStore::new(models)),
        cache_dir: dir.path().join("cache"),
    })
    .unwrap();

    assert_eq!(api.run_analysis(), 4);
    for (name, id) in photos {
        let faces: Value =
            api.call("get_photo", json!({ "photo_id": id.0 })).unwrap()["faces"].clone();
        let faces = faces
            .as_array()
            .unwrap_or_else(|| panic!("{name}: {faces}"));
        assert!(!faces.is_empty(), "{name}: no face");
        let closed: Vec<&Value> = faces.iter().map(|f| &f["eyes_closed"]).collect();
        if name.starts_with("closed") {
            assert_eq!(closed[0], &json!(true), "{name}: {faces:?}");
        } else {
            assert!(
                closed.iter().all(|c| **c == json!(false)),
                "{name}: {faces:?}"
            );
        }
        for f in faces {
            assert!((0.0..=1.0).contains(&f["sharpness"].as_f64().unwrap()));
        }
    }
}
