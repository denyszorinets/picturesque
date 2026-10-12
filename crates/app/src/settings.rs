//! Library settings: where the Clean library lives and how it is organized.

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ports::catalog::CatalogError;
use crate::ports::fs::FileSystem;
use crate::ports::settings::SettingsStore;

/// Folder structure used until the user picks another template:
/// `2026/2026-10-12/IMG_0042.CR3`.
pub const DEFAULT_PATH_TEMPLATE: &str = "{year}/{date}/{name}.{ext}";

/// User settings. Missing fields in stored settings take their defaults,
/// so new settings can be added without a migration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct Settings {
    /// Root folder of the Clean library; unset until the user chooses one.
    #[ts(type = "string | null")]
    pub library_root: Option<PathBuf>,
    /// Template for paths inside the library.
    pub path_template: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            library_root: None,
            path_template: DEFAULT_PATH_TEMPLATE.to_owned(),
        }
    }
}

/// A change to some settings; absent fields are left as they are.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, TS)]
#[serde(default, deny_unknown_fields)]
#[ts(export)]
pub struct SettingsUpdate {
    #[ts(type = "string | null", optional)]
    pub library_root: Option<PathBuf>,
    #[ts(optional)]
    pub path_template: Option<String>,
}

/// Why settings could not be read or changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsError {
    /// The requested change is not valid; the message says why and what to do.
    Invalid(String),
    Storage(CatalogError),
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(msg) => f.write_str(msg),
            Self::Storage(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for SettingsError {}

impl From<CatalogError> for SettingsError {
    fn from(e: CatalogError) -> Self {
        Self::Storage(e)
    }
}

/// Reads and validates changes to [`Settings`].
pub struct SettingsService {
    store: Arc<dyn SettingsStore>,
    fs: Arc<dyn FileSystem>,
}

impl fmt::Debug for SettingsService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SettingsService").finish_non_exhaustive()
    }
}

impl SettingsService {
    pub fn new(store: Arc<dyn SettingsStore>, fs: Arc<dyn FileSystem>) -> Self {
        Self { store, fs }
    }

    pub fn get(&self) -> Result<Settings, SettingsError> {
        Ok(self.store.load_settings()?)
    }

    /// Validates and saves `update`, returning the resulting settings.
    /// Nothing is saved if any field is invalid.
    pub fn update(&self, update: SettingsUpdate) -> Result<Settings, SettingsError> {
        let mut settings = self.store.load_settings()?;
        if let Some(root) = update.library_root {
            if !root.is_absolute() {
                return Err(SettingsError::Invalid(format!(
                    "The library folder must be an absolute path, got `{}`.",
                    root.display()
                )));
            }
            self.fs.probe_writable_dir(&root).map_err(|reason| {
                SettingsError::Invalid(format!(
                    "Cannot use `{}` as the library folder: {reason}. Choose an existing folder you can write to.",
                    root.display()
                ))
            })?;
            settings.library_root = Some(root);
        }
        if let Some(template) = update.path_template {
            let template = template.trim();
            if template.is_empty() {
                return Err(SettingsError::Invalid(
                    "The path template cannot be empty. Try the default, `{year}/{date}/{name}.{ext}`.".into(),
                ));
            }
            settings.path_template = template.to_owned();
        }
        self.store.save_settings(&settings)?;
        Ok(settings)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::path::Path;
    use std::sync::Mutex;

    use super::*;

    /// Settings kept in memory.
    #[derive(Default)]
    pub struct MemorySettings(pub Mutex<Option<Settings>>);

    impl SettingsStore for MemorySettings {
        fn load_settings(&self) -> Result<Settings, CatalogError> {
            Ok(self.0.lock().unwrap().clone().unwrap_or_default())
        }

        fn save_settings(&self, settings: &Settings) -> Result<(), CatalogError> {
            *self.0.lock().unwrap() = Some(settings.clone());
            Ok(())
        }
    }

    /// Treats only the listed directories as writable.
    #[derive(Default)]
    pub struct FakeFs {
        pub writable: Vec<PathBuf>,
    }

    impl FileSystem for FakeFs {
        fn probe_writable_dir(&self, path: &Path) -> Result<(), String> {
            if self.writable.iter().any(|w| w == path) {
                Ok(())
            } else {
                Err("it does not exist".into())
            }
        }

        fn list_dir(&self, _: &Path) -> Result<Vec<crate::ports::fs::DirEntry>, String> {
            Err("it does not exist".into())
        }
    }

    fn abs(p: &str) -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(format!("C:\\{p}"))
        } else {
            PathBuf::from(format!("/{p}"))
        }
    }

    fn service(writable: &[&str]) -> (SettingsService, Arc<MemorySettings>) {
        let store = Arc::new(MemorySettings::default());
        let fs = Arc::new(FakeFs {
            writable: writable.iter().map(|p| abs(p)).collect(),
        });
        (SettingsService::new(store.clone(), fs), store)
    }

    #[test]
    fn defaults_when_nothing_is_saved() {
        let (service, _) = service(&[]);

        assert_eq!(service.get().unwrap(), Settings::default());
        assert_eq!(service.get().unwrap().path_template, DEFAULT_PATH_TEMPLATE);
    }

    #[test]
    fn saves_a_valid_library_root() {
        let (service, store) = service(&["photos"]);

        let settings = service
            .update(SettingsUpdate {
                library_root: Some(abs("photos")),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(settings.library_root, Some(abs("photos")));
        assert_eq!(store.load_settings().unwrap(), settings);
    }

    #[test]
    fn rejects_a_relative_library_root() {
        let (service, _) = service(&[]);

        let err = service
            .update(SettingsUpdate {
                library_root: Some("photos".into()),
                ..Default::default()
            })
            .unwrap_err();

        assert!(
            matches!(&err, SettingsError::Invalid(m) if m.contains("absolute")),
            "{err}"
        );
    }

    #[test]
    fn rejects_an_unusable_library_root_with_the_reason() {
        let (service, _) = service(&[]);

        let err = service
            .update(SettingsUpdate {
                library_root: Some(abs("missing")),
                ..Default::default()
            })
            .unwrap_err();

        assert!(
            matches!(&err, SettingsError::Invalid(m) if m.contains("does not exist")),
            "{err}"
        );
    }

    #[test]
    fn invalid_update_saves_nothing() {
        let (service, store) = service(&["photos"]);

        let result = service.update(SettingsUpdate {
            library_root: Some(abs("photos")),
            path_template: Some("   ".into()),
        });

        assert!(result.is_err());
        assert_eq!(store.load_settings().unwrap(), Settings::default());
    }

    #[test]
    fn updates_only_the_given_fields() {
        let (service, _) = service(&["photos"]);
        service
            .update(SettingsUpdate {
                library_root: Some(abs("photos")),
                ..Default::default()
            })
            .unwrap();

        let settings = service
            .update(SettingsUpdate {
                path_template: Some(" {year}/{name}.{ext} ".into()),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(settings.library_root, Some(abs("photos")));
        assert_eq!(settings.path_template, "{year}/{name}.{ext}");
    }

    #[test]
    fn stored_settings_missing_fields_take_defaults() {
        let settings: Settings = serde_json::from_str("{}").unwrap();

        assert_eq!(settings, Settings::default());
    }
}
