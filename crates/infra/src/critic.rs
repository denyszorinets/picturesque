//! [`CriticServer`] over an OpenAI-compatible chat API (ADR-0016): Ollama,
//! llama.cpp's `llama-server` or LM Studio.

use std::time::Duration;

use picturesque_app::ports::critic::{CriticError, CriticRequest, CriticServer};
use picturesque_core::base64;
use serde_json::{Value, json};

/// Listing models is quick; a server that does not answer soon is not
/// running.
const STATUS_TIMEOUT: Duration = Duration::from_secs(5);
/// A critique on a CPU can take minutes, more when the model loads first.
const ASK_TIMEOUT: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone)]
pub struct HttpCritic {
    quick: ureq::Agent,
    slow: ureq::Agent,
}

impl Default for HttpCritic {
    fn default() -> Self {
        Self::new()
    }
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(5)))
        .timeout_global(Some(timeout))
        // Error bodies say what went wrong (for example an unknown model).
        .http_status_as_error(false)
        .user_agent(concat!("picturesque/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

impl HttpCritic {
    pub fn new() -> Self {
        Self {
            quick: agent(STATUS_TIMEOUT),
            slow: agent(ASK_TIMEOUT),
        }
    }
}

fn endpoint(server: &str, path: &str) -> String {
    format!("{}/{path}", server.trim_end_matches('/'))
}

/// The JSON body of a response, or why there is none.
fn json_body(
    response: Result<ureq::http::Response<ureq::Body>, ureq::Error>,
) -> Result<Value, CriticError> {
    let mut response = response.map_err(|e| CriticError::Unreachable(e.to_string()))?;
    let status = response.status();
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|e| CriticError::Unreachable(e.to_string()))?;
    let body: Option<Value> = serde_json::from_str(&text).ok();
    if !status.is_success() {
        // OpenAI style `{"error": {"message": …}}`, or Ollama's `{"error": "…"}`.
        let message = body
            .as_ref()
            .and_then(|b| {
                b["error"]["message"]
                    .as_str()
                    .or_else(|| b["error"].as_str())
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| text.chars().take(200).collect());
        return Err(CriticError::Server(format!("{status}: {message}")));
    }
    body.ok_or_else(|| CriticError::Server("the reply is not JSON".into()))
}

impl CriticServer for HttpCritic {
    fn models(&self, server: &str) -> Result<Vec<String>, CriticError> {
        let body = json_body(self.quick.get(endpoint(server, "models")).call())?;
        let mut models: Vec<String> = body["data"]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|m| m["id"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        models.sort();
        Ok(models)
    }

    fn ask(&self, r: &CriticRequest<'_>) -> Result<String, CriticError> {
        let jpeg =
            crate::images::jpeg_for_upload(r.image, r.max_edge).map_err(CriticError::Image)?;
        let request = json!({
            "model": r.model,
            "stream": false,
            "temperature": 0.2,
            "messages": [
                { "role": "system", "content": r.system },
                { "role": "user", "content": [
                    { "type": "text", "text": r.prompt },
                    { "type": "image_url", "image_url": {
                        "url": format!("data:image/jpeg;base64,{}", base64::encode(&jpeg)),
                    } },
                ] },
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": { "name": "critique", "schema": r.schema },
            },
        });
        let body = json_body(
            self.slow
                .post(endpoint(r.server, "chat/completions"))
                .content_type("application/json")
                .send(request.to_string()),
        )?;
        body["choices"][0]["message"]["content"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| CriticError::Server("the reply has no message".into()))
    }
}
