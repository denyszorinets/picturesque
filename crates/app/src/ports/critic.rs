//! Asking a vision-language model about a photo (ADR-0016).
//!
//! Two backends: the built-in critic, a helper process running a
//! downloaded model with llama.cpp, or a server with an OpenAI-compatible
//! chat API (Ollama, llama.cpp's `llama-server`, LM Studio) at an address
//! from the settings.

use std::fmt;
use std::path::Path;

/// Which model answers.
#[derive(Debug, Clone, Copy)]
pub enum CriticBackend<'a> {
    /// The helper with the downloaded model files.
    Builtin {
        model: &'a Path,
        mmproj: &'a Path,
        /// Whether to use a GPU when one is found.
        gpu: bool,
    },
    /// A server; `url` is the API's base address, for example
    /// `http://127.0.0.1:11434/v1`.
    Server { url: &'a str, model: &'a str },
}

/// One question about one image.
#[derive(Debug, Clone, Copy)]
pub struct CriticRequest<'a> {
    pub backend: CriticBackend<'a>,
    /// Instructions for the model.
    pub system: &'a str,
    pub prompt: &'a str,
    /// A cached preview JPEG; the adapter shrinks it to `max_edge`.
    pub image: &'a Path,
    pub max_edge: u32,
    /// JSON Schema the reply should follow.
    pub schema: &'a serde_json::Value,
}

/// Why no answer came.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CriticError {
    /// Nothing answered at the address, or the helper could not start.
    Unreachable(String),
    /// The server or the helper answered with an error (for example an
    /// unknown model).
    Server(String),
    /// The image could not be prepared.
    Image(String),
}

impl fmt::Display for CriticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreachable(e) => write!(f, "the model did not answer: {e}"),
            Self::Server(e) => write!(f, "the model failed: {e}"),
            Self::Image(e) => write!(f, "cannot prepare the photo: {e}"),
        }
    }
}

pub trait Critic: Send + Sync {
    /// The models a server has.
    fn models(&self, server: &str) -> Result<Vec<String>, CriticError>;
    /// Why the built-in critic cannot run here (its helper is missing), or
    /// `None`.
    fn builtin_problem(&self) -> Option<String>;
    /// The model's reply text.
    fn ask(&self, request: &CriticRequest<'_>) -> Result<String, CriticError>;
}
