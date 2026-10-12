//! The command API shared by every transport.
//!
//! The UI calls commands by name with JSON arguments. The Tauri shell and
//! the development HTTP server both forward those calls to [`Api::call`],
//! so behaviour is defined once (ADR-0010).

use std::fmt;
use std::sync::Arc;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use ts_rs::TS;

use crate::info;
use crate::ports::fs::FileSystem;
use crate::ports::settings::SettingsStore;
use crate::settings::{SettingsError, SettingsService, SettingsUpdate};

/// Category of an API failure. Transports map it to their own error form
/// (for example an HTTP status code).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ApiErrorCode {
    UnknownCommand,
    InvalidArguments,
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
    pub fs: Arc<dyn FileSystem>,
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
#[derive(Debug)]
pub struct Api {
    settings: SettingsService,
}

impl Api {
    pub fn new(ctx: AppContext) -> Self {
        Self {
            settings: SettingsService::new(ctx.settings_store, ctx.fs),
        }
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
            _ => Err(ApiError::new(
                ApiErrorCode::UnknownCommand,
                format!("unknown command `{command}`"),
            )),
        }
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
    use crate::settings::tests::{FakeFs, MemorySettings};

    fn api() -> Api {
        Api::new(AppContext {
            settings_store: Arc::new(MemorySettings::default()),
            fs: Arc::new(FakeFs::default()),
        })
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
            json!({ "library_root": null, "path_template": "{year}/{date}/{name}.{ext}" })
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
