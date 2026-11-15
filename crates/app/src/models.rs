//! Downloading and managing AI models (`docs/features/ai.rst`).
//!
//! The models the app knows are listed in a manifest with their files'
//! URLs, sizes and checksums, and their licenses. Models are downloaded
//! only when the user asks; once stored they are used offline.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ports::events::{AppEvent, EventSink};
use crate::ports::inference::{InferenceEngine, Model, ModelSource, ProviderChoice};
use crate::ports::models::{FetchError, ModelFile, ModelSpec, ModelStore};
use crate::ports::settings::SettingsStore;

/// How often download progress is sent.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

/// The CLIP image encoder (`analysis::clip`).
pub const CLIP_IMAGE: &str = "clip-image";
/// Its ONNX file inside the model folder.
pub const CLIP_IMAGE_FILE: &str = "visual/model.onnx";

/// The CLIP text encoder matching [`CLIP_IMAGE`] (`search`).
pub const CLIP_TEXT: &str = "clip-text";
pub const CLIP_TEXT_FILE: &str = "textual/model.onnx";
/// The tokenizer's merge list.
pub const CLIP_MERGES_FILE: &str = "textual/merges.txt";

/// Face detection and face landmarks (`analysis::faces`).
pub const FACES: &str = "faces";
pub const FACE_DETECTOR_FILE: &str = "face_detection_yunet_2023mar.onnx";
pub const FACE_MESH_FILE: &str = "face_mesh.onnx";

/// Qwen3-VL 4B Instruct for the built-in critic (`critic`, ADR-0016): the
/// language model and its vision projector, as GGUF for llama.cpp.
pub const CRITIC: &str = "critic-qwen3-vl";
pub const CRITIC_MODEL_FILE: &str = "Qwen3VL-4B-Instruct-Q4_K_M.gguf";
pub const CRITIC_MMPROJ_FILE: &str = "mmproj-Qwen3VL-4B-Instruct-Q8_0.gguf";

/// WingCLIP-0.3 for Bird ID (`birds`): the int8 image encoder (its weights
/// in the `.data` file beside it), the species text embeddings and their
/// names.
pub const BIRDS: &str = "birds-wingclip";
pub const BIRDS_MODEL_FILE: &str = "onnx/wingclip_visual_int8.onnx";
pub const BIRDS_WEIGHTS_FILE: &str = "onnx/wingclip_visual_int8.data";
pub const BIRDS_CLASSIFIER_FILE: &str = "text_classifier_fp32.npy";
pub const BIRDS_LABELS_FILE: &str = "labels.json";

/// RT-DETR R18 (COCO + Objects365), which finds the birds in a photo for
/// Bird ID (`birds`); ONNX export by onnx-community, pinned to a commit.
pub const BIRD_DETECTOR: &str = "bird-detector-rtdetr";
pub const BIRD_DETECTOR_FILE: &str = "onnx/model.onnx";
const BIRD_DETECTOR_REPO: &str = "https://huggingface.co/onnx-community/rtdetr_r18vd_coco_o365/resolve/37122d8b9b89eb8384fba4d688d05051eaf7bb78";

/// WingCLIP-0.3, pinned to a commit.
const BIRDS_REPO: &str =
    "https://huggingface.co/johnlian/WingCLIP-0.3/resolve/fb647cf427ef473fb83610d543dfd70549d4a15c";

/// Qwen's official GGUF files, pinned to a commit.
const CRITIC_REPO: &str = "https://huggingface.co/Qwen/Qwen3-VL-4B-Instruct-GGUF/resolve/1cd86afb9a95c410a6038ab3b40d8b578c892266";

/// Immich's ONNX export of OpenAI CLIP ViT-L/14, pinned to a commit. The
/// improved LAION aesthetic predictor was trained on its embeddings; with
/// the smaller ViT-B/32 the aesthetic score barely beat chance
/// (`spikes/aesthetic/results.md`).
const CLIP_REPO: &str = "https://huggingface.co/immich-app/ViT-L-14__openai/resolve/9b27c6b49d2ff4957fa0b5a25521eee12f109543";

