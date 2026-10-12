//! The command API shared by every transport.
//!
//! The UI calls commands by name with JSON arguments. The Tauri shell and
//! the development HTTP server both forward those calls to [`Api::call`],
//! so behaviour is defined once (ADR-0010).

use std::fmt;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use ts_rs::TS;

use crate::info;

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

/// Entry point for UI commands.
///
/// `call` is synchronous; transports run it on a blocking thread so use
/// cases can do file and database I/O directly.
#[derive(Debug, Default)]
pub struct Api {}

impl Api {
    pub fn new() -> Self {
        Self {}
    }

    /// Runs `command` with JSON `args` and returns its JSON result.
    pub fn call(&self, command: &str, args: Value) -> Result<Value, ApiError> {
        match command {
            "app_info" => {
                no_args(command, &args)?;
                respond(info::app_info())
            }
            _ => Err(ApiError::new(
                ApiErrorCode::UnknownCommand,
                format!("unknown command `{command}`"),
            )),
        }
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

#[expect(dead_code, reason = "used by the first command that takes arguments")]
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

    #[test]
    fn app_info_returns_name_and_version() {
        let result = Api::new()
            .call("app_info", Value::Null)
            .expect("app_info succeeds");

        assert_eq!(
            result,
            json!({ "name": "Picturesque", "version": env!("CARGO_PKG_VERSION") })
        );
    }

    #[test]
    fn app_info_accepts_empty_object_arguments() {
        assert!(Api::new().call("app_info", json!({})).is_ok());
    }

    #[test]
    fn app_info_rejects_arguments() {
        let err = Api::new()
            .call("app_info", json!({ "unexpected": 1 }))
            .expect_err("arguments are rejected");

        assert_eq!(err.code, ApiErrorCode::InvalidArguments);
    }

    #[test]
    fn unknown_command_is_reported() {
        let err = Api::new()
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
}
