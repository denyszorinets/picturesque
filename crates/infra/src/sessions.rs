//! Where folder sessions keep their catalogs (ADR-0013), and the context
//! each session runs on, shared by the desktop app and the development
//! server.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use picturesque_app::api::AppContext;
use picturesque_app::ports::events::EventSink;
use picturesque_app::ports::geocoder::ReverseGeocoder;
use picturesque_app::ports::inference::InferenceEngine;
use picturesque_app::ports::models::{ModelSpec, ModelStore};
use picturesque_app::sessions::ContextFactory;

use crate::catalog::SqliteCatalog;

/// The folder holding session catalogs, inside the app data folder.
const SESSIONS_DIR: &str = "sessions";

/// The file name of `folder`'s session catalog: a hash of its path, so any
/// folder name works and the same folder always gets the same catalog.
pub fn session_file(folder: &Path) -> String {
    let hash = blake3::hash(folder.to_string_lossy().as_bytes());
    format!("{}.sqlite", &hash.to_hex()[..16])
}

/// Builds the library's and folder sessions' contexts with the standard
/// adapters. Services that are global (AI models, inference, geocoding)
/// are shared by all sessions.
pub struct StandardSessions {
    data_dir: PathBuf,
    cache_dir: PathBuf,
    library: Arc<SqliteCatalog>,
    events: Arc<dyn EventSink>,
    models: Vec<ModelSpec>,
    model_store: Arc<dyn ModelStore>,
    inference: Arc<dyn InferenceEngine>,
    geocoder: Arc<dyn ReverseGeocoder>,
}

impl std::fmt::Debug for StandardSessions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StandardSessions")
            .field("data_dir", &self.data_dir)
            .finish_non_exhaustive()
    }
}

impl StandardSessions {
    /// `library` is the catalog in `data_dir` that also stores settings.
    pub fn new(
        data_dir: PathBuf,
        cache_dir: PathBuf,
        library: Arc<SqliteCatalog>,
        events: Arc<dyn EventSink>,
        models: Vec<ModelSpec>,
    ) -> Self {
        Self {
            model_store: Arc::new(crate::models::HttpModelStore::new(data_dir.join("models"))),
            inference: Arc::new(crate::inference::OrtEngine::for_background()),
            geocoder: Arc::new(crate::geocode::GeoNamesGeocoder::default()),
            data_dir,
            cache_dir,
            library,
            events,
            models,
        }
    }

    fn sessions_dir(&self) -> PathBuf {
        self.data_dir.join(SESSIONS_DIR)
    }
}

impl ContextFactory for StandardSessions {
    fn context(&self, folder: Option<&Path>) -> Result<AppContext, String> {
        let catalog = match folder {
            None => Arc::clone(&self.library),
            Some(folder) => {
                let dir = self.sessions_dir();
                std::fs::create_dir_all(&dir)
                    .map_err(|e| format!("create {}: {e}", dir.display()))?;
                Arc::new(
                    SqliteCatalog::open(&dir.join(session_file(folder)))
                        .map_err(|e| e.to_string())?,
                )
            }
        };
        Ok(AppContext {
            pipeline_store: catalog.clone(),
            settings_store: self.library.clone(),
            catalog,
            watcher: Arc::new(crate::watcher::NotifyWatcher::new()),
            critic: Arc::new(crate::critic::CriticAdapter::new()),
            power: Arc::new(crate::power::SystemPower),
            fs: Arc::new(crate::fs::StdFileSystem),
            metadata: Arc::new(crate::metadata::ExifMetadataReader),
            geocoder: Arc::clone(&self.geocoder),
            events: Arc::clone(&self.events),
            decoder: Arc::new(crate::images::VipsDecoder),
            sidecars: Arc::new(crate::xmp::XmpSidecars),
            pixels: Arc::new(crate::images::VipsPixels),
            analyzers: picturesque_app::analysis::default_analyzers(),
            inference: Arc::clone(&self.inference),
            models: self.models.clone(),
            model_store: Arc::clone(&self.model_store),
            cache_dir: self.cache_dir.clone(),
        })
    }

    fn prune(&self, keep: &[PathBuf]) {
        let keep: Vec<String> = keep
            .iter()
            .map(|f| session_file(f).trim_end_matches(".sqlite").to_owned())
            .collect();
        let Ok(entries) = std::fs::read_dir(self.sessions_dir()) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            // The catalog and SQLite's -wal and -shm files.
            let stem = name.split('.').next().unwrap_or_default();
            if !keep.iter().any(|k| k == stem) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_folder_has_its_own_stable_catalog_file() {
        let a = session_file(Path::new("/photos/2026/trip"));
        assert_eq!(a, session_file(Path::new("/photos/2026/trip")));
        assert_ne!(a, session_file(Path::new("/photos/2026/trip2")));
        assert!(a.ends_with(".sqlite") && a.len() == 23, "{a}");
    }

    #[test]
    fn prune_deletes_only_forgotten_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let library = Arc::new(SqliteCatalog::open_in_memory().unwrap());
        let sessions = StandardSessions::new(
            dir.path().into(),
            dir.path().join("cache"),
            library,
            Arc::new(picturesque_app::ports::events::NoEvents),
            Vec::new(),
        );
        let kept = Path::new("/photos/kept");
        let gone = Path::new("/photos/gone");
        sessions.context(Some(kept)).unwrap();
        sessions.context(Some(gone)).unwrap();
        let sessions_dir = dir.path().join(SESSIONS_DIR);
        std::fs::write(
            sessions_dir.join(format!("{}-wal", session_file(gone))),
            b"",
        )
        .unwrap();

        sessions.prune(&[kept.to_path_buf()]);

        let left: Vec<String> = std::fs::read_dir(&sessions_dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        let kept_stem = session_file(kept).replace(".sqlite", "");
        assert!(left.contains(&session_file(kept)), "{left:?}");
        assert!(left.iter().all(|n| n.starts_with(&kept_stem)), "{left:?}");
    }
}
