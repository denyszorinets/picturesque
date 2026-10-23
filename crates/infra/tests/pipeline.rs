//! Importing with every real adapter: std file system, SQLite catalog,
//! EXIF reader, GeoNames geocoder, XMP sidecars and file watching.

mod support;

use std::path::Path;
use std::sync::Arc;

use exif::Tag;
use picturesque_app::api::{Api, AppContext};
use picturesque_app::ports::catalog::PhotoCatalog;
use picturesque_app::ports::events::NoEvents;
use picturesque_app::ports::pipeline::{JobStage, PipelineStore};
use picturesque_app::ports::settings::SettingsStore;
use picturesque_app::ports::watcher::{FolderWatcher, NoWatcher};
use picturesque_app::settings::Settings;
use picturesque_infra::catalog::SqliteCatalog;
use picturesque_infra::fs::StdFileSystem;
use picturesque_infra::geocode::GeoNamesGeocoder;
use picturesque_infra::images::VipsDecoder;
use picturesque_infra::metadata::ExifMetadataReader;
use picturesque_infra::watcher::NotifyWatcher;
use serde_json::{Value, json};
use support::{ExifValue::*, write_jpeg};

fn api(catalog: &Arc<SqliteCatalog>, cache: &Path) -> Api {
    api_with(catalog, cache, Arc::new(NoWatcher))
}

fn api_with(catalog: &Arc<SqliteCatalog>, cache: &Path, watcher: Arc<dyn FolderWatcher>) -> Api {
    Api::new(AppContext {
        pipeline_store: catalog.clone(),
        watcher,
        settings_store: catalog.clone(),
        catalog: catalog.clone(),
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
            cache.join("models"),
        )),
        cache_dir: cache.to_path_buf(),
    })
    .unwrap()
}

fn call(api: &Api, command: &str, args: Value) -> Value {
    api.call(command, args)
        .unwrap_or_else(|e| panic!("{command}: {e:?}"))
}

fn photo(path: &Path, time: &'static str) {
    write_jpeg(
        path,
        16,
        12,
        &[
            (Tag::Make, Text("Canon")),
            (Tag::DateTimeOriginal, Text(time)),
        ],
    );
}

#[test]
fn imports_photo_by_photo_and_survives_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("card").join("DCIM");
    let lib = dir.path().join("Clean");
    std::fs::create_dir_all(&card).unwrap();
    std::fs::create_dir_all(&lib).unwrap();
    photo(&card.join("IMG_0001.JPG"), "2026:10:12 09:00:00");
    photo(&card.join("IMG_0002.JPG"), "2026:10:12 09:05:00");
    let db = dir.path().join("catalog.sqlite");
    let cache = dir.path().join("cache");

    {
        let catalog = Arc::new(SqliteCatalog::open(&db).unwrap());
        catalog
            .save_settings(&Settings {
                library_root: Some(lib.clone()),
                ..Default::default()
            })
            .unwrap();
        let api = api(&catalog, &cache);
        let source = call(
            &api,
            "add_import_source",
            json!({ "path": card.parent().unwrap() }),
        );
        assert_eq!(source["watch"], true);
        assert_eq!(api.run_imports(), 2);
        assert_eq!(catalog.photo_count().unwrap(), 2);
        assert!(lib.join("2026/2026-10-12/IMG_0001.JPG").exists());

        let jobs = call(&api, "pipeline_jobs", json!({ "filter": "imported" }));
        assert_eq!(jobs.as_array().unwrap().len(), 2);
        assert_eq!(jobs[0]["stage"], "imported");
        assert!(jobs[0]["photo_id"].is_i64());
        assert_eq!(jobs[0]["analysis"], json!([0, 0]));
        assert_eq!(jobs[0]["failed_stage"], Value::Null);
        let status = call(&api, "pipeline_status", Value::Null);
        assert_eq!(
            (status["imported"].clone(), status["queued"].clone()),
            (json!(2), json!(0))
        );
        let log = call(&api, "pipeline_log", json!({ "min_level": "info" }));
        let messages: Vec<&str> = log
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["message"].as_str().unwrap())
            .collect();
        assert!(
            messages
                .iter()
                .any(|m| m.contains("imported to 2026/2026-10-12/IMG_0002.JPG")),
            "{messages:?}"
        );

        // A new photo arrives, and the app stops while copying it.
        photo(&card.join("IMG_0003.JPG"), "2026:10:13 10:00:00");
        let job = catalog
            .offer_job(
                source["id"].as_i64().unwrap(),
                &card.join("IMG_0003.JPG"),
                &[],
            )
            .unwrap()
            .unwrap();
        catalog
            .set_job_stage(job, JobStage::Copying, None, None)
            .unwrap();
    }

    // Restart: sources, library and queue are all still there.
    let catalog = Arc::new(SqliteCatalog::open(&db).unwrap());
    let sources = PipelineStore::sources(catalog.as_ref()).unwrap();
    assert_eq!(sources.len(), 1);
    let api = api(&catalog, &cache);
    let listed = call(&api, "list_import_sources", Value::Null);
    assert_eq!(
        listed[0]["path"],
        card.parent().unwrap().display().to_string()
    );
    assert_eq!(catalog.photo_count().unwrap(), 2);
    // The photo left in progress by the stop is resumed.
    assert_eq!(api.run_imports(), 1);
    assert_eq!(catalog.photo_count().unwrap(), 3);
    assert!(lib.join("2026/2026-10-13/IMG_0003.JPG").exists());
    let log = call(&api, "pipeline_log", Value::Null);
    assert!(log.to_string().contains("Resuming 1 photos"), "{log}");

    // Errors are invalid arguments.
    let err = api
        .call("add_import_source", json!({ "path": lib }))
        .unwrap_err();
    assert_eq!(
        err.code,
        picturesque_app::api::ApiErrorCode::InvalidArguments
    );
}

