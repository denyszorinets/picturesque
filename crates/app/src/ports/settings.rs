//! Persistent application settings.

use crate::ports::catalog::CatalogError;
use crate::settings::Settings;

/// Loads and saves [`Settings`]. Stored in the catalog database.
pub trait SettingsStore: Send + Sync {
    /// Saved settings, or defaults when nothing has been saved yet.
    fn load_settings(&self) -> Result<Settings, CatalogError>;

    fn save_settings(&self, settings: &Settings) -> Result<(), CatalogError>;
}
