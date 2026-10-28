use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use picturesque_core::media::MediaFormat;
use picturesque_core::metadata::{CaptureMetadata, CaptureTime};
use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets, PhotoId};
use serde_json::{Value, json};

use super::*;
use crate::api::Api;
use crate::critic::DEFAULT_RUBRIC;
use crate::critic::ID;
use crate::critic::prompt::rubric_key;
use crate::ports::catalog::PhotoCatalog;
use crate::settings::{CriticEngine, CriticPreset, Settings};
use crate::testing::{FakeMetadata, MemFs, TestWorld, fake_hash};

const REPLY: &str = r#"{"rating": 6, "summary": "Fine."}"#;

/// A world with photos taken at the given seconds past 10:00, critiques
/// on from a server, background critiques at `intensity`.
fn setup(seconds: &[i64], intensity: CriticIntensity) -> (TestWorld, Api, Vec<PhotoId>) {
    let mut fs = MemFs::new().dir("/lib");
    for s in seconds {
        fs = fs.file(&format!("/lib/IMG_{s}.JPG"), s.to_string().as_bytes());
    }
    let world = TestWorld::new(fs, FakeMetadata::default());
    world
        .settings
        .save_settings(&Settings {
            library_root: Some("/lib".into()),
            critic: CriticSettings {
                enabled: true,
                background: true,
                backend: CriticEngine::Server,
                intensity,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
    let start = NaiveDate::from_ymd_opt(2026, 10, 12)
        .unwrap()
        .and_hms_opt(10, 0, 0)
        .unwrap();
    let ids = seconds
        .iter()
        .map(|s| {
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
                        path: PathBuf::from(format!("IMG_{s}.JPG")),
                        size: 1,
                        hash: fake_hash(s.to_string().as_bytes()),
                    }])
                    .unwrap(),
                })
                .unwrap()
        })
        .collect();
    *world.critic.models.lock().unwrap() = Some(vec!["qwen3-vl:4b".into()]);
    *world.critic.reply.lock().unwrap() = REPLY.into();
    let api = Api::new(world.context()).unwrap();
    (world, api, ids)
}

fn set_critic(api: &Api, critic: Value) {
    api.call("update_settings", json!({ "critic": critic }))
        .unwrap();
}

fn critic_json(enabled: bool, background: bool, intensity: &str, rubric: Value) -> Value {
    json!({ "enabled": enabled, "background": background, "backend": "server",
            "server": "http://127.0.0.1:11434/v1", "model": "qwen3-vl:4b",
            "rubric": rubric, "intensity": intensity })
}

/// Steps until finished; the photos critiqued, in order.
fn run_all(api: &Api) -> Vec<PhotoId> {
    let mut done = Vec::new();
    for _ in 0..50 {
        match api.background_critic().step().unwrap() {
            Step::Critiqued(id) => done.push(id),
            Step::Finished => return done,
            other => panic!("unexpected {other:?}"),
        }
    }
    panic!("never finished");
}

#[test]
fn is_off_unless_critiques_and_the_background_are_on() {
    let (_world, api, _ids) = setup(&[0], CriticIntensity::Full);
    for (enabled, background) in [(false, true), (true, false), (false, false)] {
        set_critic(&api, critic_json(enabled, background, "full", Value::Null));
        assert_eq!(api.background_critic().step().unwrap(), Step::Off);
        assert!(!api.background_critic().status().unwrap().enabled);
    }
    set_critic(&api, critic_json(true, true, "full", Value::Null));
    assert!(api.background_critic().status().unwrap().enabled);
}

#[test]
fn works_through_photos_in_order() {
    // Series A (0, 5, 10), series B (200, 205), singles and a reject.
    let (_world, api, ids) = setup(
        &[0, 5, 10, 200, 205, 1000, 2000, 3000],
        CriticIntensity::Full,
    );
    let [a1, a2, a3, b1, b2, s1, s2, reject] = ids[..] else {
        panic!("eight photos");
    };
    api.recompute_series().unwrap();
    api.call("choose_best", json!({ "photo_id": a2.0, "best": true }))
        .unwrap();
    api.call("choose_best", json!({ "photo_id": b1.0, "best": true }))
        .unwrap();
    api.call(
        "set_decisions",
        json!({ "changes": [{ "photo_id": a3.0, "flag": "pick" },
                            { "photo_id": reject.0, "flag": "reject" }] }),
    )
    .unwrap();
    api.recompute_series().unwrap();
    // On screen: a1 would otherwise come late.
    api.background_critic().prioritize(&[a1]);

    // Priority, then best-of-series and picks, singles, the rest; each
    // newest first; the reject never.
    assert_eq!(run_all(&api), [a1, b1, a3, a2, s2, s1, b2]);
    let status = api.background_critic().status().unwrap();
    assert_eq!((status.critiqued, status.missing, status.total), (7, 0, 7));
}

