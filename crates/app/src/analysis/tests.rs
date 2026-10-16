use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use picturesque_core::media::MediaFormat;
use picturesque_core::metadata::{CaptureMetadata, CaptureTime};
use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets, PhotoId};
use serde_json::{Value, json};

use super::{ANALYSIS_EDGE, AnalysisInput, AnalysisService, AnalysisStatus, Analyzer};
use crate::api::Api;
use crate::ports::catalog::{AnalyzerId, PhotoCatalog};
use crate::ports::events::AppEvent;
use crate::ports::settings::SettingsStore;
use crate::settings::Settings;
use crate::testing::{FakeMetadata, MemFs, TestWorld, fake_hash};
use crate::thumbnails::ThumbnailService;

/// Records which photos it saw; fails for photos listed in `fail`.
struct Recorder {
    id: AnalyzerId,
    seen: Mutex<Vec<PhotoId>>,
    fail: Vec<PhotoId>,
}

impl Recorder {
    fn new(name: &'static str, version: u32) -> Arc<Self> {
        Arc::new(Self {
            id: AnalyzerId { name, version },
            seen: Mutex::default(),
            fail: Vec::new(),
        })
    }

    fn seen(&self) -> Vec<PhotoId> {
        self.seen.lock().unwrap().clone()
    }
}

impl Analyzer for Recorder {
    fn id(&self) -> AnalyzerId {
        self.id
    }

    fn analyze(&self, input: &AnalysisInput<'_>) -> Result<Value, String> {
        self.seen.lock().unwrap().push(input.photo.id);
        if self.fail.contains(&input.photo.id) {
            return Err("no faces here".into());
        }
        Ok(json!({ "width": input.image.width(), "version": self.id.version }))
    }
}

fn world(photos: usize) -> (TestWorld, Vec<PhotoId>) {
    let mut fs = MemFs::new();
    for i in 0..photos {
        fs = fs.file(&format!("/lib/IMG_{i}.JPG"), format!("jpg {i}").as_bytes());
    }
    let world = TestWorld::new(fs, FakeMetadata::default());
    world
        .settings
        .save_settings(&Settings {
            library_root: Some("/lib".into()),
            ..Default::default()
        })
        .unwrap();
    let ids = (0..photos)
        .map(|i| {
            let bytes = format!("jpg {i}");
            let day = NaiveDate::from_ymd_opt(2026, 10, 1 + i as u32).unwrap();
            world
                .catalog
                .insert_photo(&NewPhoto {
                    metadata: CaptureMetadata {
                        captured_at: Some(CaptureTime::new(
                            day.and_hms_opt(12, 0, 0).unwrap(),
                            None,
                        )),
                        ..Default::default()
                    },
                    assets: PhotoAssets::new(vec![Asset {
                        format: MediaFormat::Jpeg,
                        path: PathBuf::from(format!("IMG_{i}.JPG")),
                        size: bytes.len() as u64,
                        hash: fake_hash(bytes.as_bytes()),
                    }])
                    .unwrap(),
                })
                .unwrap()
        })
        .collect();
    (world, ids)
}

fn service(world: &TestWorld, analyzers: Vec<Arc<dyn Analyzer>>) -> Arc<AnalysisService> {
    let thumbnails = Arc::new(ThumbnailService::new(
        world.catalog.clone(),
        world.settings.clone(),
        world.fs.clone(),
        world.decoder.clone(),
        PathBuf::from("/cache"),
    ));
    Arc::new(AnalysisService::new(
        world.catalog.clone(),
        thumbnails,
        world.pixels.clone(),
        analyzers,
        world.events.clone(),
    ))
}

fn result(world: &TestWorld, id: PhotoId, name: &str) -> Option<(u32, Value)> {
    world
        .catalog
        .analysis(id)
        .unwrap()
        .into_iter()
        .find(|(n, _, _)| n == name)
        .map(|(_, v, r)| (v, r))
}