/// The models the app offers. Each feature adds the models it uses; every
/// license must allow use in a GPL-3.0 application (ADR-0006), except
/// WingCLIP, whose non-commercial weights this personal fork accepts
/// (ADR-0018).
pub fn catalog() -> Vec<ModelSpec> {
    let birds = |name: &str, sha256: &str, size: u64| ModelFile {
        name: name.into(),
        url: format!("{BIRDS_REPO}/{name}"),
        sha256: sha256.into(),
        size,
    };
    vec![
        ModelSpec {
            id: BIRD_DETECTOR.into(),
            name: "RT-DETR bird finder".into(),
            purpose: "Finds every bird in a photo for Bird ID, so each can be \
                  identified (Find birds, F). Needs the bird identifier."
                .into(),
            version: 1,
            license: "Apache-2.0".into(),
            license_url: "https://huggingface.co/PekingU/rtdetr_r18vd_coco_o365".into(),
            homepage: "https://github.com/lyuwenyu/RT-DETR".into(),
            files: vec![ModelFile {
                name: BIRD_DETECTOR_FILE.into(),
                url: format!("{BIRD_DETECTOR_REPO}/{BIRD_DETECTOR_FILE}"),
                sha256: "6c74a62251534f427294342508a162a7cd142418b4dd6222d0d0198851724bfa".into(),
                size: 82_572_357,
            }],
        },
        ModelSpec {
            id: BIRDS.into(),
            name: "WingCLIP-0.3 bird identifier".into(),
            purpose: "Suggests the species of a bird you draw a box around \
                  (11,167 species). Non-commercial use only."
                .into(),
            version: 1,
            license: "CC-BY-NC-4.0".into(),
            license_url: "https://huggingface.co/johnlian/WingCLIP-0.3".into(),
            homepage: "https://github.com/jlian/wingdex".into(),
            files: vec![
                birds(
                    BIRDS_MODEL_FILE,
                    "59e19d63aaf531c29aa33f738c22ed5c63c0fdd2e0bd6b91cf63dcb7f9b245d0",
                    14_386_199,
                ),
                birds(
                    BIRDS_WEIGHTS_FILE,
                    "6bfc8c38271437a04351edbbea9478b29431bc155929da0bebcce68b14090d31",
                    25_165_824,
                ),
                birds(
                    BIRDS_CLASSIFIER_FILE,
                    "c0e14f8cac7c81cd8315870bb643f762f688d81781c920e71e400573df5e559d",
                    34_305_152,
                ),
                birds(
                    BIRDS_LABELS_FILE,
                    "ef7de9d34e49e3c7e567910c279a2ff7f5908049a3c67835282abb872673b774",
                    678_140,
                ),
            ],
        },
        ModelSpec {
            id: CLIP_IMAGE.into(),
            name: "CLIP ViT-L/14 image encoder (OpenAI)".into(),
            purpose: "Understands what photos show: scores how good they look \
                  and finds similar photos."
                .into(),
            version: 1,
            license: "MIT".into(),
            license_url: "https://github.com/openai/CLIP/blob/main/LICENSE".into(),
            homepage: "https://github.com/openai/CLIP".into(),
            files: vec![ModelFile {
                name: CLIP_IMAGE_FILE.into(),
                url: format!("{CLIP_REPO}/{CLIP_IMAGE_FILE}"),
                sha256: "2b02d572f59c509f4b97b9c54a868453cca1a652cd5d60e1d51d0052f055cb8c".into(),
                size: 1_216_297_719,
            }],
        },
        ModelSpec {
            id: CLIP_TEXT.into(),
            name: "CLIP ViT-L/14 text encoder (OpenAI)".into(),
            purpose: "Finds photos by describing them, for example \"dog on a beach\". \
                  Needs the image encoder."
                .into(),
            version: 1,
            license: "MIT".into(),
            license_url: "https://github.com/openai/CLIP/blob/main/LICENSE".into(),
            homepage: "https://github.com/openai/CLIP".into(),
            files: vec![
                ModelFile {
                    name: CLIP_TEXT_FILE.into(),
                    url: format!("{CLIP_REPO}/{CLIP_TEXT_FILE}"),
                    sha256: "9fbe72ea8d36c2effaccedcf7249e3729ad0d9b4af6604b433ecdd0105663c9c"
                        .into(),
                    size: 495_082_255,
                },
                ModelFile {
                    name: CLIP_MERGES_FILE.into(),
                    url: format!("{CLIP_REPO}/{CLIP_MERGES_FILE}"),
                    sha256: "9fd691f7c8039210e0fced15865466c65820d09b63988b0174bfe25de299051a"
                        .into(),
                    size: 524_619,
                },
            ],
        },
        ModelSpec {
            id: FACES.into(),
            name: "YuNet face detector and MediaPipe face mesh".into(),
            purpose: "Finds faces and checks whether eyes are closed and faces are sharp.".into(),
            version: 1,
            license: "MIT AND Apache-2.0".into(),
            license_url:
                "https://github.com/opencv/opencv_zoo/blob/main/models/face_detection_yunet/LICENSE"
                    .into(),
            homepage: "https://github.com/opencv/opencv_zoo/tree/main/models/face_detection_yunet"
                .into(),
            files: vec![
                ModelFile {
                    name: FACE_DETECTOR_FILE.into(),
                    url: format!(
                        "https://huggingface.co/opencv/face_detection_yunet/resolve/\
                         3cc26e7f1014a5ee5d74a42acee58bafc9d0a310/{FACE_DETECTOR_FILE}"
                    ),
                    sha256: "8f2383e4dd3cfbb4553ea8718107fc0423210dc964f9f4280604804ed2552fa4"
                        .into(),
                    size: 232_589,
                },
                ModelFile {
                    // MediaPipe Face Landmarker v2 (Apache-2.0), converted to ONNX.
                    name: FACE_MESH_FILE.into(),
                    url: "https://huggingface.co/astaileyyoung/FaceMeshONNX/resolve/\
                          71a6de2112ae66337e098bff6cfcdb3905899207/mesh.onnx"
                        .into(),
                    sha256: "c334c26b128dfc709da0f353c6157ac340b785f5f25cdab5dbea12bbea243172"
                        .into(),
                    size: 5_133_323,
                },
            ],
        },
        ModelSpec {
            id: CRITIC.into(),
            name: "Qwen3-VL 4B (built-in critic)".into(),
            purpose: "Writes critiques of photos: what works and what does not. \
                  Runs on this computer, on the GPU when it can."
                .into(),
            version: 1,
            license: "Apache-2.0".into(),
            license_url: "https://huggingface.co/Qwen/Qwen3-VL-4B-Instruct-GGUF".into(),
            homepage: "https://github.com/QwenLM/Qwen3-VL".into(),
            files: vec![
                ModelFile {
                    name: CRITIC_MODEL_FILE.into(),
                    url: format!("{CRITIC_REPO}/{CRITIC_MODEL_FILE}"),
                    sha256: "66358cb18bb6b3b1b6675aa412c7a88ef01d228f481184d13668e5201c730a0a"
                        .into(),
                    size: 2_497_281_664,
                },
                ModelFile {
                    name: CRITIC_MMPROJ_FILE.into(),
                    url: format!("{CRITIC_REPO}/{CRITIC_MMPROJ_FILE}"),
                    sha256: "30ba2c7dd3127a4561b6cba9d13d0f711c91bdb38742e2f56d73c8cb596bd06d"
                        .into(),
                    size: 453_974_304,
                },
            ],
        },
    ]
}

