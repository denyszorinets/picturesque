//! Messages between the app and its critic helper, `picturesque-critic`
//! (ADR-0016): one JSON object per line on the helper's stdin, one reply
//! per line on its stdout.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Asks the helper to critique one image.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HelperRequest {
    /// The language model (GGUF).
    pub model: PathBuf,
    /// Its vision projector (GGUF).
    pub mmproj: PathBuf,
    /// Whether to use a GPU when one is found.
    pub gpu: bool,
    pub system: String,
    pub prompt: String,
    /// The image, a JPEG in base64.
    pub image: String,
    /// JSON Schema the reply must follow, as JSON text.
    pub schema: String,
    /// Most tokens to write.
    pub max_tokens: u32,
    /// Most processor threads; 0 for all.
    #[serde(default)]
    pub threads: u32,
}

/// The helper's answer to one request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum HelperReply {
    Done {
        /// The model's reply.
        text: String,
        /// Where it ran, for example `Vulkan0` or `CPU`.
        device: String,
        /// Time taken, including loading the model if it was not loaded.
        seconds: f32,
    },
    Failed {
        message: String,
    },
}
