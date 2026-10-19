use std::path::PathBuf;
use std::time::Instant;

use chrono::NaiveDateTime;
use picturesque_app::library::{CameraKey, PhotoQuery, PhotoSort};
use picturesque_app::ports::catalog::{PhotoCatalog, PhotoScore};
use picturesque_core::decision::{Decision, Flag, Rating};
use picturesque_core::hash::ContentHash;
use picturesque_core::media::MediaFormat;
use picturesque_core::metadata::{Camera, CaptureMetadata, CaptureTime, Orientation, Place};
use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets, PhotoId};
use picturesque_infra::catalog::SqliteCatalog;

struct Spec {
    when: Option<&'static str>,
    camera: (&'static str, &'static str),
    lens: Option<&'static str>,
    city: Option<(&'static str, &'static str, &'static str)>,
    files: &'static [&'static str],
}

fn hash(n: u64) -> ContentHash {
    let mut b = [0u8; 32];
    b[..8].copy_from_slice(&n.to_le_bytes());
    ContentHash::from_bytes(b)
}

fn insert(catalog: &SqliteCatalog, n: u64, spec: &Spec) -> PhotoId {
    let metadata = CaptureMetadata {
        captured_at: spec.when.map(|w| {
            CaptureTime::new(
                NaiveDateTime::parse_from_str(w, "%Y-%m-%d %H:%M:%S").unwrap(),
                None,
            )
        }),
        camera: Some(Camera::new(spec.camera.0, spec.camera.1)),
        lens: spec.lens.map(str::to_owned),
        place: spec.city.map(|(city, country, cc)| Place {
            city: city.into(),
            region: None,
            country: country.into(),
            country_code: cc.into(),
        }),
        ..Default::default()
    };
    let assets = spec
        .files
        .iter()
        .enumerate()
        .map(|(i, f)| Asset {
            format: MediaFormat::from_file_name(f).unwrap(),
            path: PathBuf::from(format!("{n}/{f}")),
            size: n * 10 + i as u64,
            hash: hash(n * 10 + i as u64),
        })
        .collect();
    catalog
        .insert_photo(&NewPhoto {
            metadata,
            assets: PhotoAssets::new(assets).unwrap(),
        })
        .unwrap()
}

const R5: (&str, &str) = ("Canon", "Canon EOS R5");
const PHONE: (&str, &str) = ("Apple", "iPhone 15 Pro");
const LISBON: Option<(&str, &str, &str)> = Some(("Lisbon", "Portugal", "PT"));
const PORTO: Option<(&str, &str, &str)> = Some(("Porto", "Portugal", "PT"));

/// Six photos: ids 1–6.
fn sample() -> SqliteCatalog {
    let c = SqliteCatalog::open_in_memory().unwrap();
    let specs = [
        Spec {
            when: Some("2026-10-12 14:30:00"),
            camera: R5,
            lens: Some("RF50"),
            city: LISBON,
            files: &["a.CR3", "a.JPG"],
        },
        Spec {
            when: Some("2026-10-12 09:00:00"),
            camera: R5,
            lens: Some("RF24-70"),
            city: LISBON,
            files: &["b.CR3"],
        },
        Spec {
            when: Some("2026-10-11 18:00:00"),
            camera: PHONE,
            lens: None,
            city: PORTO,
            files: &["c.HEIC"],
        },
        Spec {
            when: None,
            camera: PHONE,
            lens: None,
            city: None,
            files: &["d.JPG"],
        },
        Spec {
            when: Some("2025-01-01 00:00:00"),
            camera: R5,
            lens: Some("RF50"),
            city: None,
            files: &["e.CR3"],
        },
        Spec {
            when: None,
            camera: R5,
            lens: None,
            city: None,
            files: &["f.JPG"],
        },
    ];
    for (i, s) in specs.iter().enumerate() {
        insert(&c, i as u64 + 1, s);
    }
    c
}

/// A combined score without AI signals.
fn score(id: i64, score: f32) -> PhotoScore {
    PhotoScore {
        id: PhotoId(id),
        score,
        aesthetic: Some(score),
        eyes_closed: None,
        faces: 0,
        keeper: false,
    }
}

fn ids(c: &SqliteCatalog, q: PhotoQuery) -> Vec<i64> {
    c.query(&q).unwrap().photos.iter().map(|p| p.id).collect()
}

