//! Keeping series up to date (`docs/features/series.rst`).
//!
//! Series are recomputed from scratch (capture times, perceptual hashes,
//! the user's edits and thresholds) whenever an input changes: right away
//! after an edit or a settings change, and debounced while analysis and
//! imports add data. With 10,000 photos a run takes milliseconds.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use picturesque_core::photo::PhotoId;
use picturesque_core::series::{SeriesEdit, SeriesInput, detect_series};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::analysis::similarity::SimilarityAnalyzer;
use crate::ports::catalog::{CatalogError, PhotoCatalog};
use crate::ports::events::{AppEvent, EventSink};
use crate::ports::settings::SettingsStore;

/// Quiet time before a requested recompute runs.
pub const DEBOUNCE: Duration = Duration::from_secs(1);

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

/// A photo's series, returned by `photo_series` and `edit_series`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
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
    last: Mutex<Option<Vec<Vec<PhotoId>>>>,
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

    /// Recomputes series now. Stores them and sends `series_changed` only
    /// when membership changed. Returns the number of series.
    pub fn recompute(&self) -> Result<usize, CatalogError> {
        let mut last = self.last.lock().expect("series lock");
        let thresholds = self.settings.load_settings()?.series.thresholds();
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
        let series = detect_series(&inputs, &edits, thresholds);
        let count = series.len();
        if last.as_ref() != Some(&series) {
            self.catalog.replace_series(&series)?;
            self.events.emit(&AppEvent::SeriesChanged(SeriesStatus {
                series: count,
                photos: series.iter().map(Vec::len).sum(),
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
        let (series_id, photo_ids) = match self.catalog.series_of(id)? {
            Some((series, members)) => (Some(series.0), members.iter().map(|p| p.0).collect()),
            None => (None, Vec::new()),
        };
        Ok(SeriesView {
            photo_id: id.0,
            series_id,
            photo_ids,
            edit,
        })
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
        loop {
            match rx.recv_timeout(DEBOUNCE) {
                Ok(()) => continue,
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        let _ = service.recompute();
    }
}

#[cfg(test)]
mod tests;
