use std::path::PathBuf;
use std::time::Instant;

use chrono::NaiveDateTime;
use picturesque_app::library::{CameraKey, PhotoQuery, PhotoSort};
use picturesque_app::ports::catalog::PhotoCatalog;
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
    let queries = [
        PhotoQuery::default(),
        PhotoQuery {
            city: Some("Lisbon".into()),
            lens: Some("RF50".into()),
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

    for q in queries {
        let start = Instant::now();
        let page = c.query(&q).unwrap();
        let first = start.elapsed();
        let start = Instant::now();
        if let Some(cursor) = page.next_cursor {
            c.query(&PhotoQuery {
                cursor: Some(cursor),
                ..q.clone()
            })
            .unwrap();
        }
        let second = start.elapsed();
        assert!(
            first.as_millis() < 50 && second.as_millis() < 50,
            "{q:?}: {first:?} / {second:?}"
        );
    }
    let start = Instant::now();
    c.facets(None).unwrap();
    c.facets(Some(&PhotoQuery {
        flags: Some(vec![Flag::Pick]),
        ..Default::default()
    }))
    .unwrap();
    assert!(
        start.elapsed().as_millis() < 100,
        "facets: {:?}",
        start.elapsed()
    );
}