#[test]
fn analyzes_every_photo_once_from_its_preview() {
    let (world, ids) = world(3);
    let sharpness = Recorder::new("sharpness", 1);
    let faces = Recorder::new("faces", 1);
    let analysis = service(&world, vec![sharpness.clone(), faces.clone()]);

    assert_eq!(analysis.run_until_idle(), 3);

    assert_eq!(sharpness.seen(), ids, "oldest first");
    assert_eq!(faces.seen(), ids);
    assert_eq!(
        result(&world, ids[0], "sharpness"),
        Some((1, json!({ "width": ANALYSIS_EDGE, "version": 1 })))
    );
    // Pixels come from the cached 2048 px preview, decoded once per photo.
    let pixels = world.pixels.calls.lock().unwrap().clone();
    assert_eq!(pixels.len(), 3);
    assert!(pixels.iter().all(|p| p.starts_with("/cache/v1/preview")));
    assert_eq!(
        analysis.status(),
        AnalysisStatus {
            analyzed: 3,
            total: 3,
            running: false,
            paused: None,
            waiting_for_models: Vec::new(),
        }
    );

    assert_eq!(analysis.run_until_idle(), 0, "nothing left to do");
    assert_eq!(sharpness.seen().len(), 3);
}

#[test]
fn a_new_analyzer_version_reruns_only_that_analyzer() {
    let (world, ids) = world(2);
    service(
        &world,
        vec![Recorder::new("sharpness", 1), Recorder::new("faces", 1)],
    )
    .run_until_idle();

    let sharpness = Recorder::new("sharpness", 2);
    let faces = Recorder::new("faces", 1);
    let analysis = service(&world, vec![sharpness.clone(), faces.clone()]);
    assert_eq!(analysis.status().analyzed, 0);
    analysis.run_until_idle();

    assert_eq!(sharpness.seen(), ids);
    assert!(faces.seen().is_empty());
    assert_eq!(result(&world, ids[1], "sharpness").unwrap().0, 2);
}

#[test]
fn failures_are_stored_so_photos_are_not_retried_forever() {
    let (world, ids) = world(2);
    let analyzer = Arc::new(Recorder {
        id: AnalyzerId {
            name: "faces",
            version: 1,
        },
        seen: Mutex::default(),
        fail: vec![ids[0]],
    });
    let analysis = service(&world, vec![analyzer.clone()]);

    assert_eq!(analysis.run_until_idle(), 2);
    assert_eq!(analysis.run_until_idle(), 0);

    assert_eq!(
        result(&world, ids[0], "faces"),
        Some((1, json!({ "error": "no faces here" })))
    );
    assert_eq!(analysis.status().analyzed, 2);
}

#[test]
fn a_photo_that_cannot_be_decoded_gets_an_error_from_every_analyzer() {
    let (world, ids) = world(1);
    world.fs.delete(&PathBuf::from("/lib/IMG_0.JPG"));
    let analyzer = Recorder::new("sharpness", 1);
    let analysis = service(&world, vec![analyzer.clone()]);

    analysis.run_until_idle();

    assert!(analyzer.seen().is_empty());
    let (_, error) = result(&world, ids[0], "sharpness").unwrap();
    assert!(error["error"].is_string(), "{error}");
}

#[test]
fn prioritized_photos_come_first_most_recent_request_first() {
    let (world, ids) = world(5);
    let analyzer = Recorder::new("sharpness", 1);
    let analysis = service(&world, vec![analyzer.clone()]);

    analysis.prioritize(&[ids[2], ids[3]]);
    analysis.prioritize(&[ids[4], ids[3]]);
    analysis.run_until_idle();

    assert_eq!(analyzer.seen(), [ids[4], ids[3], ids[2], ids[0], ids[1]]);
}

#[test]
fn prioritizing_unknown_or_finished_photos_is_harmless() {
    let (world, ids) = world(2);
    let analyzer = Recorder::new("sharpness", 1);
    let analysis = service(&world, vec![analyzer.clone()]);
    analysis.run_until_idle();

    analysis.prioritize(&[PhotoId(999), ids[0]]);

    assert_eq!(analysis.run_until_idle(), 0);
    assert_eq!(analyzer.seen().len(), 2);
}

