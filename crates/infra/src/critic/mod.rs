//! [`Critic`] adapters (ADR-0016): the built-in critic's helper process and
//! servers with an OpenAI-compatible chat API.

mod helper;
mod http;

pub use helper::{HelperCritic, IDLE, locate as locate_helper};
pub use http::HttpCritic;

use picturesque_app::ports::critic::{Critic, CriticBackend, CriticError, CriticRequest};
use picturesque_core::base64;
use picturesque_core::critic_protocol::HelperRequest;

/// Sends each request to the backend it names.
#[derive(Debug, Clone, Default)]
pub struct CriticAdapter {
    pub http: HttpCritic,
    pub helper: HelperCritic,
}

impl CriticAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Critic for CriticAdapter {
    fn models(&self, server: &str) -> Result<Vec<String>, CriticError> {
        self.http.models(server)
    }

    fn builtin_problem(&self) -> Option<String> {
        self.helper.problem()
    }

    fn ask(&self, r: &CriticRequest<'_>) -> Result<String, CriticError> {
        match r.backend {
            CriticBackend::Server { url, model } => self.http.ask(url, model, r),
            CriticBackend::Builtin { model, mmproj, gpu } => {
                let jpeg = crate::images::jpeg_for_upload(r.image, r.max_edge)
                    .map_err(CriticError::Image)?;
                self.helper.ask(HelperRequest {
                    model: model.to_path_buf(),
                    mmproj: mmproj.to_path_buf(),
                    gpu,
                    system: r.system.to_owned(),
                    prompt: r.prompt.to_owned(),
                    image: base64::encode(&jpeg),
                    schema: r.schema.to_string(),
                    max_tokens: 0,
                })
            }
        }
    }
}
