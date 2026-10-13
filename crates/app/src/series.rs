//! Keeping series and their best photos up to date
//! (`docs/features/series.rst`).
//!
//! Series are recomputed from scratch (capture times, perceptual hashes,
//! technical scores, decisions, the user's edits and choices, settings)
//! whenever an input changes: right away after an edit, a choice or a
//! settings change, and debounced while analysis, imports and culling
//! change data. With 10,000 photos a run takes milliseconds.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use picturesque_core::photo::PhotoId;
use picturesque_core::quality::TechnicalScores;
use picturesque_core::ranking::{Candidate, best, rank};
use picturesque_core::series::{SeriesEdit, SeriesInput, detect_series};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::analysis::similarity::SimilarityAnalyzer;
use crate::analysis::technical::TechnicalAnalyzer;
use crate::ports::catalog::{CatalogError, PhotoCatalog, StoredSeries};
use crate::ports::events::{AppEvent, EventSink};
use crate::ports::settings::SettingsStore;

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

/// Recomputes and stores series.
pub struct SeriesService {
    catalog: Arc<dyn PhotoCatalog>,
    settings: Arc<dyn SettingsStore>,
    events: Arc<dyn EventSink>,
    /// The last stored series; also serializes recomputes.
    last: Mutex<Option<Vec<StoredSeries>>>,
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
            catalog,
            settings,
            events,
            last: Mutex::new(None),
            notify: Mutex::new(None),
        }
    }

    /// Recomputes series and their best photos now. Stores them and sends
    /// `series_changed` only when something changed. Returns the number of
    /// series.
    pub fn recompute(&self) -> Result<usize, CatalogError> {
        let mut last = self.last.lock().expect("series lock");
        let settings = self.settings.load_settings()?;
        let signatures: HashMap<PhotoId, _> = self
            .catalog
            .analysis_results(SimilarityAnalyzer::ID)?
            .into_iter()
            .filter_map(|(id, result)| Some((id, SimilarityAnalyzer::signature(&result)?)))
            .collect();
        let inputs: Vec<SeriesInput> = self
            .catalog
            .capture_times()?
            .into_iter()
            .map(|(id, captured)| SeriesInput {
                id,
                captured,
                signature: signatures.get(&id).copied(),
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
        let choices = self.catalog.best_choices()?;
        let weights = settings.ranking.weights();
        let series: Vec<StoredSeries> = groups
            .into_iter()
            .filter_map(|members| {
                let candidates: Vec<Candidate> = members
                    .iter()
                    .map(|id| Candidate {
                        id: *id,
                        scores: scores.get(id).copied(),
                        rejected: rejected.contains(id),
                    })
                    .collect();
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

        let count = series.len();
        if last.as_ref() != Some(&series) {
            self.catalog.replace_series(&series)?;
            self.events.emit(&AppEvent::SeriesChanged(SeriesStatus {
                series: count,
                photos: series.iter().map(|s| s.members.len()).sum(),
            }));
            *last = Some(series);
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

#[cfg(test)]
mod tests;
