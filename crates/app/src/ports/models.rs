//! Where downloaded AI models are kept (`docs/features/ai.rst`).

use std::fmt;
use std::path::PathBuf;

use serde::Deserialize;

/// A model the app can download.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelSpec {
    /// Stable identifier, also the folder name.
    pub id: String,
    pub name: String,
    /// What the app uses it for.
    pub purpose: String,
    /// Bumped when the files change; results computed with an older
    /// version are recomputed.
    pub version: u32,
    /// SPDX identifier of the model's license.
    pub license: String,
    pub license_url: String,
    /// Where the model comes from.
    pub homepage: String,
    pub files: Vec<ModelFile>,
}

impl ModelSpec {
    pub fn size(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }
}

/// One file of a model.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelFile {
    /// Relative path inside the model's folder, for example
    /// `visual/model.onnx`.
    pub name: String,
    pub url: String,
    /// Expected SHA-256, lowercase hex.
    pub sha256: String,
    /// Expected size in bytes.
    pub size: u64,
}

/// Why a file could not be downloaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    /// The server could not be reached or answered with an error.
    Network(String),
    /// The download did not match the expected size or checksum; nothing
    /// was kept.
    Corrupt(String),
    /// Writing the file failed.
    Io(String),
    /// The progress callback asked to stop.
    Cancelled,
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(e) => write!(f, "download failed: {e}"),
            Self::Corrupt(e) => write!(f, "download was corrupt: {e}"),
            Self::Io(e) => write!(f, "cannot save the model: {e}"),
            Self::Cancelled => f.write_str("download cancelled"),
        }
    }
}

pub trait ModelStore: Send + Sync {
    /// Where `file` of `model` is (or would be) stored.
    fn path(&self, model: &ModelSpec, file: &ModelFile) -> PathBuf;
    /// Whether `file` is stored with the expected size. Never uses the
    /// network.
    fn has(&self, model: &ModelSpec, file: &ModelFile) -> bool;
    /// Downloads `file`, verifying size and checksum; it appears at `path`
    /// only once verified. `progress` gets the bytes received so far and
    /// returns `false` to cancel.
    fn fetch(
        &self,
        model: &ModelSpec,
        file: &ModelFile,
        progress: &mut dyn FnMut(u64) -> bool,
    ) -> Result<(), FetchError>;
    /// Deletes the files of other versions of `model`.
    fn prune(&self, model: &ModelSpec) -> Result<(), String>;
    /// Deletes every version of `model`.
    fn remove(&self, model: &ModelSpec) -> Result<(), String>;
}
