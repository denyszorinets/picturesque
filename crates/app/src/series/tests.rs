use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use picturesque_core::media::MediaFormat;
use picturesque_core::metadata::{CaptureMetadata, CaptureTime};
use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets, PhotoId};
use picturesque_core::similarity::{Hash64, Signature};
use serde_json::{Value, json};

use super::{SeriesService, SeriesStatus};
use crate::analysis::AnalysisStatus;
use crate::analysis::similarity::SimilarityAnalyzer;
use crate::api::{Api, ApiErrorCode};
use crate::hooks::EventHooks;
use crate::ports::catalog::PhotoCatalog;
use crate::ports::events::{AppEvent, EventSink};
use crate::testing::{FakeMetadata, MemFs, TestWorld, fake_hash};

/// Adds photos taken at the given seconds past 10:00, without signatures.
fn add(world: &TestWorld, seconds: &[i64]) -> Vec<PhotoId> {
    let start = NaiveDate::from_ymd_opt(2026, 10, 12)
        .unwrap()
        .and_hms_opt(10, 0, 0)
        .unwrap();
    seconds
        .iter()
        .map(|s| {
            let name = format!("IMG_{s}.JPG");
            world
                .catalog
                .insert_photo(&NewPhoto {
                    metadata: CaptureMetadata {
                        captured_at: Some(CaptureTime::new(
                            start + chrono::Duration::seconds(*s),
                            None,
                        )),
                        ..Default::default()
                    },
                    assets: PhotoAssets::new(vec![Asset {
                        format: MediaFormat::Jpeg,
                        path: PathBuf::from(&name),
                        size: 1,
                        hash: fake_hash(name.as_bytes()),
                    }])
                    .unwrap(),
                })
                .unwrap()
        })
        .collect()
}

fn set_signature(world: &TestWorld, id: PhotoId, phash: u64) {
    world
        .catalog
        .store_analysis(
            id,
            SimilarityAnalyzer::ID,
            &serde_json::to_value(Signature {
                phash: Hash64(phash),
            })
            .unwrap(),
        )
        .unwrap();
}

fn series(world: &TestWorld) -> Vec<Vec<i64>> {
    world
        .catalog
        .series
        .lock()
        .unwrap()
        .iter()
        .map(|s| s.iter().map(|id| id.0).collect())
        .collect()
}

fn changes(world: &TestWorld) -> Vec<SeriesStatus> {
    world
        .events
        .0
        .lock()
        .unwrap()
        .iter()
        .filter_map(|e| match e {
            AppEvent::SeriesChanged(s) => Some(*s),
            _ => None,
        })
        .collect()
}

fn setup(seconds: &[i64]) -> (TestWorld, Api, Vec<i64>) {
    let world = TestWorld::new(MemFs::new(), FakeMetadata::default());
    let ids = add(&world, seconds).into_iter().map(|id| id.0).collect();
    let api = Api::new(world.context()).unwrap();
    (world, api, ids)
}

#[test]
fn groups_by_time_and_similarity_and_reports_changes_once() {
    let (world, api, ids) = setup(&[0, 5, 10, 200, 205]);

    assert_eq!(api.recompute_series().unwrap(), 2);
    assert_eq!(
        series(&world),
        [vec![ids[0], ids[1], ids[2]], vec![ids[3], ids[4]]]
    );
    assert_eq!(
        changes(&world),
        [SeriesStatus {
            series: 2,
            photos: 5
        }]
    );

    // Unchanged inputs: nothing is rewritten or reported.
    api.recompute_series().unwrap();
    assert_eq!(changes(&world).len(), 1);

    // Analysis finds the third photo shows another scene.
    set_signature(&world, PhotoId(ids[0]), 0);
    set_signature(&world, PhotoId(ids[1]), 0b11);
    set_signature(&world, PhotoId(ids[2]), u64::MAX);
    api.recompute_series().unwrap();
    assert_eq!(series(&world), [vec![ids[0], ids[1]], vec![ids[3], ids[4]]]);
    assert_eq!(changes(&world).len(), 2);
}

