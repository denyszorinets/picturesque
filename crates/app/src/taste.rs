//! The personal aesthetic model (`docs/features/ai.rst`): learned from the
//! user's picks and rejects over the CLIP embeddings already stored, and
//! retrained with the series whenever those decisions change
//! (`core::taste`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use picturesque_core::clip::Embedding;
use picturesque_core::photo::PhotoId;
use picturesque_core::taste::{MIN_EACH, TasteModel, train};
use serde::Serialize;
use ts_rs::TS;

use crate::ports::catalog::{CatalogError, PhotoCatalog};

/// What the personal model knows (`taste_status`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct TasteStatus {
    /// Picks and rejects with an embedding, which the model learns from.
    pub picks: usize,
    pub rejects: usize,
    /// How many of each are needed before anything is learned.
    pub needed: usize,
    /// Whether a model was learned.
    pub learned: bool,
    /// Cross-validated chance (0–1) that the model scores one of the
    /// user's picks above one of their rejects; `null` until learned.
    pub agreement: Option<f32>,
}

#[derive(Debug, Default)]
struct State {
    /// The examples the model was trained on, in capture order.
    labels: Vec<(PhotoId, bool)>,
    model: Option<Arc<TasteModel>>,
}

/// Keeps the personal model in step with the user's decisions.
pub struct TasteService {
    catalog: Arc<dyn PhotoCatalog>,
    state: Mutex<Option<State>>,
}

impl std::fmt::Debug for TasteService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TasteService").finish_non_exhaustive()
    }
}

impl TasteService {
    pub fn new(catalog: Arc<dyn PhotoCatalog>) -> Self {
        Self {
            catalog,
            state: Mutex::new(None),
        }
    }

    /// Retrains from the current picks and rejects if they changed since
    /// the last training. `order` lists photos in capture order.
    pub fn update(
        &self,
        order: &[PhotoId],
        embeddings: &HashMap<PhotoId, Embedding>,
    ) -> Result<Option<Arc<TasteModel>>, CatalogError> {
        let mut flags: HashMap<PhotoId, bool> = HashMap::new();
        flags.extend(
            self.catalog
                .picked_photos()?
                .into_iter()
                .map(|id| (id, true)),
        );
        flags.extend(
            self.catalog
                .rejected_photos()?
                .into_iter()
                .map(|id| (id, false)),
        );
        let labels: Vec<(PhotoId, bool)> = order
            .iter()
            .filter(|id| embeddings.contains_key(id))
            .filter_map(|id| Some((*id, *flags.get(id)?)))
            .collect();

        let mut state = self.state.lock().expect("taste lock");
        if let Some(s) = state.as_ref()
            && s.labels == labels
        {
            return Ok(s.model.clone());
        }
        let examples: Vec<(&Embedding, bool)> = labels
            .iter()
            .map(|(id, picked)| (&embeddings[id], *picked))
            .collect();
        let model = train(&examples).map(Arc::new);
        *state = Some(State {
            labels,
            model: model.clone(),
        });
        Ok(model)
    }

    /// The model as last trained.
    pub fn model(&self) -> Option<Arc<TasteModel>> {
        self.state
            .lock()
            .expect("taste lock")
            .as_ref()
            .and_then(|s| s.model.clone())
    }

    pub fn status(&self) -> TasteStatus {
        let state = self.state.lock().expect("taste lock");
        let labels = state.as_ref().map_or(&[][..], |s| &s.labels[..]);
        let picks = labels.iter().filter(|(_, p)| *p).count();
        let model = state.as_ref().and_then(|s| s.model.as_ref());
        TasteStatus {
            picks,
            rejects: labels.len() - picks,
            needed: MIN_EACH,
            learned: model.is_some(),
            agreement: model.map(|m| m.agreement),
        }
    }
}

