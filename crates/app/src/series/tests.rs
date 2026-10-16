use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use picturesque_core::media::MediaFormat;
use picturesque_core::metadata::{CaptureMetadata, CaptureTime};
use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets, PhotoId};
use picturesque_core::similarity::{Hash64, Signature};
use serde_json::{Value, json};

use super::{SeriesService, SeriesStatus, settle};
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
        .map(|s| s.members.iter().map(|id| id.0).collect())
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
            "best_photo_id": ids[2],
            "best_chosen": false,
            "ranking": [
                { "photo_id": ids[2], "score": null },
                { "photo_id": ids[3], "score": null },
            ],
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
        json!({
            "photo_id": ids[2],
            "series_id": null,
            "photo_ids": [],
            "edit": null,
            "best_photo_id": null,
            "best_chosen": false,
            "ranking": [],
        })
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
        paused: None,
        waiting_for_models: Vec::new(),
    }));
    wait_until("debounced run", || {
        series(&world) == [vec![ids[0].0, ids[1].0, more[0].0]]
    });
    assert_eq!(changes(&world).len(), 2);
}

fn set_scores(world: &TestWorld, id: i64, laplacian_variance: f32, exposure: f32) {
    use crate::analysis::technical::TechnicalAnalyzer;
    world
        .catalog
        .store_analysis(
            PhotoId(id),
            TechnicalAnalyzer::ID,
            &json!({
                "sharpness": 1.0, "exposure": exposure, "noise": 1.0, "overall": 1.0,
                "laplacian_variance": laplacian_variance, "highlights_clipped": 0.0,
                "shadows_clipped": 0.0, "mean_brightness": 0.5, "noise_sigma": 1.0,
            }),
        )
        .unwrap();
}

fn best_of(api: &Api, id: i64) -> (Value, Value) {
    let view = api.call("photo_series", json!({ "photo_id": id })).unwrap();
    (view["best_photo_id"].clone(), view["best_chosen"].clone())
}

#[test]
fn the_best_photo_is_ranked_from_technical_scores() {
    let (world, api, ids) = setup(&[0, 1, 2]);
    set_scores(&world, ids[0], 2_000.0, 1.0);
    set_scores(&world, ids[1], 8_000.0, 1.0);
    // ids[2] is not analyzed yet.
    api.recompute_series().unwrap();

    assert_eq!(best_of(&api, ids[0]), (json!(ids[1]), json!(false)));
    let view = api
        .call("photo_series", json!({ "photo_id": ids[0] }))
        .unwrap();
    let order: Vec<i64> = view["ranking"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["photo_id"].as_i64().unwrap())
        .collect();
    assert_eq!(order, [ids[1], ids[0], ids[2]]);
    assert_eq!(view["ranking"][0]["score"], json!(1.0));
    assert_eq!(view["ranking"][2]["score"], Value::Null);
}

#[test]
fn rejecting_the_best_photo_moves_the_choice() {
    let (world, api, ids) = setup(&[0, 1]);
    set_scores(&world, ids[0], 2_000.0, 1.0);
    set_scores(&world, ids[1], 8_000.0, 1.0);
    api.recompute_series().unwrap();

    api.call(
        "set_decisions",
        json!({ "changes": [{ "photo_id": ids[1], "flag": "reject" }] }),
    )
    .unwrap();
    api.recompute_series().unwrap();

    assert_eq!(best_of(&api, ids[0]), (json!(ids[0]), json!(false)));
}

#[test]
fn the_users_choice_wins_and_survives_reanalysis_until_withdrawn() {
    let (world, api, ids) = setup(&[0, 1, 2]);
    set_scores(&world, ids[0], 8_000.0, 1.0);
    api.recompute_series().unwrap();
    assert_eq!(best_of(&api, ids[1]), (json!(ids[0]), json!(false)));

    let view = api
        .call("choose_best", json!({ "photo_id": ids[2], "best": true }))
        .unwrap();
    assert_eq!(view["best_photo_id"], ids[2]);
    assert_eq!(view["best_chosen"], true);

    // New scores favour another photo; the choice stands.
    set_scores(&world, ids[1], 50_000.0, 1.0);
    api.recompute_series().unwrap();
    assert_eq!(best_of(&api, ids[0]), (json!(ids[2]), json!(true)));

    // The latest choice in a series wins.
    api.call("choose_best", json!({ "photo_id": ids[0], "best": true }))
        .unwrap();
    assert_eq!(best_of(&api, ids[0]), (json!(ids[0]), json!(true)));

    api.call("choose_best", json!({ "photo_id": ids[0], "best": false }))
        .unwrap();
    assert_eq!(best_of(&api, ids[0]), (json!(ids[2]), json!(true)));
    api.call("choose_best", json!({ "photo_id": ids[2], "best": false }))
        .unwrap();
    assert_eq!(best_of(&api, ids[0]), (json!(ids[1]), json!(false)));

    let err = api
        .call("choose_best", json!({ "photo_id": 999, "best": true }))
        .unwrap_err();
    assert_eq!(err.code, ApiErrorCode::NotFound);
}

