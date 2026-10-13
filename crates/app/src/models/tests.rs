use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use super::{ModelError, ModelService, ModelState, ModelView};
use crate::ports::events::AppEvent;
use crate::ports::models::{ModelFile, ModelSpec};
use crate::testing::{MemoryModelStore, RecordedEvents, fake_sha256};

const WEIGHTS: &[u8] = b"model weights, version one";
const VOCAB: &[u8] = b"a b c";

fn file(name: &str, bytes: &[u8]) -> ModelFile {
    ModelFile {
        name: name.into(),
        url: format!("https://example.test/{name}"),
        sha256: fake_sha256(bytes),
        size: bytes.len() as u64,
    }
}

fn spec(version: u32) -> ModelSpec {
    ModelSpec {
        id: "clip".into(),
        name: "CLIP".into(),
        purpose: "Search".into(),
        version,
        license: "MIT".into(),
        license_url: "https://example.test/license".into(),
        homepage: "https://example.test".into(),
        files: vec![file("visual/model.onnx", WEIGHTS), file("vocab.txt", VOCAB)],
    }
}

struct Setup {
    service: Arc<ModelService>,
    store: Arc<MemoryModelStore>,
    events: Arc<RecordedEvents>,
}

fn setup(specs: Vec<ModelSpec>) -> Setup {
    let store = Arc::new(MemoryModelStore::default());
    {
        let mut served = store.served.lock().unwrap();
        served.insert(
            "https://example.test/visual/model.onnx".into(),
            WEIGHTS.into(),
        );
        served.insert("https://example.test/vocab.txt".into(), VOCAB.into());
    }
    let events = Arc::new(RecordedEvents::default());
    let service = Arc::new(ModelService::new(specs, store.clone(), events.clone()));
    Setup {
        service,
        store,
        events,
    }
}

/// Waits until the model leaves the downloading state.
fn settled(service: &ModelService, id: &str) -> ModelView {
    let start = Instant::now();
    loop {
        let view = service.list().into_iter().find(|m| m.id == id).unwrap();
        if view.state != ModelState::Downloading {
            return view;
        }
        assert!(start.elapsed() < Duration::from_secs(5), "download hangs");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn lists_models_with_licenses_and_sizes() {
    let s = setup(vec![spec(1)]);
    let [view] = s.service.list().try_into().unwrap();
    assert_eq!(view.state, ModelState::NotInstalled);
    assert_eq!(view.license, "MIT");
    assert_eq!(view.size, (WEIGHTS.len() + VOCAB.len()) as u64);
    assert!(s.service.files("clip").is_none());
}

#[test]
fn installs_every_file_and_then_works_offline() {
    let s = setup(vec![spec(1)]);
    s.store.hold.store(true, Ordering::SeqCst);
    let started = s.service.install("clip").unwrap();
    assert_eq!(started.state, ModelState::Downloading);
    s.store.hold.store(false, Ordering::SeqCst);

    let done = settled(&s.service, "clip");
    assert_eq!(done.state, ModelState::Installed);
    assert_eq!(s.store.fetches.lock().unwrap().len(), 2);
    let files = s.service.files("clip").unwrap();
    assert_eq!(
        files["visual/model.onnx"].to_str(),
        Some("/models/clip/v1/visual/model.onnx")
    );
    let last = s.events.0.lock().unwrap().last().cloned();
    assert!(matches!(last, Some(AppEvent::ModelProgress(v)) if v.state == ModelState::Installed));

    // Offline: nothing is served any more, the model stays usable and a
    // second install downloads nothing.
    s.store.served.lock().unwrap().clear();
    let again = s.service.install("clip").unwrap();
    assert_eq!(again.state, ModelState::Installed);
    assert_eq!(s.store.fetches.lock().unwrap().len(), 2);
    assert!(s.service.files("clip").is_some());
}

#[test]
fn a_corrupt_download_is_detected_and_not_kept() {
    let s = setup(vec![spec(1)]);
    s.store.served.lock().unwrap().insert(
        "https://example.test/vocab.txt".into(),
        b"x b c".to_vec(), // same size, different bytes
    );
    s.service.install("clip").unwrap();
    let failed = settled(&s.service, "clip");
    assert_eq!(failed.state, ModelState::Failed);
    assert!(failed.error.unwrap().contains("corrupt"));
    assert!(s.service.files("clip").is_none());

    // Retrying fetches only the missing file.
    s.store
        .served
        .lock()
        .unwrap()
        .insert("https://example.test/vocab.txt".into(), VOCAB.into());
    s.service.install("clip").unwrap();
    assert_eq!(settled(&s.service, "clip").state, ModelState::Installed);
    let fetches = s.store.fetches.lock().unwrap().clone();
    assert_eq!(fetches.len(), 3);
    assert!(fetches[2].ends_with("vocab.txt"));
}

#[test]
fn downloads_can_be_cancelled_and_models_removed() {
    let s = setup(vec![spec(1)]);
    s.store.hold.store(true, Ordering::SeqCst);
    s.service.install("clip").unwrap();
    assert!(matches!(s.service.remove("clip"), Err(ModelError::Busy(_))));
    s.service.cancel("clip").unwrap();
    let cancelled = settled(&s.service, "clip");
    assert_eq!(cancelled.state, ModelState::NotInstalled);
    assert_eq!(cancelled.error, None);

    s.store.hold.store(false, Ordering::SeqCst);
    s.service.install("clip").unwrap();
    assert_eq!(settled(&s.service, "clip").state, ModelState::Installed);
    let removed = s.service.remove("clip").unwrap();
    assert_eq!(removed.state, ModelState::NotInstalled);
    assert!(s.store.files.lock().unwrap().is_empty());
}

#[test]
fn a_new_version_replaces_the_old_one() {
    let s = setup(vec![spec(1)]);
    s.service.install("clip").unwrap();
    settled(&s.service, "clip");

    let s2 = Setup {
        service: Arc::new(ModelService::new(
            vec![spec(2)],
            s.store.clone(),
            s.events.clone(),
        )),
        store: s.store.clone(),
        events: s.events.clone(),
    };
    // Version 1 files do not count for version 2.
    assert_eq!(s2.service.list()[0].state, ModelState::NotInstalled);
    s2.service.install("clip").unwrap();
    assert_eq!(settled(&s2.service, "clip").state, ModelState::Installed);
    let versions: Vec<u32> = s2
        .store
        .files
        .lock()
        .unwrap()
        .keys()
        .map(|(_, v, _)| *v)
        .collect();
    assert_eq!(versions, [2, 2]);
}

#[test]
fn unknown_models_are_errors() {
    let s = setup(vec![spec(1)]);
    assert!(matches!(
        s.service.install("nope"),
        Err(ModelError::Unknown(_))
    ));
    assert!(s.service.files("nope").is_none());
}
