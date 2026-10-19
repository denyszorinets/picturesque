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
use picturesque_infra::images::VipsDecoder;
use picturesque_infra::metadata::ExifMetadataReader;
use tauri::{AppHandle, Emitter, Manager, RunEvent};

mod commands;
mod images;

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
        .plugin(tauri_plugin_opener::init())
        .register_asynchronous_uri_scheme_protocol("picturesque", |ctx, request, responder| {
            let api = ctx.app_handle().state::<Arc<Api>>().inner().clone();
            let path = request.uri().path().to_owned();
            let etag = request
                .headers()
                .get(tauri::http::header::IF_NONE_MATCH)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            // Rendering may take a while on first request; keep the web view responsive.
            tauri::async_runtime::spawn_blocking(move || {
                responder.respond(images::image_response(&api, &path, etag.as_deref()));
            });
        })
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let catalog = Arc::new(SqliteCatalog::open(&data_dir.join("catalog.sqlite"))?);
            let api = Api::new(AppContext {
                pipeline_store: catalog.clone(),
                watcher: Arc::new(picturesque_infra::watcher::NotifyWatcher::new()),
                settings_store: catalog.clone(),
                catalog,
                fs: Arc::new(StdFileSystem),
                metadata: Arc::new(ExifMetadataReader),
                geocoder: Arc::new(GeoNamesGeocoder::default()),
                events: Arc::new(TauriEvents(app.handle().clone())),
                decoder: Arc::new(VipsDecoder),
                sidecars: Arc::new(picturesque_infra::xmp::XmpSidecars),
                pixels: Arc::new(picturesque_infra::images::VipsPixels),
                analyzers: picturesque_app::analysis::default_analyzers(),
                inference: Arc::new(picturesque_infra::inference::OrtEngine::for_background()),
                models: picturesque_app::models::catalog(),
                model_store: Arc::new(picturesque_infra::models::HttpModelStore::new(
                    data_dir.join("models"),
                )),
                cache_dir: app.path().app_cache_dir()?,
            })?;
            api.start_background_work();
            app.manage(Arc::new(api));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::api])
        .build(tauri::generate_context!())
        .expect("failed to start Picturesque")
        .run(|app, event| {
            // Decisions made in the last moments still reach their sidecars.
            if let RunEvent::Exit = event
                && let Some(api) = app.try_state::<Arc<Api>>()
            {
                api.flush_sidecars();
            }
        });
}