/// A model loaded on first use and again when the provider setting
/// changes, shared by the threads that run it.
pub struct LoadedModel {
    models: Arc<ModelService>,
    engine: Arc<dyn InferenceEngine>,
    settings: Arc<dyn SettingsStore>,
    id: &'static str,
    file: &'static str,
    loaded: Mutex<Option<(ProviderChoice, Arc<dyn Model>)>>,
}

impl fmt::Debug for LoadedModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoadedModel")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl LoadedModel {
    /// `file` of model `id`.
    pub fn new(
        models: Arc<ModelService>,
        engine: Arc<dyn InferenceEngine>,
        settings: Arc<dyn SettingsStore>,
        id: &'static str,
        file: &'static str,
    ) -> Self {
        Self {
            models,
            engine,
            settings,
            id,
            file,
            loaded: Mutex::new(None),
        }
    }

    /// Whether the model is downloaded and this machine can run models.
    pub fn available(&self) -> bool {
        self.models.files(self.id).is_some() && self.unavailable().is_none()
    }

    /// Why no model can run on this machine, or `None`.
    pub fn unavailable(&self) -> Option<String> {
        self.engine.unavailable()
    }

    /// The paths of the model's files, if downloaded.
    pub fn files(&self) -> Option<HashMap<String, PathBuf>> {
        self.models.files(self.id)
    }

    pub fn get(&self) -> Result<Arc<dyn Model>, String> {
        let choice = self
            .settings
            .load_settings()
            .map_err(|e| e.to_string())?
            .ai
            .provider;
        let mut loaded = self.loaded.lock().expect("model lock");
        if let Some((c, model)) = loaded.as_ref()
            && *c == choice
        {
            return Ok(Arc::clone(model));
        }
        let files = self
            .models
            .files(self.id)
            .ok_or_else(|| format!("the model `{}` is not downloaded", self.id))?;
        let path = files
            .get(self.file)
            .ok_or_else(|| format!("the model `{}` has no `{}`", self.id, self.file))?;
        let model: Arc<dyn Model> = Arc::from(
            self.engine
                .load(ModelSource::File(path), choice)
                .map_err(|e| e.to_string())?,
        );
        *loaded = Some((choice, Arc::clone(&model)));
        Ok(model)
    }
}

