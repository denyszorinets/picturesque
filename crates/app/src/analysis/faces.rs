//! Faces, closed eyes and face sharpness (`docs/features/ai.rst`).
//!
//! YuNet finds faces; for each, the MediaPipe face mesh places landmarks,
//! from which the eye aspect ratio tells open from closed eyes. Face
//! sharpness uses the technical sharpness measure on the face alone, so a
//! photo with a sharp background but a soft face is caught.

use std::sync::Arc;

use picturesque_core::faces::{
    DETECTOR_SIZE, Detection, DetectorLevel, MESH_SIZE, decode_detections, detector_input,
    eye_openness, mesh_input,
};
use picturesque_core::pixels::RgbImage;
use picturesque_core::quality::technical_scores;
use picturesque_core::ranking::FaceSignal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::{AnalysisInput, Analyzer};
use crate::models::{FACE_DETECTOR_FILE, FACE_MESH_FILE, FACES, LoadedModel, ModelService};
use crate::ports::catalog::AnalyzerId;
use crate::ports::inference::{InferenceEngine, Tensor};
use crate::ports::settings::SettingsStore;

/// Eyes are closed when their mean aspect ratio is below this. On the
/// fixtures open eyes measured 0.21–0.36, closed ones 0.09–0.14.
pub const CLOSED_EYES: f32 = 0.17;
/// Faces smaller than this (pixels of the 1024-pixel analysis image) are
/// too small to judge eyes.
const MIN_EYE_FACE: f32 = 40.0;
/// At most this many faces per photo are examined, largest first.
const MAX_FACES: usize = 8;

/// One face, as stored and as `get_photo` returns it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FaceView {
    /// Left, top, width and height, as fractions of the photo.
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Detector confidence, 0–1.
    pub score: f32,
    /// `true`/`false` when the eyes could be judged, else `null`.
    pub eyes_closed: Option<bool>,
    /// Mean eye aspect ratio (about 0.3 open, under 0.17 closed).
    pub eye_openness: Option<f32>,
    /// Sharpness of the face, 0–1, like the photo's technical sharpness.
    pub sharpness: f32,
}

/// The faces of an analysis result (empty when none were found).
pub fn faces(result: &Value) -> Option<Vec<FaceView>> {
    serde_json::from_value(result.get("faces")?.clone()).ok()
}

/// What ranking needs to know about a photo's faces: whether the largest
/// face, or one at least half its size, has closed eyes, and how sharp the
/// largest face is. `None` without faces.
pub fn signal(faces: &[FaceView]) -> Option<FaceSignal> {
    let area = |f: &FaceView| f.width * f.height;
    let largest = faces.iter().max_by(|a, b| area(a).total_cmp(&area(b)))?;
    let important: Vec<&FaceView> = faces
        .iter()
        .filter(|f| area(f) >= area(largest) / 2.0)
        .collect();
    let eyes_closed = if important.iter().any(|f| f.eyes_closed == Some(true)) {
        Some(true)
    } else if important.iter().any(|f| f.eyes_closed == Some(false)) {
        Some(false)
    } else {
        None
    };
    Some(FaceSignal {
        eyes_closed,
        sharpness: largest.sharpness,
    })
}

/// Finds faces and judges their eyes and focus.
pub struct FaceAnalyzer {
    detector: LoadedModel,
    mesh: LoadedModel,
}

impl FaceAnalyzer {
    pub const ID: AnalyzerId = AnalyzerId {
        name: "faces",
        version: 1,
    };

    pub fn new(
        models: Arc<ModelService>,
        engine: Arc<dyn InferenceEngine>,
        settings: Arc<dyn SettingsStore>,
    ) -> Self {
        Self {
            detector: LoadedModel::new(
                Arc::clone(&models),
                Arc::clone(&engine),
                Arc::clone(&settings),
                FACES,
                FACE_DETECTOR_FILE,
            ),
            mesh: LoadedModel::new(models, engine, settings, FACES, FACE_MESH_FILE),
        }
    }

    fn detect(&self, image: &RgbImage) -> Result<Vec<Detection>, String> {
        let (input, scale) = detector_input(image);
        let size = DETECTOR_SIZE as usize;
        let outputs = self
            .detector
            .get()?
            .run(vec![Tensor::f32("input", &[1, 3, size, size], input)])
            .map_err(|e| e.to_string())?;
        let output = |name: String| {
            outputs
                .iter()
                .find(|t| t.name == name)
                .and_then(Tensor::as_f32)
                .ok_or_else(|| format!("the face detector has no output `{name}`"))
        };
        let mut levels = Vec::new();
        for stride in [8, 16, 32] {
            levels.push(DetectorLevel {
                stride,
                cls: output(format!("cls_{stride}"))?,
                obj: output(format!("obj_{stride}"))?,
                bbox: output(format!("bbox_{stride}"))?,
            });
        }
        Ok(decode_detections(&levels, scale))
    }

