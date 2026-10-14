//! Library settings: where the Clean library lives and how it is organized.

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use picturesque_core::ranking::RankWeights;
use picturesque_core::series::SeriesThresholds;
use picturesque_core::template::PathTemplate;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ports::catalog::{CatalogError, PhotoCatalog};
use crate::ports::fs::FileSystem;
use crate::ports::inference::ProviderChoice;
use crate::ports::settings::SettingsStore;

/// Folder structure used until the user picks another template (the first
/// preset): `2026/2026-10-12/IMG_0042.CR3`.
pub const DEFAULT_PATH_TEMPLATE: &str = picturesque_core::template::PRESETS[0].template;

/// User settings. Missing fields in stored settings take their defaults,
/// so new settings can be added without a migration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct Settings {
    /// Root folder of the Clean library; unset until the user chooses one.
    #[ts(type = "string | null")]
    pub library_root: Option<PathBuf>,
    /// Template for paths inside the library.
    pub path_template: String,
    /// How new XMP sidecars are named.
    pub sidecar_naming: SidecarNaming,
    /// When photos form a series.
    pub series: SeriesSettings,
    /// How the best photo of a series is chosen.
    pub ranking: RankingSettings,
    /// Local AI models.
    pub ai: AiSettings,
    /// Watched import folders.
    pub import: ImportSettings,
}

/// Watched import folders (`docs/features/import.rst`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct ImportSettings {
    /// How often watched folders are scanned again, in minutes, 0–1440;
    /// 0 rescans only when the folder reports a change.
    pub rescan_minutes: u32,
}

impl Default for ImportSettings {
    fn default() -> Self {
        Self { rescan_minutes: 5 }
    }
}

/// Local AI settings (`docs/features/ai.rst`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct AiSettings {
    /// Hardware models run on.
    pub provider: ProviderChoice,
}

/// How much each score counts when choosing the best photo of a series
/// (`docs/features/series.rst`). Only the ratios matter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct RankingSettings {
    pub sharpness: f64,
    pub exposure: f64,
    pub noise: f64,
    /// The aesthetic score (when the CLIP model ran).
    pub aesthetic: f64,
}

impl Default for RankingSettings {
    fn default() -> Self {
        // RankWeights::default(), written as f64 so it serializes exactly.
        Self {
            sharpness: 0.6,
            exposure: 0.3,
            noise: 0.1,
            aesthetic: 0.4,
        }
    }
}

impl RankingSettings {
    pub fn weights(self) -> RankWeights {
        RankWeights {
            sharpness: self.sharpness as f32,
            exposure: self.exposure as f32,
            noise: self.noise as f32,
            aesthetic: self.aesthetic as f32,
        }
    }
}

/// When photos form a series (`docs/features/series.rst`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct SeriesSettings {
    /// Longest pause between consecutive shots of a series, 1–3600 s.
    pub max_gap_seconds: u32,
    /// Most perceptual-hash bits a photo may differ from the series'
    /// latest photos, 0–32.
    pub max_distance: u32,
}

impl Default for SeriesSettings {
    fn default() -> Self {
        let d = SeriesThresholds::default();
        Self {
            max_gap_seconds: d.max_gap_seconds,
            max_distance: d.max_distance,
        }
    }
}

impl SeriesSettings {
    pub fn thresholds(self) -> SeriesThresholds {
        SeriesThresholds {
            max_gap_seconds: self.max_gap_seconds,
            max_distance: self.max_distance,
        }
    }
}

/// How new XMP sidecars are named. Existing sidecars are always updated in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SidecarNaming {
    /// `IMG_0042.CR3.xmp`: read by darktable and digiKam.
    #[default]
    FileExtension,
    /// `IMG_0042.xmp`: read by Lightroom and Bridge.
    BaseName,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            library_root: None,
            path_template: DEFAULT_PATH_TEMPLATE.to_owned(),
            sidecar_naming: SidecarNaming::default(),
            series: SeriesSettings::default(),
            ranking: RankingSettings::default(),
            ai: AiSettings::default(),
            import: ImportSettings::default(),
        }
    }
}