/// A model's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ModelState {
    NotInstalled,
    Downloading,
    Installed,
    Failed,
}

/// A model as the UI shows it; returned by `list_models` and sent as
/// `model_progress`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ModelView {
    pub id: String,
    pub name: String,
    pub purpose: String,
    pub version: u32,
    pub license: String,
    pub license_url: String,
    pub homepage: String,
    /// Total download size in bytes.
    #[ts(type = "number")]
    pub size: u64,
    pub state: ModelState,
    /// Bytes downloaded so far (while downloading).
    #[ts(type = "number")]
    pub downloaded: u64,
    /// Why the last download failed.
    pub error: Option<String>,
}

/// Arguments of `install_model`, `remove_model` and `cancel_model`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct ModelArgs {
    pub id: String,
}

/// Why a model command failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    Unknown(String),
    /// Removing while downloading, for example.
    Busy(String),
    Storage(String),
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(id) => write!(f, "There is no model `{id}`."),
            Self::Busy(msg) | Self::Storage(msg) => f.write_str(msg),
        }
    }
}

#[derive(Debug, Default, Clone)]
struct Download {
    downloaded: u64,
    cancel: bool,
}

/// Lists, downloads and removes models.
pub struct ModelService {
    specs: Vec<ModelSpec>,
    store: Arc<dyn ModelStore>,
    events: Arc<dyn EventSink>,
    downloads: Mutex<HashMap<String, Download>>,
    failures: Mutex<HashMap<String, String>>,
}

impl fmt::Debug for ModelService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModelService").finish_non_exhaustive()
    }
}

impl ModelService {
    pub fn new(
        specs: Vec<ModelSpec>,
        store: Arc<dyn ModelStore>,
        events: Arc<dyn EventSink>,
    ) -> Self {
        Self {
            specs,
            store,
            events,
            downloads: Mutex::new(HashMap::new()),
            failures: Mutex::new(HashMap::new()),
        }
    }

