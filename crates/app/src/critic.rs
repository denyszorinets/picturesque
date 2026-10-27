//! Photo critiques from a local vision-language model (ADR-0016,
//! `docs/features/ai.rst`).
//!
//! On request, sends a photo's preview with a rubric to the model server
//! from the settings and keeps the reply (a rating, a summary, strengths
//! and weaknesses) with the photo's analysis results, together with the
//! model and a hash of the rubric: a critique by another model or rubric is
//! not shown. Critiques run one at a time.

use std::sync::{Arc, Mutex};

use picturesque_core::photo::PhotoId;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use ts_rs::TS;

use crate::ports::catalog::{AnalyzerId, CatalogError, PhotoCatalog};
use crate::ports::critic::{CriticError, CriticRequest, CriticServer};
use crate::ports::settings::SettingsStore;
use crate::settings::CriticSettings;
use crate::thumbnails::{ImageSize, ThumbnailError, ThumbnailService};

/// Ollama's OpenAI-compatible API on this computer.
pub const DEFAULT_SERVER: &str = "http://127.0.0.1:11434/v1";
pub const DEFAULT_MODEL: &str = "qwen3-vl:4b";
/// Longest edge of the image sent: enough for composition and light, and
/// few image tokens.
pub const MAX_EDGE: u32 = 768;

/// What the model judges, unless the user wrote their own.
pub const DEFAULT_RUBRIC: &str = "\
- Composition: framing, balance, leading lines, distracting elements at the edges.
- Light: direction, quality and contrast of the light; exposure.
- Subject and moment: is the subject clear, is the moment or expression right?
- Colour and tone: harmony, white balance, saturation.
- Technique: focus on the subject, motion blur, noise.";

/// Where critiques are stored among the analysis results.
pub const ID: AnalyzerId = AnalyzerId {
    name: "critique",
    version: 1,
};

/// The JSON the model is asked for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Critique {
    /// 1–10.
    pub rating: f32,
    pub summary: String,
    #[serde(default)]
    pub strengths: Vec<String>,
    #[serde(default)]
    pub weaknesses: Vec<String>,
}

/// Most strengths or weaknesses kept.
const MAX_POINTS: usize = 5;

impl Critique {
    /// The first JSON object in a model's reply, tidied; `None` when there
    /// is none or it lacks a rating or summary.
    pub fn parse(reply: &str) -> Option<Self> {
        // Reasoning models think aloud first.
        let reply = reply.rsplit("</think>").next().unwrap_or(reply);
        let start = reply.find('{')?;
        let end = reply.rfind('}')?;
        let mut c: Self = serde_json::from_str(reply.get(start..=end)?).ok()?;
        if !c.rating.is_finite() {
            return None;
        }
        c.rating = c.rating.clamp(1.0, 10.0);
        c.summary = c.summary.trim().to_owned();
        let tidy = |points: Vec<String>| -> Vec<String> {
            points
                .into_iter()
                .map(|p| p.trim().to_owned())
                .filter(|p| !p.is_empty())
                .take(MAX_POINTS)
                .collect()
        };
        c.strengths = tidy(c.strengths);
        c.weaknesses = tidy(c.weaknesses);
        (!c.summary.is_empty()).then_some(c)
    }

    fn schema() -> Value {
        let points = json!({ "type": "array", "items": { "type": "string" }, "maxItems": 3 });
        json!({
            "type": "object",
            "properties": {
                "rating": { "type": "number", "minimum": 1, "maximum": 10 },
                "summary": { "type": "string" },
                "strengths": points,
                "weaknesses": points,
            },
            "required": ["rating", "summary", "strengths", "weaknesses"],
        })
    }
}

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
    pub server: String,
    /// Whether the server is on this computer; otherwise photos are sent
    /// to another one.
    pub local: bool,
    /// Whether the server answered (checked only while enabled).
    pub reachable: bool,
    /// The server's models.
    pub models: Vec<String>,
    /// The chosen model.
    pub model: String,
    pub model_installed: bool,
    /// What to do to make critiques work, if anything.
    pub problem: Option<String>,
    pub suggested: Vec<SuggestedModel>,
    /// The rubric used when the user has not written one.
    pub default_rubric: String,
}

