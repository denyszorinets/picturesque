//! Local AI: which hardware models run on (`docs/features/ai.rst`).

use std::fmt;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use ts_rs::TS;

use crate::ports::catalog::CatalogError;
use crate::ports::inference::{InferenceEngine, Provider, ProviderChoice};
use crate::ports::settings::SettingsStore;

/// Returned by `ai_status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct AiStatus {
    /// Providers this build supports, preferred first (CPU last).
    pub available: Vec<Provider>,
    /// The user's choice (from settings).
    pub choice: ProviderChoice,
    /// The provider models run on, checked with a tiny test model; `null`
    /// when even CPU failed.
    pub provider: Option<Provider>,
    /// Why the chosen provider was not used, or why nothing works.
    pub problem: Option<String>,
}

/// Knows which provider models use, probing it once per choice.
pub struct AiService {
    engine: Arc<dyn InferenceEngine>,
    settings: Arc<dyn SettingsStore>,
    probed: Mutex<Option<AiStatus>>,
}

impl fmt::Debug for AiService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AiService").finish_non_exhaustive()
    }
}

impl AiService {
    pub fn new(engine: Arc<dyn InferenceEngine>, settings: Arc<dyn SettingsStore>) -> Self {
        Self {
            engine,
            settings,
            probed: Mutex::new(None),
        }
    }

    /// The current status; probes the provider when the choice changed
    /// since the last probe.
    pub fn status(&self) -> Result<AiStatus, CatalogError> {
        let choice = self.settings.load_settings()?.ai.provider;
        let mut probed = self.probed.lock().expect("ai status lock");
        if let Some(status) = probed.as_ref().filter(|s| s.choice == choice) {
            return Ok(status.clone());
        }
        let (provider, problem) = match self.engine.probe(choice) {
            Ok(probe) => (Some(probe.provider), probe.fallback),
            Err(e) => (None, Some(e.0)),
        };
        let status = AiStatus {
            available: self.engine.available(),
            choice,
            provider,
            problem,
        };
        *probed = Some(status.clone());
        Ok(status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::tests::MemorySettings;
    use crate::settings::{AiSettings, Settings};
    use crate::testing::FakeInference;

    #[test]
    fn probes_once_per_choice_and_reports_fallbacks() {
        let engine = Arc::new(FakeInference::new(&[Provider::WebGpu]));
        let settings = Arc::new(MemorySettings::default());
        let ai = AiService::new(engine.clone(), settings.clone());

        let auto = ai.status().unwrap();
        assert_eq!(auto.available, [Provider::WebGpu, Provider::Cpu]);
        assert_eq!(auto.provider, Some(Provider::WebGpu));
        assert_eq!(auto.problem, None);
        ai.status().unwrap();
        assert_eq!(engine.probes(), 1);

        *settings.0.lock().unwrap() = Some(Settings {
            ai: AiSettings {
                provider: ProviderChoice::DirectMl,
            },
            ..Default::default()
        });
        let directml = ai.status().unwrap();
        assert_eq!(directml.provider, Some(Provider::Cpu));
        assert!(directml.problem.unwrap().contains("DirectML"));
        assert_eq!(engine.probes(), 2);
    }

    #[test]
    fn a_broken_runtime_is_reported() {
        let engine = Arc::new(FakeInference::broken());
        let ai = AiService::new(engine, Arc::new(MemorySettings::default()));
        let status = ai.status().unwrap();
        assert_eq!(status.provider, None);
        assert!(status.problem.is_some());
    }
}
