//! Development-only HTTP transport for the Picturesque UI.
//!
//! Exposes the same command API as the Tauri shell so the real UI can run
//! in a browser where no display is available (ADR-0010). Never shipped to
//! users; binds to loopback only.

#![forbid(unsafe_code)]

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use picturesque_app::api::{Api, ApiError, ApiErrorCode};
use serde_json::Value;

/// Builds the HTTP routes:
///
/// - `GET /api/health` returns `ok`.
/// - `POST /api/commands/{name}` runs a command; the body is its JSON
///   arguments (empty for none). Success returns the JSON result; failure
///   returns an [`ApiError`] with a matching status code.
pub fn router(api: Arc<Api>) -> Router {
    Router::new()
        .route("/api/health", get(|| async { "ok" }))
        .route("/api/commands/{name}", post(command))
        .with_state(api)
}

async fn command(State(api): State<Arc<Api>>, Path(name): Path<String>, body: Bytes) -> Response {
    let args = if body.is_empty() {
        Value::Null
    } else {
        match serde_json::from_slice(&body) {
            Ok(args) => args,
            Err(e) => {
                return error_response(ApiError::new(
                    ApiErrorCode::InvalidArguments,
                    format!("request body is not valid JSON: {e}"),
                ));
            }
        }
    };

    let result = tokio::task::spawn_blocking(move || api.call(&name, args))
        .await
        .unwrap_or_else(|e| Err(ApiError::internal(format!("command task failed: {e}"))));

    match result {
        Ok(value) => Json(value).into_response(),
        Err(err) => error_response(err),
    }
}

fn error_response(err: ApiError) -> Response {
    let status = match err.code {
        ApiErrorCode::UnknownCommand => StatusCode::NOT_FOUND,
        ApiErrorCode::InvalidArguments => StatusCode::BAD_REQUEST,
        ApiErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Json(err)).into_response()
}
