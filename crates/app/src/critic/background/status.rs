//! The background critic's status: what the UI shows, and when the user
//! was last active.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use ts_rs::TS;

use super::BackgroundCritic;
use crate::ports::catalog::CatalogError;
use crate::ports::events::AppEvent;

/// Returned by `background_critique_status` and sent as
/// `background_critique_progress`.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct BackgroundCritiqueStatus {
    /// Whether the background critic is turned on (with critiques on).
    pub enabled: bool,
    /// Whether it is writing a critique now.
    pub running: bool,
    /// Why it waits, or `null`.
    pub paused: Option<String>,
    /// Photos (rejects excluded) critiqued with the current model and
    /// rubric.
    pub critiqued: usize,
    /// Photos critiqued with another model or rubric, to be redone.
    pub stale: usize,
    /// Photos without a critique.
    pub missing: usize,
    /// Photos it skips this session because their critique failed.
    pub failed: usize,
    /// Photos it covers: every photo but rejects.
    pub total: usize,
    /// Average time of recent background critiques.
    pub seconds_per_photo: Option<f32>,
    /// Estimated time to finish, from that average.
    #[ts(type = "number | null")]
    pub seconds_left: Option<u64>,
}

/// When the user last used the app (`report_activity`).
#[derive(Debug)]
pub struct Activity(Mutex<Instant>);

impl Default for Activity {
    fn default() -> Self {
        Self(Mutex::new(Instant::now()))
    }
}

impl Activity {
    pub fn touch(&self) {
        *self.0.lock().expect("activity lock") = Instant::now();
    }

    /// Pretends the last activity was at `at` (tests).
    #[cfg(test)]
    pub fn since(&self, at: Instant) {
        *self.0.lock().expect("activity lock") = at;
    }

    pub fn idle_for(&self) -> Duration {
        self.0.lock().expect("activity lock").elapsed()
    }
}

impl BackgroundCritic {
    /// Sets why it waits; reports it if that changed.
    pub(super) fn set_paused(&self, reason: Option<String>) {
        let changed = {
            let mut state = self.state.lock().expect("background lock");
            let changed = state.paused != reason;
            state.paused = reason;
            changed
        };
        if changed {
            self.report();
        }
    }

    /// Clears the reason it waited, without reporting; whether there was
    /// one.
    pub(super) fn clear_paused(&self) -> bool {
        self.state
            .lock()
            .expect("background lock")
            .paused
            .take()
            .is_some()
    }

    pub fn status(&self) -> Result<BackgroundCritiqueStatus, CatalogError> {
        let settings = self.critic_settings()?;
        let plan = self.plan(&settings)?;
        let state = self.state.lock().expect("background lock");
        let seconds_per_photo = (!state.recent.is_empty())
            .then(|| state.recent.iter().sum::<f32>() / state.recent.len() as f32);
        let left = plan.missing + plan.stale;
        Ok(BackgroundCritiqueStatus {
            enabled: settings.enabled && settings.background,
            running: state.running,
            paused: state.paused.clone(),
            critiqued: plan.critiqued,
            stale: plan.stale,
            missing: plan.missing,
            failed: state.failed.len(),
            total: plan.total,
            seconds_per_photo,
            seconds_left: seconds_per_photo.map(|s| (s * left as f32).round() as u64),
        })
    }

    /// Sends the status to the UI.
    pub fn report(&self) {
        if let Ok(status) = self.status() {
            self.deps
                .events
                .emit(&AppEvent::BackgroundCritiqueProgress(status));
        }
    }
}
