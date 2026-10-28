//! The background critic (`docs/features/ai.rst`): critiques every photo of
//! the library, slowly, one at a time, so critiques count in the aesthetic
//! score everywhere.
//!
//! Off by default (`critic.background`). It works through photos without a
//! current critique: photos on screen first, then the best of each series
//! and picks, photos in no series, and the rest of each series, newest
//! first within each; rejects are skipped. Photos critiqued by another
//! model or rubric keep their critique until it is redone, after every
//! photo without one. Everything it needs is in the catalog, so it resumes
//! where it was after a restart.
//!
//! How hard it works is a setting (`critic.intensity`): *gentle* runs the
//! model at low priority on half the processor and waits while photos are
//! imported or analyzed, and on battery power; *when idle* waits until the
//! app has not been used for a few minutes; *full* does neither. A critique
//! the user asks for interrupts it (`CritiqueService::critique`).

use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use picturesque_core::photo::PhotoId;

use super::{CritiqueError, CritiqueService};
use crate::analysis::AnalysisService;
use crate::pipeline::ImportPipeline;
use crate::ports::catalog::{CatalogError, PhotoCatalog};
use crate::ports::critic::CriticError;
use crate::ports::events::EventSink;
use crate::ports::power::PowerSource;
use crate::ports::settings::SettingsStore;
use crate::series::SeriesService;
use crate::settings::{CriticIntensity, CriticSettings};

mod plan;
mod status;
use plan::Plan;
pub use plan::throttle;
pub use status::{Activity, BackgroundCritiqueStatus};

/// How long the app must go unused before *when idle* starts.
pub const IDLE_AFTER: Duration = Duration::from_secs(3 * 60);
/// How long the worker waits before looking again when it cannot work.
const RECHECK: Duration = Duration::from_secs(30);
/// Critiques the time estimate averages over.
const RECENT: usize = 20;
/// How long it waits after a critique failed.
const AFTER_FAILURE: Duration = Duration::from_secs(2);
/// How long it waits after giving way to the user's critique.
const YIELD_FOR: Duration = Duration::from_secs(2);
/// Failures in a row after which it pauses.
const MAX_FAILURES: u32 = 3;

/// What one step of the worker did.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// Turned off.
    Off,
    /// Cannot work now, and why.
    Paused(String),
    /// Every photo has a current critique.
    Finished,
    Critiqued(PhotoId),
    /// Gave way to a critique the user asked for; tried again later.
    Yielded(PhotoId),
    /// The critique failed; the photo is skipped until settings change.
    Failed(PhotoId, String),
}

/// What the background critic needs from the rest of the app.
pub struct BackgroundDeps {
    pub critiques: Arc<CritiqueService>,
    pub catalog: Arc<dyn PhotoCatalog>,
    pub settings: Arc<dyn SettingsStore>,
    pub series: Arc<SeriesService>,
    pub analysis: Arc<AnalysisService>,
    pub pipeline: Arc<ImportPipeline>,
    pub power: Arc<dyn PowerSource>,
    pub activity: Arc<Activity>,
    pub events: Arc<dyn EventSink>,
}

impl std::fmt::Debug for BackgroundDeps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackgroundDeps").finish_non_exhaustive()
    }
}

#[derive(Default)]
struct State {
    /// Photos the UI shows, most recent first.
    priority: VecDeque<PhotoId>,
    /// Photos whose critique failed, with the settings they failed with.
    failed: HashSet<PhotoId>,
    failed_with: Option<CriticSettings>,
    /// Critiques that failed one after the other.
    failures: u32,
    /// Seconds of recent critiques, newest last.
    recent: VecDeque<f32>,
    running: bool,
    paused: Option<String>,
    wake: bool,
    stopped: bool,
}

/// Most photos remembered as on screen.
const PRIORITY_LIMIT: usize = 200;

pub struct BackgroundCritic {
    deps: BackgroundDeps,
    state: Mutex<State>,
    signal: Condvar,
    started: AtomicBool,
}

impl std::fmt::Debug for BackgroundCritic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackgroundCritic").finish_non_exhaustive()
    }
}

impl BackgroundCritic {
    pub fn new(deps: BackgroundDeps) -> Self {
        Self {
            deps,
            state: Mutex::new(State::default()),
            signal: Condvar::new(),
            started: AtomicBool::new(false),
        }
    }

    /// Critiques these photos (on screen) first.
    pub fn prioritize(&self, ids: &[PhotoId]) {
        let mut state = self.state.lock().expect("background lock");
        for id in ids.iter().rev() {
            state.priority.retain(|p| p != id);
            state.priority.push_front(*id);
        }
        state.priority.truncate(PRIORITY_LIMIT);
        drop(state);
        self.wake();
    }

    /// Looks for work again now (settings, decisions or models changed).
    pub fn wake(&self) {
        self.state.lock().expect("background lock").wake = true;
        self.signal.notify_all();
    }

    pub fn stop(&self) {
        self.state.lock().expect("background lock").stopped = true;
        self.signal.notify_all();
        // Do not wait for a critique to end.
        self.deps.critiques.interrupt_background();
    }

    fn critic_settings(&self) -> Result<CriticSettings, CatalogError> {
        Ok(self.deps.settings.load_settings()?.critic)
    }

