//! `SettingsStore` on SQLite: the settings document under one key.

use picturesque_app::ports::catalog::CatalogError;
use picturesque_app::ports::settings::SettingsStore;
use picturesque_app::settings::Settings;
use rusqlite::OptionalExtension;

use super::{SqliteCatalog, storage};

const KEY: &str = "settings";

impl SettingsStore for SqliteCatalog {
    fn load_settings(&self) -> Result<Settings, CatalogError> {
        let json: Option<String> = self
            .conn()
            .query_row("SELECT value FROM setting WHERE key = ?1", [KEY], |row| {
                row.get(0)
            })
            .optional()
            .map_err(storage)?;
        match json {
            None => Ok(Settings::default()),
            Some(json) => serde_json::from_str(&json)
                .map_err(|e| storage(format!("stored settings are invalid: {e}"))),
        }
    }

    fn save_settings(&self, settings: &Settings) -> Result<(), CatalogError> {
        let json = serde_json::to_string(settings).map_err(storage)?;
        self.conn()
            .execute(
                "INSERT INTO setting (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                [KEY, json.as_str()],
            )
            .map_err(storage)?;
        Ok(())
    }
}
