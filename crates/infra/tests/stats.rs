use std::path::PathBuf;

use picturesque_app::ports::catalog::{PhotoCatalog, PhotoScore};
use picturesque_app::stats::{KeeperCounts, summarize};
use picturesque_core::decision::{Decision, Flag};
use picturesque_core::hash::ContentHash;
use picturesque_core::media::MediaFormat;
use picturesque_core::metadata::{Camera, CaptureMetadata, Exposure};
use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets, PhotoId};
use picturesque_infra::catalog::SqliteCatalog;

const R5: Option<(&str, &str)> = Some(("Canon", "Canon EOS R5"));
const PHONE: Option<(&str, &str)> = Some(("Apple", "iPhone 15 Pro"));

fn insert(
    c: &SqliteCatalog,
    n: u8,
    camera: Option<(&str, &str)>,
    lens: Option<&str>,
    focal: Option<f32>,
) -> PhotoId {
    c.insert_photo(&NewPhoto {
        metadata: CaptureMetadata {
            camera: camera.map(|(make, model)| Camera::new(make, model)),
            lens: lens.map(str::to_owned),
            exposure: Exposure {
                focal_length_mm: focal,
                ..Default::default()
            },
            ..Default::default()
        },
        assets: PhotoAssets::new(vec![Asset {
            format: MediaFormat::Jpeg,
            path: PathBuf::from(format!("{n}.JPG")),
            size: u64::from(n),
            hash: ContentHash::from_bytes([n; 32]),
        }])
        .unwrap(),
    })
    .unwrap()
}

fn flag(c: &SqliteCatalog, id: PhotoId, flag: Flag) {
    c.set_decision(
        id,
        Decision {
            flag,
            ..Default::default()
        },
    )
    .unwrap();
}

fn keeper(id: PhotoId) -> PhotoScore {
    PhotoScore {
        id,
        score: 0.8,
        aesthetic: None,
        critique: None,
        eyes_closed: None,
        faces: 0,
        keeper: true,
    }
}

fn counts(photos: usize, picks: usize, auto_picked: usize, rejects: usize) -> KeeperCounts {
    KeeperCounts {
        photos,
        picks,
        auto_picked,
        rejects,
    }
}

#[test]
fn counts_keepers_per_camera_and_lens_and_focal_lengths_per_camera() {
    let c = SqliteCatalog::open_in_memory().unwrap();
    let picked = insert(&c, 1, R5, Some("RF50"), Some(50.0));
    let auto = insert(&c, 2, R5, Some("RF24-70"), Some(24.0));
    // A keeper the user rejected is not auto-picked.
    let rejected = insert(&c, 3, R5, Some("RF24-70"), Some(24.4));
    insert(&c, 4, R5, Some("RF50"), Some(50.0));
    let phone = insert(&c, 5, PHONE, None, Some(6.86));
    // Neither camera nor lens: only in the totals.
    insert(&c, 6, None, None, None);
    flag(&c, picked, Flag::Pick);
    flag(&c, rejected, Flag::Reject);
    flag(&c, phone, Flag::Pick);
    c.replace_scores(&[keeper(auto), keeper(rejected)]).unwrap();

    let usage = c.gear_usage().unwrap();
    assert_eq!(usage.all, counts(6, 2, 1, 1));

    let stats = summarize(usage);
    let cameras: Vec<(&str, KeeperCounts)> = stats
        .cameras
        .iter()
        .map(|g| (g.label.as_str(), g.counts))
        .collect();
    // Both have few photos; the phone kept everything.
    assert_eq!(
        cameras,
        [
            ("Apple iPhone 15 Pro", counts(1, 1, 0, 0)),
            ("Canon EOS R5", counts(4, 1, 1, 1)),
        ]
    );
    let focal: Vec<(u32, usize)> = stats.cameras[1]
        .focal_lengths
        .iter()
        .map(|f| (f.value, f.count))
        .collect();
    assert_eq!(focal, [(24, 2), (50, 2)]);
    assert_eq!(stats.cameras[0].focal_lengths[0].value, 7);

    let lenses: Vec<(&str, KeeperCounts)> = stats
        .lenses
        .iter()
        .map(|g| (g.label.as_str(), g.counts))
        .collect();
    assert_eq!(
        lenses,
        [
            ("RF24-70", counts(2, 0, 1, 1)),
            ("RF50", counts(2, 1, 0, 0)),
        ]
    );
    assert!(stats.lenses.iter().all(|l| l.focal_lengths.is_empty()));
}

#[test]
fn an_empty_library_has_no_gear() {
    let c = SqliteCatalog::open_in_memory().unwrap();
    let stats = summarize(c.gear_usage().unwrap());
    assert_eq!(stats.all, KeeperCounts::default());
    assert_eq!(stats.keeper_rate, 0.0);
    assert!(stats.cameras.is_empty() && stats.lenses.is_empty());
}
