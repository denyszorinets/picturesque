//! Photo critiques from a local vision-language model (ADR-0016,
//! `docs/features/ai.rst`).
//!
//! On request, sends a photo's preview with a rubric to the built-in critic
//! (Qwen3-VL run by the `picturesque-critic` helper) or to the model server
//! from the settings, and keeps the reply (a rating, a summary, strengths
//! and weaknesses) with the photo's analysis results, together with the
//! model and a hash of the rubric: a critique by another model or rubric is
//! not shown. Critiques run one at a time.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use picturesque_core::photo::PhotoId;
use serde::Serialize;
use serde_json::json;
use ts_rs::TS;

use crate::models::{CRITIC, CRITIC_MMPROJ_FILE, CRITIC_MODEL_FILE, ModelService};
use crate::ports::catalog::{AnalyzerId, CatalogError, PhotoCatalog};
use crate::ports::critic::{Critic, CriticBackend, CriticError, CriticRequest, Throttle};
use crate::ports::inference::ProviderChoice;
use crate::ports::settings::SettingsStore;
use crate::settings::{CriticEngine, CriticSettings};
use crate::thumbnails::{ImageSize, ThumbnailError, ThumbnailService};

/// Ollama's OpenAI-compatible API on this computer.
pub const DEFAULT_SERVER: &str = "http://127.0.0.1:11434/v1";
pub const DEFAULT_MODEL: &str = "qwen3-vl:4b";
/// Longest edge of the image sent: enough for composition and light, and
/// few image tokens.
pub const MAX_EDGE: u32 = 768;
/// How critiques by the built-in critic are labelled and stored.
pub const BUILTIN_MODEL: &str = "Qwen3-VL 4B (built-in)";

mod status;
pub use status::{CriticStatus, SuggestedModel, suggested};

pub mod prompt;
#[cfg(test)]
use prompt::fingerprint;
pub use prompt::{Critique, DEFAULT_RUBRIC};
use prompt::{rubric_key, system_prompt};

/// Where critiques are stored among the analysis results.
pub const ID: AnalyzerId = AnalyzerId {
    name: "critique",
    version: 1,
};

/// A critique, as `get_critique` and `critique_photo` return it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct CritiqueView {
    /// The model that wrote it.
    pub model: String,
    /// 1–10.
    pub rating: f32,
    /// The rating as 0–1.
    pub score: f32,
    pub summary: String,
    pub strengths: Vec<String>,
    pub weaknesses: Vec<String>,
}

impl CritiqueView {
    fn new(model: &str, c: Critique) -> Self {
        Self {
            model: model.to_owned(),
            score: (c.rating - 1.0) / 9.0,
            rating: c.rating,
            summary: c.summary,
            strengths: c.strengths,
            weaknesses: c.weaknesses,
        }
    }
}

/// Why a critique could not be made.
#[derive(Debug)]
pub enum CritiqueError {
    Disabled,
    /// The built-in critic's model is not downloaded.
    NotDownloaded,
    NotFound(PhotoId),
    Image(String),
    Server(CriticError),
    /// The model's reply was not a critique.
    BadReply(String),
    Catalog(CatalogError),
}

impl std::fmt::Display for CritiqueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disabled => {
                f.write_str("Critiques are off. Turn them on in Settings → AI critic.")
            }
            Self::NotDownloaded => f.write_str(
                "The built-in critic's model is not downloaded. \
                 Download it in Settings → AI models.",
            ),
            Self::NotFound(id) => write!(f, "There is no photo {}.", id.0),
            Self::Image(e) => write!(f, "Cannot prepare the photo: {e}"),
            Self::Server(e) => write!(f, "Cannot critique the photo: {e}."),
            Self::BadReply(reply) => write!(
                f,
                "The model did not answer with a critique. Try another model. It said: {reply}"
            ),
            Self::Catalog(e) => write!(f, "{e}"),
        }
    }
}

impl From<CatalogError> for CritiqueError {
    fn from(e: CatalogError) -> Self {
        Self::Catalog(e)
    }
}

/// Whether `server` (a URL) points at this computer.
pub fn is_local(server: &str) -> bool {
    let rest = server
        .strip_prefix("http://")
        .or_else(|| server.strip_prefix("https://"))
        .unwrap_or(server);
    let authority = rest.split('/').next().unwrap_or_default();
    let authority = authority.rsplit('@').next().unwrap_or_default();
    let host = if let Some(v6) = authority.strip_prefix('[') {
        v6.split(']').next().unwrap_or_default()
    } else {
        authority.split(':').next().unwrap_or_default()
    };
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

pub struct CritiqueService {
    catalog: Arc<dyn PhotoCatalog>,
    settings: Arc<dyn SettingsStore>,
    thumbnails: Arc<ThumbnailService>,
    models: Arc<ModelService>,
    critic: Arc<dyn Critic>,
    /// Held while a critique runs.
    busy: Mutex<()>,
    /// Critiques the user asked for that are waiting or running.
    interactive: AtomicUsize,
    /// Whether a background critique is running.
    background_running: AtomicBool,
}

/// Counts a user's critique as waiting while it lives.
struct Waiting<'a>(&'a AtomicUsize);

impl<'a> Waiting<'a> {
    fn new(count: &'a AtomicUsize) -> Self {
        count.fetch_add(1, Ordering::AcqRel);
        Self(count)
    }
}

impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

impl std::fmt::Debug for CritiqueService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CritiqueService").finish_non_exhaustive()
    }
}

