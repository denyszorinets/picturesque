use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::json;

use super::{ContextFactory, Host, PRUNE_AFTER_SECONDS};
use crate::api::{ApiErrorCode, AppContext};
use crate::ports::catalog::PhotoCatalog;
use crate::ports::events::AppEvent;
use crate::ports::fs::FileSystem;
use crate::ports::settings::SettingsStore;
use crate::settings::{RecentFolder, Settings};
use crate::testing::{FakeMetadata, MemFs, MemoryCatalog, MemoryPipelineStore, TestWorld};

/// A session's catalog and import queue.
type Stores = (Arc<MemoryCatalog>, Arc<MemoryPipelineStore>);

/// One shared world (files, settings, events); a catalog per folder that
/// is kept, so reopening a folder finds its photos again.
struct Factory {
    world: TestWorld,
    catalogs: Mutex<HashMap<Option<PathBuf>, Stores>>,
    pruned: Mutex<Vec<Vec<PathBuf>>>,
}

impl ContextFactory for Factory {
    fn context(&self, folder: Option<&Path>) -> Result<AppContext, String> {
        let (catalog, pipeline) = self
            .catalogs
            .lock()
            .unwrap()
            .entry(folder.map(Path::to_path_buf))
            .or_default()
            .clone();
        Ok(AppContext {
            catalog,
            pipeline_store: pipeline,
            ..self.world.context()
        })
    }

    fn prune(&self, keep: &[PathBuf]) {
        self.pruned.lock().unwrap().push(keep.to_vec());
    }
}

const NOW: i64 = 1_800_000_000;

fn factory() -> Arc<Factory> {
    let fs = MemFs::new()
        .dir("/lib")
        .file("/shoot/IMG_1.JPG", b"one")
        .file("/shoot/IMG_2.JPG", b"two")
        .file("/other/IMG_3.JPG", b"three");
    let world = TestWorld::new(fs, FakeMetadata::default());
    world
        .settings
        .save_settings(&Settings {
            library_root: Some("/lib".into()),
            ..Default::default()
        })
        .unwrap();
    Arc::new(Factory {
        world,
        catalogs: Mutex::default(),
        pruned: Mutex::default(),
    })
}

fn host(f: &Arc<Factory>) -> Host {
    Host::start_at(f.clone(), f.world.settings.clone(), || NOW).unwrap()
}

