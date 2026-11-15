//! Bird ID (`docs/features/bird-id.rst`): the user draws a box around a
//! bird and gets the most likely species from WingCLIP, a local model,
//! optionally limited to the species eBird lists for their region. The
//! species they choose is kept with the photo.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, Mutex};

use picturesque_core::birds::{
    Classifier, DETECT_SIZE, Rect, Species, crop_square, decode_detections, detector_input,
    labels_from_rows,
};
use picturesque_core::clip::{Embedding, INPUT_SIZE, preprocess};
use picturesque_core::photo::PhotoId;
use picturesque_core::pixels::RgbImage;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::models::{
    BIRD_DETECTOR, BIRD_DETECTOR_FILE, BIRDS, BIRDS_CLASSIFIER_FILE, BIRDS_LABELS_FILE,
    BIRDS_MODEL_FILE, LoadedModel, ModelService,
};
use crate::ports::birds::SpeciesLists;
use crate::ports::catalog::{CatalogError, PhotoCatalog};
use crate::ports::inference::{InferenceEngine, Tensor};
use crate::ports::pixels::PixelLoader;
use crate::ports::settings::SettingsStore;
use crate::settings::BirdRegion;
use crate::thumbnails::{ImageSize, ThumbnailService};

/// Suggestions returned.
const SUGGESTIONS: usize = 5;
/// Longest edge of the image the box is cut from: enough detail for a
/// small bird in a large frame.
const SOURCE_EDGE: u32 = 4096;
/// COCO's "bird" among the detector's 80 classes.
const COCO_BIRD: usize = 14;
const COCO_CLASSES: usize = 80;
/// How sure the detector must be that a box holds a bird.
const DETECT_THRESHOLD: f32 = 0.4;
/// Most birds found in one photo.
const MAX_BIRDS: usize = 20;

/// A box in the photo as shown (upright), as shares of its size.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct BoxView {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl From<BoxView> for Rect {
    fn from(b: BoxView) -> Self {
        Self {
            x: b.x,
            y: b.y,
            width: b.width,
            height: b.height,
        }
    }
}

impl From<Rect> for BoxView {
    fn from(r: Rect) -> Self {
        Self {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        }
    }
}

/// Arguments of `identify_bird`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct IdentifyBirdArgs {
    #[ts(type = "number")]
    pub photo_id: i64,
    #[serde(rename = "box")]
    pub area: BoxView,
}

/// A species.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct SpeciesView {
    pub common: String,
    pub scientific: String,
    /// eBird species code.
    pub code: String,
}

impl From<&Species> for SpeciesView {
    fn from(s: &Species) -> Self {
        Self {
            common: s.common.clone(),
            scientific: s.scientific.clone(),
            code: s.code.clone(),
        }
    }
}

impl From<SpeciesView> for Species {
    fn from(s: SpeciesView) -> Self {
        Self {
            common: s.common,
            scientific: s.scientific,
            code: s.code,
        }
    }
}

/// A suggested species.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct SuggestionView {
    pub species: SpeciesView,
    /// How sure the model is, 0–1, among the species considered.
    pub confidence: f32,
}

/// What `identify_bird` found.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct BirdIdResult {
    /// Most likely first.
    pub suggestions: Vec<SuggestionView>,
    /// The region the suggestions were limited to, if any.
    pub region: Option<String>,
    /// The model's best match anywhere in the world, when it is not on the
    /// region's list (a vagrant, or a wrong guess).
    pub elsewhere: Option<SpeciesView>,
}

/// A bird `find_birds` found, with its suggestions.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct FoundBird {
    #[serde(rename = "box")]
    pub area: BoxView,
    /// How sure the detector is that it is a bird, 0–1.
    pub score: f32,
    /// Most likely species first.
    pub suggestions: Vec<SuggestionView>,
    /// The best match outside the region, as in [`BirdIdResult`].
    pub elsewhere: Option<SpeciesView>,
}

/// What `find_birds` found.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct FoundBirds {
    /// Left to right.
    pub birds: Vec<FoundBird>,
    /// The region the suggestions were limited to, if any.
    pub region: Option<String>,
}

/// Arguments of `find_birds`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct FindBirdsArgs {
    #[ts(type = "number")]
    pub photo_id: i64,
}

