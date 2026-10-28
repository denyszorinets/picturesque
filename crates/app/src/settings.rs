//! Library settings: where the Clean library lives and how it is organized.

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use picturesque_core::photo::PairView;
use picturesque_core::ranking::RankWeights;
use picturesque_core::series::SeriesThresholds;
use picturesque_core::template::PathTemplate;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ports::catalog::{CatalogError, PhotoCatalog};
use crate::ports::fs::FileSystem;
use crate::ports::inference::ProviderChoice;
use crate::ports::settings::SettingsStore;
use crate::ports::sidecars::ColorLabel;

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
    /// Which photos the app marks as keepers.
    pub auto_pick: AutoPickSettings,
    /// How much the user's own taste counts in the aesthetic score.
    pub taste: TasteSettings,
    /// Photo critiques from a local vision-language model.
    pub critic: CriticSettings,
    /// What the user has already been shown.
    pub onboarding: OnboardingSettings,
    /// The folder being culled in place, reopened at startup; `None` while
    /// working in the library (ADR-0013). In a folder session,
    /// `library_root` is this folder.
    #[ts(type = "string | null")]
    pub open_folder: Option<PathBuf>,
    /// Folders culled in place, most recently opened first.
    pub recent_folders: Vec<RecentFolder>,
    /// Colour labels written to sidecars.
    pub labels: LabelSettings,
    /// What clearing a folder after copying may delete (ADR-0014).
    pub delete_after_copy: DeleteAfterCopy,
    /// Which file of a RAW+JPEG pair the grid and loupe show.
    #[ts(type = "\"image\" | \"raw\"")]
    pub pair_view: PairView,
}

/// What **Clear folder…** may delete after keepers were copied out of a
/// folder culled in place (ADR-0014).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DeleteAfterCopy {
    /// Nothing: clearing is not offered.
    Never,
    /// Photos whose copy was verified (and is verified again first).
    #[default]
    Verified,
    /// Those, and photos you rejected.
    VerifiedAndRejects,
}

/// Colour labels written to XMP sidecars, so tools without a pick flag
/// (darktable) show picks (`docs/reference/settings.rst`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct LabelSettings {
    /// For picks; `None` writes no label.
    pub pick: Option<ColorLabel>,
    /// For auto-picked photos, written when a folder is finished.
    pub auto_pick: Option<ColorLabel>,
}

impl Default for LabelSettings {
    fn default() -> Self {
        Self {
            pick: Some(ColorLabel::Green),
            auto_pick: Some(ColorLabel::Yellow),
        }
    }
}

/// A folder culled in place, for the list of recent folders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RecentFolder {
    #[ts(type = "string")]
    pub path: PathBuf,
    /// When it was last opened, in seconds since the Unix epoch.
    #[ts(type = "number")]
    pub opened_at: i64,
}

/// First-run guidance the user has seen (`docs/getting-started`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct OnboardingSettings {
    /// The culling tour was finished or skipped.
    pub tour_done: bool,
}

/// Auto-pick (`docs/features/library.rst`): the best photo of each series,
/// or a photo in none, is suggested as a keeper when its combined quality
/// is at least `min_quality` and nobody has closed eyes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct AutoPickSettings {
    pub enabled: bool,
    /// 0–1.
    pub min_quality: f64,
}

impl Default for AutoPickSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            min_quality: 0.5,
        }
    }
}

/// The personal aesthetic model (`docs/features/ai.rst`), learned from
/// the user's picks and rejects.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct TasteSettings {
    /// How much the personal score counts in the aesthetic score once
    /// learned, 0–1: 0 uses only the general score, 1 only the personal one.
    pub personal_weight: f64,
}

impl Default for TasteSettings {
    fn default() -> Self {
        Self {
            personal_weight: 0.5,
        }
    }
}

/// Photo critiques from a local vision-language model (ADR-0016,
/// `docs/features/ai.rst`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct CriticSettings {
    pub enabled: bool,
    /// Which model writes critiques.
    pub backend: CriticEngine,
    /// Base address of an OpenAI-compatible API (`server` backend).
    pub server: String,
    pub model: String,
    /// What the model judges; `null` uses the default rubric.
    pub rubric: Option<String>,
    /// Critique the whole library in the background.
    pub background: bool,
    /// How hard the background critic may use the computer.
    pub intensity: CriticIntensity,
    /// How much critiques count in the aesthetic score, 0–1.
    pub score_weight: f64,
}

/// How hard the background critic may use the computer
/// (`docs/features/ai.rst`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CriticIntensity {
    /// Low priority and half the processor; pauses while importing or
    /// analyzing, and on battery power.
    #[default]
    Gentle,
    /// Full speed, but only after a few minutes without using the app.
    WhenIdle,
    /// Full speed whenever enabled.
    Full,
}

/// Where critiques come from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CriticEngine {
    /// The downloaded Qwen3-VL model, run by the app's helper.
    #[default]
    Builtin,
    /// A local server the user runs (Ollama, llama.cpp, LM Studio).
    Server,
}

impl Default for CriticSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            backend: CriticEngine::Builtin,
            server: crate::critic::DEFAULT_SERVER.into(),
            model: crate::critic::DEFAULT_MODEL.into(),
            rubric: None,
            background: false,
            intensity: CriticIntensity::Gentle,
            score_weight: 0.25,
        }
    }
}

impl CriticSettings {
    /// Longest rubric accepted.
    pub const MAX_RUBRIC: usize = 4_000;

    /// The rubric in use.
    pub fn rubric(&self) -> &str {
        self.rubric
            .as_deref()
            .filter(|r| !r.trim().is_empty())
            .unwrap_or(crate::critic::DEFAULT_RUBRIC)
    }

