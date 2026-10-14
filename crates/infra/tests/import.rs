//! End-to-end import with every real adapter: std file system, SQLite
//! catalog, EXIF reader and GeoNames geocoder.

mod support;

use std::path::Path;
use std::sync::Arc;

use exif::Tag;
use picturesque_app::api::{Api, AppContext};
use picturesque_app::ports::catalog::PhotoCatalog;
use picturesque_app::ports::events::NoEvents;
use picturesque_app::ports::settings::SettingsStore;
use picturesque_app::settings::Settings;
use picturesque_infra::catalog::SqliteCatalog;
use picturesque_infra::fs::StdFileSystem;
use picturesque_infra::geocode::GeoNamesGeocoder;
use picturesque_infra::images::VipsDecoder;
use picturesque_infra::metadata::ExifMetadataReader;
use serde_json::{Value, json};
use support::{ExifValue::*, write_jpeg};

fn api(catalog: &Arc<SqliteCatalog>) -> Api {
    let cache_dir =
        std::env::temp_dir().join(format!("picturesque-test-cache-{}", std::process::id()));
    Api::new(AppContext {
        pipeline_store: catalog.clone(),
        settings_store: catalog.clone(),
        catalog: catalog.clone(),
        imports: catalog.clone(),
        fs: Arc::new(StdFileSystem),
        metadata: Arc::new(ExifMetadataReader),
        geocoder: Arc::new(GeoNamesGeocoder::default()),
        events: Arc::new(NoEvents),
        decoder: Arc::new(VipsDecoder),
        sidecars: Arc::new(picturesque_infra::xmp::XmpSidecars),
        pixels: Arc::new(picturesque_infra::images::VipsPixels),
        analyzers: Vec::new(),
        inference: Arc::new(picturesque_infra::inference::OrtEngine::new(1)),
        models: Vec::new(),
        model_store: Arc::new(picturesque_infra::models::HttpModelStore::new(
            cache_dir.join("models"),
        )),
        cache_dir,
    })
    .unwrap()
}

fn call(api: &Api, command: &str, args: Value) -> Value {
    api.call(command, args)
        .unwrap_or_else(|e| panic!("{command}: {e:?}"))
}

fn lisbon_photo(path: &Path, time: &'static str) {
    write_jpeg(
        path,
        16,
        12,
        &[
            (Tag::Make, Text("Canon")),
            (Tag::Model, Text("Canon EOS R5")),
            (Tag::DateTimeOriginal, Text(time)),
            (Tag::GPSLatitudeRef, Text("N")),
            (
                Tag::GPSLatitude,
                Rationals(vec![(38, 1), (43, 1), (2028, 100)]),
            ),
            (Tag::GPSLongitudeRef, Text("W")),
            (
                Tag::GPSLongitude,
                Rationals(vec![(9, 1), (8, 1), (2148, 100)]),
            ),
        ],
    );
}

#[test]
fn imports_real_files_into_a_real_library() {
    let dir = tempfile::tempdir().unwrap();
    let (src, lib) = (dir.path().join("card"), dir.path().join("Clean"));
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::create_dir_all(&src).unwrap();
    lisbon_photo(&src.join("IMG_1.JPG"), "2026:10:12 14:30:05");
    lisbon_photo(&src.join("IMG_2.JPG"), "2026:10:12 14:31:00");
    std::fs::write(src.join("IMG_2.xmp"), "<x:xmpmeta/>").unwrap();
    let catalog = Arc::new(SqliteCatalog::open(&dir.path().join("catalog.sqlite")).unwrap());
    catalog
        .save_settings(&Settings {
            library_root: Some(lib.clone()),
            path_template: "{year}/{date} {city}/{name}.{ext}".into(),
            ..Default::default()
        })
        .unwrap();
    let api = api(&catalog);

    let status = call(&api, "plan_import", json!({ "sources": [src] }));
    let run_id = status["run_id"].clone();
    api.wait_for_imports();
    let plan = call(&api, "get_import", json!({ "run_id": run_id }));
    assert_eq!(plan["state"], "planned");
    assert_eq!(plan["summary"]["to_import"], 2);
    assert_eq!(plan["photos"][0]["place"], "Lisbon, Portugal");

    call(&api, "start_import", json!({ "run_id": run_id }));
    api.wait_for_imports();

    let done = call(&api, "get_import", json!({ "run_id": run_id }));
    assert_eq!(done["state"], "completed", "{done:#}");
    assert_eq!(done["summary"]["imported"], 2);
    let folder = lib.join("2026").join("2026-10-12 Lisbon");
    assert_eq!(
        std::fs::read(folder.join("IMG_1.JPG")).unwrap(),
        std::fs::read(src.join("IMG_1.JPG")).unwrap()
    );
    assert!(folder.join("IMG_2.xmp").exists());
    // No temporary files are left behind.
    let leftovers: Vec<_> = std::fs::read_dir(&folder)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with('.'))
        .collect();
    assert!(leftovers.is_empty());
    assert_eq!(catalog.photo_count().unwrap(), 2);

    // Thumbnails are rendered from the library copy and cached.
    let thumb = api
        .image_path(1, picturesque_app::thumbnails::ImageSize::Grid)
        .unwrap();
    assert_eq!(image::image_dimensions(&thumb).unwrap(), (16, 12));
    assert_eq!(
        api.image_path(1, picturesque_app::thumbnails::ImageSize::Grid)
            .unwrap(),
        thumb
    );

    // Importing the same card again finds only duplicates.
    let again = call(&api, "plan_import", json!({ "sources": [src] }));
    api.wait_for_imports();
    let again = call(&api, "get_import", json!({ "run_id": again["run_id"] }));
    assert_eq!(again["summary"]["duplicates"], 2);
    assert_eq!(again["summary"]["to_import"], 0);
}

