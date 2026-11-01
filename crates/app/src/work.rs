//! What the app is working on right now, for the tray icon.
//!
//! Folds the progress events into one line ("Importing 120 of 900") and
//! a busy flag. The shell decides what to do with them: hide the window
//! instead of quitting while there is work, show the line in the tray.

use std::sync::Mutex;

use crate::analysis::AnalysisStatus;
use crate::critic::background::BackgroundCritiqueStatus;
use crate::export::{ExportState, ExportStatus};
use crate::models::{ModelState, ModelView};
use crate::originals::{OriginalsState, OriginalsStatus};
use crate::pipeline::PipelineStatus;
use crate::ports::events::AppEvent;

#[derive(Debug, Default)]
struct Latest {
    pipeline: Option<PipelineStatus>,
    analysis: Option<AnalysisStatus>,
    critique: Option<BackgroundCritiqueStatus>,
    export: Option<ExportStatus>,
    originals: Option<OriginalsStatus>,
    /// Models being downloaded, by id.
    downloads: Vec<(String, String)>,
}

/// Tracks the latest progress of every kind of long job.
#[derive(Debug, Default)]
pub struct WorkTracker(Mutex<Latest>);

impl WorkTracker {
    /// Remembers the progress an event carries. Other events are ignored.
    pub fn observe(&self, event: &AppEvent) {
        let mut latest = self.0.lock().expect("work lock");
        match event {
            AppEvent::PipelineProgress(s) => latest.pipeline = Some(s.clone()),
            AppEvent::AnalysisProgress(s) => latest.analysis = Some(s.clone()),
            AppEvent::BackgroundCritiqueProgress(s) => latest.critique = Some(s.clone()),
            AppEvent::ExportProgress(s) => latest.export = Some(s.clone()),
            AppEvent::OriginalsProgress(s) => latest.originals = Some(s.clone()),
            AppEvent::ModelProgress(m) => latest.model(m),
            AppEvent::SeriesChanged(_) | AppEvent::SidecarProgress(_) => {}
        }
    }

    /// One line about the most important running job, or `None` when
    /// nothing runs. Jobs that wait for a reason (paused) do not count.
    pub fn summary(&self) -> Option<String> {
        self.0.lock().expect("work lock").summary()
    }

    pub fn busy(&self) -> bool {
        self.summary().is_some()
    }
}

impl Latest {
    fn model(&mut self, m: &ModelView) {
        self.downloads.retain(|(id, _)| *id != m.id);
        if m.state == ModelState::Downloading {
            self.downloads.push((m.id.clone(), m.name.clone()));
        }
    }

    fn summary(&self) -> Option<String> {
        // Import first: it is what the user waits for.
        if let Some(p) = &self.pipeline
            && p.paused.is_none()
            && (p.queued > 0 || p.active.is_some())
        {
            let done = p.imported + p.duplicates + p.failed;
            return Some(format!("Importing {done} of {}", done + p.queued));
        }
        if let Some(p) = &self.pipeline
            && p.paused.is_none()
            && p.scanning
        {
            return Some("Scanning import folders".into());
        }
        if let Some(o) = &self.originals
            && o.state == OriginalsState::Running
        {
            return Some(format!("Deleting originals: {} of {}", o.done, o.total));
        }
        if let Some(e) = &self.export
            && e.state == ExportState::Running
        {
            return Some(format!("Exporting {} of {}", e.done, e.total));
        }
        if let Some((_, name)) = self.downloads.first() {
            return Some(format!("Downloading {name}"));
        }
        if let Some(a) = &self.analysis
            && a.running
            && a.paused.is_none()
            && a.analyzed < a.total
        {
            return Some(format!("Analyzing {} of {}", a.analyzed, a.total));
        }
        if let Some(c) = &self.critique
            && c.enabled
            && c.running
            && c.paused.is_none()
        {
            let done = c.critiqued;
            return Some(format!("Critiquing {done} of {}", c.total));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::ActiveJob;
    use crate::ports::pipeline::JobStage;

    fn pipeline(queued: usize, imported: usize) -> PipelineStatus {
        PipelineStatus {
            queued,
            imported,
            duplicates: 0,
            failed: 0,
            active: None,
            scanning: false,
            paused: None,
        }
    }

    fn analysis(running: bool, analyzed: usize, total: usize) -> AnalysisStatus {
        AnalysisStatus {
            analyzed,
            total,
            running,
            paused: None,
            waiting_for_models: Vec::new(),
            ai_unavailable: None,
        }
    }

    fn seen(events: &[AppEvent]) -> WorkTracker {
        let t = WorkTracker::default();
        events.iter().for_each(|e| t.observe(e));
        t
    }

    #[test]
    fn nothing_runs_before_any_event() {
        let t = WorkTracker::default();
        assert_eq!(t.summary(), None);
        assert!(!t.busy());
    }

    #[test]
    fn the_summary_names_the_running_job() {
        let model = |state| ModelView {
            id: "clip".into(),
            name: "CLIP".into(),
            purpose: String::new(),
            version: 1,
            license: String::new(),
            license_url: String::new(),
            homepage: String::new(),
            size: 1,
            state,
            downloaded: 0,
            error: None,
        };
        let cases = [
            (
                AppEvent::PipelineProgress(pipeline(780, 120)),
                "Importing 120 of 900",
            ),
            (
                AppEvent::AnalysisProgress(analysis(true, 4, 10)),
                "Analyzing 4 of 10",
            ),
            (
                AppEvent::ModelProgress(model(ModelState::Downloading)),
                "Downloading CLIP",
            ),
        ];
        for (event, line) in cases {
            let t = seen(&[event]);
            assert_eq!(t.summary().as_deref(), Some(line));
            assert!(t.busy());
        }
    }

    #[test]
    fn a_finished_job_is_no_longer_work() {
        let t = seen(&[
            AppEvent::PipelineProgress(pipeline(2, 0)),
            AppEvent::AnalysisProgress(analysis(true, 1, 3)),
        ]);
        assert!(t.busy());
        t.observe(&AppEvent::PipelineProgress(pipeline(0, 2)));
        assert_eq!(t.summary().as_deref(), Some("Analyzing 1 of 3"));
        t.observe(&AppEvent::AnalysisProgress(analysis(false, 3, 3)));
        assert!(!t.busy());
    }

    #[test]
    fn a_job_that_waits_for_a_reason_is_not_work() {
        let mut paused = pipeline(5, 0);
        paused.paused = Some("library folder missing".into());
        assert!(!seen(&[AppEvent::PipelineProgress(paused)]).busy());

        let mut waiting = analysis(true, 1, 3);
        waiting.paused = Some("on battery".into());
        assert!(!seen(&[AppEvent::AnalysisProgress(waiting)]).busy());
    }

    #[test]
    fn the_photo_in_flight_counts_even_with_an_empty_queue() {
        let mut p = pipeline(0, 4);
        p.active = Some(ActiveJob {
            job_id: 5,
            file_name: "a.jpg".into(),
            stage: JobStage::Copying,
        });
        let t = seen(&[AppEvent::PipelineProgress(p)]);
        assert_eq!(t.summary().as_deref(), Some("Importing 4 of 4"));
    }

    #[test]
    fn import_outranks_analysis() {
        let t = seen(&[
            AppEvent::AnalysisProgress(analysis(true, 1, 3)),
            AppEvent::PipelineProgress(pipeline(1, 1)),
        ]);
        assert_eq!(t.summary().as_deref(), Some("Importing 1 of 2"));
    }
}
