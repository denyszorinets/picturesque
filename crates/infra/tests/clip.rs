//! CLIP embeddings end to end: decode, preprocess, ONNX Runtime, store,
//! similarity search.
//!
//! The regular test uses a tiny stand-in model (mean colour projected to
//! 768 dimensions). `real_clip_model_groups_similar_photos` runs the real
//! model when `PICTURESQUE_CLIP_MODEL` points at its `visual/model.onnx`:
//!
//!     PICTURESQUE_CLIP_MODEL=/path/to/model.onnx cargo test -p picturesque-infra --test clip -- --ignored
//!
//! `real_text_search_finds_described_photos` also needs
//! `PICTURESQUE_CLIP_TEXT_MODEL` and `PICTURESQUE_CLIP_MERGES`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use picturesque_app::api::{Api, AppContext};
use picturesque_app::models::{
    CLIP_IMAGE, CLIP_IMAGE_FILE, CLIP_MERGES_FILE, CLIP_TEXT, CLIP_TEXT_FILE,
};
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

/// A model spec for files already copied into the store.
fn installed(models: &Path, id: &str, files: &[(&str, &Path)]) -> ModelSpec {
    let files = files
        .iter()
        .map(|(name, source)| {
            let dest = models.join(id).join("v1").join(name);
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::copy(source, &dest).unwrap();
            ModelFile {
                name: (*name).into(),
                url: "http://127.0.0.1:9/unused".into(),
                sha256: String::new(),
                size: std::fs::metadata(source).unwrap().len(),
            }
        })
        .collect();
    ModelSpec {
        id: id.into(),
        name: id.into(),
        purpose: String::new(),
        version: 1,
        license: "MIT".into(),
        license_url: String::new(),
        homepage: String::new(),
        files,
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/onnx")
        .join(name)
}

/// A library whose CLIP image model is `model` and text model and merge
/// list `text`, already "downloaded".
fn library(model: &Path, text: Option<(&Path, &Path)>) -> Library {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("Clean");
    std::fs::create_dir_all(&lib).unwrap();
    let models = dir.path().join("models");
    let mut specs = vec![installed(&models, CLIP_IMAGE, &[(CLIP_IMAGE_FILE, model)])];
    if let Some((text_model, merges)) = text {
        specs.push(installed(
            &models,
            CLIP_TEXT,
            &[(CLIP_TEXT_FILE, text_model), (CLIP_MERGES_FILE, merges)],
        ));
    }
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
        models: specs,
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
    let l = library(model, None);
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
    let aesthetic = &l
        .api
        .call("get_photo", json!({ "photo_id": red.0 }))
        .unwrap()["aesthetic"];
    let score = aesthetic["score"].as_f64().unwrap();
    assert!((0.0..=1.0).contains(&score), "{aesthetic}");
    assert!(aesthetic["rating"].as_f64().unwrap().is_finite());
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
    ranks_like_colours_together(&fixture("tiny_clip.onnx"));
}

#[test]
#[ignore = "needs the real CLIP model (PICTURESQUE_CLIP_MODEL)"]
fn real_clip_model_groups_similar_photos() {
    let model = std::env::var_os("PICTURESQUE_CLIP_MODEL").expect("PICTURESQUE_CLIP_MODEL");
    ranks_like_colours_together(Path::new(&model));
}

#[test]
fn similar_photos_validates_its_arguments() {
    let l = library(&fixture("tiny_clip.onnx"), None);
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

fn search(api: &Api, text: &str) -> Result<Vec<(i64, f64)>, picturesque_app::api::ApiError> {
    Ok(api
        .call("search_photos", json!({ "text": text }))?
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["photo_id"].as_i64().unwrap(),
                s["similarity"].as_f64().unwrap(),
            )
        })
        .collect())
}

#[test]
fn text_search_runs_the_text_encoder() {
    let l = library(
        &fixture("tiny_clip.onnx"),
        Some((&fixture("tiny_clip_text.onnx"), &fixture("tiny_merges.txt"))),
    );
    let ids: Vec<i64> = (0..3)
        .map(|i| {
            add(
                &l,
                &format!("{i}.jpg"),
                &picture([200, 30 * i, 30], [20, 20, 20], i.into()),
                i,
            )
            .0
        })
        .collect();
    l.api.run_analysis();
    // The stand-in models do not understand text; every result must be a
    // photo above the relevance cut-off.
    for result in search(&l.api, "Red RED red!").unwrap() {
        assert!(ids.contains(&result.0));
        assert!(result.1 >= f64::from(picturesque_app::search::MIN_SIMILARITY));
    }
    let err = search(&l.api, " ").unwrap_err();
    assert_eq!(
        err.code,
        picturesque_app::api::ApiErrorCode::InvalidArguments
    );
}

#[test]
fn text_search_says_which_model_is_missing() {
    let l = library(&fixture("tiny_clip.onnx"), None);
    let err = search(&l.api, "red").unwrap_err();
    assert_eq!(
        err.code,
        picturesque_app::api::ApiErrorCode::InvalidArguments
    );
    assert!(err.message.contains("text encoder"), "{}", err.message);
}

#[test]
#[ignore = "needs the real CLIP models (PICTURESQUE_CLIP_MODEL, _TEXT_MODEL, _MERGES)"]
fn real_text_search_finds_described_photos() {
    let var = |name: &str| PathBuf::from(std::env::var_os(name).expect(name));
    let (image, text, merges) = (
        var("PICTURESQUE_CLIP_MODEL"),
        var("PICTURESQUE_CLIP_TEXT_MODEL"),
        var("PICTURESQUE_CLIP_MERGES"),
    );
    let l = library(&image, Some((&text, &merges)));
    let red = add(&l, "red.jpg", &picture([200, 30, 30], [120, 10, 10], 0), 1);
    let blue = add(
        &l,
        "blue.jpg",
        &picture([20, 40, 200], [200, 220, 255], 3),
        2,
    );
    l.api.run_analysis();
    let reds = search(&l.api, "red stripes").unwrap();
    assert_eq!(reds.first().map(|r| r.0), Some(red.0), "{reds:?}");
    let blues = search(&l.api, "blue and white waves").unwrap();
    assert_eq!(blues.first().map(|r| r.0), Some(blue.0), "{blues:?}");
}
