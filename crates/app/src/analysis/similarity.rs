//! Perceptual hashes as an analyzer, for series detection.

use picturesque_core::similarity::Signature;

use super::{AnalysisInput, Analyzer};
use crate::ports::catalog::AnalyzerId;

/// Stores each photo's [`Signature`] (`{"phash": "<16 hex digits>"}`).
#[derive(Debug, Default, Clone, Copy)]
pub struct SimilarityAnalyzer;

impl SimilarityAnalyzer {
    pub const ID: AnalyzerId = AnalyzerId {
        name: "similarity",
        version: 1,
    };

    /// Reads a stored result; `None` for errors.
    pub fn signature(result: &serde_json::Value) -> Option<Signature> {
        serde_json::from_value(result.clone()).ok()
    }
}

impl Analyzer for SimilarityAnalyzer {
    fn id(&self) -> AnalyzerId {
        Self::ID
    }

    fn analyze(&self, input: &AnalysisInput<'_>) -> Result<serde_json::Value, String> {
        serde_json::to_value(Signature::of(input.image)).map_err(|e| e.to_string())
    }
}
