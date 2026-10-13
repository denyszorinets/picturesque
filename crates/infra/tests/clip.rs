//! CLIP embeddings end to end: decode, preprocess, ONNX Runtime, store,
//! similarity search.
//!
//! The regular test uses a tiny stand-in model (mean colour projected to
//! 512 dimensions). `real_clip_model_groups_similar_photos` runs the real
//! model when `PICTURESQUE_CLIP_MODEL` points at its `visual/model.onnx`:
//!
//!     PICTURESQUE_CLIP_MODEL=/path/to/model.onnx cargo test -p picturesque-infra --test clip -- --ignored

use std::path::{Path, PathBuf};
use std::sync::Arc;

use picturesque_app::api::{Api, AppContext};
use picturesque_app::models::{CLIP_IMAGE, CLIP_IMAGE_FILE};
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

struct Library {
    _dir: tempfile::TempDir,
    lib: PathBuf,
    catalog: Arc<SqliteCatalog>,
    api: Api,
}

/// A library whose CLIP model is `model`, already "downloaded".
fn library(model: &Path) -> Library {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("Clean");
    std::fs::create_dir_all(&lib).unwrap();
    let models = dir.path().join("models");
    let installed = models.join(CLIP_IMAGE).join("v1").join(CLIP_IMAGE_FILE);
    std::fs::create_dir_all(installed.parent().unwrap()).unwrap();
    std::fs::copy(model, &installed).unwrap();
    let spec = ModelSpec {
        id: CLIP_IMAGE.into(),
        name: "CLIP".into(),
        purpose: String::new(),
        version: 1,
        license: "MIT".into(),
        license_url: String::new(),
        homepage: String::new(),
        files: vec![ModelFile {
            name: CLIP_IMAGE_FILE.into(),
            url: "http://127.0.0.1:9/unused".into(),
            sha256: String::new(),
            size: std::fs::metadata(model).unwrap().len(),
        }],
    };
    let catalog = Arc::new(SqliteCatalog::open_in_memory().unwrap());
    catalog
        .save_settings(&Settings {
            library_root: Some(lib.clone()),
            ..Default::default()
        })
        .unwrap();
    let api = Api::new(AppContext {
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
        inference: Arc::new(picturesque_infra::inference::OrtEngine::new(2)),
        models: vec![spec],
        model_store: Arc::new(picturesque_infra::models::HttpModelStore::new(models)),
        cache_dir: dir.path().join("cache"),
    })
    .unwrap();
    Library {
        _dir: dir,
        lib,
        catalog,
        api,
    }
}

fn add(l: &Library, name: &str, image: &image::RgbImage, n: u8) -> PhotoId {
    image.save(l.lib.join(name)).unwrap();
    let size = std::fs::metadata(l.lib.join(name)).unwrap().len();
    l.catalog
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

/// Wavy stripes in the given colours, with a little variation by `seed`.
fn picture(a: [u8; 3], b: [u8; 3], seed: u32) -> image::RgbImage {
    image::RgbImage::from_fn(900, 600, |x, y| {
        let wave = ((x as f32 / (40.0 + seed as f32)).sin() * 30.0) as i32;
        if ((y as i32 + wave) / 50) % 2 == 0 {
            image::Rgb(a)
        } else {
            image::Rgb(b)
        }
    })
}

fn similar(api: &Api, id: PhotoId) -> Vec<(i64, f64)> {
    api.call("similar_photos", json!({ "photo_id": id.0 }))
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["photo_id"].as_i64().unwrap(),
                s["similarity"].as_f64().unwrap(),
            )
        })
        .collect()
}

fn ranks_like_colours_together(model: &Path) {
    let l = library(model);
    let red = add(&l, "red.jpg", &picture([200, 30, 30], [120, 10, 10], 0), 1);
    let red2 = add(&l, "red2.jpg", &picture([210, 40, 35], [110, 20, 15], 9), 2);
    let blue = add(
        &l,
        "blue.jpg",
        &picture([20, 40, 200], [200, 220, 255], 3),
        3,
    );
    assert!(similar(&l.api, red).is_empty(), "not analyzed yet");

    assert_eq!(l.api.run_analysis(), 3);
    let ranked = similar(&l.api, red);
    assert_eq!(
        ranked.iter().map(|r| r.0).collect::<Vec<_>>(),
        [red2.0, blue.0]
    );
    assert!(ranked[0].1 > ranked[1].1 + 0.05, "{ranked:?}");
    assert_eq!(
        l.api.call("analysis_status", Value::Null).unwrap(),
        json!({ "analyzed": 3, "total": 3, "running": false })
    );
}

#[test]
fn embeds_photos_and_finds_similar_ones() {
    ranks_like_colours_together(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/onnx/tiny_clip.onnx"),
    );
}

#[test]
#[ignore = "needs the real CLIP model (PICTURESQUE_CLIP_MODEL)"]
fn real_clip_model_groups_similar_photos() {
    let model = std::env::var_os("PICTURESQUE_CLIP_MODEL").expect("PICTURESQUE_CLIP_MODEL");
    ranks_like_colours_together(Path::new(&model));
}

#[test]
fn similar_photos_validates_its_arguments() {
    let l =
        library(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/onnx/tiny_clip.onnx"));
    let err = l
        .api
        .call("similar_photos", json!({ "photo_id": 99 }))
        .unwrap_err();
    assert_eq!(err.code, picturesque_app::api::ApiErrorCode::NotFound);
    let id = add(&l, "x.jpg", &picture([1, 2, 3], [4, 5, 6], 0), 1);
    let err = l
        .api
        .call("similar_photos", json!({ "photo_id": id.0, "limit": 0 }))
        .unwrap_err();
    assert_eq!(
        err.code,
        picturesque_app::api::ApiErrorCode::InvalidArguments
    );
}
