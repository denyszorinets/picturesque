//! The command API shared by every transport.
//!
//! The UI calls commands by name with JSON arguments. The Tauri shell and
//! the development HTTP server both forward those calls to [`Api::call`],
//! so behaviour is defined once (ADR-0010).

use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use ts_rs::TS;

use crate::import::{Deps, ImportError, ImportService, PlanImportArgs, RunArgs};
use crate::info;
use crate::library::PhotoQuery;
use crate::ports::catalog::PhotoCatalog;
use crate::ports::events::EventSink;
use crate::ports::fs::FileSystem;
use crate::ports::geocoder::ReverseGeocoder;
use crate::ports::images::ImageDecoder;
use crate::ports::imports::ImportStore;
use crate::ports::metadata::MetadataReader;
use crate::ports::settings::SettingsStore;
use crate::settings::{SettingsError, SettingsService, SettingsUpdate};
use crate::templates::{self, PreviewTemplateArgs};
use crate::thumbnails::{ImageSize, PrefillAfterImport, ThumbnailError, ThumbnailService};
use picturesque_core::photo::PhotoId;

/// Category of an API failure. Transports map it to their own error form
/// (for example an HTTP status code).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ApiErrorCode {
    UnknownCommand,
    InvalidArguments,
    /// The thing asked for (for example a photo) does not exist.
    NotFound,
    Internal,
}

/// Error returned by a command, serialized as `{ "code": ..., "message": ... }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ApiError {
    pub code: ApiErrorCode,
    pub message: String,
}

impl ApiError {
    pub fn new(code: ApiErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ApiErrorCode::Internal, message)
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ApiError {}

/// Infrastructure the use cases run on, built by the entry point (desktop
/// shell or development server).
#[derive(Clone)]
pub struct AppContext {
    pub settings_store: Arc<dyn SettingsStore>,
    pub catalog: Arc<dyn PhotoCatalog>,
    pub imports: Arc<dyn ImportStore>,
    pub fs: Arc<dyn FileSystem>,
    pub metadata: Arc<dyn MetadataReader>,
    pub geocoder: Arc<dyn ReverseGeocoder>,
    pub events: Arc<dyn EventSink>,
    pub decoder: Arc<dyn ImageDecoder>,
    /// Folder for cached thumbnails and previews.
    pub cache_dir: PathBuf,
}

impl fmt::Debug for AppContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppContext").finish_non_exhaustive()
    }
}

/// Entry point for UI commands.
///
/// `call` is synchronous; transports run it on a blocking thread so use
/// cases can do file and database I/O directly.
pub struct Api {
    settings: SettingsService,
    imports: ImportService,
    thumbnails: Arc<ThumbnailService>,
    catalog: Arc<dyn PhotoCatalog>,
}

impl fmt::Debug for Api {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Api").finish_non_exhaustive()
    }
}

impl Api {
    /// Fails only if import runs left over from a previous session cannot
    /// be marked interrupted (the catalog is unusable).
    pub fn new(ctx: AppContext) -> Result<Self, ApiError> {
        let thumbnails = Arc::new(ThumbnailService::new(
            Arc::clone(&ctx.catalog),
            Arc::clone(&ctx.settings_store),
            Arc::clone(&ctx.fs),
            ctx.decoder,
            ctx.cache_dir,
        ));
        let events = Arc::new(PrefillAfterImport {
            inner: ctx.events,
            thumbnails: OnceLock::new(),
        });
        let _ = events.thumbnails.set(Arc::clone(&thumbnails));
        let catalog = Arc::clone(&ctx.catalog);
        let imports = ImportService::new(Deps {
            settings: Arc::clone(&ctx.settings_store),
            catalog: ctx.catalog,
            store: ctx.imports,
            fs: Arc::clone(&ctx.fs),
            metadata: ctx.metadata,
            geocoder: ctx.geocoder,
            events,
        })
        .map_err(import_error)?;
        Ok(Self {
            settings: SettingsService::new(ctx.settings_store, ctx.fs),
            imports,
            thumbnails,
            catalog,
        })
    }

    /// Starts background work that should run while the app is open:
    /// generating missing thumbnails. Called by entry points, not tests.
    pub fn start_background_work(&self) {
        self.thumbnails.prefill_in_background();
    }

