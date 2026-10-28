//! Keeping series and their best photos up to date
//! (`docs/features/series.rst`).
//!
//! Series are recomputed from scratch (capture times, perceptual hashes,
//! CLIP embeddings, technical and aesthetic scores, faces, decisions, the
//! user's edits and choices, settings), together with every photo's
//! combined quality score, whenever an input changes: right away after an
//! edit, a choice or a settings change, and debounced while analysis,
//! imports and culling change data. With 10,000 photos a run takes
//! milliseconds; when picks or rejects changed, the personal taste model
//! is retrained first (`taste`), which takes up to about a second.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use picturesque_core::clip::Embedding;
use picturesque_core::photo::PhotoId;
use picturesque_core::quality::TechnicalScores;
use picturesque_core::ranking::{
    Candidate, FaceSignal, Standing, best, is_keeper, photo_quality, rank,
};
use picturesque_core::series::{SeriesEdit, SeriesInput, detect_series};
use picturesque_core::taste::blend;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::analysis::clip::{ClipAnalyzer, aesthetic_score};
use crate::analysis::faces::{FaceAnalyzer, FaceView, faces as faces_of, signal as face_signal};
use crate::analysis::similarity::SimilarityAnalyzer;
use crate::analysis::technical::TechnicalAnalyzer;
use crate::ports::catalog::{CatalogError, PhotoCatalog, PhotoScore, StoredSeries};
use crate::ports::events::{AppEvent, EventSink};
use crate::ports::settings::SettingsStore;
use crate::taste::TasteService;

/// Quiet time before a requested recompute runs.
pub const DEBOUNCE: Duration = Duration::from_secs(1);
/// Longest a requested recompute waits while requests keep coming (during
/// analysis), so series appear as photos are analyzed.
pub const MAX_WAIT: Duration = Duration::from_secs(5);

/// Sent when series membership changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct SeriesStatus {
    /// Number of series.
    pub series: usize,
    /// Photos in a series.
    pub photos: usize,
}

/// A user's correction of the series boundary before a photo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SeriesEditView {
    /// The photo starts a new series.
    Split,
    /// The photo joins the series of the photo before it.
    Join,
}

impl From<SeriesEditView> for SeriesEdit {
    fn from(e: SeriesEditView) -> Self {
        match e {
            SeriesEditView::Split => Self::Split,
            SeriesEditView::Join => Self::Join,
        }
    }
}

impl From<SeriesEdit> for SeriesEditView {
    fn from(e: SeriesEdit) -> Self {
        match e {
            SeriesEdit::Split => Self::Split,
            SeriesEdit::Join => Self::Join,
        }
    }
}

/// A photo's series, returned by `photo_series`, `edit_series` and
/// `choose_best`.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct SeriesView {
    #[ts(type = "number")]
    pub photo_id: i64,
    /// The first photo's id; `null` when the photo is in no series.
    #[ts(type = "number | null")]
    pub series_id: Option<i64>,
    /// Members in capture order (empty when in no series).
    #[ts(type = "number[]")]
    pub photo_ids: Vec<i64>,
    /// The user's correction before this photo, if any.
    pub edit: Option<SeriesEditView>,
    /// The best photo of the series.
    #[ts(type = "number | null")]
    pub best_photo_id: Option<i64>,
    /// Whether the user chose the best photo (rather than the ranking).
    pub best_chosen: bool,
    /// Members best first, with their scores (0–1, `null` until analyzed).
    pub ranking: Vec<RankedView>,
}

/// A photo's place in its series' ranking.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct RankedView {
    #[ts(type = "number")]
    pub photo_id: i64,
    pub score: Option<f32>,
}

/// Arguments of `choose_best`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct ChooseBestArgs {
    #[ts(type = "number")]
    pub photo_id: i64,
    /// `false` withdraws the choice, returning to the ranking.
    pub best: bool,
}

/// Arguments of `edit_series`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct EditSeriesArgs {
    #[ts(type = "number")]
    pub photo_id: i64,
    /// `null` removes the correction.
    pub edit: Option<SeriesEditView>,
}

