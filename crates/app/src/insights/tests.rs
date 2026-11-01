use std::path::PathBuf;

use chrono::NaiveDate;
use picturesque_core::media::MediaFormat;
use picturesque_core::metadata::{CaptureMetadata, CaptureTime, Exposure};
use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets, PhotoId};
use serde_json::{Value, json};

use super::*;
use crate::api::{Api, ApiErrorCode};
use crate::critic::ID;
use crate::testing::{FakeMetadata, MemFs, TestWorld, fake_hash};

fn world() -> TestWorld {
    TestWorld::new(MemFs::new(), FakeMetadata::default())
}

/// A photo taken `days` before 2026-10-01 at the given ISO.
fn add_photo(world: &TestWorld, name: &str, days: i64, iso: u32) -> PhotoId {
    let taken = NaiveDate::from_ymd_opt(2026, 10, 1)
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap()
        - chrono::Duration::days(days);
    world
        .catalog
        .insert_photo(&NewPhoto {
            metadata: CaptureMetadata {
                captured_at: Some(CaptureTime::new(taken, None)),
                exposure: Exposure {
                    iso: Some(iso),
                    ..Default::default()
                },
                ..Default::default()
            },
            assets: PhotoAssets::new(vec![Asset {
                format: MediaFormat::Jpeg,
                path: PathBuf::from(name),
                size: 1,
                hash: fake_hash(name.as_bytes()),
            }])
            .unwrap(),
        })
        .unwrap()
}

fn critique(world: &TestWorld, id: PhotoId, critique: Value) {
    world
        .catalog
        .store_analysis(
            id,
            ID,
            &json!({ "model": "m", "rubric": "r", "critique": critique }),
        )
        .unwrap();
}

#[test]
fn a_library_without_critiques_has_empty_insights() {
    let w = world();
    add_photo(&w, "a.jpg", 0, 100);

    let i = library_insights(&*w.catalog).unwrap();

    assert_eq!((i.critiques, i.with_issues, i.min_critiques), (0, 0, 10));
    assert_eq!(
        (i.as_of, i.mean_rating, i.weighted_rating),
        (None, None, None)
    );
    assert!(i.issues.is_empty() && i.trend.is_empty() && i.habits.is_empty());
}

#[test]
fn photos_without_a_capture_date_have_no_as_of_and_no_trend() {
    let w = world();
    let id = w
        .catalog
        .insert_photo(&NewPhoto {
            metadata: CaptureMetadata::default(),
            assets: PhotoAssets::new(vec![Asset {
                format: MediaFormat::Jpeg,
                path: PathBuf::from("undated.jpg"),
                size: 1,
                hash: fake_hash(b"undated"),
            }])
            .unwrap(),
        })
        .unwrap();
    critique(
        &w,
        id,
        json!({ "rating": 5.0, "summary": "s", "issues": ["focus"] }),
    );

    let i = library_insights(&*w.catalog).unwrap();

    assert_eq!((i.critiques, i.as_of), (1, None));
    assert!(i.trend.is_empty());
    assert_eq!(i.issues[0].issue, Issue::Focus);
}

#[test]
fn stored_critiques_become_issues_trend_and_examples() {
    let w = world();
    let recent = add_photo(&w, "recent.jpg", 0, 6400);
    let older = add_photo(&w, "older.jpg", 400, 100);
    let before_v4 = add_photo(&w, "old-prompt.jpg", 10, 100);
    let unrated = add_photo(&w, "unrated.jpg", 5, 100);
    critique(
        &w,
        recent,
        json!({ "rating": 3.0, "summary": "s", "issues": ["noise", "unknown_kind", "noise"] }),
    );
    critique(
        &w,
        older,
        json!({ "rating": 8.0, "summary": "s", "issues": ["focus", "noise"] }),
    );
    // A critique from before prompt version 4 has no `issues`.
    critique(&w, before_v4, json!({ "rating": 7.0, "summary": "s" }));
    critique(&w, unrated, json!({ "summary": "s" }));

    let i = library_insights(&*w.catalog).unwrap();

    assert_eq!((i.critiques, i.with_issues), (3, 2));
    assert_eq!(i.as_of.as_deref(), Some("2026-10-01"));
    assert!((i.mean_rating.unwrap() - 6.0).abs() < 1e-5);
    // Noise is in both critiques with categories, focus in one; unknown
    // categories and repeats are dropped.
    let issues: Vec<_> = i.issues.iter().map(|s| (s.issue, s.count)).collect();
    assert_eq!(issues, [(Issue::Noise, 2), (Issue::Focus, 1)]);
    // The recent, low-rated photo is the clearest example of noise.
    assert_eq!(i.issues[0].examples, [recent.0, older.0]);
    let quarters: Vec<_> = i.trend.iter().map(|q| q.quarter.as_str()).collect();
    assert_eq!(quarters, ["2025-Q3", "2026-Q3", "2026-Q4"]);
    // 8, then 7, then 3: getting worse.
    assert!(i.trend_slope.unwrap() < 0.0, "{:?}", i.trend_slope);
}

#[test]
fn the_command_takes_no_arguments_and_serializes_the_shape() {
    let w = world();
    let id = add_photo(&w, "a.jpg", 0, 100);
    critique(
        &w,
        id,
        json!({ "rating": 4.0, "summary": "s", "issues": ["exposure"] }),
    );
    let api = Api::new(w.context()).unwrap();

    let v = api.call("library_insights", Value::Null).unwrap();

    assert_eq!(v["critiques"], 1);
    assert_eq!(v["issues"][0]["issue"], "exposure");
    assert_eq!(v["issues"][0]["examples"], json!([id.0]));
    assert_eq!(v["trend"][0]["quarter"], "2026-Q4");
    assert_eq!(v["half_life_days"], 365.0);
    let err = api.call("library_insights", json!({ "x": 1 })).unwrap_err();
    assert_eq!(err.code, ApiErrorCode::InvalidArguments);
}