#[test]
fn default_order_is_newest_first_with_undated_last() {
    let c = sample();

    assert_eq!(ids(&c, PhotoQuery::default()), [1, 2, 3, 5, 6, 4]);
    assert_eq!(
        ids(
            &c,
            PhotoQuery {
                sort: Some(PhotoSort::CapturedAsc),
                ..Default::default()
            }
        ),
        [5, 3, 2, 1, 4, 6]
    );
    assert_eq!(
        ids(
            &c,
            PhotoQuery {
                sort: Some(PhotoSort::ImportedDesc),
                ..Default::default()
            }
        ),
        [6, 5, 4, 3, 2, 1]
    );
}

#[test]
fn pages_cover_everything_exactly_once_in_every_order() {
    let c = sample();
    for sort in [
        PhotoSort::CapturedDesc,
        PhotoSort::CapturedAsc,
        PhotoSort::ImportedDesc,
    ] {
        let all = ids(
            &c,
            PhotoQuery {
                sort: Some(sort),
                ..Default::default()
            },
        );
        let mut paged = Vec::new();
        let mut cursor = None;
        loop {
            let page = c
                .query(&PhotoQuery {
                    sort: Some(sort),
                    limit: Some(2),
                    cursor: cursor.clone(),
                    ..Default::default()
                })
                .unwrap();
            assert_eq!(page.total, 6);
            paged.extend(page.photos.iter().map(|p| p.id));
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        assert_eq!(paged, all, "{sort:?}");
    }
}

#[test]
fn filters_combine() {
    let c = sample();
    let q = |f: fn(&mut PhotoQuery)| {
        let mut q = PhotoQuery::default();
        f(&mut q);
        ids(&c, q)
    };

    assert_eq!(q(|q| q.from = Some("2026-10-12".into())), [1, 2]);
    assert_eq!(q(|q| q.to = Some("2026-10-11".into())), [3, 5]);
    assert_eq!(
        q(|q| {
            q.from = Some("2026-10-11".into());
            q.to = Some("2026-10-11".into())
        }),
        [3]
    );
    assert_eq!(q(|q| q.country_code = Some("PT".into())), [1, 2, 3]);
    assert_eq!(q(|q| q.city = Some("Porto".into())), [3]);
    assert_eq!(
        q(|q| q.camera = Some(CameraKey {
            make: "Apple".into(),
            model: "iPhone 15 Pro".into()
        })),
        [3, 4]
    );
    assert_eq!(q(|q| q.lens = Some("RF50".into())), [1, 5]);
    assert_eq!(
        q(|q| {
            q.lens = Some("RF50".into());
            q.city = Some("Lisbon".into())
        }),
        [1]
    );
    assert_eq!(q(|q| q.photo_ids = Some(vec![5, 1, 42])), [1, 5]);
    assert_eq!(
        q(|q| {
            q.photo_ids = Some(vec![1, 3, 5]);
            q.country_code = Some("PT".into())
        }),
        [1, 3]
    );
    assert!(q(|q| q.photo_ids = Some(Vec::new())).is_empty());
}

#[test]
fn flag_and_rating_filters_use_decisions() {
    let c = sample();
    c.set_decision(
        PhotoId(1),
        Decision {
            flag: Flag::Pick,
            rating: Rating::new(4).unwrap(),
        },
    )
    .unwrap();
    c.set_decision(
        PhotoId(2),
        Decision {
            flag: Flag::Reject,
            rating: Rating::default(),
        },
    )
    .unwrap();
    c.set_decision(
        PhotoId(3),
        Decision {
            flag: Flag::Unflagged,
            rating: Rating::new(2).unwrap(),
        },
    )
    .unwrap();

    assert_eq!(
        ids(
            &c,
            PhotoQuery {
                flags: Some(vec![Flag::Pick]),
                ..Default::default()
            }
        ),
        [1]
    );
    assert_eq!(
        ids(
            &c,
            PhotoQuery {
                flags: Some(vec![Flag::Unflagged]),
                ..Default::default()
            }
        ),
        [3, 5, 6, 4]
    );
    assert_eq!(
        ids(
            &c,
            PhotoQuery {
                flags: Some(vec![Flag::Pick, Flag::Reject]),
                ..Default::default()
            }
        ),
        [1, 2]
    );
    assert_eq!(
        ids(
            &c,
            PhotoQuery {
                min_rating: Some(2),
                ..Default::default()
            }
        ),
        [1, 3]
    );
    assert_eq!(c.decision(PhotoId(1)).unwrap().rating.stars(), 4);
    assert_eq!(c.decision(PhotoId(6)).unwrap(), Decision::default());
}

#[test]
fn decisions_update_in_place_and_need_an_existing_photo() {
    let c = sample();
    c.set_decision(
        PhotoId(1),
        Decision {
            flag: Flag::Pick,
            rating: Rating::new(1).unwrap(),
        },
    )
    .unwrap();
    c.set_decision(
        PhotoId(1),
        Decision {
            flag: Flag::Reject,
            rating: Rating::new(3).unwrap(),
        },
    )
    .unwrap();

    assert_eq!(
        c.decision(PhotoId(1)).unwrap(),
        Decision {
            flag: Flag::Reject,
            rating: Rating::new(3).unwrap()
        }
    );
    assert!(c.set_decision(PhotoId(99), Decision::default()).is_err());
}

#[test]
fn decisions_read_from_sidecars_are_not_written_back() {
    let c = sample();
    let read = Decision {
        flag: Flag::Reject,
        rating: Rating::new(2).unwrap(),
    };

    c.set_decision_saved(PhotoId(1), read).unwrap();

    assert_eq!(c.decision(PhotoId(1)).unwrap(), read);
    assert!(c.dirty_decisions().unwrap().is_empty());
    // A later change by the user is written as usual.
    c.set_decision(PhotoId(1), Decision::default()).unwrap();
    assert_eq!(c.dirty_decisions().unwrap().len(), 1);
}

#[test]
fn auto_labels_mark_exactly_the_given_photos_and_dirty_the_changes() {
    let c = sample();
    for d in c.dirty_decisions().unwrap() {
        c.mark_sidecar_written(d.id, &d.token).unwrap();
    }

    // Photo 6 has no decision yet: it gets the default one.
    assert_eq!(c.set_auto_labeled(&[PhotoId(1), PhotoId(6)]).unwrap(), 2);
    assert_eq!(
        c.auto_labeled().unwrap(),
        [PhotoId(1), PhotoId(6)].into_iter().collect()
    );
    let dirty = c.dirty_decisions().unwrap();
    assert_eq!(
        dirty
            .iter()
            .map(|d| (d.id, d.auto_labeled))
            .collect::<Vec<_>>(),
        [(PhotoId(1), true), (PhotoId(6), true)]
    );
    for d in dirty {
        c.mark_sidecar_written(d.id, &d.token).unwrap();
    }

    // Unchanged marks stay clean; a removed one is dirty again.
    assert_eq!(c.set_auto_labeled(&[PhotoId(6)]).unwrap(), 1);
    let dirty = c.dirty_decisions().unwrap();
    assert_eq!(dirty.len(), 1);
    assert_eq!((dirty[0].id, dirty[0].auto_labeled), (PhotoId(1), false));
}

#[test]
fn copies_are_recorded_and_go_with_their_photo() {
    use picturesque_app::ports::catalog::CopyRecord;
    use picturesque_core::hash::ContentHash;
    let c = sample();
    let copy = |id: i64, source: &str, n: u8| CopyRecord {
        photo_id: PhotoId(id),
        source: source.into(),
        destination: std::env::temp_dir().join("out").join(source),
        hash: ContentHash::from_bytes([n; 32]),
        size: u64::from(n),
    };

    c.record_copies(&[copy(1, "2026/a.CR3", 1), copy(2, "2026/b.JPG", 2)])
        .unwrap();
    // A second copy of the same file replaces the first record.
    c.record_copies(&[copy(1, "2026/a.CR3", 3)]).unwrap();

    assert_eq!(
        c.copies().unwrap(),
        [copy(1, "2026/a.CR3", 3), copy(2, "2026/b.JPG", 2)]
    );
    c.remove_photo(PhotoId(1)).unwrap();
    assert!(c.photo(PhotoId(1)).unwrap().is_none());
    assert_eq!(c.copies().unwrap(), [copy(2, "2026/b.JPG", 2)]);
}

#[test]
fn summaries_carry_what_the_grid_needs() {
    let c = sample();
    c.set_dimensions(PhotoId(2), 6000, 4000).unwrap();

    let page = c.query(&PhotoQuery::default()).unwrap();
    let first = &page.photos[0];
    assert_eq!(first.captured_at.as_deref(), Some("2026-10-12T14:30:00"));
    assert_eq!(first.day.as_deref(), Some("2026-10-12"));
    assert_eq!(first.camera.as_deref(), Some("Canon EOS R5"));
    assert_eq!(first.city.as_deref(), Some("Lisbon"));
    assert!(first.has_raw);
    // The JPEG of a RAW+JPEG pair is what the grid shows.
    assert_eq!(first.file_name, "a.JPG");
    assert_eq!(
        (page.photos[1].width, page.photos[1].height),
        (Some(6000), Some(4000))
    );
    assert!(!page.photos.iter().find(|p| p.id == 3).unwrap().has_raw);
}

#[test]
fn display_size_follows_orientation() {
    let c = SqliteCatalog::open_in_memory().unwrap();
    let metadata = CaptureMetadata {
        dimensions: Some((6000, 4000)),
        orientation: Orientation::Rotate90,
        ..Default::default()
    };
    let assets = PhotoAssets::new(vec![Asset {
        format: MediaFormat::Jpeg,
        path: "p.jpg".into(),
        size: 1,
        hash: hash(1),
    }])
    .unwrap();
    c.insert_photo(&NewPhoto { metadata, assets }).unwrap();

    let p = &c.query(&PhotoQuery::default()).unwrap().photos[0];

    assert_eq!((p.width, p.height), (Some(4000), Some(6000)));
}

#[test]
fn facets_list_values_with_counts() {
    let c = sample();
    c.set_decision(
        PhotoId(1),
        Decision {
            flag: Flag::Pick,
            rating: Rating::new(5).unwrap(),
        },
    )
    .unwrap();

    let f = c.facets(None).unwrap();

    assert_eq!(f.total, 6);
    assert_eq!(
        (f.first_day.as_deref(), f.last_day.as_deref()),
        (Some("2025-01-01"), Some("2026-10-12"))
    );
    assert_eq!(
        f.days
            .iter()
            .map(|d| (d.value.as_str(), d.count))
            .collect::<Vec<_>>(),
        [("2026-10-12", 2), ("2026-10-11", 1), ("2025-01-01", 1)]
    );
    assert_eq!(f.cameras[0].value.label, "Canon EOS R5");
    assert_eq!(f.cameras[0].count, 4);
    assert_eq!(f.lenses[0].value, "RF50");
    assert_eq!(
        f.places
            .iter()
            .map(|p| (p.value.city.as_str(), p.count))
            .collect::<Vec<_>>(),
        [("Lisbon", 2), ("Porto", 1)]
    );
    assert_eq!(
        f.flags
            .iter()
            .map(|x| (x.flag, x.count))
            .collect::<Vec<_>>(),
        [(Flag::Pick, 1), (Flag::Unflagged, 5)]
    );
    assert_eq!(f.ratings, [5, 0, 0, 0, 0, 1]);
}

#[test]
fn filtered_facets_count_matching_days_but_keep_every_option() {
    let c = sample();

    let f = c
        .facets(Some(&PhotoQuery {
            city: Some("Lisbon".into()),
            ..Default::default()
        }))
        .unwrap();

    assert_eq!(f.total, 2);
    assert_eq!(
        f.days
            .iter()
            .map(|d| (d.value.as_str(), d.count))
            .collect::<Vec<_>>(),
        [("2026-10-12", 2)]
    );
    // Options still list the whole library.
    assert_eq!(f.places.len(), 2);
    assert_eq!(f.cameras.len(), 2);
}

#[test]
fn queries_over_ten_thousand_photos_are_fast() {
    let c = SqliteCatalog::open_in_memory().unwrap();
    let cities = [LISBON, PORTO, None];
    for n in 0..10_000u64 {
        let day = 1 + (n % 28);
        let month = 1 + (n / 28) % 12;
        let when: &'static str =
            Box::leak(format!("2025-{month:02}-{day:02} {:02}:00:00", n % 24).into_boxed_str());
        insert(
            &c,
            n + 1,
            &Spec {
                when: if n % 50 == 0 { None } else { Some(when) },
                camera: if n % 3 == 0 { PHONE } else { R5 },
                lens: if n % 2 == 0 { Some("RF50") } else { None },
                city: cities[(n % 3) as usize],
                files: if n % 4 == 0 {
                    &["x.CR3", "x.JPG"]
                } else {
                    &["x.JPG"]
                },
            },
        );
        if n % 7 == 0 {
            c.set_decision(
                PhotoId(n as i64 + 1),
                Decision {
                    flag: Flag::Pick,
                    rating: Rating::new((n % 6) as u8).unwrap(),
                },
            )
            .unwrap();
        }
    }
    for n in 0..10_000u64 {
        let score = ((n * 7919) % 1000) as f64 / 1000.0;
        c.store_analysis(
            PhotoId(n as i64 + 1),
            picturesque_app::analysis::technical::TechnicalAnalyzer::ID,
            &serde_json::json!({ "sharpness": score, "exposure": 0.8, "noise": 0.7, "overall": score }),
        )
        .unwrap();
    }
    let all_scores: Vec<PhotoScore> = (0..10_000u64)
        .map(|n| PhotoScore {
            eyes_closed: Some(n % 10 == 0),
            faces: (n % 3) as u32,
            ..score(n as i64 + 1, ((n * 7919) % 1000) as f32 / 1000.0)
        })
        .collect();
    c.replace_scores(&all_scores).unwrap();
    let queries = [
        PhotoQuery::default(),
        PhotoQuery {
            sort: Some(PhotoSort::QualityDesc),
            ..Default::default()
        },
        PhotoQuery {
            min_sharpness: Some(0.5),
            best_only: Some(true),
            ..Default::default()
        },
        PhotoQuery {
            city: Some("Lisbon".into()),
            lens: Some("RF50".into()),
            ..Default::default()
        },
        // Matches every spelling of the camera (a function per row).
        PhotoQuery {
            camera: Some(CameraKey {
                make: R5.0.into(),
                model: R5.1.into(),
            }),
            ..Default::default()
        },
        PhotoQuery {
            flags: Some(vec![Flag::Pick]),
            min_rating: Some(3),
            ..Default::default()
        },
        PhotoQuery {
            from: Some("2025-06-01".into()),
            to: Some("2025-06-30".into()),
            sort: Some(PhotoSort::CapturedAsc),
            ..Default::default()
        },
    ];

    // Best of three runs: shared CI machines pause processes at random.
    let best = |run: &dyn Fn()| {
        (0..3)
            .map(|_| {
                let start = Instant::now();
                run();
                start.elapsed()
            })
            .min()
            .unwrap()
    };
    for q in queries {
        let page = c.query(&q).unwrap();
        let first = best(&|| {
            c.query(&q).unwrap();
        });
        let second = best(&|| {
            if let Some(cursor) = &page.next_cursor {
                c.query(&PhotoQuery {
                    cursor: Some(cursor.clone()),
                    ..q.clone()
                })
                .unwrap();
            }
        });
        assert!(
            first.as_millis() < 50 && second.as_millis() < 50,
            "{q:?}: {first:?} / {second:?}"
        );
    }
    let facets = best(&|| {
        c.facets(None).unwrap();
        c.facets(Some(&PhotoQuery {
            flags: Some(vec![Flag::Pick]),
            ..Default::default()
        }))
        .unwrap();
    });
    assert!(facets.as_millis() < 100, "facets: {facets:?}");
}

/// A series ranked in capture order, best first, unscored.
fn stored(members: &[PhotoId]) -> picturesque_app::ports::catalog::StoredSeries {
    picturesque_app::ports::catalog::StoredSeries {
        members: members.to_vec(),
        ranking: members
            .iter()
            .map(|id| picturesque_core::ranking::Ranked {
                id: *id,
                score: None,
            })
            .collect(),
        best: members[0],
        best_chosen: false,
    }
}

#[test]
fn series_are_stored_replaced_and_shown_in_summaries() {
    use chrono::NaiveDate;
    use picturesque_core::series::SeriesEdit;

    let c = sample();
    let (p1, p2, p3, p5) = (PhotoId(1), PhotoId(2), PhotoId(3), PhotoId(5));

    let times = c.capture_times().unwrap();
    assert_eq!(times.len(), 6);
    assert_eq!(
        times[0],
        (
            p1,
            Some(
                NaiveDate::from_ymd_opt(2026, 10, 12)
                    .unwrap()
                    .and_hms_opt(14, 30, 0)
                    .unwrap()
            )
        )
    );
    assert_eq!(times[3], (PhotoId(4), None));

    c.replace_series(&[stored(&[p2, p1]), stored(&[p3, p5])])
        .unwrap();
    assert_eq!(c.series_of(p1).unwrap(), Some((p2, stored(&[p2, p1]))));
    assert_eq!(c.series_of(PhotoId(4)).unwrap(), None);
    let page = c.query(&PhotoQuery::default()).unwrap();
    let series: Vec<(i64, Option<i64>)> = page.photos.iter().map(|p| (p.id, p.series_id)).collect();
    assert_eq!(
        series,
        [
            (1, Some(2)),
            (2, Some(2)),
            (3, Some(3)),
            (5, Some(3)),
            (6, None),
            (4, None)
        ]
    );

    // Replacing drops old membership.
    c.replace_series(&[stored(&[p1, p2, p3])]).unwrap();
    assert_eq!(c.series_of(p5).unwrap(), None);
    assert_eq!(c.series_of(p3).unwrap(), Some((p1, stored(&[p1, p2, p3]))));
    c.replace_series(&[]).unwrap();
    assert_eq!(c.series_of(p1).unwrap(), None);

    // Edits are kept separately and can be changed or removed.
    c.set_series_edit(p3, Some(SeriesEdit::Split)).unwrap();
    c.set_series_edit(p1, Some(SeriesEdit::Join)).unwrap();
    c.set_series_edit(p3, Some(SeriesEdit::Join)).unwrap();
    assert_eq!(
        c.series_edits().unwrap(),
        [(p1, SeriesEdit::Join), (p3, SeriesEdit::Join)]
    );
    c.set_series_edit(p1, None).unwrap();
    assert_eq!(c.series_edits().unwrap(), [(p3, SeriesEdit::Join)]);
    assert!(
        c.set_series_edit(PhotoId(99), Some(SeriesEdit::Split))
            .is_err()
    );
}

#[test]
fn analysis_results_are_listed_per_analyzer_version() {
    use picturesque_app::ports::catalog::AnalyzerId;
    use serde_json::json;

    let c = sample();
    let v1 = AnalyzerId {
        name: "similarity",
        version: 1,
    };
    let v2 = AnalyzerId {
        name: "similarity",
        version: 2,
    };
    c.store_analysis(PhotoId(2), v1, &json!({ "phash": "00" }))
        .unwrap();
    c.store_analysis(PhotoId(1), v1, &json!({ "phash": "01" }))
        .unwrap();
    c.store_analysis(PhotoId(3), v2, &json!({ "phash": "02" }))
        .unwrap();
    c.store_analysis(
        PhotoId(1),
        AnalyzerId {
            name: "technical",
            version: 1,
        },
        &json!({}),
    )
    .unwrap();

    assert_eq!(
        c.analysis_results(v1).unwrap(),
        [
            (PhotoId(1), json!({ "phash": "01" })),
            (PhotoId(2), json!({ "phash": "00" }))
        ]
    );
    assert_eq!(c.analysis_results(v2).unwrap().len(), 1);
}

#[test]
fn rankings_best_photos_and_choices_round_trip() {
    use picturesque_app::ports::catalog::StoredSeries;
    use picturesque_core::ranking::Ranked;

    let c = sample();
    let (p1, p2, p3) = (PhotoId(1), PhotoId(2), PhotoId(3));
    let series = StoredSeries {
        members: vec![p2, p1, p3],
        ranking: vec![
            Ranked {
                id: p3,
                score: Some(0.75),
            },
            Ranked {
                id: p2,
                score: Some(0.5),
            },
            Ranked {
                id: p1,
                score: None,
            },
        ],
        best: p1,
        best_chosen: true,
    };
    c.replace_series(std::slice::from_ref(&series)).unwrap();
    assert_eq!(c.series_of(p3).unwrap(), Some((p2, series)));

    let best: Vec<(i64, bool)> = c
        .query(&PhotoQuery::default())
        .unwrap()
        .photos
        .iter()
        .map(|p| (p.id, p.best_in_series))
        .collect();
    assert!(best.contains(&(1, true)));
    assert!(best.contains(&(2, false)));
    assert!(best.contains(&(4, false)));

    c.set_best_choice(p2, true).unwrap();
    c.set_best_choice(p1, true).unwrap();
    c.set_best_choice(p2, true).unwrap(); // chosen again: now the latest
    assert_eq!(c.best_choices().unwrap(), [p1, p2]);
    c.set_best_choice(p1, false).unwrap();
    assert_eq!(c.best_choices().unwrap(), [p2]);

    c.set_decision(
        p3,
        Decision {
            flag: Flag::Reject,
            rating: Rating::new(0).unwrap(),
        },
    )
    .unwrap();
    c.set_decision(
        p1,
        Decision {
            flag: Flag::Pick,
            rating: Rating::new(0).unwrap(),
        },
    )
    .unwrap();
    assert_eq!(c.rejected_photos().unwrap(), [p3]);
}

#[test]
fn collapsing_series_shows_the_best_matching_photo_of_each() {
    use picturesque_app::ports::catalog::StoredSeries;
    use picturesque_core::ranking::Ranked;

    let c = sample();
    let ids = |q: PhotoQuery| -> Vec<i64> { ids(&c, q) };
    let ranked = |order: &[i64]| -> Vec<Ranked> {
        order
            .iter()
            .map(|id| Ranked {
                id: PhotoId(*id),
                score: None,
            })
            .collect()
    };
    // Series A = photos 2, 1 (ranked 1 first); series B = 3, 5 (5 chosen
    // by the user although ranked second).
    c.replace_series(&[
        StoredSeries {
            members: vec![PhotoId(2), PhotoId(1)],
            ranking: ranked(&[1, 2]),
            best: PhotoId(1),
            best_chosen: false,
        },
        StoredSeries {
            members: vec![PhotoId(5), PhotoId(3)],
            ranking: ranked(&[3, 5]),
            best: PhotoId(5),
            best_chosen: true,
        },
    ])
    .unwrap();
    let collapsed = PhotoQuery {
        collapse_series: Some(true),
        ..Default::default()
    };

    assert_eq!(ids(PhotoQuery::default()), [1, 2, 3, 5, 6, 4]);
    assert_eq!(ids(collapsed.clone()), [1, 5, 6, 4]);
    let page = c.query(&collapsed).unwrap();
    assert_eq!(page.total, 4);
    assert_eq!(page.photos[0].series_size, 2);
    assert!(page.photos[0].best_in_series);
    assert_eq!(page.photos[2].series_size, 0);
    // Day counts follow, so the timeline layout stays exact.
    let facets = c.facets(Some(&collapsed)).unwrap();
    assert_eq!(facets.total, 4);
    assert_eq!(
        facets.days.iter().map(|d| d.count).sum::<usize>(),
        2,
        "{:?}",
        facets.days
    );

    // Expanding one series shows it whole.
    assert_eq!(
        ids(PhotoQuery {
            expanded_series: Some(vec![2]),
            ..collapsed.clone()
        }),
        [1, 2, 5, 6, 4]
    );

    // When the best photo does not match the filters, the next matching
    // member stands in for its series.
    c.set_decision(
        PhotoId(2),
        Decision {
            flag: Flag::Pick,
            rating: Rating::new(0).unwrap(),
        },
    )
    .unwrap();
    assert_eq!(
        ids(PhotoQuery {
            flags: Some(vec![Flag::Pick]),
            ..collapsed.clone()
        }),
        [2]
    );
}

#[test]
fn score_thresholds_best_only_and_quality_sort() {
    use picturesque_app::analysis::technical::TechnicalAnalyzer;
    use picturesque_app::library::PhotoSort;
    use serde_json::json;

    let c = sample();
    // (photo, sharpness, exposure, noise, overall); 4 and 6 not analyzed,
    // 5 failed.
    for (id, s, e, n, o) in [
        (1, 0.9, 0.8, 0.7, 0.85),
        (2, 0.4, 0.9, 0.9, 0.55),
        (3, 0.95, 0.3, 0.5, 0.6),
    ] {
        c.store_analysis(
            PhotoId(id),
            TechnicalAnalyzer::ID,
            &json!({ "sharpness": s, "exposure": e, "noise": n, "overall": o }),
        )
        .unwrap();
    }
    c.store_analysis(PhotoId(5), TechnicalAnalyzer::ID, &json!({ "error": "x" }))
        .unwrap();
    let q = |f: &dyn Fn(&mut PhotoQuery)| {
        let mut q = PhotoQuery::default();
        f(&mut q);
        ids(&c, q)
    };

    assert_eq!(q(&|q| q.min_sharpness = Some(0.5)), [1, 3]);
    assert_eq!(q(&|q| q.min_exposure = Some(0.85)), [2]);
    assert_eq!(
        q(&|q| {
            q.min_sharpness = Some(0.5);
            q.min_noise = Some(0.6)
        }),
        [1]
    );
    assert_eq!(q(&|q| q.min_sharpness = Some(0.0)), [1, 2, 3]);
    let facets = c
        .facets(Some(&PhotoQuery {
            min_sharpness: Some(0.5),
            ..Default::default()
        }))
        .unwrap();
    assert_eq!(facets.total, 2);

    // Sorting uses the combined scores the series service stores; best
    // first, photos without a score last, newest id first among them.
    c.replace_scores(&[score(1, 0.85), score(2, 0.55), score(3, 0.6)])
        .unwrap();
    let sorted = |limit| {
        let mut all = Vec::new();
        let mut cursor = None;
        loop {
            let page = c
                .query(&PhotoQuery {
                    sort: Some(PhotoSort::QualityDesc),
                    limit: Some(limit),
                    cursor,
                    ..Default::default()
                })
                .unwrap();
            all.extend(page.photos.iter().map(|p| (p.id, p.quality)));
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => return all,
            }
        }
    };
    let all = sorted(100);
    assert_eq!(
        all.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        [1, 3, 2, 6, 5, 4]
    );
    assert_eq!(all[0].1, Some(0.85));
    assert_eq!(all[3].1, None);
    for limit in 1..=4 {
        assert_eq!(sorted(limit), all, "limit {limit}");
    }

    // Best only hides series members other than the best.
    c.replace_series(&[stored(&[PhotoId(2), PhotoId(1)])])
        .unwrap();
    assert_eq!(q(&|q| q.best_only = Some(true)), [2, 3, 5, 6, 4]);
}