/// Why a critique could not be made.
#[derive(Debug)]
pub enum CritiqueError {
    Disabled,
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

/// 64-bit FNV-1a, stable across builds, to recognize the rubric a critique
/// was written with.
fn fingerprint(text: &str) -> String {
    let hash = text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    });
    format!("{hash:016x}")
}

fn system_prompt(rubric: &str) -> String {
    format!(
        "You are an experienced photo editor helping an amateur photographer decide \
         which photos to keep and learn from them. Judge the photo against this rubric:\n\
         {rubric}\n\n\
         Reply with JSON only: {{\"rating\": a number from 1 to 10, \"summary\": one \
         sentence, \"strengths\": up to 3 short phrases, \"weaknesses\": up to 3 short \
         phrases}}. Rate 5 for an ordinary snapshot, 7 for a good photo, and 9 or 10 only \
         for outstanding ones. Be specific about this photo, kind but honest."
    )
}

pub struct CritiqueService {
    catalog: Arc<dyn PhotoCatalog>,
    settings: Arc<dyn SettingsStore>,
    thumbnails: Arc<ThumbnailService>,
    server: Arc<dyn CriticServer>,
    /// Held while a critique runs.
    busy: Mutex<()>,
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
        server: Arc<dyn CriticServer>,
    ) -> Self {
        Self {
            catalog,
            settings,
            thumbnails,
            server,
            busy: Mutex::new(()),
        }
    }

    fn settings(&self) -> Result<CriticSettings, CatalogError> {
        Ok(self.settings.load_settings()?.critic)
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
        let matches = stored["model"] == settings.model.as_str()
            && stored["rubric"] == fingerprint(settings.rubric()).as_str();
        if !matches {
            return Ok(None);
        }
        Ok(
            serde_json::from_value::<Critique>(stored["critique"].clone())
                .ok()
                .map(|c| CritiqueView::new(&settings.model, c)),
        )
    }

    /// Critiques a photo, or returns the stored critique unless `refresh`.
    pub fn critique(&self, id: PhotoId, refresh: bool) -> Result<CritiqueView, CritiqueError> {
        let settings = self.settings()?;
        if !settings.enabled {
            return Err(CritiqueError::Disabled);
        }
        if !refresh && let Some(view) = self.cached(id)? {
            return Ok(view);
        }
        let _busy = self.busy.lock().expect("critic lock");
        let view = self.thumbnails.pair_view().map_err(thumbnail_error(id))?;
        let image = self
            .thumbnails
            .image(id, ImageSize::Preview, view)
            .map_err(thumbnail_error(id))?;
        let rubric = settings.rubric();
        let reply = self
            .server
            .ask(&CriticRequest {
                server: &settings.server,
                model: &settings.model,
                system: &system_prompt(rubric),
                prompt: "Critique this photo.",
                image: &image,
                max_edge: MAX_EDGE,
                schema: &Critique::schema(),
            })
            .map_err(CritiqueError::Server)?;
        let critique = Critique::parse(&reply)
            .ok_or_else(|| CritiqueError::BadReply(reply.chars().take(200).collect::<String>()))?;
        self.catalog.store_analysis(
            id,
            ID,
            &json!({
                "model": settings.model,
                "rubric": fingerprint(rubric),
                "critique": critique,
            }),
        )?;
        Ok(CritiqueView::new(&settings.model, critique))
    }

    /// Whether critiques can run; asks the server only while enabled.
    pub fn status(&self) -> Result<CriticStatus, CatalogError> {
        let settings = self.settings()?;
        let mut status = CriticStatus {
            enabled: settings.enabled,
            local: is_local(&settings.server),
            server: settings.server.clone(),
            reachable: false,
            models: Vec::new(),
            model: settings.model.clone(),
            model_installed: false,
            problem: None,
            suggested: suggested(),
            default_rubric: DEFAULT_RUBRIC.into(),
        };
        if !settings.enabled {
            return Ok(status);
        }
        match self.server.models(&settings.server) {
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

fn thumbnail_error(id: PhotoId) -> impl Fn(ThumbnailError) -> CritiqueError {
    move |e| match e {
        ThumbnailError::NotFound(_) => CritiqueError::NotFound(id),
        other => CritiqueError::Image(other.to_string()),
    }
}

#[cfg(test)]
mod tests;
