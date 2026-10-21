//! Finding photos by describing them (`docs/features/ai.rst`).
//!
//! The text is encoded with CLIP's text encoder into the same space as the
//! photos' image embeddings, and photos are ranked by cosine similarity.

use std::fmt;
use std::sync::{Arc, OnceLock};

use picturesque_core::clip::Embedding;
use picturesque_core::clip_tokenizer::{CONTEXT_LENGTH, ClipTokenizer};
use serde::Deserialize;
use ts_rs::TS;

use crate::embeddings::{EmbeddingIndex, SimilarPhoto};
use crate::models::{CLIP_MERGES_FILE, CLIP_TEXT, CLIP_TEXT_FILE, LoadedModel, ModelService};
use crate::ports::inference::{InferenceEngine, Tensor};
use crate::ports::settings::SettingsStore;

/// Matches less similar than this are left out. Relevant photos score
/// about 0.19–0.25 with CLIP ViT-L/14, the median photo about 0.1
/// (`spikes/search/results.md`).
pub const MIN_SIMILARITY: f32 = 0.17;
pub const DEFAULT_LIMIT: usize = 200;
pub const MAX_LIMIT: usize = 1000;

/// Arguments of `search_photos`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct SearchArgs {
    pub text: String,
    /// 1–1000, default 200.
    #[ts(optional)]
    pub limit: Option<usize>,
}

/// Why a search could not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchError {
    /// The request is not valid; the message says why.
    Invalid(String),
    /// A model is not downloaded.
    ModelMissing(String),
    Failed(String),
}

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(m) | Self::ModelMissing(m) | Self::Failed(m) => f.write_str(m),
        }
    }
}

/// Text search over the photos' CLIP embeddings.
pub struct TextSearch {
    model: LoadedModel,
    tokenizer: OnceLock<ClipTokenizer>,
    embeddings: Arc<EmbeddingIndex>,
    models: Arc<ModelService>,
}

impl fmt::Debug for TextSearch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextSearch").finish_non_exhaustive()
    }
}

impl TextSearch {
    pub fn new(
        models: Arc<ModelService>,
        engine: Arc<dyn InferenceEngine>,
        settings: Arc<dyn SettingsStore>,
        embeddings: Arc<EmbeddingIndex>,
    ) -> Self {
        Self {
            model: LoadedModel::new(
                Arc::clone(&models),
                engine,
                settings,
                CLIP_TEXT,
                CLIP_TEXT_FILE,
            ),
            tokenizer: OnceLock::new(),
            embeddings,
            models,
        }
    }

    fn tokenizer(&self) -> Result<&ClipTokenizer, SearchError> {
        if let Some(t) = self.tokenizer.get() {
            return Ok(t);
        }
        let bytes = self
            .models
            .read(CLIP_TEXT, CLIP_MERGES_FILE)
            .map_err(|e| SearchError::Failed(format!("cannot read the merge list: {e}")))?;
        let tokenizer = ClipTokenizer::from_merges(&String::from_utf8_lossy(&bytes))
            .ok_or_else(|| SearchError::Failed("the merge list is not valid".into()))?;
        Ok(self.tokenizer.get_or_init(|| tokenizer))
    }

    /// Photos matching `text`, best first.
    pub fn search(&self, text: &str, limit: usize) -> Result<Vec<SimilarPhoto>, SearchError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(SearchError::Invalid("Type what to search for.".into()));
        }
        if !(1..=MAX_LIMIT).contains(&limit) {
            return Err(SearchError::Invalid(format!(
                "`limit` must be 1-{MAX_LIMIT}."
            )));
        }
        if let Some(reason) = self.model.unavailable() {
            return Err(SearchError::ModelMissing(reason));
        }
        if !self.model.available() {
            return Err(missing_text_model());
        }
        if self.models.files(crate::models::CLIP_IMAGE).is_none() {
            return Err(SearchError::ModelMissing(
                "Searching needs the CLIP image encoder: download it under AI models.".into(),
            ));
        }
        let ids = self.tokenizer()?.encode(text);
        let model = self.model.get().map_err(SearchError::Failed)?;
        let outputs = model
            .run(vec![Tensor::i32("text", &[1, CONTEXT_LENGTH], ids)])
            .map_err(|e| SearchError::Failed(e.to_string()))?;
        let query = outputs
            .first()
            .and_then(Tensor::as_f32)
            .and_then(Embedding::new)
            .ok_or_else(|| SearchError::Failed("the text encoder returned nothing".into()))?;
        let mut found = self
            .embeddings
            .nearest(&query, limit)
            .map_err(|e| SearchError::Failed(e.to_string()))?;
        found.retain(|p| p.similarity >= MIN_SIMILARITY);
        Ok(found)
    }
}