#[test]
fn redoes_stale_critiques_only_after_photos_without_one() {
    let (world, api, ids) = setup(&[0, 1000, 2000], CriticIntensity::Full);
    // The newest has a critique by another model.
    world
        .catalog
        .store_analysis(
            ids[2],
            ID,
            &json!({ "model": "other", "rubric": "x",
                     "critique": { "rating": 5.0, "summary": "Old." } }),
        )
        .unwrap();
    let status = api.background_critic().status().unwrap();
    assert_eq!((status.stale, status.missing, status.critiqued), (1, 2, 0));
    assert_eq!(run_all(&api), [ids[1], ids[0], ids[2]]);
}

#[test]
fn gentle_waits_on_battery_and_full_does_not() {
    let (world, api, _ids) = setup(&[0], CriticIntensity::Gentle);
    world.power.0.store(true, Ordering::Relaxed);
    let step = api.background_critic().step().unwrap();
    assert_eq!(step, Step::Paused("Paused on battery power.".into()));
    let status = api.background_critic().status().unwrap();
    assert_eq!(status.paused.as_deref(), Some("Paused on battery power."));
    assert!(world.critic.asked.lock().unwrap().is_empty());

    set_critic(&api, critic_json(true, true, "full", Value::Null));
    assert!(matches!(
        api.background_critic().step().unwrap(),
        Step::Critiqued(_)
    ));
    // On mains again, gentle works.
    set_critic(&api, critic_json(true, true, "gentle", Value::Null));
    world.power.0.store(false, Ordering::Relaxed);
    assert_eq!(api.background_critic().step().unwrap(), Step::Finished);
}

#[test]
fn when_idle_waits_for_the_app_to_rest() {
    let (_world, api, ids) = setup(&[0], CriticIntensity::WhenIdle);
    assert!(matches!(
        api.background_critic().step().unwrap(),
        Step::Paused(_)
    ));
    api.activity()
        .since(Instant::now() - IDLE_AFTER - Duration::from_secs(1));
    assert_eq!(
        api.background_critic().step().unwrap(),
        Step::Critiqued(ids[0])
    );
}

#[test]
fn skips_failed_photos_until_the_rubric_changes() {
    let (world, api, ids) = setup(&[0], CriticIntensity::Full);
    *world.critic.reply.lock().unwrap() = "I cannot say.".into();
    assert!(matches!(
        api.background_critic().step().unwrap(),
        Step::Failed(id, _) if id == ids[0]
    ));
    assert_eq!(api.background_critic().step().unwrap(), Step::Finished);
    assert_eq!(api.background_critic().status().unwrap().failed, 1);
    assert_eq!(world.critic.asked.lock().unwrap().len(), 1);

    set_critic(
        &api,
        critic_json(true, true, "full", json!("Only the light.")),
    );
    assert!(matches!(
        api.background_critic().step().unwrap(),
        Step::Failed(..)
    ));
    assert_eq!(world.critic.asked.lock().unwrap().len(), 2);
}

#[test]
fn status_counts_photos_and_estimates_the_time_left() {
    let (_world, api, _ids) = setup(&[0, 1000, 2000], CriticIntensity::Full);
    let before = api.background_critic().status().unwrap();
    assert_eq!(
        (before.critiqued, before.stale, before.missing, before.total),
        (0, 0, 3, 3)
    );
    assert_eq!(
        (before.seconds_per_photo, before.seconds_left),
        (None, None)
    );
    assert!(matches!(
        api.background_critic().step().unwrap(),
        Step::Critiqued(_)
    ));
    let after = api.background_critic().status().unwrap();
    assert_eq!(
        (after.critiqued, after.stale, after.missing, after.total),
        (1, 0, 2, 3)
    );
    assert!(after.seconds_per_photo.is_some());
    assert!(after.seconds_left.is_some());
}

