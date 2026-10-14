//! Reactions to events: background work that follows other work.

use std::sync::{Arc, OnceLock};

use crate::analysis::AnalysisService;
use crate::embeddings::EmbeddingIndex;
use crate::models::ModelState;
use crate::ports::events::{AppEvent, EventSink};
use crate::series::SeriesService;

/// Forwards events to the UI and starts follow-up work:
///
/// - a photo was analyzed: recompute series (debounced), reload embeddings;
/// - a model was downloaded or removed: analyzers using it start or stop.
///
/// Services are set after construction because they emit through this
/// sink themselves.
pub struct EventHooks {
    pub inner: Arc<dyn EventSink>,
    pub analysis: OnceLock<Arc<AnalysisService>>,
    pub series: OnceLock<Arc<SeriesService>>,
    pub embeddings: OnceLock<Arc<EmbeddingIndex>>,
}

impl EventHooks {
    pub fn new(inner: Arc<dyn EventSink>) -> Self {
        Self {
            inner,
            analysis: OnceLock::new(),
            series: OnceLock::new(),
            embeddings: OnceLock::new(),
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
            AppEvent::AnalysisProgress(_) => {
                if let Some(series) = self.series.get() {
                    series.notify();
                }
                if let Some(embeddings) = self.embeddings.get() {
                    embeddings.invalidate();
                }
            }
            // A model came or went: analyzers using it start or stop.
            AppEvent::ModelProgress(model)
                if matches!(
                    model.state,
                    ModelState::Installed | ModelState::NotInstalled
                ) =>
            {
                if let Some(analysis) = self.analysis.get() {
                    analysis.wake();
                    analysis.report();
                }
            }
            AppEvent::SeriesChanged(_)
            | AppEvent::ModelProgress(_)
            | AppEvent::ExportProgress(_)
            | AppEvent::PipelineProgress(_) => {}
        }
    }
}
