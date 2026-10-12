use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use picturesque_core::decision::{Decision, Flag, Rating};
use picturesque_core::hash::ContentHash;
use picturesque_core::media::MediaFormat;
use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets, PhotoId};

use super::{SidecarService, sidecar_path};
use crate::ports::catalog::PhotoCatalog;
use crate::ports::settings::SettingsStore;
use crate::ports::sidecars::SidecarStore;
use crate::settings::tests::MemorySettings;
use crate::settings::{Settings, SidecarNaming};
use crate::testing::MemoryCatalog;

#[derive(Default)]
struct FakeSidecars {
    files: Mutex<HashMap<PathBuf, Decision>>,
    writes: Mutex<usize>,
    broken: Vec<PathBuf>,
}

impl SidecarStore for FakeSidecars {
    fn read(&self, path: &Path) -> Result<Option<Decision>, String> {
        Ok(self.files.lock().unwrap().get(path).copied())
    }

    fn write(&self, path: &Path, decision: Decision) -> Result<(ContentHash, u64), String> {
        if self.broken.iter().any(|b| b == path) {
            return Err("cannot parse existing sidecar".into());
        }
        *self.writes.lock().unwrap() += 1;
        self.files
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), decision);
        Ok((ContentHash::from_bytes([decision.rating.stars(); 32]), 100))
    }
}

fn asset(path: &str, n: u8) -> Asset {
    Asset {
        format: MediaFormat::from_file_name(path).unwrap(),
        path: path.into(),
        size: 1,
        hash: ContentHash::from_bytes([n; 32]),
    }
}

fn d(flag: Flag, stars: u8) -> Decision {
    Decision {
        flag,
        rating: Rating::new(stars).unwrap(),
    }
}

struct World {
    catalog: Arc<MemoryCatalog>,
    store: Arc<FakeSidecars>,
    service: Arc<SidecarService>,
}

fn world(store: FakeSidecars, naming: SidecarNaming) -> World {
    let catalog = Arc::new(MemoryCatalog::default());
    let settings = Arc::new(MemorySettings::default());
    settings
        .save_settings(&Settings {
            library_root: Some("/lib".into()),
            sidecar_naming: naming,
            ..Default::default()
        })
        .unwrap();
    let store = Arc::new(store);
    let service = Arc::new(SidecarService::new(
        catalog.clone(),
        settings,
        store.clone(),
    ));
    World {
        catalog,
        store,
        service,
    }
}

fn add(catalog: &MemoryCatalog, assets: Vec<Asset>) -> PhotoId {
    catalog
        .insert_photo(&NewPhoto {
            metadata: Default::default(),
            assets: PhotoAssets::new(assets).unwrap(),
        })
        .unwrap()
}

#[test]
fn names_new_sidecars_after_the_primary_file() {
    let w = world(FakeSidecars::default(), SidecarNaming::FileExtension);
    let pair = add(
        &w.catalog,
        vec![asset("2026/IMG_1.CR3", 1), asset("2026/IMG_1.JPG", 2)],
    );
    let photo = w.catalog.photo(pair).unwrap().unwrap();

    assert_eq!(
        sidecar_path(&photo, SidecarNaming::FileExtension),
        Path::new("2026/IMG_1.CR3.xmp")
    );
    assert_eq!(
        sidecar_path(&photo, SidecarNaming::BaseName),
        Path::new("2026/IMG_1.xmp")
    );
}

#[test]
fn existing_sidecars_are_updated_in_place() {
    let w = world(FakeSidecars::default(), SidecarNaming::FileExtension);
    let id = add(
        &w.catalog,
        vec![asset("2026/IMG_1.CR3", 1), asset("2026/IMG_1.xmp", 3)],
    );
    let photo = w.catalog.photo(id).unwrap().unwrap();

    assert_eq!(
        sidecar_path(&photo, SidecarNaming::FileExtension),
        Path::new("2026/IMG_1.xmp")
    );
}

#[test]
fn flush_writes_dirty_decisions_once_and_records_the_sidecar() {
    let w = world(FakeSidecars::default(), SidecarNaming::FileExtension);
    let id = add(&w.catalog, vec![asset("2026/IMG_1.CR3", 1)]);
    w.catalog.set_decision(id, d(Flag::Pick, 4)).unwrap();

    let report = w.service.flush();
    let again = w.service.flush();

    assert_eq!(report.written, 1);
    assert_eq!(again.written, 0);
    assert_eq!(
        w.store.files.lock().unwrap()[Path::new("/lib/2026/IMG_1.CR3.xmp")],
        d(Flag::Pick, 4)
    );
    let photo = w.catalog.photo(id).unwrap().unwrap();
    assert_eq!(
        photo.assets.sidecars()[0].path,
        Path::new("2026/IMG_1.CR3.xmp")
    );
    assert_eq!(photo.assets.sidecars()[0].size, 100);
}

#[test]
fn a_change_after_reading_is_written_on_the_next_flush() {
    let w = world(FakeSidecars::default(), SidecarNaming::FileExtension);
    let id = add(&w.catalog, vec![asset("a.JPG", 1)]);
    w.catalog.set_decision(id, d(Flag::Pick, 1)).unwrap();
    let stale = w.catalog.dirty_decisions().unwrap();
    w.catalog.set_decision(id, d(Flag::Reject, 0)).unwrap();

    // A writer finishing for the older revision does not mark the newer one written.
    w.catalog.mark_sidecar_written(id, &stale[0].token).unwrap();

    assert_eq!(w.catalog.dirty_decisions().unwrap().len(), 1);
    w.service.flush();
    assert_eq!(
        w.store.files.lock().unwrap()[Path::new("/lib/a.JPG.xmp")],
        d(Flag::Reject, 0)
    );
}

#[test]
fn failures_are_reported_and_retried_later() {
    let w = world(
        FakeSidecars {
            broken: vec!["/lib/b.JPG.xmp".into()],
            ..Default::default()
        },
        SidecarNaming::FileExtension,
    );
    let ok = add(&w.catalog, vec![asset("a.JPG", 1)]);
    let bad = add(&w.catalog, vec![asset("b.JPG", 2)]);
    w.catalog.set_decision(ok, d(Flag::Pick, 0)).unwrap();
    w.catalog.set_decision(bad, d(Flag::Pick, 0)).unwrap();

    let report = w.service.flush();

    assert_eq!(report.written, 1);
    assert_eq!(report.failed.len(), 1);
    assert_eq!(report.failed[0].0, bad.0);
    assert_eq!(
        w.catalog.dirty_decisions().unwrap().len(),
        1,
        "still dirty, retried later"
    );
}

#[test]
fn the_worker_writes_once_after_a_burst_of_changes() {
    let w = world(FakeSidecars::default(), SidecarNaming::FileExtension);
    let id = add(&w.catalog, vec![asset("a.JPG", 1)]);
    w.service.start();
    // Let the startup catch-up flush (which finds nothing) finish first.
    std::thread::sleep(Duration::from_millis(100));

    for stars in 1..=5 {
        w.catalog
            .set_decision(id, d(Flag::Unflagged, stars))
            .unwrap();
        w.service.notify();
        std::thread::sleep(Duration::from_millis(50));
    }

    let deadline = Instant::now() + Duration::from_secs(5);
    while !w.catalog.dirty_decisions().unwrap().is_empty() {
        assert!(Instant::now() < deadline, "worker did not write");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        w.store.files.lock().unwrap()[Path::new("/lib/a.JPG.xmp")],
        d(Flag::Unflagged, 5)
    );
    assert_eq!(
        *w.store.writes.lock().unwrap(),
        1,
        "debounced into one write"
    );
}