#[test]
fn ai_signals_filter_and_show_in_summaries() {
    let c = sample();
    c.replace_scores(&[
        PhotoScore {
            aesthetic: Some(0.8),
            eyes_closed: Some(false),
            faces: 2,
            ..score(1, 0.9)
        },
        PhotoScore {
            aesthetic: Some(0.3),
            eyes_closed: Some(true),
            faces: 1,
            ..score(2, 0.4)
        },
        PhotoScore {
            aesthetic: None,
            ..score(3, 0.5)
        },
    ])
    .unwrap();
    let q = |f: &dyn Fn(&mut PhotoQuery)| {
        let mut q = PhotoQuery::default();
        f(&mut q);
        ids(&c, q)
    };
    assert_eq!(q(&|q| q.min_aesthetic = Some(0.5)), [1]);
    assert_eq!(q(&|q| q.hide_closed_eyes = Some(true)), [1, 3, 5, 6, 4]);
    assert_eq!(q(&|q| q.people = Some(true)), [1, 2]);
    assert_eq!(q(&|q| q.people = Some(false)), [3, 5, 6, 4]);
    let facets = c
        .facets(Some(&PhotoQuery {
            people: Some(true),
            ..Default::default()
        }))
        .unwrap();
    assert_eq!(facets.total, 2);

    let photos = c.query(&PhotoQuery::default()).unwrap().photos;
    let by_id = |id: i64| photos.iter().find(|p| p.id == id).unwrap();
    assert_eq!(by_id(1).aesthetic, Some(0.8));
    assert_eq!((by_id(1).eyes_closed, by_id(1).faces), (Some(false), 2));
    assert_eq!((by_id(2).eyes_closed, by_id(2).faces), (Some(true), 1));
    assert_eq!(
        (by_id(4).aesthetic, by_id(4).eyes_closed, by_id(4).faces),
        (None, None, 0)
    );
}

