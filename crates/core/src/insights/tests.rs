use chrono::{Duration, NaiveDate};

use super::*;

fn now() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 10, 1)
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap()
}

fn iso(iso: u32) -> Exposure {
    Exposure {
        iso: Some(iso),
        ..Default::default()
    }
}

fn sample(
    id: i64,
    rating: f32,
    issues: Option<&[&'static str]>,
    days_ago: i64,
) -> Sample<&'static str> {
    Sample {
        id: PhotoId(id),
        rating,
        issues: issues.map(<[_]>::to_vec),
        captured: Some(now() - Duration::days(days_ago)),
        exposure: Exposure::default(),
    }
}

fn stat<'a>(i: &'a Insights<&'static str>, issue: &str) -> &'a IssueStat<&'static str> {
    i.issues.iter().find(|s| s.issue == issue).unwrap()
}

#[test]
fn weight_halves_every_half_life_and_ignores_the_future() {
    // (age in days, half-life, expected)
    let cases = [
        (0.0, 365.0, 1.0),
        (365.0, 365.0, 0.5),
        (730.0, 365.0, 0.25),
        (180.0, 180.0, 0.5),
        (-30.0, 365.0, 1.0),
    ];
    for (age, half_life, expected) in cases {
        let got = weight(age, half_life);
        assert!((got - expected).abs() < 1e-9, "{age} {half_life}: {got}");
    }
}

#[test]
fn recent_critiques_count_more_than_old_ones() {
    // Four old noise problems against one new focus problem.
    let mut samples: Vec<_> = (0..4)
        .map(|i| sample(i, 5.0, Some(&["noise"]), 3 * 365))
        .collect();
    samples.push(sample(9, 5.0, Some(&["focus"]), 0));

    let i = summarize(&samples, now(), HALF_LIFE_DAYS);

    assert!((stat(&i, "noise").share - 0.8).abs() < 1e-6);
    assert!((stat(&i, "focus").share - 0.2).abs() < 1e-6);
    // Three half-lives old: each old critique weighs 1/8.
    assert!((stat(&i, "focus").weighted_share - 2.0 / 3.0).abs() < 1e-4);
    assert!((stat(&i, "noise").weighted_share - 1.0 / 3.0).abs() < 1e-4);
    assert_eq!(i.issues[0].issue, "focus");
    // With a half-life of a century nothing fades.
    let flat = summarize(&samples, now(), 36_500.0);
    assert_eq!(flat.issues[0].issue, "noise");
    assert!(i.weighted_rating.unwrap() > 0.0);
}

#[test]
fn critiques_without_issues_stay_out_of_the_issue_statistics() {
    let samples = [
        sample(1, 4.0, Some(&["focus"]), 0),
        sample(2, 4.0, Some(&[]), 0),
        sample(3, 9.0, None, 0),
    ];

    let i = summarize(&samples, now(), HALF_LIFE_DAYS);

    assert_eq!((i.critiques, i.with_issues), (3, 2));
    assert!((stat(&i, "focus").share - 0.5).abs() < 1e-6);
    // The rating still counts them all.
    assert!((i.mean_rating.unwrap() - 17.0 / 3.0).abs() < 1e-5);
}

#[test]
fn critiques_that_name_no_problem_give_no_issues() {
    let samples = [sample(1, 9.0, Some(&[]), 0), sample(2, 8.0, Some(&[]), 30)];

    let i = summarize(&samples, now(), HALF_LIFE_DAYS);

    assert_eq!((i.critiques, i.with_issues), (2, 2));
    assert!(i.issues.is_empty() && i.habits.is_empty());
}

#[test]
fn photos_without_a_capture_time_weigh_half_and_stay_out_of_the_trend() {
    let undated = |id, rating, issue: &'static str| Sample {
        captured: None,
        ..sample(id, rating, Some(&[issue]), 0)
    };
    let samples = [undated(1, 4.0, "focus"), undated(2, 6.0, "noise")];

    let i = summarize(&samples, now(), HALF_LIFE_DAYS);

    assert!(i.trend.is_empty() && i.trend_slope.is_none());
    assert!((i.weighted_rating.unwrap() - 5.0).abs() < 1e-5);
    assert!((stat(&i, "focus").weighted_share - 0.5).abs() < 1e-6);
}

#[test]
fn nothing_to_summarize_gives_empty_insights() {
    let i = summarize::<&str>(&[], now(), HALF_LIFE_DAYS);

    assert_eq!(
        (i.critiques, i.mean_rating, i.weighted_rating),
        (0, None, None)
    );
    assert!(i.issues.is_empty() && i.trend.is_empty() && i.habits.is_empty());
    assert_eq!(i.trend_slope, None);
}

#[test]
fn examples_are_the_worst_recent_photos_first_and_capped() {
    // id, rating, days ago
    let rows = [
        (1, 3.0, 0),
        (2, 3.0, 365),
        (3, 8.0, 0),
        (4, 1.0, 730),
        (5, 3.0, 0),
    ];
    let samples: Vec<_> = rows
        .iter()
        .map(|(id, r, d)| sample(*id, *r, Some(&["focus"]), *d))
        .collect();

    let i = summarize(&samples, now(), HALF_LIFE_DAYS);

    // Severity: w × (10 − rating) = 7, 3.5, 2, 2.25, 7 (ties by id).
    let ids: Vec<i64> = stat(&i, "focus").examples.iter().map(|p| p.0).collect();
    assert_eq!(ids, [1, 5, 2, 4, 3]);
    let many: Vec<_> = (0..20)
        .map(|id| sample(id, 4.0, Some(&["focus"]), 0))
        .collect();
    assert_eq!(
        stat(&summarize(&many, now(), HALF_LIFE_DAYS), "focus")
            .examples
            .len(),
        EXAMPLES
    );
}