    /// Mean eye aspect ratio, or `None` when the face mesh is not sure it
    /// sees a face.
    fn eyes(&self, image: &RgbImage, face: &Detection) -> Result<Option<f32>, String> {
        let n = MESH_SIZE as usize;
        let outputs = self
            .mesh
            .get()?
            .run(vec![Tensor::f32(
                "input_12",
                &[1, n, n, 3],
                mesh_input(image, face),
            )])
            .map_err(|e| e.to_string())?;
        let landmarks = outputs.first().and_then(Tensor::as_f32);
        // The second output is the face presence as a logit.
        let present = outputs
            .get(1)
            .and_then(Tensor::as_f32)
            .and_then(|p| p.first())
            .is_some_and(|logit| *logit > 0.0);
        Ok(landmarks
            .filter(|_| present)
            .and_then(eye_openness)
            .map(|(right, left)| (right + left) / 2.0))
    }
}

impl std::fmt::Debug for FaceAnalyzer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FaceAnalyzer").finish_non_exhaustive()
    }
}

impl Analyzer for FaceAnalyzer {
    fn id(&self) -> AnalyzerId {
        Self::ID
    }

    fn ready(&self) -> bool {
        self.detector.available()
    }

    fn purpose(&self) -> &'static str {
        "faces and closed eyes"
    }

    fn analyze(&self, input: &AnalysisInput<'_>) -> Result<Value, String> {
        let image = input.image;
        let (w, h) = (image.width() as f32, image.height() as f32);
        let mut detections = self.detect(image)?;
        detections.sort_by(|a, b| (b.width * b.height).total_cmp(&(a.width * a.height)));
        detections.truncate(MAX_FACES);
        let mut faces = Vec::new();
        for face in detections {
            let eye_openness = if face.width.min(face.height) >= MIN_EYE_FACE {
                self.eyes(image, &face)?
            } else {
                None
            };
            // The box clipped to the image.
            let (x, y) = (face.x.clamp(0.0, w), face.y.clamp(0.0, h));
            let width = (face.x + face.width).clamp(0.0, w) - x;
            let height = (face.y + face.height).clamp(0.0, h) - y;
            let crop = image.crop(x as u32, y as u32, width as u32, height as u32);
            faces.push(FaceView {
                x: x / w,
                y: y / h,
                width: width / w,
                height: height / h,
                score: face.score,
                eyes_closed: eye_openness.map(|e| e < CLOSED_EYES),
                eye_openness,
                sharpness: if crop.width() >= 8 && crop.height() >= 8 {
                    technical_scores(&crop).sharpness
                } else {
                    0.0
                },
            });
        }
        Ok(serde_json::json!({ "faces": faces }))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use picturesque_core::media::MediaFormat;
    use picturesque_core::photo::{Asset, Photo, PhotoAssets, PhotoId};

    use super::*;
    use crate::models::catalog;
    use crate::ports::events::NoEvents;
    use crate::ports::inference::{
        InferenceError, Model, ModelSource, Probe, Provider, ProviderChoice,
    };
    use crate::settings::tests::MemorySettings;
    use crate::testing::MemoryModelStore;

    /// Plays YuNet (one face in grid cell 21 of stride 32) and the face
    /// mesh (eyes with the given aspect ratio, face present).
    #[derive(Debug)]
    struct Scripted {
        eye_ratio: f32,
    }

    impl Model for Scripted {
        fn provider(&self) -> Provider {
            Provider::Cpu
        }

        fn fallback(&self) -> Option<&str> {
            None
        }

        fn run(&self, inputs: Vec<Tensor>) -> Result<Vec<Tensor>, InferenceError> {
            if inputs[0].name == "input" {
                let mut out = Vec::new();
                for stride in [8usize, 16, 32] {
                    let n = (640 / stride).pow(2);
                    let mut cls = vec![0.0; n];
                    let mut bbox = vec![0.0; n * 4];
                    if stride == 32 {
                        cls[21] = 0.9;
                        // A 128-pixel face.
                        bbox[21 * 4 + 2] = 4.0f32.ln();
                        bbox[21 * 4 + 3] = 4.0f32.ln();
                    }
                    out.push(Tensor::f32(
                        &format!("cls_{stride}"),
                        &[1, n, 1],
                        cls.clone(),
                    ));
                    out.push(Tensor::f32(&format!("obj_{stride}"), &[1, n, 1], cls));
                    out.push(Tensor::f32(&format!("bbox_{stride}"), &[1, n, 4], bbox));
                }
                return Ok(out);
            }
            let mut mesh = vec![0.0f32; 478 * 3];
            let half = 15.0 * self.eye_ratio;
            for (eye, dx) in [
                ([33, 160, 158, 133, 153, 144], 0.0),
                ([362, 385, 387, 263, 373, 380], 100.0),
            ] {
                let points = [
                    (0.0, 0.0),
                    (10.0, -half),
                    (20.0, -half),
                    (30.0, 0.0),
                    (20.0, half),
                    (10.0, half),
                ];
                for (i, (x, y)) in eye.iter().zip(points) {
                    mesh[i * 3] = x + dx;
                    mesh[i * 3 + 1] = y;
                }
            }
            Ok(vec![
                Tensor::f32("Identity", &[1, 1, 1, 1434], mesh),
                Tensor::f32("Identity_1", &[1, 1, 1, 1], vec![5.0]),
            ])
        }
    }

    #[derive(Debug)]
    struct Engine(f32);

    impl InferenceEngine for Engine {
        fn available(&self) -> Vec<Provider> {
            vec![Provider::Cpu]
        }

        fn load(
            &self,
            _: ModelSource<'_>,
            _: ProviderChoice,
        ) -> Result<Box<dyn Model>, InferenceError> {
            Ok(Box::new(Scripted { eye_ratio: self.0 }))
        }

        fn probe(&self, _: ProviderChoice) -> Result<Probe, InferenceError> {
            unreachable!()
        }
    }

    fn analyze(eye_ratio: f32) -> (Vec<FaceView>, bool) {
        let store = Arc::new(MemoryModelStore::default());
        let models = Arc::new(ModelService::new(
            catalog(),
            store.clone(),
            Arc::new(NoEvents),
        ));
        let analyzer = FaceAnalyzer::new(
            models,
            Arc::new(Engine(eye_ratio)),
            Arc::new(MemorySettings::default()),
        );
        let ready_before = analyzer.ready();
        let spec = catalog().into_iter().find(|m| m.id == FACES).unwrap();
        store.install(&spec, &[]);
        assert!(analyzer.ready());
        let photo = Photo {
            id: PhotoId(1),
            metadata: Default::default(),
            assets: PhotoAssets::new(vec![Asset {
                format: MediaFormat::Jpeg,
                path: "a.jpg".into(),
                size: 1,
                hash: crate::testing::fake_hash(b"a"),
            }])
            .unwrap(),
        };
        let image = RgbImage::filled(640, 640, [120, 110, 100]);
        let result = analyzer
            .analyze(&AnalysisInput {
                photo: &photo,
                image: &image,
            })
            .unwrap();
        (faces(&result).unwrap(), ready_before)
    }

    #[test]
    fn finds_faces_and_closed_eyes() {
        let (faces, ready_before) = analyze(0.08);
        assert!(!ready_before, "not ready before the models are downloaded");
        assert_eq!(faces.len(), 1);
        let face = faces[0];
        // Centre at cell (1, 1) × 32 = 32 px, 128 px box: −32 to 96 px,
        // clipped to the 640-pixel image.
        assert!((face.x - 0.0).abs() < 1e-6, "{face:?}");
        assert!((face.width - 96.0 / 640.0).abs() < 1e-6, "{face:?}");
        assert_eq!(face.eyes_closed, Some(true));
        assert!((face.eye_openness.unwrap() - 0.08).abs() < 1e-4);
        assert!((0.0..=1.0).contains(&face.sharpness));
    }

    #[test]
    fn the_signal_looks_at_important_faces() {
        let face = |size: f32, eyes_closed, sharpness| FaceView {
            x: 0.0,
            y: 0.0,
            width: size,
            height: size,
            score: 0.9,
            eyes_closed,
            eye_openness: None,
            sharpness,
        };
        assert_eq!(signal(&[]), None);
        // A small face in the background blinking does not count.
        let s = signal(&[face(0.4, Some(false), 0.8), face(0.1, Some(true), 0.2)]).unwrap();
        assert_eq!(s.eyes_closed, Some(false));
        assert_eq!(s.sharpness, 0.8);
        // Two people of similar size: one blink counts.
        let s = signal(&[face(0.4, Some(false), 0.8), face(0.35, Some(true), 0.9)]).unwrap();
        assert_eq!(s.eyes_closed, Some(true));
        assert_eq!(signal(&[face(0.4, None, 0.5)]).unwrap().eyes_closed, None);
    }

    #[test]
    fn open_eyes_are_open() {
        let (faces, _) = analyze(0.3);
        assert_eq!(faces[0].eyes_closed, Some(false));
    }
}