/// Arguments of `set_photo_species`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct SetSpeciesArgs {
    #[ts(type = "number")]
    pub photo_id: i64,
    /// Every species on the photo, in the order chosen; empty forgets them.
    pub species: Vec<SpeciesView>,
}

/// Arguments of `photo_species`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct PhotoSpeciesArgs {
    #[ts(type = "number[]")]
    pub photo_ids: Vec<i64>,
}

/// A photo's chosen species.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct PhotoSpeciesView {
    #[ts(type = "number")]
    pub photo_id: i64,
    /// In the order chosen.
    pub species: Vec<SpeciesView>,
}

/// What `refresh_bird_region` fetched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct BirdRegionView {
    pub region: String,
    /// Species on the region's list.
    pub species: usize,
    /// Of those, species the model knows.
    pub known: usize,
}

/// Why Bird ID could not answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BirdError {
    /// The model is not downloaded (or cannot run here); the message says
    /// what to do.
    NotReady(String),
    /// The request cannot be answered as asked.
    Invalid(String),
    /// Something failed; the message says what.
    Failed(String),
}

impl fmt::Display for BirdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotReady(m) | Self::Invalid(m) | Self::Failed(m) => f.write_str(m),
        }
    }
}

impl From<CatalogError> for BirdError {
    fn from(e: CatalogError) -> Self {
        Self::Failed(e.to_string())
    }
}

/// The species matrix and names, read once from the downloaded model.
struct Known {
    classifier: Classifier,
    labels: Vec<Species>,
    by_code: HashMap<String, usize>,
}

/// Identifies birds and keeps the species chosen for photos.
pub struct BirdService {
    model: LoadedModel,
    detector: LoadedModel,
    models: Arc<ModelService>,
    thumbnails: Arc<ThumbnailService>,
    pixels: Arc<dyn PixelLoader>,
    catalog: Arc<dyn PhotoCatalog>,
    settings: Arc<dyn SettingsStore>,
    lists: Arc<dyn SpeciesLists>,
    species: Mutex<Option<Arc<Known>>>,
}

impl fmt::Debug for BirdService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BirdService").finish_non_exhaustive()
    }
}

impl fmt::Debug for BirdDeps {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BirdDeps").finish_non_exhaustive()
    }
}

/// The services Bird ID uses.
pub struct BirdDeps {
    pub models: Arc<ModelService>,
    pub engine: Arc<dyn InferenceEngine>,
    pub thumbnails: Arc<ThumbnailService>,
    pub pixels: Arc<dyn PixelLoader>,
    pub catalog: Arc<dyn PhotoCatalog>,
    pub settings: Arc<dyn SettingsStore>,
    pub lists: Arc<dyn SpeciesLists>,
}

impl BirdService {
    pub fn new(deps: BirdDeps) -> Self {
        Self {
            model: LoadedModel::new(
                Arc::clone(&deps.models),
                Arc::clone(&deps.engine),
                Arc::clone(&deps.settings),
                BIRDS,
                BIRDS_MODEL_FILE,
            ),
            detector: LoadedModel::new(
                Arc::clone(&deps.models),
                deps.engine,
                Arc::clone(&deps.settings),
                BIRD_DETECTOR,
                BIRD_DETECTOR_FILE,
            ),
            models: deps.models,
            thumbnails: deps.thumbnails,
            pixels: deps.pixels,
            catalog: deps.catalog,
            settings: deps.settings,
            lists: deps.lists,
            species: Mutex::new(None),
        }
    }

    fn not_ready() -> BirdError {
        BirdError::NotReady(
            "Download the WingCLIP bird identifier in Settings → AI models first.".into(),
        )
    }

