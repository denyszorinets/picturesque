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
use crate::ports::models::{FetchError, ModelFile, ModelSpec, ModelStore};

/// How often download progress is sent.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

/// The CLIP image encoder (`analysis::clip`).
pub const CLIP_IMAGE: &str = "clip-image";
/// Its ONNX file inside the model folder.
pub const CLIP_IMAGE_FILE: &str = "visual/model.onnx";

/// Immich's ONNX export of OpenAI CLIP ViT-L/14, pinned to a commit. The
/// improved LAION aesthetic predictor was trained on its embeddings; with
/// the smaller ViT-B/32 the aesthetic score barely beat chance
/// (`spikes/aesthetic/results.md`).
const CLIP_REPO: &str = "https://huggingface.co/immich-app/ViT-L-14__openai/resolve/9b27c6b49d2ff4957fa0b5a25521eee12f109543";

/// The models the app offers. Each feature adds the models it uses; every
/// license must allow use in a GPL-3.0 application (ADR-0006).
pub fn catalog() -> Vec<ModelSpec> {
    vec![ModelSpec {
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
    }]
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
