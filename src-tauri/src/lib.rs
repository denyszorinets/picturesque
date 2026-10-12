//! Picturesque desktop shell: wires the application layer into Tauri.
//!
//! The UI calls every command through the single `api` Tauri command,
//! which forwards to `picturesque_app::api::Api`, and receives backend
//! events as the `app-event` Tauri event (ADR-0010).

use std::sync::Arc;

use picturesque_app::api::{Api, AppContext};
use picturesque_app::ports::events::{AppEvent, EventSink};
use picturesque_infra::catalog::SqliteCatalog;
use picturesque_infra::fs::StdFileSystem;
use picturesque_infra::geocode::GeoNamesGeocoder;
use picturesque_infra::metadata::ExifMetadataReader;
use tauri::{AppHandle, Emitter, Manager};

mod commands;

/// Name of the Tauri event carrying [`AppEvent`]s.
pub const EVENT_NAME: &str = "app-event";

struct TauriEvents(AppHandle);

impl EventSink for TauriEvents {
    fn emit(&self, event: &AppEvent) {
        let _ = self.0.emit(EVENT_NAME, event);
    }
}

/// Builds and runs the Tauri application.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let catalog = Arc::new(SqliteCatalog::open(&data_dir.join("catalog.sqlite"))?);
            let api = Api::new(AppContext {
                settings_store: catalog.clone(),
                catalog: catalog.clone(),
                imports: catalog,
                fs: Arc::new(StdFileSystem),
                metadata: Arc::new(ExifMetadataReader),
                geocoder: Arc::new(GeoNamesGeocoder::default()),
                events: Arc::new(TauriEvents(app.handle().clone())),
            })?;
            app.manage(Arc::new(api));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::api])
        .run(tauri::generate_context!())
        .expect("failed to start Picturesque");
}