/// Series and every photo's combined score, as last stored.
type Stored = (Vec<StoredSeries>, Vec<PhotoScore>);

/// Recomputes and stores series.
pub struct SeriesService {
    catalog: Arc<dyn PhotoCatalog>,
    settings: Arc<dyn SettingsStore>,
    events: Arc<dyn EventSink>,
    taste: TasteService,
    /// Critique scores as last computed (`critique_score`).
    critique_scores: Mutex<HashMap<PhotoId, f32>>,
    /// For RAW photos whose rating was raised by their potential: the
    /// rating and the potential (`critique_lift`).
    critique_lifts: Mutex<HashMap<PhotoId, (f32, f32)>>,
    /// The last stored series and scores; also serializes recomputes.
    last: Mutex<Option<Stored>>,
    notify: Mutex<Option<Sender<()>>>,
}

impl std::fmt::Debug for SeriesService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SeriesService").finish_non_exhaustive()
    }
}

impl SeriesService {
    pub fn new(
        catalog: Arc<dyn PhotoCatalog>,
        settings: Arc<dyn SettingsStore>,
        events: Arc<dyn EventSink>,
    ) -> Self {
        Self {
            taste: TasteService::new(Arc::clone(&catalog)),
            critique_scores: Mutex::default(),
            critique_lifts: Mutex::default(),
            catalog,
            settings,
            events,
            last: Mutex::new(None),
            notify: Mutex::new(None),
        }
    }

    /// The series as last stored (empty before the first recompute).
    pub fn stored_series(&self) -> Vec<StoredSeries> {
        self.last
            .lock()
            .expect("series lock")
            .as_ref()
            .map(|(series, _)| series.clone())
            .unwrap_or_default()
    }

    /// A photo's critique score (0–1, its rating's percentile among the
    /// library's critiques) as last computed; `None` without a critique or
    /// while critiques are off.
    pub fn critique_score(&self, id: PhotoId) -> Option<f32> {
        self.critique_scores
            .lock()
            .expect("critique scores lock")
            .get(&id)
            .copied()
    }

    /// A RAW photo's critique rating and the potential rating it was raised
    /// toward, as last computed; `None` when the potential did not raise it.
    pub fn critique_lift(&self, id: PhotoId) -> Option<(f32, f32)> {
        self.critique_lifts
            .lock()
            .expect("critique lifts lock")
            .get(&id)
            .copied()
    }

    /// The personal taste model, retrained by [`Self::recompute`].
    pub fn taste(&self) -> &TasteService {
        &self.taste
    }