fn eventually(what: &str, check: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !check() {
        assert!(std::time::Instant::now() < deadline, "timed out: {what}");
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

#[test]
fn a_photo_dropped_into_a_watched_folder_is_imported() {
    let dir = tempfile::tempdir().unwrap();
    let (card, lib) = (dir.path().join("Inbox"), dir.path().join("Clean"));
    std::fs::create_dir_all(&card).unwrap();
    std::fs::create_dir_all(&lib).unwrap();
    photo(&card.join("IMG_0001.JPG"), "2026:10:12 09:00:00");
    let catalog = Arc::new(SqliteCatalog::open(&dir.path().join("catalog.sqlite")).unwrap());
    catalog
        .save_settings(&Settings {
            library_root: Some(lib.clone()),
            ..Default::default()
        })
        .unwrap();
    let api = api_with(
        &catalog,
        &dir.path().join("cache"),
        Arc::new(NotifyWatcher::new()),
    );
    call(&api, "add_import_source", json!({ "path": card }));
    api.start_background_work();
    eventually("the folder is imported", || {
        catalog.photo_count().unwrap() == 1
    });

    // A new photo in a subfolder, written in two steps like a slow copy.
    std::fs::create_dir_all(card.join("2026-10-13")).unwrap();
    let staged = dir.path().join("staged.jpg");
    photo(&staged, "2026:10:13 10:00:00");
    let bytes = std::fs::read(&staged).unwrap();
    let target = card.join("2026-10-13/IMG_0002.JPG");
    std::fs::write(&target, &bytes[..bytes.len() / 2]).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    std::fs::write(&target, &bytes).unwrap();
    eventually("the new photo is imported", || {
        catalog.photo_count().unwrap() == 2
    });
    assert_eq!(
        std::fs::read(lib.join("2026/2026-10-13/IMG_0002.JPG")).unwrap(),
        bytes
    );
    api.wait_for_thumbnails();
}

#[test]
fn a_failed_job_remembers_where_it_failed() {
    let catalog = SqliteCatalog::open_in_memory().unwrap();
    let source = catalog.add_source(Path::new("/card"), true).unwrap();
    let id = catalog
        .offer_job(source.id, Path::new("/card/a.jpg"), &[])
        .unwrap()
        .unwrap();
    catalog
        .set_job_stage(id, JobStage::Copying, None, None)
        .unwrap();
    catalog
        .set_job_stage(id, JobStage::Failed, None, Some("disk full"))
        .unwrap();
    let job = catalog.job(id).unwrap().unwrap();
    assert_eq!(
        (job.stage, job.failed_stage),
        (JobStage::Failed, Some(JobStage::Copying))
    );
    assert_eq!(job.error.as_deref(), Some("disk full"));
    // Retrying clears it.
    catalog.retry_failed(None).unwrap();
    catalog
        .set_job_stage(id, JobStage::Reading, None, None)
        .unwrap();
    assert_eq!(catalog.job(id).unwrap().unwrap().failed_stage, None);
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
    let api = api(&catalog, &dir.path().join("cache"));
    call(&api, "add_import_source", json!({ "path": src }));
    assert_eq!(api.run_imports(), 2);

    // The place comes from GPS through the offline geocoder.
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
        .image_path(1, picturesque_app::thumbnails::ImageSize::Grid, None)
        .unwrap();
    assert_eq!(image::image_dimensions(&thumb).unwrap(), (16, 12));
    let jobs = call(&api, "pipeline_jobs", Value::Null);
    assert!(
        jobs.as_array()
            .unwrap()
            .iter()
            .all(|j| j["thumbnail"] == true),
        "{jobs}"
    );

    // The same card in another folder finds only duplicates.
    let copy = dir.path().join("backup");
    std::fs::create_dir_all(&copy).unwrap();
    for name in ["IMG_1.JPG", "IMG_2.JPG"] {
        std::fs::copy(src.join(name), copy.join(name)).unwrap();
    }
    call(&api, "add_import_source", json!({ "path": copy }));
    assert_eq!(api.run_imports(), 2);
    let status = call(&api, "pipeline_status", Value::Null);
    assert_eq!(
        (status["imported"].clone(), status["duplicates"].clone()),
        (json!(2), json!(2))
    );
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
    let api = api(&catalog, &dir.path().join("cache"));
    call(&api, "add_import_source", json!({ "path": src }));
    assert_eq!(api.run_imports(), 2);

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