fn missing_text_model() -> SearchError {
    SearchError::ModelMissing(
        "Searching by text needs the CLIP text encoder: download it under AI models.".into(),
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use picturesque_core::media::MediaFormat;
    use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets, PhotoId};
    use serde_json::json;

    use super::*;
    use crate::analysis::clip::ClipAnalyzer;
    use crate::models::{CLIP_IMAGE, catalog};
    use crate::ports::catalog::PhotoCatalog;
    use crate::ports::events::NoEvents;
    use crate::settings::tests::MemorySettings;
    use crate::testing::{FakeInference, MemoryCatalog, MemoryModelStore, fake_hash};

    const MERGES: &str = "#version: 0.2\nd o\ndo g</w>\nc a\nca t</w>\n";

    struct Setup {
        search: TextSearch,
        store: Arc<MemoryModelStore>,
        catalog: Arc<MemoryCatalog>,
        tokenizer: ClipTokenizer,
    }

    fn install(store: &MemoryModelStore, id: &str) {
        let spec = catalog().into_iter().find(|m| m.id == id).unwrap();
        store.install(&spec, &[(CLIP_MERGES_FILE, MERGES.as_bytes())]);
    }

    fn setup() -> Setup {
        let store = Arc::new(MemoryModelStore::default());
        let models = Arc::new(ModelService::new(
            catalog(),
            store.clone(),
            Arc::new(NoEvents),
        ));
        let catalog = Arc::new(MemoryCatalog::default());
        let embeddings = Arc::new(EmbeddingIndex::new(catalog.clone()));
        let search = TextSearch::new(
            models,
            Arc::new(FakeInference::new(&[])),
            Arc::new(MemorySettings::default()),
            embeddings,
        );
        Setup {
            search,
            store,
            catalog,
            tokenizer: ClipTokenizer::from_merges(MERGES).unwrap(),
        }
    }

    /// A photo whose embedding equals the fake text embedding of `text`.
    fn photo(s: &Setup, text: &str, n: u8) -> PhotoId {
        let id = s
            .catalog
            .insert_photo(&NewPhoto {
                metadata: Default::default(),
                assets: PhotoAssets::new(vec![Asset {
                    format: MediaFormat::Jpeg,
                    path: format!("{n}.jpg").into(),
                    size: 1,
                    hash: fake_hash(&[n]),
                }])
                .unwrap(),
            })
            .unwrap();
        let raw: Vec<f32> = s.tokenizer.encode(text).iter().map(|&t| t as f32).collect();
        let e = Embedding::new(&raw).unwrap();
        s.catalog
            .store_analysis(id, ClipAnalyzer::ID, &json!({ "embedding": e.encode() }))
            .unwrap();
        id
    }

    #[test]
    fn needs_both_models() {
        // Fails before anything is downloaded (the fake files are empty).
        let fresh = setup();
        let err = fresh.search.search("dog", 10).unwrap_err();
        assert!(matches!(err, SearchError::ModelMissing(ref m) if m.contains("text encoder")));
        install(&fresh.store, CLIP_TEXT);
        let err = fresh.search.search("dog", 10).unwrap_err();
        assert!(matches!(err, SearchError::ModelMissing(ref m) if m.contains("image encoder")));
    }

    #[test]
    fn ranks_photos_by_the_text() {
        let s = setup();
        install(&s.store, CLIP_TEXT);
        install(&s.store, CLIP_IMAGE);
        let dog = photo(&s, "dog", 1);
        let cat = photo(&s, "cat", 2);

        let found = s.search.search("  Dog ", 10).unwrap();
        assert_eq!(found[0].photo_id, dog.0);
        assert!((found[0].similarity - 1.0).abs() < 1e-5);
        assert_eq!(found[1].photo_id, cat.0);
        assert_eq!(s.search.search("cat", 1).unwrap()[0].photo_id, cat.0);

        assert!(matches!(
            s.search.search("   ", 10),
            Err(SearchError::Invalid(_))
        ));
        assert!(matches!(
            s.search.search("dog", 0),
            Err(SearchError::Invalid(_))
        ));
    }
}