#[cfg(test)]
mod tests {
    use picturesque_core::decision::{Decision, Flag, Rating};
    use picturesque_core::media::MediaFormat;
    use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets};

    use super::*;
    use crate::testing::{MemoryCatalog, fake_hash};

    /// Photos 1..=n whose embeddings lean towards the first axis when even.
    fn library(
        n: usize,
    ) -> (
        Arc<MemoryCatalog>,
        Vec<PhotoId>,
        HashMap<PhotoId, Embedding>,
    ) {
        let catalog = Arc::new(MemoryCatalog::default());
        let mut order = Vec::new();
        let mut embeddings = HashMap::new();
        for i in 0..n {
            let id = catalog
                .insert_photo(&NewPhoto {
                    metadata: Default::default(),
                    assets: PhotoAssets::new(vec![Asset {
                        format: MediaFormat::Jpeg,
                        path: format!("{i}.jpg").into(),
                        size: 1,
                        hash: fake_hash(format!("{i}").as_bytes()),
                    }])
                    .unwrap(),
                })
                .unwrap();
            let lean = if id.0 % 2 == 0 { 1.0 } else { -1.0 };
            let raw: Vec<f32> = (0..16)
                .map(|d| {
                    if d == 0 {
                        lean
                    } else {
                        ((i * 7 + d * 13) % 11) as f32 / 10.0
                    }
                })
                .collect();
            embeddings.insert(id, Embedding::new(&raw).unwrap());
            order.push(id);
        }
        (catalog, order, embeddings)
    }

    fn flag(catalog: &MemoryCatalog, id: PhotoId, flag: Flag) {
        catalog
            .set_decision(
                id,
                Decision {
                    flag,
                    rating: Rating::new(0).unwrap(),
                },
            )
            .unwrap();
    }

    #[test]
    fn learns_once_there_are_enough_picks_and_rejects() {
        let (catalog, order, embeddings) = library(40);
        let taste = TasteService::new(catalog.clone());
        for id in &order[..10] {
            flag(
                &catalog,
                *id,
                if id.0 % 2 == 0 {
                    Flag::Pick
                } else {
                    Flag::Reject
                },
            );
        }
        assert!(taste.update(&order, &embeddings).unwrap().is_none());
        assert_eq!(
            taste.status(),
            TasteStatus {
                picks: 5,
                rejects: 5,
                needed: MIN_EACH,
                learned: false,
                agreement: None,
            }
        );

        for id in &order[10..] {
            flag(
                &catalog,
                *id,
                if id.0 % 2 == 0 {
                    Flag::Pick
                } else {
                    Flag::Reject
                },
            );
        }
        let model = taste.update(&order, &embeddings).unwrap().expect("learned");
        let status = taste.status();
        assert_eq!(
            (status.picks, status.rejects, status.learned),
            (20, 20, true)
        );
        assert!(status.agreement.unwrap() > 0.9);
        assert!(
            model.score(&embeddings[&PhotoId(2)]).unwrap()
                > model.score(&embeddings[&PhotoId(3)]).unwrap()
        );
    }

    #[test]
    fn retrains_only_when_decisions_change() {
        let (catalog, order, embeddings) = library(40);
        let taste = TasteService::new(catalog.clone());
        for id in &order {
            flag(
                &catalog,
                *id,
                if id.0 % 2 == 0 {
                    Flag::Pick
                } else {
                    Flag::Reject
                },
            );
        }
        let first = taste.update(&order, &embeddings).unwrap().unwrap();
        let same = taste.update(&order, &embeddings).unwrap().unwrap();
        assert!(Arc::ptr_eq(&first, &same));

        flag(&catalog, order[0], Flag::Unflagged);
        let changed = taste.update(&order, &embeddings).unwrap().unwrap();
        assert!(!Arc::ptr_eq(&first, &changed));
        assert_eq!(taste.status().picks + taste.status().rejects, 39);
    }

    #[test]
    fn photos_without_embeddings_are_not_examples() {
        let (catalog, order, mut embeddings) = library(40);
        for id in &order {
            flag(
                &catalog,
                *id,
                if id.0 % 2 == 0 {
                    Flag::Pick
                } else {
                    Flag::Reject
                },
            );
        }
        embeddings.retain(|id, _| id.0 <= 10);
        let taste = TasteService::new(catalog);
        assert!(taste.update(&order, &embeddings).unwrap().is_none());
        assert_eq!(taste.status().picks + taste.status().rejects, 10);
    }
}