#[test]
fn auto_picked_photos_are_keepers_without_a_flag() {
    use picturesque_core::decision::{Decision, Flag};
    let c = sample();
    c.replace_scores(&[
        PhotoScore {
            keeper: true,
            ..score(1, 0.9)
        },
        PhotoScore {
            keeper: true,
            ..score(2, 0.8)
        },
        PhotoScore {
            keeper: true,
            ..score(3, 0.7)
        },
        score(5, 0.2),
    ])
    .unwrap();
    // The user's own decisions win over the suggestion.
    c.set_decision(
        PhotoId(2),
        Decision {
            flag: Flag::Pick,
            ..Default::default()
        },
    )
    .unwrap();
    c.set_decision(
        PhotoId(3),
        Decision {
            flag: Flag::Reject,
            ..Default::default()
        },
    )
    .unwrap();

    let photos = c.query(&PhotoQuery::default()).unwrap().photos;
    // Keepers stay keepers; flags decide whether they show as auto-picked.
    let keepers: Vec<i64> = photos.iter().filter(|p| p.keeper).map(|p| p.id).collect();
    assert_eq!(keepers, [1, 2, 3]);
    let only = |q: PhotoQuery| ids(&c, q);
    assert_eq!(
        only(PhotoQuery {
            auto_picked: Some(true),
            ..Default::default()
        }),
        [1]
    );
    // With flags: photos matching either.
    assert_eq!(
        only(PhotoQuery {
            auto_picked: Some(true),
            flags: Some(vec![Flag::Pick]),
            ..Default::default()
        }),
        [1, 2]
    );
    let facets = c.facets(None).unwrap();
    assert_eq!(facets.auto_picked, 1);
}