    /// Recomputes series, their best photos and every photo's combined
    /// quality score now. Stores them and sends `series_changed` only when
    /// something changed. Returns the number of series.
    pub fn recompute(&self) -> Result<usize, CatalogError> {
        let mut last = self.last.lock().expect("series lock");
        let settings = self.settings.load_settings()?;
        let signatures: HashMap<PhotoId, _> = self
            .catalog
            .analysis_results(SimilarityAnalyzer::ID)?
            .into_iter()
            .filter_map(|(id, result)| Some((id, SimilarityAnalyzer::signature(&result)?)))
            .collect();
        let clip = self.catalog.analysis_results(ClipAnalyzer::ID)?;
        let embeddings: HashMap<PhotoId, Embedding> = clip
            .iter()
            .filter_map(|(id, result)| Some((*id, ClipAnalyzer::embedding(result)?)))
            .collect();
        let general: HashMap<PhotoId, f32> = clip
            .iter()
            .filter_map(|(id, result)| {
                Some((*id, aesthetic_score(ClipAnalyzer::aesthetic(result)?)))
            })
            .collect();
        let mut captured = self.catalog.capture_times()?;
        captured.sort_by_key(|(id, time)| (*time, *id));
        let order: Vec<PhotoId> = captured.iter().map(|(id, _)| *id).collect();
        let personal = self.taste.update(&order, &embeddings)?;
        let personal_weight = settings.taste.personal_weight as f32;
        let (critiques, lifts) = self.critique_scores(&settings)?;
        let critique_weight = settings.critic.score_weight as f32;
        let aesthetic: HashMap<PhotoId, f32> = embeddings
            .keys()
            .chain(critiques.keys())
            .collect::<HashSet<_>>()
            .into_iter()
            .filter_map(|id| {
                let p = personal.as_ref().and_then(|m| m.score(embeddings.get(id)?));
                let looks = blend(general.get(id).copied(), p, personal_weight);
                Some((
                    *id,
                    blend(looks, critiques.get(id).copied(), critique_weight)?,
                ))
            })
            .collect();
        *self.critique_scores.lock().expect("critique scores lock") = critiques;
        *self.critique_lifts.lock().expect("critique lifts lock") = lifts;
        let found_faces: HashMap<PhotoId, Vec<FaceView>> = self
            .catalog
            .analysis_results(FaceAnalyzer::ID)?
            .into_iter()
            .filter_map(|(id, result)| Some((id, faces_of(&result)?)))
            .collect();
        let faces: HashMap<PhotoId, FaceSignal> = found_faces
            .iter()
            .filter_map(|(id, f)| Some((*id, face_signal(f)?)))
            .collect();
        let face_counts: HashMap<PhotoId, u32> = found_faces
            .iter()
            .map(|(id, f)| (*id, f.len() as u32))
            .collect();
        let inputs: Vec<SeriesInput> = captured
            .into_iter()
            .map(|(id, captured)| SeriesInput {
                id,
                captured,
                signature: signatures.get(&id).copied(),
                embedding: embeddings.get(&id),
            })
            .collect();
        let edits = self.catalog.series_edits()?.into_iter().collect();
        let groups = detect_series(&inputs, &edits, settings.series.thresholds());

        let scores: HashMap<PhotoId, TechnicalScores> = self
            .catalog
            .analysis_results(TechnicalAnalyzer::ID)?
            .into_iter()
            .filter_map(|(id, result)| Some((id, serde_json::from_value(result).ok()?)))
            .collect();
        let rejected: HashSet<PhotoId> = self.catalog.rejected_photos()?.into_iter().collect();
        let candidate = |id: PhotoId| Candidate {
            id,
            scores: scores.get(&id).copied(),
            rejected: rejected.contains(&id),
            aesthetic: aesthetic.get(&id).copied(),
            face: faces.get(&id).copied(),
        };
        let choices = self.catalog.best_choices()?;
        let weights = settings.ranking.weights();
        let series: Vec<StoredSeries> = groups
            .into_iter()
            .filter_map(|members| {
                let candidates: Vec<Candidate> = members.iter().map(|id| candidate(*id)).collect();
                let ranking = rank(&candidates, weights);
                let (best, best_chosen) = best(&ranking, &choices)?;
                Some(StoredSeries {
                    members,
                    ranking,
                    best,
                    best_chosen,
                })
            })
            .collect();
        let mut standing: HashMap<PhotoId, Standing> = HashMap::new();
        for s in &series {
            for id in &s.members {
                standing.insert(*id, Standing::Other);
            }
            standing.insert(
                s.best,
                Standing::Best {
                    chosen: s.best_chosen,
                },
            );
        }
        let auto_pick = settings.auto_pick;
        let mut qualities: Vec<PhotoScore> = scores
            .keys()
            .filter_map(|id| {
                let score = photo_quality(&candidate(*id))?;
                let eyes_closed = faces.get(id).and_then(|f| f.eyes_closed);
                Some(PhotoScore {
                    id: *id,
                    score,
                    aesthetic: aesthetic.get(id).copied(),
                    eyes_closed,
                    faces: face_counts.get(id).copied().unwrap_or(0),
                    keeper: auto_pick.enabled
                        && is_keeper(
                            Some(score),
                            eyes_closed,
                            standing.get(id).copied().unwrap_or(Standing::Alone),
                            auto_pick.min_quality as f32,
                        ),
                })
            })
            .collect();
        qualities.sort_by_key(|s| s.id);

        let count = series.len();
        let (old_series, old_qualities) = match last.as_ref() {
            Some((s, q)) => (Some(s), Some(q)),
            None => (None, None),
        };
        let series_changed = old_series != Some(&series);
        let scores_changed = old_qualities != Some(&qualities);
        if series_changed {
            self.catalog.replace_series(&series)?;
        }
        if scores_changed {
            self.catalog.replace_scores(&qualities)?;
        }
        if series_changed || scores_changed {
            self.events.emit(&AppEvent::SeriesChanged(SeriesStatus {
                series: count,
                photos: series.iter().map(|s| s.members.len()).sum(),
            }));
            *last = Some((series, qualities));
        }
        Ok(count)
    }