#[test]
fn a_run_survives_reopening_the_catalog() {
    let dir = tempfile::tempdir().unwrap();
    let (src, lib) = (dir.path().join("src"), dir.path().join("lib"));
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::create_dir_all(&src).unwrap();
    lisbon_photo(&src.join("a.jpg"), "2026:01:01 10:00:00");
    let db = dir.path().join("catalog.sqlite");
    let run_id = {
        let catalog = Arc::new(SqliteCatalog::open(&db).unwrap());
        catalog
            .save_settings(&Settings {
                library_root: Some(lib.clone()),
                ..Default::default()
            })
            .unwrap();
        let api = api(&catalog);
        let status = call(&api, "plan_import", json!({ "sources": [src] }));
        api.wait_for_imports();
        status["run_id"].clone()
    };

    let catalog = Arc::new(SqliteCatalog::open(&db).unwrap());
    let api = api(&catalog);
    call(&api, "start_import", json!({ "run_id": run_id }));
    api.wait_for_imports();

    assert_eq!(
        call(&api, "import_status", json!({ "run_id": run_id }))["state"],
        "completed"
    );
    assert!(lib.join("2026").join("2026-01-01").join("a.jpg").exists());
}

#[test]
fn sidecars_are_read_on_import_and_updated_with_decisions() {
    let dir = tempfile::tempdir().unwrap();
    let (src, lib) = (dir.path().join("src"), dir.path().join("lib"));
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::create_dir_all(&src).unwrap();
    lisbon_photo(&src.join("IMG_7.JPG"), "2026:05:05 10:00:00");
    std::fs::write(
        src.join("IMG_7.JPG.xmp"),
        r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmlns:darktable="http://darktable.sf.net/"
 xmp:Rating="2" darktable:xmp_version="5"/></rdf:RDF></x:xmpmeta>"#,
    )
    .unwrap();
    lisbon_photo(&src.join("IMG_8.JPG"), "2026:05:05 10:00:01");
    let catalog = Arc::new(SqliteCatalog::open_in_memory().unwrap());
    catalog
        .save_settings(&Settings {
            library_root: Some(lib.clone()),
            path_template: "{name}.{ext}".into(),
            ..Default::default()
        })
        .unwrap();
    let api = api(&catalog);
    let run = call(&api, "plan_import", json!({ "sources": [src] }))["run_id"].clone();
    api.wait_for_imports();
    call(&api, "start_import", json!({ "run_id": run }));
    api.wait_for_imports();

    // The darktable rating came in with the photo.
    let page = call(&api, "query_photos", json!({ "sort": "captured_asc" }));
    let (first, second) = (&page["photos"][0], &page["photos"][1]);
    assert_eq!(first["rating"], 2);

    // Picking both photos updates the imported sidecar and creates the other.
    call(
        &api,
        "set_decisions",
        json!({ "changes": [{ "photo_id": first["id"], "flag": "pick" }, { "photo_id": second["id"], "flag": "pick", "rating": 5 }] }),
    );
    let report = api.flush_sidecars();
    assert_eq!((report.written, report.failed.len()), (2, 0), "{report:?}");

    let updated = std::fs::read_to_string(lib.join("IMG_7.JPG.xmp")).unwrap();
    assert!(
        updated.contains(r#"darktable:xmp_version="5""#),
        "{updated}"
    );
    assert!(
        updated.contains(r#"xmp:Rating="2""#) && updated.contains(r#"digiKam:PickLabel="3""#),
        "{updated}"
    );
    let created = std::fs::read_to_string(lib.join("IMG_8.JPG.xmp")).unwrap();
    assert!(created.contains(r#"xmp:Rating="5""#), "{created}");
    // The new sidecar is now one of the photo's files.
    let detail = call(&api, "get_photo", json!({ "photo_id": second["id"] }));
    assert!(
        detail["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["path"] == "IMG_8.JPG.xmp"),
        "{detail}"
    );
    assert_eq!(api.flush_sidecars().written, 0);
}