#[test]
fn ranking_weights_are_settings() {
    let (world, api, ids) = setup(&[0, 1]);
    set_scores(&world, ids[0], 8_000.0, 0.4); // sharp, badly exposed
    set_scores(&world, ids[1], 6_000.0, 1.0);
    api.recompute_series().unwrap();
    assert_eq!(best_of(&api, ids[0]).0, json!(ids[1]));

    api.call(
        "update_settings",
        json!({ "ranking": { "sharpness": 1.0, "exposure": 0.0, "noise": 0.0 } }),
    )
    .unwrap();
    assert_eq!(best_of(&api, ids[0]).0, json!(ids[0]));

    let err = api
        .call(
            "update_settings",
            json!({ "ranking": { "sharpness": 0.0, "exposure": 0.0, "noise": 0.0 } }),
        )
        .unwrap_err();
    assert_eq!(err.code, ApiErrorCode::InvalidArguments);
    let err = api
        .call(
            "update_settings",
            json!({ "ranking": { "sharpness": -1.0 } }),
        )
        .unwrap_err();
    assert_eq!(err.code, ApiErrorCode::InvalidArguments);
}

#[test]
fn a_steady_stream_of_requests_waits_at_most_max_wait() {
    let (tx, rx) = std::sync::mpsc::channel();
    let sender = std::thread::spawn(move || {
        for _ in 0..100 {
            if tx.send(()).is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    let start = Instant::now();
    assert!(settle(
        &rx,
        Duration::from_millis(50),
        Duration::from_millis(150)
    ));
    let waited = start.elapsed();
    assert!(waited >= Duration::from_millis(150), "{waited:?}");
    assert!(waited < Duration::from_millis(400), "{waited:?}");
    drop(rx);
    sender.join().unwrap();

    // Quiet: returns after the quiet time; no sender: reports it.
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let start = Instant::now();
    assert!(settle(
        &rx,
        Duration::from_millis(20),
        Duration::from_secs(5)
    ));
    assert!(start.elapsed() < Duration::from_secs(1));
    drop(tx);
    assert!(!settle(
        &rx,
        Duration::from_millis(20),
        Duration::from_secs(5)
    ));
}

fn set_faces(world: &TestWorld, id: i64, eyes_closed: bool, sharpness: f32) {
    world
        .catalog
        .store_analysis(
            PhotoId(id),
            crate::analysis::faces::FaceAnalyzer::ID,
            &json!({ "faces": [{
                "x": 0.3, "y": 0.2, "width": 0.3, "height": 0.4, "score": 0.9,
                "eyes_closed": eyes_closed, "eye_openness": 0.3, "sharpness": sharpness,
            }] }),
        )
        .unwrap();
}

fn set_clip(world: &TestWorld, id: i64, embedding: &[f32], rating: f32) {
    let e = picturesque_core::clip::Embedding::new(embedding).unwrap();
    world
        .catalog
        .store_analysis(
            PhotoId(id),
            crate::analysis::clip::ClipAnalyzer::ID,
            &json!({ "embedding": e.encode(), "aesthetic": rating }),
        )
        .unwrap();
}

#[test]
fn a_blink_loses_to_open_eyes() {
    let (world, api, ids) = setup(&[0, 1]);
    // The crispest frame has closed eyes.
    set_scores(&world, ids[0], 8_000.0, 1.0);
    set_scores(&world, ids[1], 6_000.0, 1.0);
    set_faces(&world, ids[0], true, 0.9);
    set_faces(&world, ids[1], false, 0.9);
    api.recompute_series().unwrap();
    assert_eq!(best_of(&api, ids[0]).0, json!(ids[1]));

    // The loupe explains it.
    let reasons = |id: i64| -> Vec<String> {
        api.call("get_photo", json!({ "photo_id": id })).unwrap()["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                format!(
                    "{}: {}",
                    r["tone"].as_str().unwrap(),
                    r["text"].as_str().unwrap()
                )
            })
            .collect()
    };
    let blink = reasons(ids[0]);
    assert_eq!(blink[0], "info: Ranked 2 of 2 in its series");
    assert!(blink.contains(&"bad: The best photo of the series has open eyes".to_owned()));
    assert!(blink.contains(&"bad: Eyes closed: the score is halved".to_owned()));
    assert_eq!(reasons(ids[1])[0], "good: Best of its series of 2");
}

#[test]
fn embeddings_join_retakes_and_scores_are_stored() {
    let (world, api, ids) = setup(&[0, 5, 10]);
    // Hashes far apart: only the embeddings say the first two match.
    set_signature(&world, PhotoId(ids[0]), 0);
    set_signature(&world, PhotoId(ids[1]), u64::MAX);
    set_signature(&world, PhotoId(ids[2]), 0xFFFF_FFFF);
    set_clip(&world, ids[0], &[1.0, 0.05, 0.0], 6.5);
    set_clip(&world, ids[1], &[1.0, 0.0, 0.05], 4.5);
    set_clip(&world, ids[2], &[0.0, 1.0, 0.0], 5.5);
    for id in &ids {
        set_scores(&world, *id, 1_000.0, 1.0);
    }
    api.recompute_series().unwrap();
    assert_eq!(series(&world), [vec![ids[0], ids[1]]]);

    // Combined scores: technical overall 1.0 averaged with the aesthetic
    // score (6.5 → 1.0, 4.5 → 0.0, 5.5 → 0.5).
    let scores = world.catalog.scores.lock().unwrap().clone();
    let score = |id: i64| scores[&PhotoId(id)].score;
    assert!((score(ids[0]) - 1.0).abs() < 1e-6);
    assert!((score(ids[1]) - 0.5).abs() < 1e-6);
    assert!((score(ids[2]) - 0.75).abs() < 1e-6);
    let detail = api
        .call("get_photo", json!({ "photo_id": ids[2] }))
        .unwrap();
    assert!((detail["score"].as_f64().unwrap() - 0.75).abs() < 1e-6);
}

fn keepers(world: &TestWorld) -> Vec<i64> {
    let mut ids: Vec<i64> = world
        .catalog
        .scores
        .lock()
        .unwrap()
        .values()
        .filter(|s| s.keeper)
        .map(|s| s.id.0)
        .collect();
    ids.sort_unstable();
    ids
}

fn set_overall(world: &TestWorld, id: i64, overall: f32) {
    use crate::analysis::technical::TechnicalAnalyzer;
    world
        .catalog
        .store_analysis(
            PhotoId(id),
            TechnicalAnalyzer::ID,
            &json!({
                "sharpness": overall, "exposure": 1.0, "noise": 1.0, "overall": overall,
                "laplacian_variance": 1000.0 * overall, "highlights_clipped": 0.0,
                "shadows_clipped": 0.0, "mean_brightness": 0.5, "noise_sigma": 1.0,
            }),
        )
        .unwrap();
}

#[test]
fn keepers_are_the_good_best_and_single_photos() {
    // A burst of two, a good single photo and a poor one.
    let (world, api, ids) = setup(&[0, 1, 300, 600]);
    set_overall(&world, ids[0], 0.7);
    set_overall(&world, ids[1], 0.9);
    set_overall(&world, ids[2], 0.6);
    set_overall(&world, ids[3], 0.2);
    api.recompute_series().unwrap();
    assert_eq!(keepers(&world), [ids[1], ids[2]]);

    // Rejecting the best of the burst makes the other one the keeper.
    api.call(
        "set_decisions",
        json!({ "changes": [{ "photo_id": ids[1], "flag": "reject" }] }),
    )
    .unwrap();
    api.recompute_series().unwrap();
    assert_eq!(keepers(&world), [ids[0], ids[2]]);

    // A lower threshold takes the poor one too; switching off takes none.
    api.call(
        "update_settings",
        json!({ "auto_pick": { "enabled": true, "min_quality": 0.1 } }),
    )
    .unwrap();
    assert_eq!(keepers(&world), [ids[0], ids[2], ids[3]]);
    api.call(
        "update_settings",
        json!({ "auto_pick": { "enabled": false, "min_quality": 0.1 } }),
    )
    .unwrap();
    assert!(keepers(&world).is_empty());
    let err = api
        .call(
            "update_settings",
            json!({ "auto_pick": { "enabled": true, "min_quality": 1.5 } }),
        )
        .unwrap_err();
    assert_eq!(err.code, ApiErrorCode::InvalidArguments);
}