/// A change to some settings; absent fields are left as they are.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, TS)]
#[serde(default, deny_unknown_fields)]
#[ts(export)]
pub struct SettingsUpdate {
    #[ts(type = "string | null", optional)]
    pub library_root: Option<PathBuf>,
    #[ts(optional)]
    pub path_template: Option<String>,
    #[ts(optional)]
    pub sidecar_naming: Option<SidecarNaming>,
    #[ts(optional)]
    pub series: Option<SeriesSettings>,
    #[ts(optional)]
    pub ranking: Option<RankingSettings>,
    #[ts(optional)]
    pub ai: Option<AiSettings>,
    #[ts(optional)]
    pub import: Option<ImportSettings>,
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
    catalog: Arc<dyn PhotoCatalog>,
}

impl fmt::Debug for SettingsService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SettingsService").finish_non_exhaustive()
    }
}

impl SettingsService {
    pub fn new(
        store: Arc<dyn SettingsStore>,
        fs: Arc<dyn FileSystem>,
        catalog: Arc<dyn PhotoCatalog>,
    ) -> Self {
        Self { store, fs, catalog }
    }

    pub fn get(&self) -> Result<Settings, SettingsError> {
        Ok(self.store.load_settings()?)
    }

    /// Validates and saves `update`, returning the resulting settings.
    /// Nothing is saved if any field is invalid.
    pub fn update(&self, update: SettingsUpdate) -> Result<Settings, SettingsError> {
        let mut settings = self.store.load_settings()?;
        if let Some(root) = update.library_root {
            // Photo paths are relative to the library folder: pointing it
            // elsewhere would break every photo already in the catalog.
            if let Some(current) = &settings.library_root
                && *current != root
            {
                let photos = self.catalog.photo_count()?;
                if photos > 0 {
                    return Err(SettingsError::Invalid(format!(
                        "The library in `{}` already has {photos} photos; choosing a different library folder is not supported yet.",
                        current.display()
                    )));
                }
            }
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
            let parsed = PathTemplate::parse(&template)
                .map_err(|e| SettingsError::Invalid(e.to_string()))?;
            settings.path_template = parsed.as_str().to_owned();
        }
        if let Some(naming) = update.sidecar_naming {
            settings.sidecar_naming = naming;
        }
        if let Some(series) = update.series {
            if !(1..=3600).contains(&series.max_gap_seconds) {
                return Err(SettingsError::Invalid(format!(
                    "The longest pause in a series must be 1–3600 seconds, got {}.",
                    series.max_gap_seconds
                )));
            }
            if series.max_distance > 32 {
                return Err(SettingsError::Invalid(format!(
                    "The similarity limit for series must be 0–32, got {}.",
                    series.max_distance
                )));
            }
            settings.series = series;
        }
        if let Some(ranking) = update.ranking {
            if !ranking.weights().is_valid() {
                return Err(SettingsError::Invalid(
                    "Ranking weights must be zero or more, and at least one must be above zero."
                        .into(),
                ));
            }
            settings.ranking = ranking;
        }
        if let Some(ai) = update.ai {
            settings.ai = ai;
        }
        if let Some(import) = update.import {
            if import.rescan_minutes > 1440 {
                return Err(SettingsError::Invalid(format!(
                    "Watched folders must be rescanned at least once a day (0–1440 minutes), got {}.",
                    import.rescan_minutes
                )));
            }
            settings.import = import;
        }
        self.store.save_settings(&settings)?;
        Ok(settings)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::testing::{MemFs, MemoryCatalog};

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

    fn abs(p: &str) -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(format!("C:\\{p}"))
        } else {
            PathBuf::from(format!("/{p}"))
        }
    }

    fn service(writable: &[&str]) -> (SettingsService, Arc<MemorySettings>) {
        let store = Arc::new(MemorySettings::default());
        let fs = writable
            .iter()
            .fold(MemFs::new(), |fs, p| fs.dir(abs(p).to_str().unwrap()));
        (
            SettingsService::new(
                store.clone(),
                Arc::new(fs),
                Arc::new(MemoryCatalog::default()),
            ),
            store,
        )
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
    fn rejects_a_read_only_library_root() {
        let store = Arc::new(MemorySettings::default());
        let fs = MemFs::new()
            .dir(abs("ro").to_str().unwrap())
            .read_only(abs("ro").to_str().unwrap());
        let service = SettingsService::new(store, Arc::new(fs), Arc::new(MemoryCatalog::default()));

        let err = service
            .update(SettingsUpdate {
                library_root: Some(abs("ro")),
                ..Default::default()
            })
            .unwrap_err();

        assert!(
            matches!(&err, SettingsError::Invalid(m) if m.contains("not writable")),
            "{err}"
        );
    }

    #[test]
    fn the_library_folder_cannot_change_once_it_has_photos() {
        use picturesque_core::hash::ContentHash;
        use picturesque_core::media::MediaFormat;
        use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets};

        let store = Arc::new(MemorySettings::default());
        let fs = MemFs::new()
            .dir(abs("a").to_str().unwrap())
            .dir(abs("b").to_str().unwrap());
        let catalog = Arc::new(MemoryCatalog::default());
        let service = SettingsService::new(store, Arc::new(fs), catalog.clone());
        service
            .update(SettingsUpdate {
                library_root: Some(abs("a")),
                ..Default::default()
            })
            .unwrap();
        // While empty, it can still change.
        service
            .update(SettingsUpdate {
                library_root: Some(abs("b")),
                ..Default::default()
            })
            .unwrap();
        catalog
            .insert_photo(&NewPhoto {
                metadata: Default::default(),
                assets: PhotoAssets::new(vec![Asset {
                    format: MediaFormat::Jpeg,
                    path: "x.jpg".into(),
                    size: 1,
                    hash: ContentHash::from_bytes([1; 32]),
                }])
                .unwrap(),
            })
            .unwrap();

        let err = service
            .update(SettingsUpdate {
                library_root: Some(abs("a")),
                ..Default::default()
            })
            .unwrap_err();
        assert!(
            matches!(&err, SettingsError::Invalid(m) if m.contains("1 photos")),
            "{err}"
        );
        // Saving the same folder again is fine.
        assert!(
            service
                .update(SettingsUpdate {
                    library_root: Some(abs("b")),
                    ..Default::default()
                })
                .is_ok()
        );
    }

    #[test]
    fn invalid_update_saves_nothing() {
        let (service, store) = service(&["photos"]);

        let result = service.update(SettingsUpdate {
            library_root: Some(abs("photos")),
            path_template: Some("   ".into()),
            ..Default::default()
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
    fn rejects_an_invalid_template_with_the_parser_message() {
        let (service, _) = service(&[]);

        let err = service
            .update(SettingsUpdate {
                path_template: Some("{year}/{name}".into()),
                ..Default::default()
            })
            .unwrap_err();

        assert!(
            matches!(&err, SettingsError::Invalid(m) if m.contains(".{ext}")),
            "{err}"
        );
    }

    #[test]
    fn default_template_is_valid() {
        assert!(PathTemplate::parse(DEFAULT_PATH_TEMPLATE).is_ok());
    }

    #[test]
    fn stored_settings_missing_fields_take_defaults() {
        let settings: Settings = serde_json::from_str("{}").unwrap();

        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn saves_valid_series_thresholds_and_rejects_others() {
        let (service, store) = service(&[]);
        let series = SeriesSettings {
            max_gap_seconds: 90,
            max_distance: 12,
        };

        let saved = service
            .update(SettingsUpdate {
                series: Some(series),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(saved.series, series);
        assert_eq!(
            store
                .load_settings()
                .unwrap()
                .series
                .thresholds()
                .max_gap_seconds,
            90
        );

        for bad in [
            SeriesSettings {
                max_gap_seconds: 0,
                max_distance: 10,
            },
            SeriesSettings {
                max_gap_seconds: 3601,
                max_distance: 10,
            },
            SeriesSettings {
                max_gap_seconds: 30,
                max_distance: 33,
            },
        ] {
            let err = service
                .update(SettingsUpdate {
                    series: Some(bad),
                    ..Default::default()
                })
                .unwrap_err();
            assert!(matches!(err, SettingsError::Invalid(_)), "{bad:?}");
        }
        assert_eq!(store.load_settings().unwrap().series, series);
    }

    #[test]
    fn partial_series_settings_take_defaults() {
        let settings: Settings =
            serde_json::from_str(r#"{"series": {"max_gap_seconds": 5}}"#).unwrap();
        assert_eq!(settings.series.max_gap_seconds, 5);
        assert_eq!(
            settings.series.max_distance,
            SeriesSettings::default().max_distance
        );
    }

    #[test]
    fn rescan_interval_is_validated() {
        let (service, _) = service(&[]);
        assert_eq!(service.get().unwrap().import.rescan_minutes, 5);
        for (minutes, ok) in [(0, true), (1440, true), (1441, false)] {
            let result = service.update(SettingsUpdate {
                import: Some(ImportSettings {
                    rescan_minutes: minutes,
                }),
                ..Default::default()
            });
            assert_eq!(result.is_ok(), ok, "{minutes}");
        }
        assert_eq!(service.get().unwrap().import.rescan_minutes, 1440);
    }
}
