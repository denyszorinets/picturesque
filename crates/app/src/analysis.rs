//! Background analysis of photos (technical scores, similarity, and later
//! AI models).
//!
//! Each [`Analyzer`] turns a photo's preview into a JSON result, stored per
//! photo, analyzer and version. A photo is pending while any analyzer lacks
//! a current result, so analysis resumes after a restart and reruns when an
//! analyzer's version changes. Workers take photos the UI asked for first,
//! then the rest in capture order.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use picturesque_core::photo::{Photo, PhotoId};
use picturesque_core::pixels::RgbImage;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ports::catalog::{AnalyzerId, PhotoCatalog};
use crate::ports::events::{AppEvent, EventSink};
use crate::ports::pixels::PixelLoader;
use crate::thumbnails::{ImageSize, ThumbnailService};

pub mod clip;
pub mod faces;
pub mod similarity;
pub mod technical;

/// The analyzers the app runs.
pub fn default_analyzers() -> Vec<Arc<dyn Analyzer>> {
    vec![
        Arc::new(technical::TechnicalAnalyzer),
        Arc::new(similarity::SimilarityAnalyzer),
    ]
}

/// Longest edge of the pixels analyzers receive.
pub const ANALYSIS_EDGE: u32 = 1024;

/// What an analyzer gets for one photo.
#[derive(Debug)]
pub struct AnalysisInput<'a> {
    pub photo: &'a Photo,
    /// The photo's preview, upright, longest edge at most [`ANALYSIS_EDGE`].
    pub image: &'a RgbImage,
}

/// One kind of analysis.
pub trait Analyzer: Send + Sync {
    fn id(&self) -> AnalyzerId;

    fn analyze(&self, input: &AnalysisInput<'_>) -> Result<serde_json::Value, String>;

    /// Whether the analyzer can run now (an AI analyzer needs its model
    /// downloaded). Photos are not pending for analyzers that are not
    /// ready.
    fn ready(&self) -> bool {
        true
    }
}

/// Analysis progress, sent as an event and returned by `analysis_status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct AnalysisStatus {
    /// Photos with every analyzer done.
    pub analyzed: usize,
    pub total: usize,
    /// Whether workers are busy.
    pub running: bool,
}

/// Arguments of `prioritize_analysis`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct PrioritizeArgs {
    #[ts(type = "number[]")]
    pub photo_ids: Vec<i64>,
}

struct Queue {
    /// Photos the UI asked for, most recent first.
    priority: VecDeque<PhotoId>,
    /// Photos being analyzed now.
    in_progress: Vec<PhotoId>,
    /// Set when there may be new work.
    wake: bool,
    busy: usize,
}

/// Runs analyzers over the library on background threads.
pub struct AnalysisService {
    catalog: Arc<dyn PhotoCatalog>,
    thumbnails: Arc<ThumbnailService>,
    pixels: Arc<dyn PixelLoader>,
    analyzers: Vec<Arc<dyn Analyzer>>,
    events: Arc<dyn EventSink>,
    queue: Mutex<Queue>,
    signal: Condvar,
    started: AtomicBool,
}

impl std::fmt::Debug for AnalysisService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnalysisService")
            .field("analyzers", &self.ids())
            .finish_non_exhaustive()
    }
}

const PRIORITY_LIMIT: usize = 500;

impl AnalysisService {
    pub fn new(
        catalog: Arc<dyn PhotoCatalog>,
        thumbnails: Arc<ThumbnailService>,
        pixels: Arc<dyn PixelLoader>,
        analyzers: Vec<Arc<dyn Analyzer>>,
        events: Arc<dyn EventSink>,
    ) -> Self {
        Self {
            catalog,
            thumbnails,
            pixels,
            analyzers,
            events,
            queue: Mutex::new(Queue {
                priority: VecDeque::new(),
                in_progress: Vec::new(),
                wake: true,
                busy: 0,
            }),
            signal: Condvar::new(),
            started: AtomicBool::new(false),
        }
    }

    /// The analyzers that can run now.
    fn active(&self) -> Vec<&Arc<dyn Analyzer>> {
        self.analyzers.iter().filter(|a| a.ready()).collect()
    }

    fn ids(&self) -> Vec<AnalyzerId> {
        self.active().iter().map(|a| a.id()).collect()
    }

    pub fn status(&self) -> AnalysisStatus {
        let (pending, total) = self
            .catalog
            .analysis_progress(&self.ids())
            .unwrap_or((0, 0));
        let running = self.queue.lock().expect("queue lock").busy > 0;
        AnalysisStatus {
            analyzed: total.saturating_sub(pending),
            total,
            running,
        }
    }