/// Waits until the current session's background import has catalogued
/// `n` photos (it runs on its own threads, as in the app).
fn wait_for_photos(f: &Factory, host: &Host, n: u64) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while photos(f, host) != n {
        assert!(
            std::time::Instant::now() < deadline,
            "expected {n} photos, have {}",
            photos(f, host)
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// Photos in the current session's catalog.
fn photos(f: &Factory, host: &Host) -> u64 {
    let settings = host.call("get_settings", serde_json::Value::Null).unwrap();
    let folder = settings["open_folder"].as_str().map(PathBuf::from);
    f.catalogs.lock().unwrap()[&folder].0.photo_count().unwrap()
}

#[test]
fn starts_in_the_library_and_opens_a_folder_in_place() {
    let f = factory();
    let host = host(&f);

    let settings = host
        .call("open_folder", json!({ "path": "/shoot" }))
        .unwrap();

    assert_eq!(settings["open_folder"], "/shoot");
    assert_eq!(settings["library_root"], "/shoot");
    assert_eq!(
        settings["recent_folders"],
        json!([{ "path": "/shoot", "opened_at": NOW }])
    );
    // The library folder itself is unchanged.
    let home = f.world.settings.load_settings().unwrap();
    assert_eq!(home.library_root, Some("/lib".into()));
    assert_eq!(home.open_folder, Some("/shoot".into()));
    // The folder is its only import folder, culled in place.
    let sources = host
        .call("list_import_sources", serde_json::Value::Null)
        .unwrap();
    assert_eq!(sources[0]["path"], "/shoot");
    wait_for_photos(&f, &host, 2);
    assert!(f.world.fs.exists(Path::new("/shoot/IMG_1.JPG")));
}

#[test]
fn photos_stay_with_their_folder() {
    let f = factory();
    let host = host(&f);
    host.call("open_folder", json!({ "path": "/shoot" }))
        .unwrap();
    wait_for_photos(&f, &host, 2);

    host.call("open_folder", json!({ "path": "/other" }))
        .unwrap();
    wait_for_photos(&f, &host, 1);

    let settings = host.call("open_library", serde_json::Value::Null).unwrap();
    assert_eq!(settings["open_folder"], serde_json::Value::Null);
    assert_eq!(settings["library_root"], "/lib");
    assert_eq!(photos(&f, &host), 0);

    host.call("open_folder", json!({ "path": "/shoot" }))
        .unwrap();
    assert_eq!(photos(&f, &host), 2);
    let recent = &host.call("get_settings", serde_json::Value::Null).unwrap()["recent_folders"];
    assert_eq!(recent[0]["path"], "/shoot");
    assert_eq!(recent[1]["path"], "/other");
    assert_eq!(recent.as_array().unwrap().len(), 2);
}

#[test]
fn settings_changed_in_a_folder_apply_everywhere() {
    let f = factory();
    let host = host(&f);
    host.call("open_folder", json!({ "path": "/shoot" }))
        .unwrap();

    host.call("update_settings", json!({ "sidecar_naming": "base_name" }))
        .unwrap();
    host.call("open_library", serde_json::Value::Null).unwrap();

    let settings = host.call("get_settings", serde_json::Value::Null).unwrap();
    assert_eq!(settings["sidecar_naming"], "base_name");
    assert_eq!(settings["library_root"], "/lib");
}

#[test]
fn reopens_the_last_folder_at_start() {
    let f = factory();
    host(&f)
        .call("open_folder", json!({ "path": "/shoot" }))
        .unwrap();

    let again = host(&f);

    let settings = again.call("get_settings", serde_json::Value::Null).unwrap();
    assert_eq!(settings["open_folder"], "/shoot");
}

#[test]
fn starts_in_the_library_when_the_last_folder_is_gone() {
    let f = factory();
    host(&f)
        .call("open_folder", json!({ "path": "/shoot" }))
        .unwrap();
    f.world.fs.delete_dir(Path::new("/shoot"));

    let again = host(&f);

    let settings = again.call("get_settings", serde_json::Value::Null).unwrap();
    assert_eq!(settings["open_folder"], serde_json::Value::Null);
    assert_eq!(settings["library_root"], "/lib");
}

#[test]
fn refuses_folders_that_cannot_be_culled_in_place() {
    let f = factory();
    let host = host(&f);

    for (path, code) in [
        ("relative/path", ApiErrorCode::InvalidArguments),
        ("/missing", ApiErrorCode::InvalidArguments),
        ("/lib", ApiErrorCode::InvalidArguments),
        ("/", ApiErrorCode::InvalidArguments),
    ] {
        let err = host
            .call("open_folder", json!({ "path": path }))
            .unwrap_err();
        assert_eq!(err.code, code, "{path}");
    }
    // Still in the library.
    let settings = host.call("get_settings", serde_json::Value::Null).unwrap();
    assert_eq!(settings["open_folder"], serde_json::Value::Null);
}

#[test]
fn a_replaced_session_sends_no_more_events() {
    let f = factory();
    let host = host(&f);
    let old = host.api();
    host.call("open_folder", json!({ "path": "/shoot" }))
        .unwrap();
    let before = f.world.events.0.lock().unwrap().len();

    // Reports the library's sidecar status, unless silenced.
    old.flush_sidecars();

    let events = f.world.events.0.lock().unwrap();
    let from_library = events[before..]
        .iter()
        .any(|e| matches!(e, AppEvent::SidecarProgress(s) if s.folder.as_deref() == Some("/lib")));
    assert!(!from_library, "{:?}", &events[before..]);
}

#[test]
fn forgets_and_prunes_folders_not_opened_for_a_while() {
    let f = factory();
    let old = NOW - PRUNE_AFTER_SECONDS - 1;
    f.world
        .settings
        .save_settings(&Settings {
            library_root: Some("/lib".into()),
            recent_folders: vec![
                RecentFolder {
                    path: "/shoot".into(),
                    opened_at: NOW - 60,
                },
                RecentFolder {
                    path: "/other".into(),
                    opened_at: old,
                },
            ],
            ..Default::default()
        })
        .unwrap();

    let host = host(&f);

    assert_eq!(*f.pruned.lock().unwrap(), [vec![PathBuf::from("/shoot")]]);
    let recent = &host.call("get_settings", serde_json::Value::Null).unwrap()["recent_folders"];
    assert_eq!(recent.as_array().unwrap().len(), 1);
}