fn wait_for(what: &str, done: impl Fn() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "timed out: {what}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn background_workers_analyze_new_photos_when_woken_and_report_progress() {
    let (world, _) = world(4);
    let analyzer = Recorder::new("sharpness", 1);
    let analysis = service(&world, vec![analyzer.clone()]);

    let progress = || -> Vec<AnalysisStatus> {
        world
            .events
            .0
            .lock()
            .unwrap()
            .iter()
            .filter_map(|e| match e {
                AppEvent::AnalysisProgress(s) => Some(s.clone()),
                _ => None,
            })
            .collect()
    };
    analysis.start(3);
    // The count is updated just before its progress event is sent.
    wait_for("first pass", || {
        analysis.status().analyzed == 4 && progress().len() >= 4
    });

    let progress = progress();
    assert_eq!(progress.len(), 4);
    assert!(progress.iter().any(|s| s.analyzed == 4 && s.total == 4));

    // An import adds a photo; workers pick it up when woken.
    world.fs.write(&PathBuf::from("/lib/new.JPG"), b"new");
    let new = world
        .catalog
        .insert_photo(&NewPhoto {
            metadata: CaptureMetadata::default(),
            assets: PhotoAssets::new(vec![Asset {
                format: MediaFormat::Jpeg,
                path: PathBuf::from("new.JPG"),
                size: 3,
                hash: fake_hash(b"new"),
            }])
            .unwrap(),
        })
        .unwrap();
    analysis.wake();
    wait_for("new photo", || analyzer.seen().contains(&new));
    wait_for("idle", || !analysis.status().running);

    let mut seen = analyzer.seen();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), 5, "each photo analyzed exactly once");
    assert_eq!(analyzer.seen().len(), 5);
}

#[test]
fn api_reports_status_and_accepts_priorities() {
    let (world, ids) = world(2);
    let mut ctx = world.context();
    ctx.analyzers = vec![Recorder::new("sharpness", 1)];
    let api = Api::new(ctx).unwrap();

    assert_eq!(
        api.call("analysis_status", Value::Null).unwrap(),
        json!({
            "analyzed": 0, "total": 2, "running": false, "paused": null,
            "waiting_for_models": ["AI scores and similar photos", "faces and closed eyes"],
        })
    );
    api.call("prioritize_analysis", json!({ "photo_ids": [ids[1].0] }))
        .unwrap();
    assert_eq!(api.run_analysis(), 2);
    assert_eq!(
        api.call("analysis_status", json!({})).unwrap(),
        json!({
            "analyzed": 2, "total": 2, "running": false, "paused": null,
            "waiting_for_models": ["AI scores and similar photos", "faces and closed eyes"],
        })
    );

    let err = api
        .call("prioritize_analysis", json!({ "photo_ids": ["x"] }))
        .unwrap_err();
    assert_eq!(err.code, crate::api::ApiErrorCode::InvalidArguments);
}

mod clip {
    use std::sync::Arc;

    use picturesque_core::clip::Embedding;

    use super::{result, service, world};
    use crate::analysis::clip::ClipAnalyzer;
    use crate::analysis::technical::TechnicalAnalyzer;
    use crate::models::{CLIP_IMAGE, ModelService, catalog};
    use crate::ports::inference::Provider;
    use crate::testing::{FakeInference, MemoryModelStore};

    fn clip(models: &Arc<ModelService>, world: &crate::testing::TestWorld) -> Arc<ClipAnalyzer> {
        Arc::new(ClipAnalyzer::new(
            Arc::clone(models),
            Arc::new(FakeInference::new(&[Provider::WebGpu])),
            world.settings.clone(),
        ))
    }

    /// Puts the CLIP model's files in the store, as if downloaded.
    fn install(store: &MemoryModelStore) {
        let spec = catalog().into_iter().find(|m| m.id == CLIP_IMAGE).unwrap();
        store.install(&spec, &[]);
    }

    #[test]
    fn embeds_photos_once_the_model_is_downloaded() {
        let (world, ids) = world(2);
        let store = Arc::new(MemoryModelStore::default());
        let models = Arc::new(ModelService::new(
            catalog(),
            store.clone(),
            world.events.clone(),
        ));
        let clip = clip(&models, &world);
        let analysis = service(&world, vec![Arc::new(TechnicalAnalyzer), clip.clone()]);

        // Without the model, photos are done once the other analyzers ran.
        assert!(!crate::analysis::Analyzer::ready(clip.as_ref()));
        assert_eq!(analysis.run_until_idle(), 2);
        assert_eq!(analysis.status().analyzed, 2);
        assert!(result(&world, ids[0], "clip").is_none());
        // The status says what waits for the model.
        assert_eq!(
            analysis.status().waiting_for_models,
            ["AI scores and similar photos"]
        );

        install(&store);
        assert_eq!(analysis.status().analyzed, 0);
        assert!(analysis.status().waiting_for_models.is_empty());
        assert_eq!(analysis.run_until_idle(), 2);
        let (version, stored) = result(&world, ids[0], "clip").unwrap();
        assert_eq!(version, ClipAnalyzer::ID.version);
        // The fake model echoes its input: the preprocessed 224 × 224 image.
        let embedding = ClipAnalyzer::embedding(&stored).unwrap();
        assert_eq!(embedding.values().len(), 3 * 224 * 224);
        assert!(Embedding::new(embedding.values()).is_some());
    }
}

