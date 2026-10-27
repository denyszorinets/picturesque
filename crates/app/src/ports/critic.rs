//! Asking a local vision-language model about a photo (ADR-0016).
//!
//! The adapter talks to a server with an OpenAI-compatible chat API
//! (Ollama, llama.cpp's `llama-server`, LM Studio) at an address from the
//! settings.

use std::fmt;
use std::path::Path;

/// One question about one image.
#[derive(Debug, Clone, Copy)]
pub struct CriticRequest<'a> {
    /// The API's base address, for example `http://127.0.0.1:11434/v1`.
    pub server: &'a str,
    pub model: &'a str,
    /// Instructions for the model.
    pub system: &'a str,
    pub prompt: &'a str,
    /// A cached preview JPEG; the adapter shrinks it to `max_edge`.
    pub image: &'a Path,
    pub max_edge: u32,
    /// JSON Schema the reply should follow.
    pub schema: &'a serde_json::Value,
}

/// Why the server gave no answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CriticError {
    /// Nothing answered at the address.
    Unreachable(String),
    /// The server answered with an error (for example an unknown model).
    Server(String),
    /// The image could not be prepared.
    Image(String),
}

impl fmt::Display for CriticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreachable(e) => write!(f, "no model server answered: {e}"),
            Self::Server(e) => write!(f, "the model server failed: {e}"),
            Self::Image(e) => write!(f, "cannot prepare the photo: {e}"),
        }
    }
}

pub trait CriticServer: Send + Sync {
    /// The models the server has.
    fn models(&self, server: &str) -> Result<Vec<String>, CriticError>;
    /// The model's reply text.
    fn ask(&self, request: &CriticRequest<'_>) -> Result<String, CriticError>;
}