#[test]
fn edits_apply_immediately_and_survive_reanalysis() {
    let (world, api, ids) = setup(&[0, 5, 10, 15]);
    api.recompute_series().unwrap();

    let view = api
        .call(
            "edit_series",
            json!({ "photo_id": ids[2], "edit": "split" }),
        )
        .unwrap();
    assert_eq!(
        view,
        json!({
            "photo_id": ids[2],
            "series_id": ids[2],
            "photo_ids": [ids[2], ids[3]],
            "edit": "split",
        })
    );
    assert_eq!(series(&world), [vec![ids[0], ids[1]], vec![ids[2], ids[3]]]);

    // New analysis results do not undo the split.
    for id in &ids {
        set_signature(&world, PhotoId(*id), 42);
    }
    api.recompute_series().unwrap();
    assert_eq!(series(&world).len(), 2);

    // Removing the edit restores automatic grouping.
    let view = api
        .call("edit_series", json!({ "photo_id": ids[2], "edit": null }))
        .unwrap();
    assert_eq!(view["edit"], Value::Null);
    assert_eq!(view["series_id"], ids[0]);
    assert_eq!(series(&world).len(), 1);
}

#[test]
fn photo_series_describes_membership_and_validates_arguments() {
    let (_world, api, ids) = setup(&[0, 5, 500]);
    api.recompute_series().unwrap();

    assert_eq!(
        api.call("photo_series", json!({ "photo_id": ids[1] }))
            .unwrap()["photo_ids"],
        json!([ids[0], ids[1]])
    );
    assert_eq!(
        api.call("photo_series", json!({ "photo_id": ids[2] }))
            .unwrap(),
        json!({ "photo_id": ids[2], "series_id": null, "photo_ids": [], "edit": null })
    );

    let err = api
        .call("photo_series", json!({ "photo_id": 999 }))
        .unwrap_err();
    assert_eq!(err.code, ApiErrorCode::NotFound);
    let err = api
        .call("edit_series", json!({ "photo_id": 999, "edit": "join" }))
        .unwrap_err();
    assert_eq!(err.code, ApiErrorCode::NotFound);
    let err = api
        .call(
            "edit_series",
            json!({ "photo_id": ids[0], "edit": "merge" }),
        )
        .unwrap_err();
    assert_eq!(err.code, ApiErrorCode::InvalidArguments);
}

#[test]
fn changing_thresholds_regroups_right_away() {
    let (world, api, _) = setup(&[0, 40, 80]);
    api.recompute_series().unwrap();
    assert!(series(&world).is_empty());

    api.call(
        "update_settings",
        json!({ "series": { "max_gap_seconds": 60, "max_distance": 16 } }),
    )
    .unwrap();

    assert_eq!(series(&world).len(), 1);
}

fn wait_until(what: &str, done: impl Fn() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "timed out: {what}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn analysis_progress_triggers_a_debounced_recompute_in_the_background() {
    let world = TestWorld::new(MemFs::new(), FakeMetadata::default());
    let ids = add(&world, &[0, 3]);
    let hooks = Arc::new(EventHooks::new(world.events.clone()));
    let service = Arc::new(SeriesService::new(
        world.catalog.clone(),
        world.settings.clone(),
        hooks.clone(),
    ));
    let _ = hooks.series.set(Arc::clone(&service));

    service.start();
    wait_until("initial run", || {
        series(&world) == [vec![ids[0].0, ids[1].0]]
    });

    let more = add(&world, &[6]);
    hooks.emit(&AppEvent::AnalysisProgress(AnalysisStatus {
        analyzed: 3,
        total: 3,
        running: false,
    }));
    wait_until("debounced run", || {
        series(&world) == [vec![ids[0].0, ids[1].0, more[0].0]]
    });
    assert_eq!(changes(&world).len(), 2);
}