#[test]
fn a_full_cache_disk_pauses_analysis_instead_of_storing_errors() {
    let (world, ids) = world(1);
    // The preview cannot be made, and the cache disk is full.
    world.fs.delete(&PathBuf::from("/lib/IMG_0.JPG"));
    world.fs.set_space(Some(0));
    let analyzer = Recorder::new("sharpness", 1);
    let analysis = service(&world, vec![analyzer.clone()]);

    assert_eq!(analysis.run_until_idle(), 0);
    assert!(result(&world, ids[0], "sharpness").is_none());
    let status = analysis.status();
    assert!(status.paused.unwrap().contains("thumbnail cache is full"));
    assert_eq!((status.analyzed, status.total), (0, 1));
}

#[test]
fn a_full_catalog_disk_pauses_analysis_until_space_is_freed() {
    use std::sync::atomic::Ordering;
    let (world, ids) = world(2);
    let analyzer = Recorder::new("sharpness", 1);
    let analysis = service(&world, vec![analyzer.clone()]);
    world.catalog.full.store(true, Ordering::Relaxed);

    assert_eq!(analysis.run_until_idle(), 0);
    // It stopped at the first photo instead of going round in circles.
    assert_eq!(analyzer.seen().len(), 1);
    assert!(
        analysis
            .status()
            .paused
            .unwrap()
            .contains("catalog is full")
    );

    world.catalog.full.store(false, Ordering::Relaxed);
    assert_eq!(analysis.run_until_idle(), 2);
    assert!(analysis.status().paused.is_none());
    assert!(result(&world, ids[1], "sharpness").is_some());
}

#[test]
fn reanalyzes_chosen_photos_keeping_earlier_results_meanwhile() {
    let (world, ids) = world(3);
    let analyzer = Recorder::new("sharpness", 1);
    let analysis = service(&world, vec![analyzer.clone()]);
    assert_eq!(analysis.run_until_idle(), 3);

    // Unknown ids are ignored.
    assert_eq!(
        analysis.reanalyze(Some(&[ids[1], PhotoId(999)])).unwrap(),
        1
    );
    let status = analysis.status();
    assert_eq!((status.analyzed, status.total), (2, 3));
    // The earlier result is still there until replaced.
    assert!(result(&world, ids[1], "sharpness").is_some());
    let id = AnalyzerId {
        name: "sharpness",
        version: 1,
    };
    assert_eq!(world.catalog.analysis_results(id).unwrap().len(), 3);

    assert_eq!(analysis.run_until_idle(), 1);
    assert_eq!(analyzer.seen(), [ids[0], ids[1], ids[2], ids[1]]);
    assert_eq!(analysis.status().analyzed, 3);
}

#[test]
fn reanalyzes_every_photo() {
    let (world, _) = world(2);
    let analyzer = Recorder::new("sharpness", 1);
    let analysis = service(&world, vec![analyzer.clone()]);
    analysis.run_until_idle();

    assert_eq!(analysis.reanalyze(None).unwrap(), 2);
    assert_eq!(analysis.status().analyzed, 0);
    // Progress is reported right away.
    assert!(world.events.0.lock().unwrap().iter().any(|e| matches!(
        e,
        AppEvent::AnalysisProgress(s) if s.analyzed == 0 && s.total == 2
    )));
    assert_eq!(analysis.run_until_idle(), 2);
    assert_eq!(analyzer.seen().len(), 4);
}

#[test]
fn reanalyzing_says_which_analyses_wait_for_models() {
    let (world, _) = world(1);
    // The API adds the AI analyzers; no model is downloaded.
    let api = Api::new(world.context()).unwrap();

    let result = api.call("reanalyze_photos", Value::Null).unwrap();
    assert_eq!(
        result,
        json!({
            "photos": 1,
            "waiting_for_models": ["AI scores and similar photos", "faces and closed eyes"],
        })
    );
}