impl CritiqueService {
    pub fn new(
        catalog: Arc<dyn PhotoCatalog>,
        settings: Arc<dyn SettingsStore>,
        thumbnails: Arc<ThumbnailService>,
        models: Arc<ModelService>,
        critic: Arc<dyn Critic>,
    ) -> Self {
        Self {
            catalog,
            settings,
            thumbnails,
            models,
            critic,
            busy: Mutex::new(()),
            interactive: AtomicUsize::new(0),
            background_running: AtomicBool::new(false),
        }
    }

    /// Pretends a background critique is running (tests).
    #[cfg(test)]
    pub(crate) fn mark_background_running(&self) {
        self.background_running.store(true, Ordering::Release);
    }

    fn settings(&self) -> Result<CriticSettings, CatalogError> {
        Ok(self.settings.load_settings()?.critic)
    }

    /// The model critiques are written by, as shown and stored.
    pub(crate) fn model_name(settings: &CriticSettings) -> &str {
        match settings.backend {
            CriticEngine::Builtin => BUILTIN_MODEL,
            CriticEngine::Server => &settings.model,
        }
    }

    /// The stored critique by the current model and rubric, if any.
    pub fn cached(&self, id: PhotoId) -> Result<Option<CritiqueView>, CritiqueError> {
        let settings = self.settings()?;
        let stored = self
            .catalog
            .analysis(id)?
            .into_iter()
            .find(|(name, version, _)| *name == ID.name && *version == ID.version)
            .map(|(_, _, result)| result);
        let Some(stored) = stored else {
            return Ok(None);
        };
        let model = Self::model_name(&settings);
        let matches =
            stored["model"] == model && stored["rubric"] == rubric_key(settings.rubric()).as_str();
        if !matches {
            return Ok(None);
        }
        Ok(
            serde_json::from_value::<Critique>(stored["critique"].clone())
                .ok()
                .map(|c| CritiqueView::new(model, c)),
        )
    }

    /// Critiques a photo, or returns the stored critique unless `refresh`.
    /// Takes precedence over the background critic, interrupting it.
    pub fn critique(&self, id: PhotoId, refresh: bool) -> Result<CritiqueView, CritiqueError> {
        let settings = self.settings()?;
        if !settings.enabled {
            return Err(CritiqueError::Disabled);
        }
        if !refresh && let Some(view) = self.cached(id)? {
            return Ok(view);
        }
        let _waiting = Waiting::new(&self.interactive);
        self.interrupt_background();
        let _busy = self.busy.lock().expect("critic lock");
        self.run(id, &settings, None)
    }

    /// Ends the background critique that is running, if any (shutdown).
    pub fn interrupt_background(&self) {
        if self.background_running.load(Ordering::Acquire) {
            self.critic.interrupt();
        }
    }

    /// Critiques a photo for the background critic, unless the user is
    /// waiting for a critique; `Ok(None)` when it gave way (or was
    /// interrupted) and the photo should be tried again later.
    pub fn critique_in_background(
        &self,
        id: PhotoId,
        throttle: Throttle,
    ) -> Result<Option<CritiqueView>, CritiqueError> {
        let settings = self.settings()?;
        if !settings.enabled {
            return Err(CritiqueError::Disabled);
        }
        let _busy = self.busy.lock().expect("critic lock");
        self.background_running.store(true, Ordering::Release);
        let result = if self.interactive.load(Ordering::Acquire) > 0 {
            Ok(None)
        } else {
            match self.run(id, &settings, Some(throttle)) {
                Ok(view) => Ok(Some(view)),
                Err(CritiqueError::Server(CriticError::Interrupted)) => Ok(None),
                Err(e) => Err(e),
            }
        };
        self.background_running.store(false, Ordering::Release);
        result
    }

    fn run(
        &self,
        id: PhotoId,
        settings: &CriticSettings,
        background: Option<Throttle>,
    ) -> Result<CritiqueView, CritiqueError> {
        let view = self.thumbnails.pair_view().map_err(thumbnail_error(id))?;
        let image = self
            .thumbnails
            .image(id, ImageSize::Preview, view)
            .map_err(thumbnail_error(id))?;
        let rubric = settings.rubric();
        let files = match settings.backend {
            CriticEngine::Builtin => Some(
                self.models
                    .files(CRITIC)
                    .ok_or(CritiqueError::NotDownloaded)?,
            ),
            CriticEngine::Server => None,
        };
        let gpu = self.settings.load_settings()?.ai.provider != ProviderChoice::Cpu;
        let backend = match &files {
            Some(files) => CriticBackend::Builtin {
                model: &files[CRITIC_MODEL_FILE],
                mmproj: &files[CRITIC_MMPROJ_FILE],
                gpu,
            },
            None => CriticBackend::Server {
                url: &settings.server,
                model: &settings.model,
            },
        };
        let reply = self
            .critic
            .ask(&CriticRequest {
                backend,
                background,
                system: &system_prompt(rubric),
                prompt: "Critique this photo.",
                image: &image,
                max_edge: MAX_EDGE,
                schema: &Critique::schema(),
            })
            .map_err(CritiqueError::Server)?;
        let critique = Critique::parse(&reply)
            .ok_or_else(|| CritiqueError::BadReply(reply.chars().take(200).collect::<String>()))?;
        let model = Self::model_name(settings);
        self.catalog.store_analysis(
            id,
            ID,
            &json!({
                "model": model,
                "rubric": rubric_key(rubric),
                "critique": critique,
            }),
        )?;
        Ok(CritiqueView::new(model, critique))
    }
}

fn thumbnail_error(id: PhotoId) -> impl Fn(ThumbnailError) -> CritiqueError {
    move |e| match e {
        ThumbnailError::NotFound(_) => CritiqueError::NotFound(id),
        other => CritiqueError::Image(other.to_string()),
    }
}

pub mod background;

#[cfg(test)]
mod tests;