#[test]
fn one_camera_spelled_two_ways_is_one_filter_option() {
    let c = SqliteCatalog::open_in_memory().unwrap();
    for (n, model) in [(1, "Canon EOS R5"), (2, "EOS R5"), (3, "EOS R5")] {
        insert(
            &c,
            n,
            &Spec {
                when: Some("2026-10-12 10:00:00"),
                camera: ("Canon", model),
                lens: None,
                city: None,
                files: &["x.JPG"],
            },
        );
    }
    insert(
        &c,
        4,
        &Spec {
            when: Some("2026-10-12 11:00:00"),
            camera: PHONE,
            lens: None,
            city: None,
            files: &["y.JPG"],
        },
    );

    let cameras = c.facets(None).unwrap().cameras;
    let labels: Vec<(&str, usize)> = cameras
        .iter()
        .map(|c| (c.value.label.as_str(), c.count))
        .collect();
    assert_eq!(labels, [("Canon EOS R5", 3), ("Apple iPhone 15 Pro", 1)]);
    // The most used spelling stands for the camera.
    assert_eq!(cameras[0].value.key.model, "EOS R5");

    // Filtering by either spelling finds all three.
    for model in ["EOS R5", "Canon EOS R5"] {
        let q = PhotoQuery {
            camera: Some(CameraKey {
                make: "Canon".into(),
                model: model.into(),
            }),
            ..Default::default()
        };
        assert_eq!(ids(&c, q).len(), 3, "{model}");
    }
}
