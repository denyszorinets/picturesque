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

use crate::ports::catalog::{AnalyzerId, CatalogError, PhotoCatalog};
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

    /// What the analyzer adds, to tell users what waits for a model.
    fn purpose(&self) -> &'static str {
        self.id().name
    }
}

/// Analysis progress, sent as an event and returned by `analysis_status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct AnalysisStatus {
    /// Photos with every analyzer done.
    pub analyzed: usize,
    pub total: usize,
    /// Whether workers are busy.
    pub running: bool,
    /// Why analysis waits (a disk is full), or `null`.
    pub paused: Option<String>,
    /// What is not analyzed until its model is downloaded (for example
    /// "faces and closed eyes").
    pub waiting_for_models: Vec<String>,
}

/// Arguments of `prioritize_analysis`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct PrioritizeArgs {
    #[ts(type = "number[]")]
    pub photo_ids: Vec<i64>,
}

/// Arguments of `reanalyze_photos`.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(default, deny_unknown_fields)]
#[ts(export)]
pub struct ReanalyzeArgs {
    /// The photos to analyze again; every photo when absent.
    #[ts(optional, type = "number[]")]
    pub photo_ids: Option<Vec<i64>>,
}

/// Result of `reanalyze_photos`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ReanalyzeResult {
    /// Photos that will be analyzed again.
    pub photos: usize,
    /// What is not analyzed until its model is downloaded.
    pub waiting_for_models: Vec<String>,
}

struct Queue {
    /// Photos the UI asked for, most recent first.
    priority: VecDeque<PhotoId>,
    /// Photos being analyzed now.
    in_progress: Vec<PhotoId>,
    /// Set when there may be new work.
    wake: bool,
    busy: usize,
    /// Why analysis waits; it tries again after `PAUSE_RETRY`.
    paused: Option<String>,
    /// Set by [`AnalysisService::stop`]: workers exit.
    stopped: bool,
}

const CACHE_FULL: &str =
    "the disk holding the thumbnail cache is full; analysis continues when space is freed";
const CATALOG_FULL: &str =
    "the disk holding the catalog is full; analysis continues when space is freed";

/// While a disk is full, how often analysis tries again.
const PAUSE_RETRY: std::time::Duration = std::time::Duration::from_secs(30);

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
                paused: None,
                stopped: false,
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

    /// What the analyzers that wait for their model add.
    pub fn waiting_for_models(&self) -> Vec<String> {
        self.analyzers
            .iter()
            .filter(|a| !a.ready())
            .map(|a| a.purpose().to_owned())
            .collect()
    }

    pub fn status(&self) -> AnalysisStatus {
        let (pending, total) = self
            .catalog
            .analysis_progress(&self.ids())
            .unwrap_or((0, 0));
        let q = self.queue.lock().expect("queue lock");
        AnalysisStatus {
            analyzed: total.saturating_sub(pending),
            total,
            running: q.busy > 0,
            paused: q.paused.clone(),
            waiting_for_models: self.waiting_for_models(),
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

    /// Analyzes these photos (every photo with `None`) again, the given
    /// ones first. Their earlier results stay in use until replaced.
    /// Returns how many photos will be analyzed.
    pub fn reanalyze(&self, ids: Option<&[PhotoId]>) -> Result<usize, CatalogError> {
        let photos = self.catalog.mark_analysis_stale(ids)?;
        match ids {
            Some(ids) => self.prioritize(ids),
            None => self.wake(),
        }
        self.report();
        Ok(photos)
    }

    /// Tells workers there may be new photos (after an import).
    pub fn wake(&self) {
        self.queue.lock().expect("queue lock").wake = true;
        self.signal.notify_all();
    }

    /// Stops the background workers after their current photo.
    pub fn stop(&self) {
        self.queue.lock().expect("queue lock").stopped = true;
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
            let result = self.process(id);
            self.finish(id);
            if let Err(reason) = result {
                self.queue.lock().expect("queue lock").paused = Some(reason);
                break;
            }
            self.queue.lock().expect("queue lock").paused = None;
            done += 1;
        }
        if done > 0 {
            self.report();
        }
        done
    }

    fn worker(&self) {
        loop {
            if self.queue.lock().expect("queue lock").stopped {
                return;
            }
            match self.next() {
                Some(id) => {
                    let result = self.process(id);
                    self.finish(id);
                    match result {
                        Ok(()) => {
                            let resumed = self
                                .queue
                                .lock()
                                .expect("queue lock")
                                .paused
                                .take()
                                .is_some();
                            if resumed {
                                self.report();
                            }
                        }
                        Err(reason) => {
                            self.queue.lock().expect("queue lock").paused = Some(reason);
                            self.report();
                            // Not the photo's fault: wait, then try again.
                            let deadline = std::time::Instant::now() + PAUSE_RETRY;
                            let mut q = self.queue.lock().expect("queue lock");
                            while let Some(left) =
                                deadline.checked_duration_since(std::time::Instant::now())
                                && !q.stopped
                            {
                                q = self.signal.wait_timeout(q, left).expect("queue lock").0;
                            }
                            continue;
                        }
                    }
                    self.report();
                }
                None => {
                    let mut q = self.queue.lock().expect("queue lock");
                    while !q.wake && !q.stopped {
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
    /// `{"error": ...}` result so the photo is not retried forever; a full
    /// disk is not the photo's fault, so it stores nothing and returns why
    /// analysis must wait.
    fn process(&self, id: PhotoId) -> Result<(), String> {
        let ids = self.ids();
        let Ok(Some(photo)) = self.catalog.photo(id) else {
            return Ok(());
        };
        let done = self.catalog.analyzed(id, &ids).unwrap_or_default();
        let needed: Vec<&Arc<dyn Analyzer>> = self
            .active()
            .into_iter()
            .filter(|a| !done.contains(&a.id()))
            .collect();
        if needed.is_empty() {
            return Ok(());
        }
        let image = self
            .thumbnails
            .image(id, ImageSize::Preview)
            .map_err(|e| e.to_string())
            .and_then(|path| self.pixels.load(&path, ANALYSIS_EDGE));
        if image.is_err() && self.thumbnails.cache_full() {
            return Err(CACHE_FULL.into());
        }
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
            if let Err(CatalogError::Full) = self.catalog.store_analysis(id, analyzer.id(), &result)
            {
                return Err(CATALOG_FULL.into());
            }
        }
        Ok(())
    }

    /// Sends the current status (also when analyzers became ready or not).
    pub fn report(&self) {
        self.events.emit(&AppEvent::AnalysisProgress(self.status()));
    }
}

#[cfg(test)]
mod tests;
