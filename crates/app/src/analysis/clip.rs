//! CLIP image embeddings and the aesthetic score (`docs/features/ai.rst`).
//!
//! Runs the CLIP ViT-L/14 image encoder on each photo and stores the
//! unit-length 768-dimensional embedding, which similarity search and text
//! search build on, and the aesthetic rating of the improved LAION
//! aesthetic predictor: a linear model on the embedding trained on people's
//! 1–10 ratings (`aesthetic/convert.py`).

use std::sync::Arc;

use picturesque_core::clip::{Embedding, INPUT_SIZE, LinearHead, preprocess};
use serde::Serialize;
use serde_json::{Value, json};
use ts_rs::TS;

use super::{AnalysisInput, Analyzer};
use crate::models::{CLIP_IMAGE, CLIP_IMAGE_FILE, LoadedModel, ModelService};
use crate::ports::catalog::AnalyzerId;
use crate::ports::inference::{InferenceEngine, Tensor};
use crate::ports::settings::SettingsStore;

/// The improved LAION aesthetic predictor for CLIP ViT-L/14 (Apache-2.0).
const AESTHETIC_HEAD: &[u8] = include_bytes!("aesthetic/vit_l_14.bin");

/// Ratings mapped to 0 and 1 by [`aesthetic_score`]: about the 2nd and
/// 98th percentile of a mixed sample of ordinary and outstanding photos
/// (`spikes/aesthetic/results.md`).
const AESTHETIC_RANGE: (f32, f32) = (4.5, 6.5);

/// The aesthetic predictor: embedding → rating, about 1–10.
pub fn aesthetic_head() -> LinearHead {
    LinearHead::from_le_bytes(AESTHETIC_HEAD).expect("valid aesthetic weights")
}

/// A rating as a 0–1 score for display and ranking.
pub fn aesthetic_score(rating: f32) -> f32 {
    let (low, high) = AESTHETIC_RANGE;
    ((rating - low) / (high - low)).clamp(0.0, 1.0)
}

/// The aesthetic score of a photo, as `get_photo` returns it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct AestheticView {
    /// 0–1 (see [`aesthetic_score`]).
    pub score: f32,
    /// The predictor's rating, about 1–10.
    pub rating: f32,
}

impl AestheticView {
    pub fn from_result(result: &Value) -> Option<Self> {
        let rating = ClipAnalyzer::aesthetic(result)?;
        Some(Self {
            score: aesthetic_score(rating),
            rating,
        })
    }
}

/// Computes CLIP embeddings once the model is downloaded.
pub struct ClipAnalyzer {
    model: LoadedModel,
    aesthetic: LinearHead,
}

impl ClipAnalyzer {
    /// Bump with the model's version: embeddings of different models do
    /// not compare.
    pub const ID: AnalyzerId = AnalyzerId {
        name: "clip",
        version: 3,
    };

    pub fn new(
        models: Arc<ModelService>,
        engine: Arc<dyn InferenceEngine>,
        settings: Arc<dyn SettingsStore>,
    ) -> Self {
        Self {
            model: LoadedModel::new(models, engine, settings, CLIP_IMAGE, CLIP_IMAGE_FILE),
            aesthetic: aesthetic_head(),
        }
    }

    /// The stored aesthetic rating (about 1–10) of an analysis result.
    pub fn aesthetic(result: &Value) -> Option<f32> {
        result.get("aesthetic")?.as_f64().map(|v| v as f32)
    }

    /// The stored embedding of an analysis result.
    pub fn embedding(result: &Value) -> Option<Embedding> {
        Embedding::decode(result.get("embedding")?.as_str()?)
    }
}

impl std::fmt::Debug for ClipAnalyzer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClipAnalyzer").finish_non_exhaustive()
    }
}

impl Analyzer for ClipAnalyzer {
    fn id(&self) -> AnalyzerId {
        Self::ID
    }

    fn ready(&self) -> bool {
        self.model.available()
    }

    fn analyze(&self, input: &AnalysisInput<'_>) -> Result<Value, String> {
        let model = self.model.get()?;
        let size = INPUT_SIZE as usize;
        let outputs = model
            .run(vec![Tensor::f32(
                "image",
                &[1, 3, size, size],
                preprocess(input.image),
            )])
            .map_err(|e| e.to_string())?;
        let raw = outputs
            .first()
            .and_then(Tensor::as_f32)
            .ok_or("the CLIP model returned no embedding")?;
        let embedding = Embedding::new(raw).ok_or("the CLIP embedding is empty")?;
        Ok(json!({
            "embedding": embedding.encode(),
            "aesthetic": self.aesthetic.apply(&embedding),
        }))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn the_aesthetic_head_matches_clip_vit_l14() {
        let head = aesthetic_head();
        let flat = Embedding::new(&[1.0; 768]).unwrap();
        assert!(head.apply(&flat).is_some());
        assert!(head.apply(&Embedding::new(&[1.0; 512]).unwrap()).is_none());
    }

    #[test]
    fn ratings_map_to_scores() {
        assert_eq!(aesthetic_score(3.0), 0.0);
        assert_eq!(aesthetic_score(5.5), 0.5);
        assert_eq!(aesthetic_score(9.0), 1.0);
        let view = AestheticView::from_result(&json!({ "aesthetic": 6.0 })).unwrap();
        assert!((view.score - 0.75).abs() < 1e-6);
        assert!(AestheticView::from_result(&json!({ "error": "x" })).is_none());
        assert!(AestheticView::from_result(&json!({ "aesthetic": null })).is_none());
    }
}