    /// The species matrix and names, loaded on first use.
    fn species(&self) -> Result<Arc<Known>, BirdError> {
        let mut cached = self.species.lock().expect("species lock");
        if let Some(s) = cached.as_ref() {
            return Ok(Arc::clone(s));
        }
        let read = |file| self.models.read(BIRDS, file).map_err(|_| Self::not_ready());
        let classifier =
            Classifier::from_npy(&read(BIRDS_CLASSIFIER_FILE)?).map_err(BirdError::Failed)?;
        let rows: Vec<Vec<String>> = serde_json::from_slice(&read(BIRDS_LABELS_FILE)?)
            .map_err(|e| BirdError::Failed(format!("the species names are unreadable: {e}")))?;
        let labels = labels_from_rows(rows).map_err(BirdError::Failed)?;
        if labels.len() != classifier.species() {
            return Err(BirdError::Failed(format!(
                "the model knows {} species but names {}",
                classifier.species(),
                labels.len()
            )));
        }
        let by_code = labels
            .iter()
            .enumerate()
            .map(|(i, s)| (s.code.clone(), i))
            .collect();
        let species = Arc::new(Known {
            classifier,
            labels,
            by_code,
        });
        *cached = Some(Arc::clone(&species));
        Ok(species)
    }

    /// Checks the species identifier can run.
    fn ready(&self) -> Result<Arc<Known>, BirdError> {
        if let Some(why) = self.model.unavailable() {
            return Err(BirdError::NotReady(why));
        }
        if !self.model.available() {
            return Err(Self::not_ready());
        }
        self.species()
    }

    /// The photo as shown (upright), longest edge at most [`SOURCE_EDGE`].
    fn load(&self, id: PhotoId) -> Result<RgbImage, BirdError> {
        let view = self
            .thumbnails
            .pair_view()
            .map_err(|e| BirdError::Failed(e.to_string()))?;
        let path = self
            .thumbnails
            .image(id, ImageSize::Full, view)
            .map_err(|e| BirdError::Failed(e.to_string()))?;
        self.pixels
            .load(&path, SOURCE_EDGE)
            .map_err(|e| BirdError::Failed(format!("Cannot read the photo: {e}")))
    }

    /// The species of the region set now (classifier rows), if any.
    fn region(&self, known: &Known) -> Result<Option<(HashSet<usize>, String)>, BirdError> {
        let settings = self.settings.load_settings()?;
        let allowed: Option<HashSet<usize>> = settings.region_species().map(|codes| {
            codes
                .iter()
                .filter_map(|c| known.by_code.get(c).copied())
                .collect()
        });
        Ok(allowed
            .filter(|a| !a.is_empty())
            .map(|a| (a, settings.bird_id.region.clone())))
    }

    /// Ranks species for the bird in `rect` of `image`: (suggestions,
    /// best match outside the region).
    fn classify(
        &self,
        image: &RgbImage,
        rect: Rect,
        known: &Known,
        allowed: Option<&HashSet<usize>>,
    ) -> Result<(Vec<SuggestionView>, Option<SpeciesView>), BirdError> {
        let (x, y, side) = crop_square(rect, image.width(), image.height())
            .ok_or_else(|| BirdError::Invalid("Draw a box around the bird first.".into()))?;
        let crop = image.crop(x, y, side, side);
        let size = INPUT_SIZE as usize;
        let outputs = self
            .model
            .get()
            .map_err(BirdError::Failed)?
            .run(vec![Tensor::f32(
                "image",
                &[1, 3, size, size],
                preprocess(&crop),
            )])
            .map_err(|e| BirdError::Failed(e.to_string()))?;
        let embedding = outputs
            .first()
            .and_then(Tensor::as_f32)
            .and_then(Embedding::new)
            .ok_or_else(|| BirdError::Failed("the bird identifier returned nothing".into()))?;
        let view = |i: usize| SpeciesView::from(&known.labels[i]);
        let suggestions = known
            .classifier
            .rank(&embedding, SUGGESTIONS, allowed)
            .into_iter()
            .map(|c| SuggestionView {
                species: view(c.index),
                confidence: c.confidence,
            })
            .collect();
        let elsewhere = allowed.and_then(|allowed| {
            let best = known
                .classifier
                .rank(&embedding, 1, None)
                .into_iter()
                .next()?;
            (!allowed.contains(&best.index)).then(|| view(best.index))
        });
        Ok((suggestions, elsewhere))
    }