    fn spec(&self, id: &str) -> Result<&ModelSpec, ModelError> {
        self.specs
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| ModelError::Unknown(id.to_owned()))
    }

    fn installed(&self, spec: &ModelSpec) -> bool {
        spec.files.iter().all(|f| self.store.has(spec, f))
    }

    fn view(&self, spec: &ModelSpec) -> ModelView {
        let download = self
            .downloads
            .lock()
            .expect("downloads lock")
            .get(&spec.id)
            .cloned();
        let error = self
            .failures
            .lock()
            .expect("failures lock")
            .get(&spec.id)
            .cloned();
        let state = if download.is_some() {
            ModelState::Downloading
        } else if self.installed(spec) {
            ModelState::Installed
        } else if error.is_some() {
            ModelState::Failed
        } else {
            ModelState::NotInstalled
        };
        ModelView {
            id: spec.id.clone(),
            name: spec.name.clone(),
            purpose: spec.purpose.clone(),
            version: spec.version,
            license: spec.license.clone(),
            license_url: spec.license_url.clone(),
            homepage: spec.homepage.clone(),
            size: spec.size(),
            state,
            downloaded: download.map_or(0, |d| d.downloaded),
            error: if state == ModelState::Failed {
                error
            } else {
                None
            },
        }
    }

    pub fn list(&self) -> Vec<ModelView> {
        self.specs.iter().map(|s| self.view(s)).collect()
    }

    /// The stored files of an installed model, by file name; `None` when
    /// it is not installed (or still downloading).
    pub fn files(&self, id: &str) -> Option<HashMap<String, PathBuf>> {
        let spec = self.spec(id).ok()?;
        if self
            .downloads
            .lock()
            .expect("downloads lock")
            .contains_key(id)
            || !self.installed(spec)
        {
            return None;
        }
        Some(
            spec.files
                .iter()
                .map(|f| (f.name.clone(), self.store.path(spec, f)))
                .collect(),
        )
    }

    /// The contents of an installed model's file.
    pub fn read(&self, id: &str, file: &str) -> Result<Vec<u8>, ModelError> {
        let spec = self.spec(id)?;
        let file = spec
            .files
            .iter()
            .find(|f| f.name == file)
            .ok_or_else(|| ModelError::Unknown(format!("{id}/{file}")))?;
        self.store.read(spec, file).map_err(ModelError::Storage)
    }

    /// Starts downloading a model in the background; progress and the
    /// outcome are sent as `model_progress`.
    pub fn install(self: &Arc<Self>, id: &str) -> Result<ModelView, ModelError> {
        let spec = self.spec(id)?.clone();
        {
            let mut downloads = self.downloads.lock().expect("downloads lock");
            if !downloads.contains_key(id) && !self.installed(&spec) {
                downloads.insert(spec.id.clone(), Download::default());
                self.failures.lock().expect("failures lock").remove(id);
                let service = Arc::clone(self);
                std::thread::spawn(move || service.download(&spec));
            }
        }
        Ok(self.view(self.spec(id)?))
    }

    /// Asks a running download to stop.
    pub fn cancel(&self, id: &str) -> Result<ModelView, ModelError> {
        let spec = self.spec(id)?;
        if let Some(d) = self.downloads.lock().expect("downloads lock").get_mut(id) {
            d.cancel = true;
        }
        Ok(self.view(spec))
    }

    /// Deletes a model's files.
    pub fn remove(&self, id: &str) -> Result<ModelView, ModelError> {
        let spec = self.spec(id)?;
        if self
            .downloads
            .lock()
            .expect("downloads lock")
            .contains_key(id)
        {
            return Err(ModelError::Busy(format!(
                "{} is downloading; cancel the download first.",
                spec.name
            )));
        }
        self.store.remove(spec).map_err(ModelError::Storage)?;
        self.failures.lock().expect("failures lock").remove(id);
        let view = self.view(spec);
        self.events.emit(&AppEvent::ModelProgress(view.clone()));
        Ok(view)
    }

    /// Runs on a background thread.
    fn download(&self, spec: &ModelSpec) {
        let result = self.fetch_all(spec);
        self.downloads
            .lock()
            .expect("downloads lock")
            .remove(&spec.id);
        if let Err(e) = result
            && e != FetchError::Cancelled
        {
            self.failures
                .lock()
                .expect("failures lock")
                .insert(spec.id.clone(), e.to_string());
        }
        self.events.emit(&AppEvent::ModelProgress(self.view(spec)));
    }

    fn fetch_all(&self, spec: &ModelSpec) -> Result<(), FetchError> {
        let mut done = 0;
        let mut last_sent = Instant::now();
        for file in &spec.files {
            if !self.store.has(spec, file) {
                let mut progress = |received: u64| {
                    let mut downloads = self.downloads.lock().expect("downloads lock");
                    let Some(d) = downloads.get_mut(&spec.id) else {
                        return false;
                    };
                    d.downloaded = done + received;
                    let cancel = d.cancel;
                    drop(downloads);
                    if last_sent.elapsed() >= PROGRESS_INTERVAL {
                        last_sent = Instant::now();
                        self.events.emit(&AppEvent::ModelProgress(self.view(spec)));
                    }
                    !cancel
                };
                self.store.fetch(spec, file, &mut progress)?;
            }
            done += file.size;
        }
        self.store.prune(spec).map_err(FetchError::Io)
    }
}

#[cfg(test)]
mod tests;
