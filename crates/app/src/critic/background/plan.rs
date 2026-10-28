//! The order the background critic works in, and what is left to do.

use std::collections::{HashMap, HashSet};

use picturesque_core::photo::PhotoId;

use super::super::prompt::rubric_key;
use super::super::{CritiqueService, ID};
use crate::ports::catalog::{CatalogError, PhotoCatalog};
use crate::ports::critic::Throttle;
use crate::series::SeriesService;
use crate::settings::{CriticIntensity, CriticSettings};

/// Where a photo comes in the order (lower first).
fn tier(
    id: PhotoId,
    picks: &HashSet<PhotoId>,
    best: &HashSet<PhotoId>,
    in_series: &HashSet<PhotoId>,
) -> u8 {
    if best.contains(&id) || picks.contains(&id) {
        0
    } else if !in_series.contains(&id) {
        1
    } else {
        2
    }
}

/// The throttle for an intensity: gentle uses half the threads at low
/// priority.
pub fn throttle(intensity: CriticIntensity) -> Throttle {
    match intensity {
        CriticIntensity::Gentle => Throttle {
            low_priority: true,
            threads: Some(
                std::thread::available_parallelism()
                    .map_or(2, |n| n.get() as u32)
                    .div_ceil(2),
            ),
        },
        CriticIntensity::WhenIdle | CriticIntensity::Full => Throttle {
            low_priority: false,
            threads: None,
        },
    }
}

/// The photos to critique, in order: missing critiques before stale ones,
/// then by tier, then newest first.
pub(super) struct Plan {
    pub queue: Vec<PhotoId>,
    pub critiqued: usize,
    pub stale: usize,
    pub missing: usize,
    pub total: usize,
}

impl Plan {
    pub(super) fn load(
        catalog: &dyn PhotoCatalog,
        series: &SeriesService,
        settings: &CriticSettings,
    ) -> Result<Self, CatalogError> {
        let rejected: HashSet<PhotoId> = catalog.rejected_photos()?.into_iter().collect();
        let picks: HashSet<PhotoId> = catalog.picked_photos()?.into_iter().collect();
        let series = series.stored_series();
        let best: HashSet<PhotoId> = series.iter().map(|s| s.best).collect();
        let in_series: HashSet<PhotoId> = series
            .iter()
            .flat_map(|s| s.members.iter().copied())
            .collect();
        let model = CritiqueService::model_name(settings);
        let rubric = rubric_key(settings.preset, settings.rubric());
        // true: current; false: stale.
        let critiques: HashMap<PhotoId, bool> = catalog
            .analysis_results(ID)?
            .into_iter()
            .map(|(id, r)| (id, r["model"] == model && r["rubric"] == rubric.as_str()))
            .collect();
        let mut photos = catalog.capture_times()?;
        photos.retain(|(id, _)| !rejected.contains(id));
        // Newest first; undated last.
        photos.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.cmp(&a.0)));
        let mut plan = Self {
            queue: Vec::new(),
            critiqued: 0,
            stale: 0,
            missing: 0,
            total: photos.len(),
        };
        let mut todo: Vec<(bool, u8, usize, PhotoId)> = Vec::new();
        for (i, (id, _)) in photos.iter().enumerate() {
            match critiques.get(id) {
                Some(true) => plan.critiqued += 1,
                Some(false) => {
                    plan.stale += 1;
                    todo.push((true, tier(*id, &picks, &best, &in_series), i, *id));
                }
                None => {
                    plan.missing += 1;
                    todo.push((false, tier(*id, &picks, &best, &in_series), i, *id));
                }
            }
        }
        todo.sort_unstable();
        plan.queue = todo.into_iter().map(|(_, _, _, id)| id).collect();
        Ok(plan)
    }
}
