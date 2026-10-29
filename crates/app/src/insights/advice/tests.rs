use super::*;

fn issue(issue: Issue, count: u32, weighted_share: f32) -> IssueInsight {
    IssueInsight {
        issue,
        count,
        share: weighted_share,
        weighted_share,
        examples: vec![7, 3],
    }
}

fn habit(issue: Issue, setting: InsightSetting, label: &str, lift: f32) -> SettingHabit {
    SettingHabit {
        issue,
        setting,
        label: label.into(),
        critiques: 12,
        share: 0.5,
        baseline: 0.5 / lift,
        lift,
    }
}

#[test]
fn only_frequent_problems_get_advice_and_other_never() {
    // (issue, count, weighted share, advised)
    let cases = [
        (Issue::Overexposed, 3, 0.10, true),
        (Issue::Overexposed, 2, 0.50, false), // too few photos
        (Issue::Overexposed, 9, 0.09, false), // too rare
        (Issue::Other, 20, 0.60, false),
        (Issue::SubjectPlacement, 5, 0.30, true),
    ];
    for (i, count, share, advised) in cases {
        let got = advise(&[issue(i, count, share)], &[]);
        assert_eq!(!got.is_empty(), advised, "{i:?} {count} {share}");
    }
}

#[test]
fn every_problem_the_critic_can_name_has_a_remedy_but_other() {
    for name in [
        "overexposed",
        "underexposed",
        "missed_focus",
        "motion_blur",
        "subject_placement",
        "subject_cut_off",
        "busy_background",
        "noise",
        "colour",
        "horizon",
        "light",
        "timing",
        "exposure",
        "focus",
        "composition",
    ] {
        let i: Issue = serde_json::from_value(serde_json::json!(name)).unwrap();
        let (title, text) = remedy(i).unwrap_or_else(|| panic!("{name}"));
        assert!(!title.is_empty() && text.len() > 40, "{name}");
    }
    assert!(remedy(Issue::Other).is_none());
}

#[test]
fn advice_keeps_the_frequency_order_and_is_capped() {
    let many: Vec<_> = [
        Issue::Overexposed,
        Issue::Noise,
        Issue::Horizon,
        Issue::Light,
        Issue::Timing,
        Issue::Colour,
        Issue::MotionBlur,
    ]
    .into_iter()
    .map(|i| issue(i, 10, 0.4))
    .collect();

    let got = advise(&many, &[]);

    let kinds: Vec<_> = got.iter().map(|a| a.issue).collect();
    assert_eq!(
        kinds,
        &[
            Issue::Overexposed,
            Issue::Noise,
            Issue::Horizon,
            Issue::Light,
            Issue::Timing
        ]
    );
    assert_eq!(got[0].examples, [7, 3]);
}

#[test]
fn the_strongest_habit_adds_what_to_change_at_that_setting() {
    let issues = [issue(Issue::MotionBlur, 14, 0.35)];
    let habits = [
        habit(
            Issue::MotionBlur,
            InsightSetting::FocalLength,
            "100 mm or longer",
            1.6,
        ),
        habit(
            Issue::MotionBlur,
            InsightSetting::Shutter,
            "1/30 s or longer",
            2.4,
        ),
        habit(Issue::Noise, InsightSetting::Iso, "ISO above 3200", 9.0),
    ];

    let got = &advise(&issues, &habits)[0];

    assert_eq!(got.title, "Motion blur is costing you sharp shots");
    assert!(
        got.text
            .ends_with("Most of them were shot with a slow shutter: use a faster one, or stabilisation and a support."),
        "{}",
        got.text
    );
    assert_eq!(
        got.evidence,
        "Named in 35% of your recent critiques (14 photos); 2.4× as often (1/30 s or longer)"
    );
}

#[test]
fn setting_advice_applies_only_to_the_buckets_it_is_true_for() {
    // (issue, setting, bucket, has a specific tip)
    let cases = [
        (
            Issue::MotionBlur,
            InsightSetting::Shutter,
            "1/30 s or longer",
            true,
        ),
        (
            Issue::MotionBlur,
            InsightSetting::Shutter,
            "1/250 s or faster",
            false,
        ),
        (Issue::Noise, InsightSetting::Iso, "ISO 200 or lower", false),
        (Issue::Noise, InsightSetting::Iso, "ISO above 3200", true),
        (
            Issue::Underexposed,
            InsightSetting::Iso,
            "ISO 200 or lower",
            true,
        ),
        (
            Issue::Underexposed,
            InsightSetting::Iso,
            "ISO above 3200",
            false,
        ),
        (
            Issue::MissedFocus,
            InsightSetting::Aperture,
            "wider than f/2.8",
            true,
        ),
        (
            Issue::MissedFocus,
            InsightSetting::Aperture,
            "f/11 or narrower",
            false,
        ),
        (
            Issue::Horizon,
            InsightSetting::Aperture,
            "wider than f/2.8",
            false,
        ),
    ];
    for (i, setting, range, expected) in cases {
        assert_eq!(
            at_setting(i, setting, range).is_some(),
            expected,
            "{i:?} {setting:?} {range}"
        );
    }
}

#[test]
fn a_habit_without_a_specific_rule_stays_in_the_evidence_only() {
    let issues = [issue(Issue::Horizon, 6, 0.2)];
    let habits = [habit(
        Issue::Horizon,
        InsightSetting::Iso,
        "ISO 801–3200",
        2.0,
    )];

    let got = &advise(&issues, &habits)[0];

    assert_eq!(got.text, remedy(Issue::Horizon).unwrap().1);
    assert!(got.evidence.ends_with("2.0× as often (ISO 801–3200)"));
}

#[test]
fn the_setting_tips_match_the_buckets_core_really_reports() {
    use picturesque_core::insights::{HALF_LIFE_DAYS, Sample, summarize};
    use picturesque_core::metadata::Exposure;
    use picturesque_core::photo::PhotoId;
    // Ten high-ISO photos, eight of them noisy, among ten clean low-ISO ones.
    let at = |id: i64, iso: u32, noisy: bool| Sample {
        id: PhotoId(id),
        rating: 5.0,
        issues: Some(if noisy { vec![Issue::Noise] } else { vec![] }),
        captured: None,
        exposure: Exposure {
            iso: Some(iso),
            ..Default::default()
        },
    };
    let mut samples: Vec<_> = (0..10).map(|i| at(i, 6400, i < 8)).collect();
    samples.extend((10..20).map(|i| at(i, 100, false)));
    let result = summarize(&samples, Default::default(), HALF_LIFE_DAYS);

    let habit = result
        .habits
        .iter()
        .find(|h| h.issue == Issue::Noise)
        .unwrap();

    assert!(
        at_setting(Issue::Noise, habit.setting.into(), habit.label).is_some(),
        "{}",
        habit.label
    );
}