    /// The series `id` belongs to.
    pub fn view(&self, id: PhotoId) -> Result<SeriesView, CatalogError> {
        let edit = self
            .catalog
            .series_edits()?
            .into_iter()
            .find(|(p, _)| *p == id)
            .map(|(_, e)| e.into());
        let mut view = SeriesView {
            photo_id: id.0,
            series_id: None,
            photo_ids: Vec::new(),
            edit,
            best_photo_id: None,
            best_chosen: false,
            ranking: Vec::new(),
        };
        if let Some((series_id, series)) = self.catalog.series_of(id)? {
            view.series_id = Some(series_id.0);
            view.photo_ids = series.members.iter().map(|p| p.0).collect();
            view.best_photo_id = Some(series.best.0);
            view.best_chosen = series.best_chosen;
            view.ranking = series
                .ranking
                .iter()
                .map(|r| RankedView {
                    photo_id: r.id.0,
                    score: r.score,
                })
                .collect();
        }
        Ok(view)
    }

    /// Makes `id` the best photo of its series (or withdraws that choice)
    /// and recomputes right away.
    pub fn choose_best(&self, id: PhotoId, chosen: bool) -> Result<SeriesView, CatalogError> {
        self.catalog.set_best_choice(id, chosen)?;
        self.recompute()?;
        self.view(id)
    }

    /// Records a correction and recomputes right away.
    pub fn edit(&self, id: PhotoId, edit: Option<SeriesEdit>) -> Result<SeriesView, CatalogError> {
        self.catalog.set_series_edit(id, edit)?;
        self.recompute()?;
        self.view(id)
    }

    /// Asks for a recompute once things are quiet (after `start`).
    pub fn notify(&self) {
        if let Some(tx) = self.notify.lock().expect("notify lock").as_ref() {
            let _ = tx.send(());
        }
    }

    /// Stops the background worker (it exits once it has no sender).
    pub fn stop(&self) {
        *self.notify.lock().expect("notify lock") = None;
    }

    /// Recomputes once now and then whenever notified.
    pub fn start(self: &Arc<Self>) {
        let (tx, rx) = channel();
        *self.notify.lock().expect("notify lock") = Some(tx);
        let service = Arc::clone(self);
        std::thread::spawn(move || {
            let _ = service.recompute();
            worker(&service, &rx);
        });
    }
}

fn worker(service: &SeriesService, rx: &Receiver<()>) {
    while rx.recv().is_ok() {
        if !settle(rx, DEBOUNCE, MAX_WAIT) {
            return;
        }
        let _ = service.recompute();
    }
}

/// After a request, waits until no request came for `quiet`, or `max_wait`
/// passed. Returns `false` when the sender is gone.
fn settle(rx: &Receiver<()>, quiet: Duration, max_wait: Duration) -> bool {
    let deadline = Instant::now() + max_wait;
    loop {
        let wait = quiet.min(deadline.saturating_duration_since(Instant::now()));
        if wait.is_zero() {
            return true;
        }
        match rx.recv_timeout(wait) {
            Ok(()) => continue,
            Err(RecvTimeoutError::Timeout) => return true,
            Err(RecvTimeoutError::Disconnected) => return false,
        }
    }
}

mod critique;

#[cfg(test)]
mod tests;
