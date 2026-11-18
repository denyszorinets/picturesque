//! Human review is independent of decisions, exports, series and XMP.
use chrono::NaiveDate;
use picturesque_app::library::PhotoQuery;
use picturesque_app::ports::catalog::PhotoCatalog;
use picturesque_app::review::{ApplyReviewArgs, ReviewService};
use picturesque_core::decision::{Decision, Flag};
use picturesque_core::hash::ContentHash;
use picturesque_core::media::MediaFormat;
use picturesque_core::metadata::{CaptureMetadata, CaptureTime};
use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets, PhotoId};
use picturesque_infra::catalog::SqliteCatalog;
use std::sync::Arc;

fn photo(n: u64, day: u32) -> NewPhoto {
    let asset = |suffix: &str, format, h: u64| {
        let mut hash = [0; 32];
        hash[..8].copy_from_slice(&h.to_le_bytes());
        Asset {
            format,
            path: format!("photo-{n}.{suffix}").into(),
            size: 100,
            hash: ContentHash::from_bytes(hash),
        }
    };
    NewPhoto {
        metadata: CaptureMetadata {
            captured_at: Some(CaptureTime::new(
                NaiveDate::from_ymd_opt(2026, 11, day)
                    .unwrap()
                    .and_hms_opt(12, 0, 0)
                    .unwrap(),
                None,
            )),
            ..Default::default()
        },
        assets: PhotoAssets::new(vec![
            asset("RAF", MediaFormat::Raw, n * 2),
            asset("JPG", MediaFormat::Jpeg, n * 2 + 1),
        ])
        .unwrap(),
    }
}

#[test]
fn review_is_persistent_reversible_atomic_and_does_not_dirty_sidecars() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("catalog.sqlite");
    let c = SqliteCatalog::open(&path).unwrap();
    let a = c.insert_photo(&photo(1, 17)).unwrap();
    let b = c.insert_photo(&photo(2, 18)).unwrap();
    let decision = Decision {
        flag: Flag::Pick,
        ..Default::default()
    };
    c.set_decision_saved(a, decision).unwrap();
    assert_eq!(c.set_reviewed(&[a], true).unwrap(), 1);
    assert_eq!(c.set_reviewed(&[a], true).unwrap(), 0);
    assert!(c.set_reviewed(&[b, PhotoId(9999)], true).is_err());
    assert!(c.dirty_decisions().unwrap().is_empty());
    assert_eq!(c.decision(a).unwrap(), decision);
    drop(c);
    let c = SqliteCatalog::open(&path).unwrap();
    let q = PhotoQuery {
        reviewed: Some(true),
        ..Default::default()
    };
    assert_eq!(c.query(&q).unwrap().photos[0].id, a.0);
    assert_eq!(c.facets(Some(&q)).unwrap().total, 1);
    assert_eq!(
        c.query(&PhotoQuery {
            reviewed: Some(false),
            ..Default::default()
        })
        .unwrap()
        .total,
        1
    );
    assert_eq!(c.set_reviewed(&[a], false).unwrap(), 1);
    assert_eq!(c.query(&q).unwrap().total, 0);
}

#[test]
fn preview_freezes_filtered_scope_and_counts_raw_jpeg_as_one() {
    let c = Arc::new(SqliteCatalog::open_in_memory().unwrap());
    for n in 1..=5189 {
        c.insert_photo(&photo(n, if n <= 3000 { 17 } else { 18 }))
            .unwrap();
    }
    c.set_decision_saved(
        PhotoId(1),
        Decision {
            flag: Flag::Pick,
            ..Default::default()
        },
    )
    .unwrap();
    let service = ReviewService::new(c.clone());
    let picks = service
        .plan(PhotoQuery {
            from: Some("2026-11-17".into()),
            to: Some("2026-11-17".into()),
            flags: Some(vec![Flag::Pick]),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(picks.count, 1);
    assert_eq!(
        service
            .apply(ApplyReviewArgs {
                token: picks.token,
                reviewed: true
            })
            .unwrap(),
        1
    );
    let all_dates = service
        .plan(PhotoQuery {
            from: Some("2026-11-17".into()),
            to: Some("2026-11-17".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(all_dates.count, 3000);
    c.insert_photo(&photo(6000, 17)).unwrap();
    assert_eq!(
        service
            .apply(ApplyReviewArgs {
                token: all_dates.token.clone(),
                reviewed: true
            })
            .unwrap(),
        2999
    );
    assert!(
        service
            .apply(ApplyReviewArgs {
                token: all_dates.token,
                reviewed: true
            })
            .is_err()
    );
    assert_eq!(
        c.query(&PhotoQuery {
            reviewed: Some(true),
            ..Default::default()
        })
        .unwrap()
        .total,
        3000
    );
    assert_eq!(
        c.query(&PhotoQuery {
            reviewed: Some(false),
            ..Default::default()
        })
        .unwrap()
        .total,
        2190
    );
    assert!(c.dirty_decisions().unwrap().is_empty());
    let plan = service.plan(PhotoQuery::default()).unwrap();
    let other_session = ReviewService::new(c);
    assert!(
        other_session
            .apply(ApplyReviewArgs {
                token: plan.token,
                reviewed: false
            })
            .is_err()
    );
}