#[test]
fn a_users_critique_interrupts_a_running_background_one() {
    let (world, api, ids) = setup(&[0], CriticIntensity::Full);
    api.critique_service().critique(ids[0], false).unwrap();
    assert_eq!(world.critic.interrupts.load(Ordering::Relaxed), 0);

    api.critique_service().mark_background_running();
    api.critique_service().critique(ids[0], true).unwrap();
    assert_eq!(world.critic.interrupts.load(Ordering::Relaxed), 1);
}

/// 26 photos, far apart in time, all with the same CLIP aesthetic and each
/// a critique of a different rating; returns the aesthetic scores.
fn aesthetic_scores(critiques_on: bool) -> (Vec<f32>, f32) {
    let seconds: Vec<i64> = (0..26).map(|i| i * 1000).collect();
    let (world, api, ids) = setup(&seconds, CriticIntensity::Full);
    for (i, id) in ids.iter().enumerate() {
        let e = picturesque_core::clip::Embedding::new(&[1.0, 0.0, 0.0]).unwrap();
        world
            .catalog
            .store_analysis(
                *id,
                crate::analysis::clip::ClipAnalyzer::ID,
                &json!({ "embedding": e.encode(), "aesthetic": 5.0 }),
            )
            .unwrap();
        world
            .catalog
            .store_analysis(
                *id,
                crate::analysis::technical::TechnicalAnalyzer::ID,
                &json!({
                    "sharpness": 1.0, "exposure": 1.0, "noise": 1.0, "overall": 1.0,
                    "laplacian_variance": 5_000.0, "highlights_clipped": 0.0,
                    "shadows_clipped": 0.0, "mean_brightness": 0.5, "noise_sigma": 1.0,
                }),
            )
            .unwrap();
        world
            .catalog
            .store_analysis(
                *id,
                ID,
                &json!({ "model": "m", "rubric": "r",
                         "critique": { "rating": 1.0 + i as f32 * 0.3, "summary": "s" } }),
            )
            .unwrap();
    }
    set_critic(&api, critic_json(critiques_on, false, "full", Value::Null));
    api.recompute_series().unwrap();
    let scores = world.catalog.scores.lock().unwrap();
    let general = crate::analysis::clip::aesthetic_score(5.0);
    (
        ids.iter().map(|id| scores[id].aesthetic.unwrap()).collect(),
        general,
    )
}

#[test]
fn critiques_move_the_aesthetic_score_by_the_weight() {
    let (with, general) = aesthetic_scores(true);
    let weight = CriticSettings::default().score_weight as f32;
    for (i, got) in with.iter().enumerate() {
        // Percentile of the i-th of 26 distinct ratings.
        let percentile = (i as f32 + 0.5) / 26.0;
        let want = general + (percentile - general) * weight;
        assert!((got - want).abs() < 1e-4, "{i}: {got} vs {want}");
    }
    let (without, general) = aesthetic_scores(false);
    assert!(without.iter().all(|s| (s - general).abs() < 1e-6));
}

fn store_critique(world: &TestWorld, id: PhotoId, rating: f64) {
    world
        .catalog
        .store_analysis(
            id,
            ID,
            &json!({ "model": "m", "rubric": "r",
                     "critique": { "rating": rating, "summary": "s" } }),
        )
        .unwrap();
}

#[test]
fn an_unreachable_server_pauses_without_failing_the_photo() {
    let (world, api, ids) = setup(&[0, 1000], CriticIntensity::Full);
    world.critic.ask_unreachable.store(true, Ordering::Relaxed);
    let step = api.background_critic().step().unwrap();
    assert!(
        matches!(&step, Step::Paused(r) if r.contains("connection lost")),
        "{step:?}"
    );
    let status = api.background_critic().status().unwrap();
    assert_eq!((status.failed, status.missing), (0, 2));
    assert!(status.paused.is_some());
    world.critic.ask_unreachable.store(false, Ordering::Relaxed);
    assert!(matches!(
        api.background_critic().step().unwrap(),
        Step::Critiqued(id) if ids.contains(&id)
    ));
    assert_eq!(api.background_critic().status().unwrap().paused, None);
}

