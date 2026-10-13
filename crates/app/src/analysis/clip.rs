//! CLIP image embeddings (`docs/features/ai.rst`).
//!
//! Runs the CLIP ViT-B/32 image encoder on each photo and stores the
//! unit-length 512-dimensional embedding, which similarity search, the
//! aesthetic score and text search build on.

use std::sync::{Arc, Mutex};

use picturesque_core::clip::{Embedding, INPUT_SIZE, preprocess};
use serde_json::{Value, json};

use super::{AnalysisInput, Analyzer};
use crate::models::{CLIP_IMAGE, CLIP_IMAGE_FILE, ModelService};
use crate::ports::catalog::AnalyzerId;
use crate::ports::inference::{InferenceEngine, Model, ModelSource, ProviderChoice, Tensor};
use crate::ports::settings::SettingsStore;

/// Computes CLIP embeddings once the model is downloaded.
pub struct ClipAnalyzer {
    models: Arc<ModelService>,
    engine: Arc<dyn InferenceEngine>,
    settings: Arc<dyn SettingsStore>,
    /// The loaded model and the provider choice it was loaded with.
    loaded: Mutex<Option<(ProviderChoice, Arc<dyn Model>)>>,
}

impl ClipAnalyzer {
    /// Bump with the model's version: embeddings of different models do
    /// not compare.
    pub const ID: AnalyzerId = AnalyzerId {
        name: "clip",
        version: 1,
    };

    pub fn new(
        models: Arc<ModelService>,
        engine: Arc<dyn InferenceEngine>,
        settings: Arc<dyn SettingsStore>,
    ) -> Self {
        Self {
            models,
            engine,
            settings,
            loaded: Mutex::new(None),
        }
    }

    /// The stored embedding of an analysis result.
    pub fn embedding(result: &Value) -> Option<Embedding> {
        Embedding::decode(result.get("embedding")?.as_str()?)
    }

    /// The model, loaded on first use and again when the provider choice
    /// changes.
    fn model(&self) -> Result<Arc<dyn Model>, String> {
        let choice = self
            .settings
            .load_settings()
            .map_err(|e| e.to_string())?
            .ai
            .provider;
        let mut loaded = self.loaded.lock().expect("clip model lock");
        if let Some((c, model)) = loaded.as_ref()
            && *c == choice
        {
            return Ok(Arc::clone(model));
        }
        let files = self
            .models
            .files(CLIP_IMAGE)
            .ok_or("the CLIP model is not downloaded")?;
        let path = files
            .get(CLIP_IMAGE_FILE)
            .ok_or("the CLIP model has no image encoder")?;
        let model: Arc<dyn Model> = Arc::from(
            self.engine
                .load(ModelSource::File(path), choice)
                .map_err(|e| e.to_string())?,
        );
        *loaded = Some((choice, Arc::clone(&model)));
        Ok(model)
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
        self.models.files(CLIP_IMAGE).is_some()
    }

    fn analyze(&self, input: &AnalysisInput<'_>) -> Result<Value, String> {
        let model = self.model()?;
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
        Ok(json!({ "embedding": embedding.encode() }))
    }
}
