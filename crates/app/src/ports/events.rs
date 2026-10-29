//! Events pushed from the backend to the UI (ADR-0010).

use serde::Serialize;
use ts_rs::TS;

use crate::analysis::AnalysisStatus;
use crate::export::ExportStatus;
use crate::models::ModelView;
use crate::originals::OriginalsStatus;
use crate::pipeline::PipelineStatus;
use crate::series::SeriesStatus;
use crate::sidecars::SidecarStatus;

/// An event for the UI. Serialized with a `type` field, for example
/// `{ "type": "pipeline_progress", "queued": 3, ... }`.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum AppEvent {
    AnalysisProgress(AnalysisStatus),
    SeriesChanged(SeriesStatus),
    ModelProgress(ModelView),
    ExportProgress(ExportStatus),
    OriginalsProgress(OriginalsStatus),
    PipelineProgress(PipelineStatus),
    SidecarProgress(SidecarStatus),
    BackgroundCritiqueProgress(crate::critic::background::BackgroundCritiqueStatus),
}

/// Delivers events to the UI: Tauri events in the desktop app,
/// server-sent events in the development server.
pub trait EventSink: Send + Sync {
    fn emit(&self, event: &AppEvent);
}

/// Discards events.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoEvents;

impl EventSink for NoEvents {
    fn emit(&self, _event: &AppEvent) {}
}
