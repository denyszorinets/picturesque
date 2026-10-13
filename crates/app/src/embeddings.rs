//! Finding visually similar photos from CLIP embeddings
//! (`docs/features/ai.rst`).
//!
//! All embeddings are kept in memory (10,000 photos take 20 MB) and
//! compared by brute force, which takes milliseconds. The index reloads
//! from the catalog after new photos were analyzed.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use picturesque_core::clip::Embedding;
use picturesque_core::photo::PhotoId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::analysis::clip::ClipAnalyzer;
use crate::ports::catalog::{CatalogError, PhotoCatalog};

/// Default and largest number of results.
pub const DEFAULT_LIMIT: usize = 50;
pub const MAX_LIMIT: usize = 500;

/// Arguments of `similar_photos`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct SimilarArgs {
    #[ts(type = "number")]
    pub photo_id: i64,
    /// 1–500, default 50.
    #[ts(optional)]
    pub limit: Option<usize>,
}

/// A photo and how much it looks like the one asked about.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct SimilarPhoto {
    #[ts(type = "number")]
    pub photo_id: i64,
    /// Cosine similarity of the CLIP embeddings, −1 to 1.
    pub similarity: f32,
}

/// Embeddings of every analyzed photo.
pub struct EmbeddingIndex {
    catalog: Arc<dyn PhotoCatalog>,
    loaded: Mutex<Option<Vec<(PhotoId, Embedding)>>>,
    stale: AtomicBool,
}

impl fmt::Debug for EmbeddingIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EmbeddingIndex").finish_non_exhaustive()
    }
}

impl EmbeddingIndex {
    pub fn new(catalog: Arc<dyn PhotoCatalog>) -> Self {
        Self {
            catalog,
            loaded: Mutex::new(None),
            stale: AtomicBool::new(true),
        }
    }

    /// Marks the index for reloading (after analysis progress).
    pub fn invalidate(&self) {
        self.stale.store(true, Ordering::Release);
    }

    fn with_embeddings<T>(
        &self,
        f: impl FnOnce(&[(PhotoId, Embedding)]) -> T,
    ) -> Result<T, CatalogError> {
        let mut loaded = self.loaded.lock().expect("embeddings lock");
        if self.stale.swap(false, Ordering::AcqRel) || loaded.is_none() {
            let all = self
                .catalog
                .analysis_results(ClipAnalyzer::ID)?
                .into_iter()
                .filter_map(|(id, result)| Some((id, ClipAnalyzer::embedding(&result)?)))
                .collect();
            *loaded = Some(all);
        }
        Ok(f(loaded.as_deref().unwrap_or_default()))
    }

    /// The photos most similar to `id`, most similar first, without `id`
    /// itself. Empty when `id` has no embedding yet.
    pub fn similar(&self, id: PhotoId, limit: usize) -> Result<Vec<SimilarPhoto>, CatalogError> {
        self.with_embeddings(|all| {
            let Some((_, target)) = all.iter().find(|(p, _)| *p == id) else {
                return Vec::new();
            };
            let mut scored: Vec<SimilarPhoto> = all
                .iter()
                .filter(|(p, _)| *p != id)
                .map(|(p, e)| SimilarPhoto {
                    photo_id: p.0,
                    similarity: target.similarity(e),
                })
                .collect();
            scored.sort_by(|a, b| {
                b.similarity
                    .total_cmp(&a.similarity)
                    .then(a.photo_id.cmp(&b.photo_id))
            });
            scored.truncate(limit);
            scored
        })
    }

    /// The photos most similar to an embedding (for text search).
    pub fn nearest(
        &self,
        query: &Embedding,
        limit: usize,
    ) -> Result<Vec<SimilarPhoto>, CatalogError> {
        self.with_embeddings(|all| {
            let mut scored: Vec<SimilarPhoto> = all
                .iter()
                .map(|(p, e)| SimilarPhoto {
                    photo_id: p.0,
                    similarity: query.similarity(e),
                })
                .collect();
            scored.sort_by(|a, b| {
                b.similarity
                    .total_cmp(&a.similarity)
                    .then(a.photo_id.cmp(&b.photo_id))
            });
            scored.truncate(limit);
            scored
        })
    }
}

#[cfg(test)]
mod tests {
    use picturesque_core::media::MediaFormat;
    use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets};
    use serde_json::json;

    use super::*;
    use crate::testing::{MemoryCatalog, fake_hash};

    fn catalog_with(photos: usize) -> Arc<MemoryCatalog> {
        let catalog = Arc::new(MemoryCatalog::default());
        for i in 0..photos {
            catalog
                .insert_photo(&NewPhoto {
                    metadata: Default::default(),
                    assets: PhotoAssets::new(vec![Asset {
                        format: MediaFormat::Jpeg,
                        path: format!("{i}.jpg").into(),
                        size: 1,
                        hash: fake_hash(format!("{i}").as_bytes()),
                    }])
                    .unwrap(),
                })
                .unwrap();
        }
        catalog
    }

    fn store(catalog: &MemoryCatalog, id: i64, raw: &[f32]) {
        let e = Embedding::new(raw).unwrap();
        catalog
            .store_analysis(
                PhotoId(id),
                ClipAnalyzer::ID,
                &json!({ "embedding": e.encode() }),
            )
            .unwrap();
    }

    #[test]
    fn ranks_photos_by_similarity_and_reloads_when_invalidated() {
        let catalog = catalog_with(5);
        store(&catalog, 1, &[1.0, 0.0, 0.0]);
        store(&catalog, 2, &[0.9, 0.1, 0.0]);
        store(&catalog, 3, &[0.0, 1.0, 0.0]);
        // Failed analyses have no embedding.
        catalog
            .store_analysis(PhotoId(4), ClipAnalyzer::ID, &json!({ "error": "x" }))
            .unwrap();
        let index = EmbeddingIndex::new(catalog.clone());

        let similar = index.similar(PhotoId(1), 10).unwrap();
        assert_eq!(
            similar.iter().map(|s| s.photo_id).collect::<Vec<_>>(),
            [2, 3]
        );
        assert!(similar[0].similarity > 0.99);
        assert!(similar[1].similarity.abs() < 1e-6);
        assert_eq!(index.similar(PhotoId(1), 1).unwrap().len(), 1);
        assert!(index.similar(PhotoId(4), 10).unwrap().is_empty());

        // New embeddings appear after invalidation only.
        store(&catalog, 5, &[1.0, 0.0, 0.01]);
        assert_eq!(index.similar(PhotoId(1), 1).unwrap()[0].photo_id, 2);
        index.invalidate();
        assert_eq!(index.similar(PhotoId(1), 1).unwrap()[0].photo_id, 5);

        let query = Embedding::new(&[0.0, 1.0, 0.0]).unwrap();
        assert_eq!(index.nearest(&query, 1).unwrap()[0].photo_id, 3);
    }
}
