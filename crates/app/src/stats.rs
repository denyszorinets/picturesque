//! Library statistics: which cameras and lenses produce keepers, and which
//! focal lengths each camera is used at.

use picturesque_core::metadata::Camera;
use serde::Serialize;
use ts_rs::TS;

use crate::library::{CameraKey, Counted};

/// Below this many photos, a camera's or lens's keeper rate is too noisy
/// to compare; such gear is listed after the rest.
pub const MIN_PHOTOS: usize = 20;

/// Photos by decision. `auto_picked` counts unflagged photos auto-pick
/// suggests keeping (`picks` and `auto_picked` never overlap).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct KeeperCounts {
    pub photos: usize,
    pub picks: usize,
    pub auto_picked: usize,
    pub rejects: usize,
}

impl KeeperCounts {
    /// Picked or auto-picked photos.
    pub fn keepers(&self) -> usize {
        self.picks + self.auto_picked
    }

    /// Share of photos that are keepers, 0–1 (0 without photos).
    pub fn keeper_rate(&self) -> f32 {
        if self.photos == 0 {
            0.0
        } else {
            self.keepers() as f32 / self.photos as f32
        }
    }
}

/// Photos taken with one camera, as the catalog counts them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraUsage {
    pub key: CameraKey,
    pub counts: KeeperCounts,
    /// Photos per focal length (rounded to whole millimetres), any order.
    pub focal_lengths: Vec<Counted<u32>>,
}

/// Photos taken with one lens, as the catalog counts them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LensUsage {
    pub lens: String,
    pub counts: KeeperCounts,
}

/// What the catalog counts for the statistics.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GearUsage {
    /// Every photo in the library.
    pub all: KeeperCounts,
    pub cameras: Vec<CameraUsage>,
    pub lenses: Vec<LensUsage>,
}

/// A camera or lens with how its photos were decided.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct GearStats {
    /// Display name.
    pub label: String,
    /// The camera as stored, on camera rows.
    pub camera: Option<CameraKey>,
    /// The lens, on lens rows.
    pub lens: Option<String>,
    pub counts: KeeperCounts,
    /// Share of photos picked or auto-picked, 0–1.
    pub keeper_rate: f32,
    /// Fewer than `MIN_PHOTOS` photos: too few to compare; listed last.
    pub few_photos: bool,
    /// Photos per focal length in millimetres, most used first (cameras
    /// only; empty for lenses and cameras that record none).
    pub focal_lengths: Vec<Counted<u32>>,
}

/// Result of `library_stats`.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct LibraryStats {
    /// Every photo in the library.
    pub all: KeeperCounts,
    pub keeper_rate: f32,
    /// Most successful first: by keeper rate, then by photos; gear with
    /// few photos last.
    pub cameras: Vec<GearStats>,
    pub lenses: Vec<GearStats>,
    /// The `few_photos` threshold.
    pub min_photos: usize,
}

impl GearStats {
    fn new(label: String, counts: KeeperCounts) -> Self {
        Self {
            label,
            camera: None,
            lens: None,
            counts,
            keeper_rate: counts.keeper_rate(),
            few_photos: counts.photos < MIN_PHOTOS,
            focal_lengths: Vec::new(),
        }
    }
}

/// Labels and ranks what the catalog counted.
pub fn summarize(usage: GearUsage) -> LibraryStats {
    let mut cameras: Vec<GearStats> = usage
        .cameras
        .into_iter()
        .map(|c| {
            let label = Camera::new(&c.key.make, &c.key.model).display_name();
            let mut focal_lengths = c.focal_lengths;
            focal_lengths.sort_by(|a, b| b.count.cmp(&a.count).then(a.value.cmp(&b.value)));
            GearStats {
                camera: Some(c.key),
                focal_lengths,
                ..GearStats::new(label, c.counts)
            }
        })
        .collect();
    let mut lenses: Vec<GearStats> = usage
        .lenses
        .into_iter()
        .map(|l| GearStats {
            lens: Some(l.lens.clone()),
            ..GearStats::new(l.lens, l.counts)
        })
        .collect();
    rank(&mut cameras);
    rank(&mut lenses);
    LibraryStats {
        all: usage.all,
        keeper_rate: usage.all.keeper_rate(),
        cameras,
        lenses,
        min_photos: MIN_PHOTOS,
    }
}

