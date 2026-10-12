//! Picturesque desktop shell: wires the application layer into Tauri.
//!
//! The UI calls every command through the single `api` Tauri command,
//! which forwards to `picturesque_app::api::Api` (ADR-0010).

use std::sync::Arc;

use picturesque_app::api::{Api, AppContext};
use picturesque_infra::catalog::SqliteCatalog;
use picturesque_infra::fs::StdFileSystem;
use tauri::Manager;

mod commands;

/// Builds and runs the Tauri application.
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let catalog = Arc::new(SqliteCatalog::open(&data_dir.join("catalog.sqlite"))?);
            app.manage(Arc::new(Api::new(AppContext {
                settings_store: catalog,
                fs: Arc::new(StdFileSystem),
            })));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::api])
        .run(tauri::generate_context!())
        .expect("failed to start Picturesque");
}
