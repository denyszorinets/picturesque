//! Exporting with real files: verified originals, resized JPEGs that keep
//! their metadata and are upright.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use picturesque_app::api::{Api, AppContext};
use picturesque_app::ports::catalog::PhotoCatalog;
use picturesque_app::ports::events::NoEvents;
use picturesque_app::ports::metadata::MetadataReader;
use picturesque_app::ports::settings::SettingsStore;
use picturesque_app::settings::Settings;
use picturesque_core::hash::ContentHash;
use picturesque_core::media::MediaFormat;
use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets};
use picturesque_infra::catalog::SqliteCatalog;
use picturesque_infra::fs::StdFileSystem;
use picturesque_infra::geocode::GeoNamesGeocoder;
use picturesque_infra::images::{VipsDecoder, VipsPixels};
use picturesque_infra::metadata::ExifMetadataReader;
use serde_json::{Value, json};

fn wait(api: &Api) -> Value {
    let start = Instant::now();
    loop {
        let status = api.call("export_status", Value::Null).unwrap();
        if status["state"] != "running" {
            return status;
        }
        assert!(start.elapsed() < Duration::from_secs(30), "export hangs");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn orientation(path: &Path) -> Option<u32> {
    let file = std::fs::File::open(path).ok()?;
    let exif = exif::Reader::new()
        .read_from_container(&mut std::io::BufReader::new(file))
        .ok()?;
    exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)?
        .value
        .get_uint(0)
}

#[test]
fn exports_verified_originals_and_upright_jpegs_with_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("Clean");
    let out = dir.path().join("Export");
    std::fs::create_dir_all(lib.join("2026")).unwrap();
    std::fs::create_dir_all(&out).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/orient_6.jpg");
    std::fs::copy(&source, lib.join("2026/IMG_6.jpg")).unwrap();
    let bytes = std::fs::read(&source).unwrap();
    let catalog = Arc::new(SqliteCatalog::open_in_memory().unwrap());
    catalog
        .save_settings(&Settings {
            library_root: Some(lib.clone()),
            ..Default::default()
        })
        .unwrap();
    let metadata = ExifMetadataReader.read(&source, MediaFormat::Jpeg).unwrap();
    assert_eq!(metadata.orientation.to_exif(), 6);
    let id = catalog
        .insert_photo(&NewPhoto {
            metadata,
            assets: PhotoAssets::new(vec![Asset {
                format: MediaFormat::Jpeg,
                path: "2026/IMG_6.jpg".into(),
                size: bytes.len() as u64,
                hash: ContentHash::from_bytes(*blake3::hash(&bytes).as_bytes()),
            }])
            .unwrap(),
        })
        .unwrap();
    let api = Api::new(AppContext {
        pipeline_store: catalog.clone(),
        watcher: Arc::new(picturesque_app::ports::watcher::NoWatcher),
        critic: Arc::new(picturesque_infra::critic::HttpCritic::new()),
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
        json!({ "changes": [{ "photo_id": id.0, "flag": "pick" }] }),
    )
    .unwrap();

    // Originals: byte for byte.
    api.call("start_export", json!({ "destination": out }))
        .unwrap();
    let status = wait(&api);
    // The photo and a sidecar with its decision.
    assert_eq!(
        (status["state"].clone(), status["files"].clone()),
        (json!("completed"), json!(2))
    );
    assert_eq!(std::fs::read(out.join("IMG_6.jpg")).unwrap(), bytes);
    let sidecar = std::fs::read_to_string(out.join("IMG_6.jpg.xmp")).unwrap();
    assert!(sidecar.contains(r#"picturesque:Flag="pick""#), "{sidecar}");
    assert!(
        sidecar.contains("<rdf:li>2</rdf:li>"),
        "green label: {sidecar}"
    );

    // Resized JPEGs: upright, at most 100 px, metadata kept, named by
    // template, the earlier file untouched.
    api.call(
        "start_export",
        json!({
            "destination": out, "format": "jpeg", "max_edge": 100, "quality": 80,
            "naming": "small/{name}.{ext}",
        }),
    )
    .unwrap();
    let status = wait(&api);
    assert_eq!(status["files"], 1, "{status}");
    let small = out.join("small/IMG_6.jpg");
    let size = imagesize::size(&small).unwrap();
    let original = imagesize::size(&source).unwrap();
    // Orientation 6 turns the stored landscape into a portrait.
    assert_eq!(original.width > original.height, size.height > size.width);
    assert!(size.width.max(size.height) <= 100);
    assert_ne!(orientation(&small), Some(6), "would be rotated twice");
    assert!(
        exif::Reader::new()
            .read_from_container(&mut std::io::BufReader::new(
                std::fs::File::open(&small).unwrap()
            ))
            .is_ok(),
        "metadata kept"
    );
    assert_eq!(std::fs::read(out.join("IMG_6.jpg")).unwrap(), bytes);

    // Validation errors are invalid arguments.
    let err = api
        .call("start_export", json!({ "destination": lib.join("2026") }))
        .unwrap_err();
    assert_eq!(
        err.code,
        picturesque_app::api::ApiErrorCode::InvalidArguments
    );
}