    /// The most likely species of the bird in `area` of the photo.
    pub fn identify(&self, id: PhotoId, area: BoxView) -> Result<BirdIdResult, BirdError> {
        let known = self.ready()?;
        let image = self.load(id)?;
        let region = self.region(&known)?;
        let (suggestions, elsewhere) =
            self.classify(&image, area.into(), &known, region.as_ref().map(|r| &r.0))?;
        Ok(BirdIdResult {
            suggestions,
            region: region.map(|r| r.1),
            elsewhere,
        })
    }

    /// Finds every bird in the photo and suggests each one's species.
    pub fn find_birds(&self, id: PhotoId) -> Result<FoundBirds, BirdError> {
        let known = self.ready()?;
        if let Some(why) = self.detector.unavailable() {
            return Err(BirdError::NotReady(why));
        }
        if !self.detector.available() {
            return Err(BirdError::NotReady(
                "Download the RT-DETR bird finder in Settings → AI models first.".into(),
            ));
        }
        let image = self.load(id)?;
        let size = DETECT_SIZE as usize;
        let outputs = self
            .detector
            .get()
            .map_err(BirdError::Failed)?
            .run(vec![Tensor::f32(
                "pixel_values",
                &[1, 3, size, size],
                detector_input(&image),
            )])
            .map_err(|e| BirdError::Failed(e.to_string()))?;
        let (Some(logits), Some(boxes)) = (
            outputs.first().and_then(Tensor::as_f32),
            outputs.get(1).and_then(Tensor::as_f32),
        ) else {
            return Err(BirdError::Failed("the bird finder returned nothing".into()));
        };
        let mut found = decode_detections(logits, boxes, COCO_CLASSES, COCO_BIRD, DETECT_THRESHOLD);
        found.truncate(MAX_BIRDS);
        let region = self.region(&known)?;
        let birds = found
            .into_iter()
            .map(|d| {
                let (suggestions, elsewhere) =
                    self.classify(&image, d.rect, &known, region.as_ref().map(|r| &r.0))?;
                Ok(FoundBird {
                    area: d.rect.into(),
                    score: d.score,
                    suggestions,
                    elsewhere,
                })
            })
            .collect::<Result<Vec<_>, BirdError>>()?;
        Ok(FoundBirds {
            birds,
            region: region.map(|r| r.1),
        })
    }

    /// Fetches the species list of the region in the settings from eBird
    /// and keeps it.
    pub fn refresh_region(&self) -> Result<BirdRegionView, BirdError> {
        let mut settings = self.settings.load_settings()?;
        let region = settings.bird_id.region.trim().to_owned();
        if region.is_empty() {
            return Err(BirdError::Invalid(
                "Set your region first (Settings → Bird ID).".into(),
            ));
        }
        let key = settings.bird_id.ebird_key.trim().to_owned();
        if key.is_empty() {
            return Err(BirdError::Invalid(
                "Enter your eBird API key first (Settings → Bird ID).".into(),
            ));
        }
        let codes = self
            .lists
            .species_in_region(&region, &key)
            .map_err(BirdError::Failed)?;
        if codes.is_empty() {
            return Err(BirdError::Failed(format!(
                "eBird lists no species for `{region}`."
            )));
        }
        let known = match self.species() {
            Ok(s) => codes.iter().filter(|c| s.by_code.contains_key(*c)).count(),
            // Not downloaded yet: counted once it is.
            Err(_) => 0,
        };
        let view = BirdRegionView {
            region: region.clone(),
            species: codes.len(),
            known,
        };
        settings.bird_region = BirdRegion {
            region,
            species: codes,
        };
        self.settings.save_settings(&settings)?;
        Ok(view)
    }

    /// Keeps the species chosen for a photo (replacing earlier ones; none
    /// forgets them).
    pub fn set_species(&self, id: PhotoId, species: Vec<SpeciesView>) -> Result<(), BirdError> {
        let species: Vec<Species> = species.into_iter().map(Species::from).collect();
        self.catalog.set_bird_species(id, &species)?;
        Ok(())
    }

    /// The chosen species of those photos that have any.
    pub fn photo_species(&self, ids: &[PhotoId]) -> Result<Vec<PhotoSpeciesView>, BirdError> {
        Ok(self
            .catalog
            .bird_species(ids)?
            .iter()
            .map(|(id, s)| PhotoSpeciesView {
                photo_id: id.0,
                species: s.iter().map(SpeciesView::from).collect(),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests;
