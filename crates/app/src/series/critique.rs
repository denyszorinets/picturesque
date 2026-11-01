//! The critique part of the aesthetic score.

use std::collections::{HashMap, HashSet};

use picturesque_core::critique::{effective_rating, scores};
use picturesque_core::photo::PhotoId;

use super::SeriesService;
use crate::ports::catalog::CatalogError;

/// Critique scores, and for RAW photos whose rating the potential raised,
/// the rating and the potential.
type CritiqueScores = (HashMap<PhotoId, f32>, HashMap<PhotoId, (f32, f32)>);

impl SeriesService {
    /// Every critiqued photo's critique score, while critiques are on
    /// (`core::critique`): old critiques count until they are redone.
    ///
    /// A RAW original is scored on its rating moved toward its potential by
    /// `critic.potential_weight`, before the percentiles are computed.
    pub(super) fn critique_scores(
        &self,
        settings: &crate::settings::Settings,
    ) -> Result<CritiqueScores, CatalogError> {
        if !settings.critic.enabled {
            return Ok(Default::default());
        }
        // Rejects are not critiqued by the background critic, so they stay
        // out of the population the percentiles and the threshold use.
        let rejected: HashSet<PhotoId> = self.catalog.rejected_photos()?.into_iter().collect();
        let weight = settings.critic.potential_weight as f32;
        let raws: HashSet<PhotoId> = if weight > 0.0 {
            self.catalog
                .all_photos()?
                .into_iter()
                .filter(|p| p.assets.raw().is_some())
                .map(|p| p.id)
                .collect()
        } else {
            HashSet::new()
        };
        let mut lifts = HashMap::new();
        let ratings: Vec<(PhotoId, f32)> = self
            .catalog
            .analysis_results(crate::critic::ID)?
            .into_iter()
            .filter(|(id, _)| !rejected.contains(id))
            .filter_map(|(id, r)| {
                let rating = r["critique"]["rating"].as_f64()? as f32;
                let potential = r["critique"]["potential"].as_f64().map(|p| p as f32);
                let effective = effective_rating(rating, potential, raws.contains(&id), weight);
                if let Some(potential) = potential.filter(|_| effective > rating) {
                    lifts.insert(id, (rating, potential));
                }
                Some((id, effective))
            })
            .collect();
        Ok((scores(&ratings), lifts))
    }
}