#[test]
fn three_failures_in_a_row_pause_and_a_settings_change_resets() {
    let (world, api, _ids) = setup(&[0, 1000, 2000, 3000], CriticIntensity::Full);
    *world.critic.reply.lock().unwrap() = "I cannot say.".into();
    for _ in 0..2 {
        assert!(matches!(
            api.background_critic().step().unwrap(),
            Step::Failed(..)
        ));
    }
    let step = api.background_critic().step().unwrap();
    assert!(
        matches!(&step, Step::Paused(r) if r.starts_with("Several critiques in a row failed: ")),
        "{step:?}"
    );
    assert_eq!(api.background_critic().status().unwrap().failed, 3);

    // Turning the background critic off and on tries the photos again.
    set_critic(&api, critic_json(true, false, "full", Value::Null));
    assert_eq!(api.background_critic().step().unwrap(), Step::Off);
    set_critic(&api, critic_json(true, true, "full", Value::Null));
    assert!(matches!(
        api.background_critic().step().unwrap(),
        Step::Failed(..)
    ));
    assert_eq!(api.background_critic().status().unwrap().failed, 1);
}

#[test]
fn stopping_interrupts_a_running_critique() {
    let (world, api, _ids) = setup(&[0], CriticIntensity::Full);
    api.background_critic().stop();
    assert_eq!(world.critic.interrupts.load(Ordering::Relaxed), 0);
    api.critique_service().mark_background_running();
    api.background_critic().stop();
    assert_eq!(world.critic.interrupts.load(Ordering::Relaxed), 1);
}

#[test]
fn a_photo_that_gave_way_keeps_its_place_on_screen() {
    let (_world, api, ids) = setup(&[0, 1000, 2000], CriticIntensity::Full);
    api.background_critic().prioritize(&[ids[0]]);
    // A critique the user asked for is waiting.
    api.critique_service()
        .interactive
        .fetch_add(1, Ordering::AcqRel);
    assert_eq!(
        api.background_critic().step().unwrap(),
        Step::Yielded(ids[0])
    );
    api.critique_service()
        .interactive
        .fetch_sub(1, Ordering::AcqRel);
    assert_eq!(
        api.background_critic().step().unwrap(),
        Step::Critiqued(ids[0])
    );
}

#[test]
fn a_users_critique_updates_the_scores() {
    let seconds: Vec<i64> = (0..25).map(|i| i * 1000).collect();
    let (world, api, ids) = setup(&seconds, CriticIntensity::Full);
    for (i, id) in ids[..24].iter().enumerate() {
        store_critique(&world, *id, 1.0 + i as f64 * 0.3);
    }
    api.series_service().start();
    let last = ids[24];
    api.recompute_series().unwrap();
    assert_eq!(api.series_service().critique_score(last), None);
    api.call(
        "critique_photo",
        json!({ "photo_id": last.0, "refresh": false }),
    )
    .unwrap();
    let start = Instant::now();
    while api.series_service().critique_score(last).is_none() {
        assert!(start.elapsed() < Duration::from_secs(10), "never updated");
        std::thread::sleep(Duration::from_millis(10));
    }
    api.series_service().stop();
}

fn store_critique_with_potential(world: &TestWorld, id: PhotoId, rating: f64, potential: f64) {
    world
        .catalog
        .store_analysis(
            id,
            ID,
            &json!({ "model": "m", "rubric": "r",
                     "critique": { "rating": rating, "potential": potential,
                                   "summary": "s" } }),
        )
        .unwrap();
}

/// A photo with the given files, inserted without a capture time.
fn insert_with(world: &TestWorld, files: &[(MediaFormat, &str)]) -> PhotoId {
    let assets = files
        .iter()
        .map(|(format, path)| Asset {
            format: *format,
            path: PathBuf::from(path),
            size: 1,
            hash: fake_hash(path.as_bytes()),
        })
        .collect();
    world
        .catalog
        .insert_photo(&NewPhoto {
            metadata: CaptureMetadata::default(),
            assets: PhotoAssets::new(assets).unwrap(),
        })
        .unwrap()
}