    /// Analyses done and to do for one photo.
    pub fn progress_of(&self, id: PhotoId) -> [usize; 2] {
        let ids = self.ids();
        let done = self.catalog.analyzed(id, &ids).map_or(0, |d| d.len());
        [done, ids.len()]
    }

    /// Analyzes these photos next (for example the ones on screen).
    pub fn prioritize(&self, ids: &[PhotoId]) {
        let mut q = self.queue.lock().expect("queue lock");
        for id in ids.iter().rev() {
            q.priority.retain(|p| p != id);
            q.priority.push_front(*id);
        }
        q.priority.truncate(PRIORITY_LIMIT);
        q.wake = true;
        self.signal.notify_all();
    }

    /// Tells workers there may be new photos (after an import).
    pub fn wake(&self) {
        self.queue.lock().expect("queue lock").wake = true;
        self.signal.notify_all();
    }

    /// Starts `workers` background threads (once).
    pub fn start(self: &Arc<Self>, workers: usize) {
        if self.analyzers.is_empty() || self.started.swap(true, Ordering::AcqRel) {
            return;
        }
        for _ in 0..workers.max(1) {
            let service = Arc::clone(self);
            std::thread::spawn(move || service.worker());
        }
    }

    /// Analyzes everything pending on the calling thread; returns how many
    /// photos were processed. For tests and tools.
    pub fn run_until_idle(&self) -> usize {
        let mut done = 0;
        while let Some(id) = self.next() {
            self.process(id);
            self.finish(id);
            done += 1;
        }
        if done > 0 {
            self.report();
        }
        done
    }

    fn worker(&self) {
        loop {
            match self.next() {
                Some(id) => {
                    self.process(id);
                    self.finish(id);
                    self.report();
                }
                None => {
                    let mut q = self.queue.lock().expect("queue lock");
                    while !q.wake {
                        q = self.signal.wait(q).expect("queue lock");
                    }
                }
            }
        }
    }

    /// The next photo to analyze, or `None` (clearing the wake flag) when
    /// nothing is pending.
    fn next(&self) -> Option<PhotoId> {
        let ids = self.ids();
        let mut q = self.queue.lock().expect("queue lock");
        while let Some(id) = q.priority.pop_front() {
            if q.in_progress.contains(&id) {
                continue;
            }
            // Skip photos already done, and ids that are not (or no longer) photos.
            let exists = matches!(self.catalog.photo(id), Ok(Some(_)));
            let done = self
                .catalog
                .analyzed(id, &ids)
                .map(|d| d.len() == ids.len())
                .unwrap_or(true);
            if exists && !done {
                q.in_progress.push(id);
                q.busy += 1;
                return Some(id);
            }
        }
        let skip = q.in_progress.len();
        let pending = self
            .catalog
            .pending_analysis(&ids, skip + 1)
            .unwrap_or_default();
        match pending.into_iter().find(|id| !q.in_progress.contains(id)) {
            Some(id) => {
                q.in_progress.push(id);
                q.busy += 1;
                Some(id)
            }
            None => {
                q.wake = false;
                None
            }
        }
    }

    fn finish(&self, id: PhotoId) {
        let mut q = self.queue.lock().expect("queue lock");
        q.in_progress.retain(|p| *p != id);
        q.busy -= 1;
    }

    /// Runs the analyzers a photo still needs. A failure is stored as an
    /// `{"error": ...}` result so the photo is not retried forever.
    fn process(&self, id: PhotoId) {
        let ids = self.ids();
        let Ok(Some(photo)) = self.catalog.photo(id) else {
            return;
        };
        let done = self.catalog.analyzed(id, &ids).unwrap_or_default();
        let needed: Vec<&Arc<dyn Analyzer>> = self
            .active()
            .into_iter()
            .filter(|a| !done.contains(&a.id()))
            .collect();
        if needed.is_empty() {
            return;
        }
        let image = self
            .thumbnails
            .image(id, ImageSize::Preview)
            .map_err(|e| e.to_string())
            .and_then(|path| self.pixels.load(&path, ANALYSIS_EDGE));
        for analyzer in needed {
            let result = match &image {
                Ok(image) => analyzer
                    .analyze(&AnalysisInput {
                        photo: &photo,
                        image,
                    })
                    .unwrap_or_else(|e| serde_json::json!({ "error": e })),
                Err(e) => serde_json::json!({ "error": e }),
            };
            let _ = self.catalog.store_analysis(id, analyzer.id(), &result);
        }
    }

    /// Sends the current status (also when analyzers became ready or not).
    pub fn report(&self) {
        self.events.emit(&AppEvent::AnalysisProgress(self.status()));
    }
}

#[cfg(test)]
mod tests;