    fn validate(&self) -> Result<(), String> {
        if !(self.server.starts_with("http://") || self.server.starts_with("https://")) {
            return Err(format!(
                "The critic's server must be an http:// or https:// address, got `{}`.",
                self.server
            ));
        }
        if self.model.trim().is_empty() {
            return Err("Choose a model for critiques.".into());
        }
        if !(0.0..=1.0).contains(&self.score_weight) {
            return Err(format!(
                "The critique's share of the aesthetic score must be 0–100%, got {}%.",
                (self.score_weight * 100.0).round()
            ));
        }
        if self
            .rubric
            .as_ref()
            .is_some_and(|r| r.chars().count() > Self::MAX_RUBRIC)
        {
            return Err(format!(
                "The rubric is too long: at most {} characters.",
                Self::MAX_RUBRIC
            ));
        }
        Ok(())
    }
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
            auto_pick: AutoPickSettings::default(),
            taste: TasteSettings::default(),
            critic: CriticSettings::default(),
            onboarding: OnboardingSettings::default(),
            open_folder: None,
            recent_folders: Vec::new(),
            labels: LabelSettings::default(),
            delete_after_copy: DeleteAfterCopy::default(),
            pair_view: PairView::default(),
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
    #[ts(optional)]
    pub auto_pick: Option<AutoPickSettings>,
    #[ts(optional)]
    pub taste: Option<TasteSettings>,
    #[ts(optional)]
    pub critic: Option<CriticSettings>,
    #[ts(optional)]
    pub onboarding: Option<OnboardingSettings>,
    #[ts(optional)]
    pub labels: Option<LabelSettings>,
    #[ts(optional)]
    pub delete_after_copy: Option<DeleteAfterCopy>,
    #[ts(type = "\"image\" | \"raw\"", optional)]
    pub pair_view: Option<PairView>,
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
            if !crate::ports::fs::is_absolute(&root) {
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
        if let Some(auto_pick) = update.auto_pick {
            if !(0.0..=1.0).contains(&auto_pick.min_quality) {
                return Err(SettingsError::Invalid(format!(
                    "The auto-pick quality must be 0–100%, got {}%.",
                    (auto_pick.min_quality * 100.0).round()
                )));
            }
            settings.auto_pick = auto_pick;
        }
        if let Some(taste) = update.taste {
            if !(0.0..=1.0).contains(&taste.personal_weight) {
                return Err(SettingsError::Invalid(format!(
                    "Your taste's share of the aesthetic score must be 0–100%, got {}%.",
                    (taste.personal_weight * 100.0).round()
                )));
            }
            settings.taste = taste;
        }
        if let Some(critic) = update.critic {
            critic.validate().map_err(SettingsError::Invalid)?;
            settings.critic = critic;
        }
        if let Some(onboarding) = update.onboarding {
            settings.onboarding = onboarding;
        }
        if let Some(labels) = update.labels {
            settings.labels = labels;
        }
        if let Some(mode) = update.delete_after_copy {
            settings.delete_after_copy = mode;
        }
        if let Some(view) = update.pair_view {
            settings.pair_view = view;
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
    fn the_personal_weight_is_a_share() {
        let (service, _) = service(&[]);
        let update = |personal_weight| {
            service.update(SettingsUpdate {
                taste: Some(TasteSettings { personal_weight }),
                ..Default::default()
            })
        };

        assert_eq!(update(1.0).unwrap().taste.personal_weight, 1.0);
        let err = update(1.5).unwrap_err();
        assert!(
            matches!(&err, SettingsError::Invalid(m) if m.contains("150%")),
            "{err}"
        );
    }

    #[test]
    fn critic_settings_are_validated() {
        let (service, _) = service(&[]);
        let update = |critic: CriticSettings| {
            service.update(SettingsUpdate {
                critic: Some(critic),
                ..Default::default()
            })
        };
        let ok = CriticSettings {
            enabled: true,
            rubric: Some("Light only.".into()),
            ..Default::default()
        };
        assert_eq!(update(ok.clone()).unwrap().critic.rubric(), "Light only.");
        for (bad, message) in [
            (
                CriticSettings {
                    server: "ftp://host".into(),
                    ..ok.clone()
                },
                "http://",
            ),
            (
                CriticSettings {
                    model: " ".into(),
                    ..ok.clone()
                },
                "Choose a model",
            ),
            (
                CriticSettings {
                    rubric: Some("x".repeat(CriticSettings::MAX_RUBRIC + 1)),
                    ..ok.clone()
                },
                "too long",
            ),
        ] {
            let err = update(bad).unwrap_err();
            assert!(
                matches!(&err, SettingsError::Invalid(m) if m.contains(message)),
                "{err}"
            );
        }
        let err = update(CriticSettings {
            score_weight: 2.0,
            ..ok.clone()
        })
        .unwrap_err();
        assert!(
            matches!(&err, SettingsError::Invalid(m) if m.contains("200%")),
            "{err}"
        );
        // An empty rubric means the default one.
        let blank = CriticSettings {
            rubric: Some("  ".into()),
            ..ok
        };
        assert_eq!(blank.rubric(), crate::critic::DEFAULT_RUBRIC);
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

    #[test]
    fn remembers_that_the_culling_tour_was_seen() {
        let (service, _) = service(&[]);
        assert!(!service.get().unwrap().onboarding.tour_done);

        let saved = service
            .update(SettingsUpdate {
                onboarding: Some(OnboardingSettings { tour_done: true }),
                ..Default::default()
            })
            .unwrap();
        assert!(saved.onboarding.tour_done);
        assert!(service.get().unwrap().onboarding.tour_done);
    }
}