#[test]
fn a_raw_photos_potential_lifts_its_score_by_the_weight_and_never_redoes_critiques() {
    let (world, api, ids) = setup(&[0], CriticIntensity::Full);
    let jpeg = ids[0];
    let raw = insert_with(&world, &[(MediaFormat::Raw, "IMG_RAW.CR2")]);
    // A RAW+JPEG pair still has the RAW to edit.
    let pair = insert_with(
        &world,
        &[
            (MediaFormat::Raw, "IMG_PAIR.CR2"),
            (MediaFormat::Jpeg, "IMG_PAIR.JPG"),
        ],
    );
    for id in [jpeg, raw, pair] {
        store_critique_with_potential(&world, id, 6.0, 9.0);
    }
    let series = api.series_service();

    api.recompute_series().unwrap();
    let score = |id| series.critique_score(id).unwrap();
    assert!(
        score(raw) > score(jpeg),
        "{} <= {}",
        score(raw),
        score(jpeg)
    );
    assert_eq!(score(pair), score(raw));
    assert_eq!(series.critique_lift(jpeg), None);
    assert_eq!(series.critique_lift(raw), Some((6.0, 9.0)));
    assert_eq!(series.critique_lift(pair), Some((6.0, 9.0)));
    let stored = world.catalog.analysis_results(ID).unwrap();

    // Weight 0 turns it off: same scores, no lift, nothing to redo.
    let mut critic = critic_json(true, true, "full", Value::Null);
    critic["potential_weight"] = json!(0.0);
    set_critic(&api, critic);
    api.recompute_series().unwrap();
    assert_eq!(score(raw), score(jpeg));
    assert_eq!(score(pair), score(jpeg));
    assert_eq!(series.critique_lift(raw), None);
    assert_eq!(world.catalog.analysis_results(ID).unwrap(), stored);
    assert!(world.critic.asked.lock().unwrap().is_empty());
}

#[test]
fn rejected_photos_stay_out_of_the_percentiles() {
    let seconds: Vec<i64> = (0..22).map(|i| i * 1000).collect();
    let (world, api, ids) = setup(&seconds, CriticIntensity::Full);
    for (i, id) in ids[..21].iter().enumerate() {
        store_critique(&world, *id, 2.0 + i as f64 * 0.1);
    }
    // An extreme critique of a reject.
    store_critique(&world, ids[21], 10.0);
    api.call(
        "set_decisions",
        json!({ "changes": [{ "photo_id": ids[21].0, "flag": "reject" }] }),
    )
    .unwrap();
    api.recompute_series().unwrap();
    let series = api.series_service();
    assert_eq!(series.critique_score(ids[21]), None);
    let best = series.critique_score(ids[20]).unwrap();
    assert!((best - 20.5 / 21.0).abs() < 1e-4, "{best}");
}

#[test]
fn a_stopped_critic_starts_no_critique() {
    let (world, api, _ids) = setup(&[0], CriticIntensity::Full);
    api.background_critic().stop();
    assert_eq!(api.background_critic().step().unwrap(), Step::Off);
    assert!(world.critic.asked.lock().unwrap().is_empty());
}

#[test]
fn critiques_of_another_preset_are_stale_and_redone() {
    let (world, api, ids) = setup(&[0], CriticIntensity::Full);
    let stored = |preset| {
        json!({ "model": "qwen3-vl:4b", "rubric": rubric_key(preset, DEFAULT_RUBRIC),
                "critique": { "rating": 5.0, "summary": "Old." } })
    };
    // (stored preset, current preset, stale)
    let cases = [
        (CriticPreset::Professional, "professional", false),
        (CriticPreset::Professional, "amateur", true),
        (CriticPreset::Amateur, "amateur", false),
        (CriticPreset::Amateur, "beginner", true),
        (CriticPreset::Beginner, "professional", true),
    ];
    for (was, now, stale) in cases {
        world
            .catalog
            .store_analysis(ids[0], ID, &stored(was))
            .unwrap();
        let mut critic = critic_json(true, true, "full", Value::Null);
        critic["preset"] = json!(now);
        set_critic(&api, critic);
        let status = api.background_critic().status().unwrap();
        assert_eq!(
            (status.stale, status.critiqued),
            (usize::from(stale), usize::from(!stale)),
            "{was:?} -> {now}"
        );
    }
}