#[test]
fn the_trend_is_per_quarter_and_needs_three_quarters_for_a_slope() {
    // days ago, rating: Q3 2025 → Q3 2026 improving.
    let rising = [(400, 4.0), (310, 5.0), (220, 6.0), (130, 7.0), (10, 8.0)];
    let samples: Vec<_> = rising
        .iter()
        .enumerate()
        .map(|(i, (d, r))| sample(i as i64, *r, Some(&[]), *d))
        .collect();

    let i = summarize(&samples, now(), HALF_LIFE_DAYS);

    assert_eq!(i.trend.len(), 5);
    assert!(
        i.trend
            .windows(2)
            .all(|w| (w[0].year, w[0].quarter) < (w[1].year, w[1].quarter))
    );
    assert!(i.trend_slope.unwrap() > 1.0, "{:?}", i.trend_slope);
    // Falling ratings give a negative slope.
    let falling: Vec<_> = samples
        .iter()
        .map(|s| Sample {
            rating: 10.0 - s.rating,
            ..s.clone()
        })
        .collect();
    assert!(
        summarize(&falling, now(), HALF_LIFE_DAYS)
            .trend_slope
            .unwrap()
            < -1.0
    );
    // Two quarters are too few.
    let two = summarize(&samples[3..], now(), HALF_LIFE_DAYS);
    assert_eq!((two.trend.len(), two.trend_slope), (2, None));
}

/// `bucket_n` photos at ISO 6400, `flagged` of them with noise, among 10
/// photos at ISO 100 of which 1 has it.
fn iso_world(bucket_n: i64, flagged: i64) -> Vec<Sample<&'static str>> {
    let mut samples = Vec::new();
    for id in 0..bucket_n {
        let issues: &[&str] = if id < flagged { &["noise"] } else { &[] };
        samples.push(Sample {
            exposure: iso(6400),
            ..sample(id, 5.0, Some(issues), 0)
        });
    }
    for id in 0..10 {
        let issues: &[&str] = if id == 0 { &["noise"] } else { &[] };
        samples.push(Sample {
            exposure: iso(100),
            ..sample(100 + id, 5.0, Some(issues), 0)
        });
    }
    samples
}

#[test]
fn habits_need_enough_photos_enough_flags_and_a_clear_lift() {
    // (photos at ISO 6400, flagged, habit expected)
    let cases = [
        (8, 6, true),
        (7, 6, false), // too few photos
        (8, 2, false), // too few flagged
        (8, 3, true),  // 3 of 8 against a baseline of 4/18: lift 1.69
        (8, 1, false), // no more often than usual
    ];
    for (n, flagged, expected) in cases {
        let i = summarize(&iso_world(n, flagged), now(), HALF_LIFE_DAYS);
        let found = i.habits.iter().any(|h| {
            h.issue == "noise" && h.setting == Setting::Iso && h.label == "ISO above 3200"
        });
        assert_eq!(found, expected, "{n} photos, {flagged} flagged");
    }
    let i = summarize(&iso_world(8, 6), now(), HALF_LIFE_DAYS);
    let h = &i.habits[0];
    assert_eq!((h.critiques, h.setting), (8, Setting::Iso));
    assert!((h.share - 0.75).abs() < 1e-6 && (h.baseline - 7.0 / 18.0).abs() < 1e-6);
    assert!((h.lift - 0.75 / (7.0 / 18.0)).abs() < 1e-5);
}

#[test]
fn setting_buckets_cover_their_edges() {
    let at = |setting, e: Exposure| bucket(setting, &e).map(|(i, _)| i);
    let time = |n, d| Exposure {
        exposure_time: Some(crate::metadata::ExposureTime {
            numerator: n,
            denominator: d,
        }),
        ..Default::default()
    };
    let f = |v| Exposure {
        f_number: Some(v),
        ..Default::default()
    };
    let mm = |v| Exposure {
        focal_length_mm: Some(v),
        ..Default::default()
    };
    assert_eq!(
        [200, 201, 800, 801, 3200, 3201].map(|v| at(Setting::Iso, iso(v))),
        [0, 1, 1, 2, 2, 3].map(Some)
    );
    assert_eq!(
        [(1, 2), (1, 30), (1, 31), (1, 250), (1, 251)]
            .map(|(n, d)| at(Setting::Shutter, time(n, d))),
        [0, 0, 1, 2, 2].map(Some)
    );
    assert_eq!(
        [1.8, 2.8, 5.6, 11.0].map(|v| at(Setting::Aperture, f(v))),
        [0, 1, 2, 3].map(Some)
    );
    assert_eq!(
        [16.0, 24.0, 50.0, 100.0].map(|v| at(Setting::FocalLength, mm(v))),
        [0, 1, 2, 3].map(Some)
    );
    // Unknown or nonsense settings fall in no bucket.
    assert_eq!(at(Setting::Iso, Exposure::default()), None);
    assert_eq!(at(Setting::Shutter, time(0, 1)), None);
    assert_eq!(at(Setting::Aperture, f(0.0)), None);
}