    /// Resolves an image request path `{size}/{photo_id}` (as used by the
    /// `picturesque://` protocol and `/api/images/`) to the cached file.
    pub fn image_request(&self, path: &str) -> Result<PathBuf, ApiError> {
        let invalid = || {
            ApiError::new(
                ApiErrorCode::InvalidArguments,
                format!("invalid image path `{path}`"),
            )
        };
        let (size, id) = path.trim_matches('/').split_once('/').ok_or_else(invalid)?;
        let size = ImageSize::from_name(size).ok_or_else(invalid)?;
        let id: i64 = id.parse().map_err(|_| invalid())?;
        self.image_path(id, size)
    }

    /// Path of a photo's cached image, generating it if needed. Images are
    /// served as files by each transport rather than through JSON.
    pub fn image_path(&self, photo_id: i64, size: ImageSize) -> Result<PathBuf, ApiError> {
        self.thumbnails
            .image(PhotoId(photo_id), size)
            .map_err(|e| match e {
                ThumbnailError::NotFound(_) => ApiError::new(ApiErrorCode::NotFound, e.to_string()),
                ThumbnailError::NoLibrary => {
                    ApiError::new(ApiErrorCode::InvalidArguments, e.to_string())
                }
                ThumbnailError::Decode(_) | ThumbnailError::Storage(_) => {
                    ApiError::internal(e.to_string())
                }
            })
    }

    /// Waits for a running import, if any (used on shutdown and in tests).
    pub fn wait_for_imports(&self) {
        self.imports.wait();
    }

    /// Runs `command` with JSON `args` and returns its JSON result.
    pub fn call(&self, command: &str, args: Value) -> Result<Value, ApiError> {
        match command {
            "app_info" => {
                no_args(command, &args)?;
                respond(info::app_info())
            }
            "get_settings" => {
                no_args(command, &args)?;
                respond(self.settings.get().map_err(settings_error)?)
            }
            "update_settings" => {
                let update: SettingsUpdate = parse_args(command, args)?;
                respond(self.settings.update(update).map_err(settings_error)?)
            }
            "template_presets" => {
                no_args(command, &args)?;
                respond(templates::presets())
            }
            "preview_template" => {
                let args: PreviewTemplateArgs = parse_args(command, args)?;
                respond(templates::preview(&args.template))
            }
            "query_photos" => {
                let query: PhotoQuery = parse_args(
                    command,
                    if args.is_null() {
                        Value::Object(Default::default())
                    } else {
                        args
                    },
                )?;
                query
                    .validate()
                    .map_err(|m| ApiError::new(ApiErrorCode::InvalidArguments, m))?;
                respond(self.catalog.query(&query).map_err(catalog_error)?)
            }
            "photo_facets" => {
                no_args(command, &args)?;
                respond(self.catalog.facets().map_err(catalog_error)?)
            }
            "plan_import" => {
                let args: PlanImportArgs = parse_args(command, args)?;
                let run_id = self.imports.plan(args.sources).map_err(import_error)?;
                respond(self.imports.status(run_id).map_err(import_error)?)
            }
            "get_import" => {
                let RunArgs { run_id } = parse_args(command, args)?;
                respond(self.imports.plan_view(run_id).map_err(import_error)?)
            }
            "import_status" => {
                let RunArgs { run_id } = parse_args(command, args)?;
                respond(self.imports.status(run_id).map_err(import_error)?)
            }
            "start_import" => {
                let RunArgs { run_id } = parse_args(command, args)?;
                self.imports.start(run_id).map_err(import_error)?;
                respond(self.imports.status(run_id).map_err(import_error)?)
            }
            "cancel_import" => {
                let RunArgs { run_id } = parse_args(command, args)?;
                self.imports.cancel(run_id).map_err(import_error)?;
                respond(self.imports.status(run_id).map_err(import_error)?)
            }
            _ => Err(ApiError::new(
                ApiErrorCode::UnknownCommand,
                format!("unknown command `{command}`"),
            )),
        }
    }
}

fn catalog_error(e: crate::ports::catalog::CatalogError) -> ApiError {
    ApiError::internal(e.to_string())
}