/// Most successful first; gear with few photos after the rest.
fn rank(gear: &mut [GearStats]) {
    gear.sort_by(|a, b| {
        a.few_photos
            .cmp(&b.few_photos)
            .then(b.keeper_rate.total_cmp(&a.keeper_rate))
            .then(b.counts.photos.cmp(&a.counts.photos))
            .then_with(|| a.label.cmp(&b.label))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(photos: usize, picks: usize, auto_picked: usize) -> KeeperCounts {
        KeeperCounts {
            photos,
            picks,
            auto_picked,
            rejects: 0,
        }
    }

    fn camera(model: &str, counts: KeeperCounts, focal: &[(u32, usize)]) -> CameraUsage {
        CameraUsage {
            key: CameraKey {
                make: "Canon".into(),
                model: model.into(),
            },
            counts,
            focal_lengths: focal
                .iter()
                .map(|&(value, count)| Counted { value, count })
                .collect(),
        }
    }

    #[test]
    fn keeper_rate_counts_picks_and_auto_picks() {
        assert_eq!(counts(10, 2, 3).keepers(), 5);
        assert_eq!(counts(10, 2, 3).keeper_rate(), 0.5);
        assert_eq!(counts(0, 0, 0).keeper_rate(), 0.0);
    }

    #[test]
    fn ranks_by_keeper_rate_with_few_photos_last() {
        let stats = summarize(GearUsage {
            all: counts(250, 60, 20),
            cameras: vec![
                camera("EOS R5", counts(100, 20, 10), &[]),
                camera("EOS R6", counts(100, 30, 10), &[]),
                // 100%, but only 3 photos.
                camera("EOS M50", counts(3, 3, 0), &[]),
                camera("EOS RP", counts(47, 7, 0), &[]),
            ],
            lenses: vec![],
        });
        let order: Vec<&str> = stats.cameras.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(
            order,
            [
                "Canon EOS R6",
                "Canon EOS R5",
                "Canon EOS RP",
                "Canon EOS M50"
            ]
        );
        assert!(stats.cameras[3].few_photos);
        assert!(!stats.cameras[0].few_photos);
        assert_eq!(stats.cameras[0].keeper_rate, 0.4);
        assert_eq!(stats.keeper_rate, 80.0 / 250.0);
        assert_eq!(stats.min_photos, MIN_PHOTOS);
    }

    #[test]
    fn equal_rates_rank_the_more_used_gear_first() {
        let stats = summarize(GearUsage {
            all: KeeperCounts::default(),
            cameras: vec![],
            lenses: vec![
                LensUsage {
                    lens: "RF 50mm".into(),
                    counts: counts(40, 10, 0),
                },
                LensUsage {
                    lens: "RF 35mm".into(),
                    counts: counts(80, 10, 10),
                },
            ],
        });
        assert_eq!(stats.lenses[0].lens.as_deref(), Some("RF 35mm"));
        assert_eq!(stats.lenses[0].label, "RF 35mm");
        assert!(stats.lenses[0].camera.is_none());
    }

    #[test]
    fn lists_focal_lengths_most_used_first() {
        let stats = summarize(GearUsage {
            all: KeeperCounts::default(),
            cameras: vec![camera(
                "EOS R5",
                counts(30, 0, 0),
                &[(70, 5), (24, 12), (35, 12), (50, 1)],
            )],
            lenses: vec![],
        });
        let focal: Vec<(u32, usize)> = stats.cameras[0]
            .focal_lengths
            .iter()
            .map(|c| (c.value, c.count))
            .collect();
        assert_eq!(focal, [(24, 12), (35, 12), (70, 5), (50, 1)]);
        assert_eq!(
            stats.cameras[0].camera.as_ref().map(|k| k.model.as_str()),
            Some("EOS R5")
        );
    }
}
