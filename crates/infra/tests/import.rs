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
        settings_store: catalog.clone(),
        catalog: catalog.clone(),
        imports: catalog.clone(),
        fs: Arc::new(StdFileSystem),
        metadata: Arc::new(ExifMetadataReader),
        geocoder: Arc::new(GeoNamesGeocoder::default()),
        events: Arc::new(NoEvents),
        decoder: Arc::new(VipsDecoder),
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
