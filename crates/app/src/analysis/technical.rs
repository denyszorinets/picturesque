//! Technical quality (sharpness, exposure, noise) as an analyzer.

use picturesque_core::quality::{TechnicalScores, technical_scores};
use serde::Serialize;
use ts_rs::TS;

use super::{AnalysisInput, Analyzer};
use crate::ports::catalog::AnalyzerId;

/// Scores from [`picturesque_core::quality`]. Bump the version whenever
/// the algorithm or its constants change, so photos are rescored.
#[derive(Debug, Default, Clone, Copy)]
pub struct TechnicalAnalyzer;

impl TechnicalAnalyzer {
    pub const ID: AnalyzerId = AnalyzerId {
        name: "technical",
        version: 1,
    };
}

impl Analyzer for TechnicalAnalyzer {
    fn id(&self) -> AnalyzerId {
        Self::ID
    }

    fn analyze(&self, input: &AnalysisInput<'_>) -> Result<serde_json::Value, String> {
        serde_json::to_value(technical_scores(input.image)).map_err(|e| e.to_string())
    }
}

/// Technical scores shown to the user, each 0–1 (higher is better).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct QualityView {
    pub sharpness: f32,
    pub exposure: f32,
    pub noise: f32,
    pub overall: f32,
    /// Share of pixels with blown highlights / black shadows.
    pub highlights_clipped: f32,
    pub shadows_clipped: f32,
}

impl QualityView {
    /// Reads a stored `technical` result; `None` for errors or results of
    /// another shape.
    pub fn from_result(result: &serde_json::Value) -> Option<Self> {
        let s: TechnicalScores = serde_json::from_value(result.clone()).ok()?;
        Some(Self {
            sharpness: s.sharpness,
            exposure: s.exposure,
            noise: s.noise,
            overall: s.overall,
            highlights_clipped: s.highlights_clipped,
            shadows_clipped: s.shadows_clipped,
        })
    }
}

#[cfg(test)]
mod tests {
    use picturesque_core::photo::{Asset, Photo, PhotoAssets, PhotoId};
    use picturesque_core::pixels::RgbImage;
    use serde_json::json;

    use super::*;

    #[test]
    fn stores_scores_that_read_back_as_a_view() {
        let photo = Photo {
            id: PhotoId(1),
            metadata: Default::default(),
            assets: PhotoAssets::new(vec![Asset {
                format: picturesque_core::media::MediaFormat::Jpeg,
                path: "a.jpg".into(),
                size: 1,
                hash: crate::testing::fake_hash(b"a"),
            }])
            .unwrap(),
        };
        let image = RgbImage::from_fn(64, 64, |x, y| {
            if (x / 4 + y / 4) % 2 == 0 {
                [30, 30, 30]
            } else {
                [200, 200, 200]
            }
        });

        let result = TechnicalAnalyzer
            .analyze(&AnalysisInput {
                photo: &photo,
                image: &image,
            })
            .unwrap();
        let view = QualityView::from_result(&result).unwrap();

        assert!(view.sharpness > 0.9, "{view:?}");
        assert_eq!(view.overall, result["overall"].as_f64().unwrap() as f32);
        assert_eq!(QualityView::from_result(&json!({ "error": "x" })), None);
    }
}