fn import_error(e: ImportError) -> ApiError {
    match e {
        ImportError::Invalid(_) | ImportError::NotFound(_) => {
            ApiError::new(ApiErrorCode::InvalidArguments, e.to_string())
        }
        ImportError::Storage(e) => ApiError::internal(e.to_string()),
    }
}

fn settings_error(e: SettingsError) -> ApiError {
    match e {
        SettingsError::Invalid(msg) => ApiError::new(ApiErrorCode::InvalidArguments, msg),
        SettingsError::Storage(e) => ApiError::internal(e.to_string()),
    }
}

/// Accepts a missing argument (`null`) or an empty object.
fn no_args(command: &str, args: &Value) -> Result<(), ApiError> {
    match args {
        Value::Null => Ok(()),
        Value::Object(map) if map.is_empty() => Ok(()),
        _ => Err(ApiError::new(
            ApiErrorCode::InvalidArguments,
            format!("`{command}` takes no arguments"),
        )),
    }
}

fn parse_args<T: DeserializeOwned>(command: &str, args: Value) -> Result<T, ApiError> {
    serde_json::from_value(args).map_err(|e| {
        ApiError::new(
            ApiErrorCode::InvalidArguments,
            format!("invalid arguments for `{command}`: {e}"),
        )
    })
}

fn respond<T: Serialize>(result: T) -> Result<Value, ApiError> {
    serde_json::to_value(result).map_err(|e| ApiError::internal(format!("serialize result: {e}")))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::testing::{FakeMetadata, MemFs, TestWorld};

    fn api() -> Api {
        Api::new(TestWorld::new(MemFs::new(), FakeMetadata::default()).context()).unwrap()
    }

    #[test]
    fn app_info_returns_name_and_version() {
        let result = api()
            .call("app_info", Value::Null)
            .expect("app_info succeeds");

        assert_eq!(
            result,
            json!({ "name": "Picturesque", "version": env!("CARGO_PKG_VERSION") })
        );
    }

    #[test]
    fn app_info_accepts_empty_object_arguments() {
        assert!(api().call("app_info", json!({})).is_ok());
    }

    #[test]
    fn app_info_rejects_arguments() {
        let err = api()
            .call("app_info", json!({ "unexpected": 1 }))
            .expect_err("arguments are rejected");

        assert_eq!(err.code, ApiErrorCode::InvalidArguments);
    }

    #[test]
    fn unknown_command_is_reported() {
        let err = api()
            .call("does_not_exist", Value::Null)
            .expect_err("unknown command fails");

        assert_eq!(err.code, ApiErrorCode::UnknownCommand);
        assert!(err.message.contains("does_not_exist"));
    }

    #[test]
    fn errors_serialize_with_snake_case_codes() {
        let err = ApiError::new(ApiErrorCode::InvalidArguments, "bad");

        assert_eq!(
            serde_json::to_value(err).expect("serializes"),
            json!({ "code": "invalid_arguments", "message": "bad" })
        );
    }

    #[test]
    fn get_settings_returns_defaults() {
        let result = api().call("get_settings", Value::Null).unwrap();

        assert_eq!(
            result,
            json!({ "library_root": null, "path_template": crate::settings::DEFAULT_PATH_TEMPLATE })
        );
    }

    #[test]
    fn update_settings_applies_and_returns_settings() {
        let api = api();

        let result = api
            .call(
                "update_settings",
                json!({ "path_template": "{year}/{name}.{ext}" }),
            )
            .unwrap();

        assert_eq!(result["path_template"], "{year}/{name}.{ext}");
        assert_eq!(api.call("get_settings", Value::Null).unwrap(), result);
    }

    #[test]
    fn invalid_settings_are_invalid_arguments() {
        let err = api()
            .call(
                "update_settings",
                json!({ "library_root": "relative/path" }),
            )
            .unwrap_err();

        assert_eq!(err.code, ApiErrorCode::InvalidArguments);
        assert!(err.message.contains("absolute"));
    }

    #[test]
    fn unknown_settings_fields_are_rejected() {
        let err = api()
            .call("update_settings", json!({ "colour": "blue" }))
            .unwrap_err();

        assert_eq!(err.code, ApiErrorCode::InvalidArguments);
    }
}
