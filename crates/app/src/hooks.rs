//! Reactions to events: background work that follows other work.

use std::sync::{Arc, OnceLock};

use crate::analysis::AnalysisService;
use crate::import::model::ImportState;
use crate::ports::events::{AppEvent, EventSink};
use crate::series::SeriesService;
use crate::thumbnails::ThumbnailService;

/// Forwards events to the UI and starts follow-up work:
///
/// - an import completed: prefill thumbnails, wake analysis, recompute
///   series;
/// - a photo was analyzed: recompute series (debounced).
///
/// Services are set after construction because they emit through this
/// sink themselves.
pub struct EventHooks {
    pub inner: Arc<dyn EventSink>,
    pub thumbnails: OnceLock<Arc<ThumbnailService>>,
    pub analysis: OnceLock<Arc<AnalysisService>>,
    pub series: OnceLock<Arc<SeriesService>>,
}

impl EventHooks {
    pub fn new(inner: Arc<dyn EventSink>) -> Self {
        Self {
            inner,
            thumbnails: OnceLock::new(),
            analysis: OnceLock::new(),
            series: OnceLock::new(),
        }
    }
}

impl std::fmt::Debug for EventHooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventHooks").finish_non_exhaustive()
    }
}

impl EventSink for EventHooks {
    fn emit(&self, event: &AppEvent) {
        self.inner.emit(event);
        match event {
            AppEvent::ImportProgress(status) if status.state == ImportState::Completed => {
                if let Some(thumbnails) = self.thumbnails.get() {
                    thumbnails.prefill_in_background();
                }
                if let Some(analysis) = self.analysis.get() {
                    analysis.wake();
                }
                if let Some(series) = self.series.get() {
                    series.notify();
                }
            }
            AppEvent::AnalysisProgress(_) => {
                if let Some(series) = self.series.get() {
                    series.notify();
                }
            }
            AppEvent::ImportProgress(_) | AppEvent::SeriesChanged(_) => {}
        }
    }
}
