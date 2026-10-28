//! What `critic_status` reports: whether critiques can run, and models to
//! suggest.

use serde::Serialize;
use ts_rs::TS;

use super::{CritiqueService, DEFAULT_RUBRIC, is_local};
use crate::models::CRITIC;
use crate::ports::catalog::CatalogError;
use crate::settings::CriticEngine;

/// A model the app suggests: one whose license was checked (ADR-0016).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct SuggestedModel {
    /// The name to pull with Ollama.
    pub name: String,
    /// Approximate download size in bytes.
    #[ts(type = "number")]
    pub size: u64,
    pub license: String,
    pub license_url: String,
    pub note: String,
}

/// Models offered in Settings, smallest first.
pub fn suggested() -> Vec<SuggestedModel> {
    let qwen = |name: &str, size: u64, note: &str| SuggestedModel {
        name: name.into(),
        size,
        license: "Apache-2.0".into(),
        license_url: "https://huggingface.co/Qwen/Qwen3-VL-4B-Instruct".into(),
        note: note.into(),
    };
    vec![
        qwen(
            "qwen3-vl:2b",
            1_900_000_000,
            "Fastest; fine on a laptop CPU, shallower critiques.",
        ),
        qwen(
            "qwen3-vl:4b",
            3_300_000_000,
            "Recommended: good critiques, about 8 GB of memory.",
        ),
        qwen(
            "qwen3-vl:8b",
            6_200_000_000,
            "Best critiques; wants a GPU or 16 GB of memory.",
        ),
    ]
}

/// Returned by `critic_status`.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct CriticStatus {
    pub enabled: bool,
    pub backend: CriticEngine,
    pub server: String,
    /// Whether the server is on this computer; otherwise photos are sent
    /// to another one.
    pub local: bool,
    /// Whether the server answered, or the built-in critic's helper is
    /// installed (checked only while enabled).
    pub reachable: bool,
    /// The server's models.
    pub models: Vec<String>,
    /// The model in use.
    pub model: String,
    /// Whether the server has the model, or the built-in model is
    /// downloaded.
    pub model_installed: bool,
    /// What to do to make critiques work, if anything.
    pub problem: Option<String>,
    pub suggested: Vec<SuggestedModel>,
    /// The rubric used when the user has not written one.
    pub default_rubric: String,
}

impl CritiqueService {
    /// Whether critiques can run; asks the server only while enabled.
    pub fn status(&self) -> Result<CriticStatus, CatalogError> {
        let settings = self.settings()?;
        let mut status = CriticStatus {
            enabled: settings.enabled,
            backend: settings.backend,
            local: settings.backend == CriticEngine::Builtin || is_local(&settings.server),
            server: settings.server.clone(),
            reachable: false,
            models: Vec::new(),
            model: Self::model_name(&settings).to_owned(),
            model_installed: false,
            problem: None,
            suggested: suggested(),
            default_rubric: DEFAULT_RUBRIC.into(),
        };
        if !settings.enabled {
            return Ok(status);
        }
        if settings.backend == CriticEngine::Builtin {
            status.reachable = self.critic.builtin_problem().is_none();
            status.model_installed = self.models.files(CRITIC).is_some();
            status.problem = self.critic.builtin_problem().or_else(|| {
                (!status.model_installed).then(|| {
                    "Download “Qwen3-VL 4B (built-in critic)” in Settings → AI models \
                     (2.9 GB)."
                        .to_owned()
                })
            });
            return Ok(status);
        }
        match self.critic.models(&settings.server) {
            Ok(models) => {
                status.reachable = true;
                // Ollama lists `name:tag`; `name` alone means `:latest`.
                status.model_installed = models
                    .iter()
                    .any(|m| *m == settings.model || *m == format!("{}:latest", settings.model));
                if !status.model_installed {
                    status.problem = Some(format!(
                        "The server has no model `{}`. With Ollama, run `ollama pull {}`.",
                        settings.model, settings.model
                    ));
                }
                status.models = models;
            }
            Err(e) => {
                status.problem = Some(format!(
                    "{e}. Install Ollama (ollama.com) and start it, or check the address."
                ));
            }
        }
        Ok(status)
    }
}