    /// Why it cannot work now, or `None`.
    fn pause_reason(&self, settings: &CriticSettings) -> Result<Option<String>, CatalogError> {
        let status = self.deps.critiques.status()?;
        if let Some(problem) = status.problem {
            return Ok(Some(problem));
        }
        Ok(match settings.intensity {
            CriticIntensity::Gentle => {
                if self.deps.pipeline.status().queued > 0 {
                    Some("Waiting for the import to finish.".into())
                } else if self.deps.analysis.status().running {
                    Some("Waiting for analysis to finish.".into())
                } else if self.deps.power.on_battery() {
                    Some("Paused on battery power.".into())
                } else {
                    None
                }
            }
            CriticIntensity::WhenIdle => (self.deps.activity.idle_for() < IDLE_AFTER)
                .then(|| "Waits until Picturesque has not been used for a few minutes.".into()),
            CriticIntensity::Full => None,
        })
    }

    fn plan(&self, settings: &CriticSettings) -> Result<Plan, CatalogError> {
        Plan::load(&*self.deps.catalog, &self.deps.series, settings)
    }

    /// Does one thing: critiques the next photo, or says why it does not.
    pub fn step(&self) -> Result<Step, CatalogError> {
        let settings = self.critic_settings()?;
        {
            // Any change of settings gives failed photos another try.
            let mut state = self.state.lock().expect("background lock");
            if state.failed_with.as_ref() != Some(&settings) {
                state.failed.clear();
                state.failures = 0;
                state.failed_with = Some(settings.clone());
            }
        }
        if !settings.enabled || !settings.background {
            self.set_paused(None);
            return Ok(Step::Off);
        }
        if let Some(reason) = self.pause_reason(&settings)? {
            self.set_paused(Some(reason.clone()));
            return Ok(Step::Paused(reason));
        }
        let cleared = self.clear_paused();
        let plan = self.plan(&settings)?;
        let (next, from_priority) = {
            let mut state = self.state.lock().expect("background lock");
            let wanted: HashSet<PhotoId> = plan.queue.iter().copied().collect();
            let failed = state.failed.clone();
            state
                .priority
                .retain(|id| wanted.contains(id) && !failed.contains(id));
            match state.priority.pop_front() {
                Some(id) => (Some(id), true),
                None => (
                    plan.queue.iter().copied().find(|id| !failed.contains(id)),
                    false,
                ),
            }
        };
        let Some(id) = next else {
            if cleared {
                self.report();
            }
            return Ok(Step::Finished);
        };
        {
            let mut state = self.state.lock().expect("background lock");
            // `stop` came while it planned.
            if state.stopped {
                return Ok(Step::Off);
            }
            state.running = true;
        }
        self.report();
        let start = Instant::now();
        let result = self
            .deps
            .critiques
            .critique_in_background(id, throttle(settings.intensity));
        let step = {
            let mut state = self.state.lock().expect("background lock");
            state.running = false;
            match result {
                Ok(Some(_)) => {
                    state.failures = 0;
                    state.recent.push_back(start.elapsed().as_secs_f32());
                    if state.recent.len() > RECENT {
                        state.recent.pop_front();
                    }
                    Step::Critiqued(id)
                }
                Ok(None) => {
                    // Keep a photo on screen first in line.
                    if from_priority {
                        state.priority.push_front(id);
                    }
                    Step::Yielded(id)
                }
                Err(CritiqueError::Catalog(e)) => return Err(e),
                // The server went away: wait, the photo is fine.
                Err(e @ CritiqueError::Server(CriticError::Unreachable(_))) => {
                    let reason = e.to_string();
                    state.paused = Some(reason.clone());
                    Step::Paused(reason)
                }
                Err(e) => {
                    state.failed.insert(id);
                    state.failures += 1;
                    if state.failures >= MAX_FAILURES {
                        state.failures = 0;
                        let reason = format!("Several critiques in a row failed: {e}");
                        state.paused = Some(reason.clone());
                        Step::Paused(reason)
                    } else {
                        Step::Failed(id, e.to_string())
                    }
                }
            }
        };
        if matches!(step, Step::Critiqued(_)) {
            // The critique counts in the scores.
            self.deps.series.notify();
        }
        self.report();
        Ok(step)
    }

    /// Works in the background until stopped.
    pub fn start(self: &Arc<Self>) {
        if self.started.swap(true, Ordering::AcqRel) {
            return;
        }
        let worker = Arc::clone(self);
        std::thread::spawn(move || worker.run());
    }

    fn run(&self) {
        loop {
            let step = self.step();
            let wait = match step {
                Ok(Step::Critiqued(_)) => Duration::ZERO,
                Ok(Step::Failed(..)) => AFTER_FAILURE,
                // Let the user's critique take the critic first.
                Ok(Step::Yielded(_)) => YIELD_FOR,
                Ok(Step::Off | Step::Paused(_) | Step::Finished) | Err(_) => RECHECK,
            };
            let mut state = self.state.lock().expect("background lock");
            if state.stopped {
                return;
            }
            if !wait.is_zero() {
                let (s, _) = self
                    .signal
                    .wait_timeout_while(state, wait, |s| !s.wake && !s.stopped)
                    .expect("background lock");
                state = s;
                if state.stopped {
                    return;
                }
            }
            state.wake = false;
        }
    }
}

#[cfg(test)]
mod tests;
