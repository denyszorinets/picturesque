//! Development-only HTTP transport for the Picturesque UI.
//!
//! Exposes the same command API as the Tauri shell so the real UI can run
//! in a browser where no display is available (ADR-0010). Never shipped to
//! users; binds to loopback only.

#![forbid(unsafe_code)]

use std::convert::Infallible;
use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use futures_util::Stream;
use futures_util::stream::unfold;
use std::path::PathBuf;

use picturesque_app::api::{Api, ApiError, ApiErrorCode};
use picturesque_app::ports::events::{AppEvent, EventSink};
use picturesque_app::sessions::Host;
use serde_json::Value;
use tokio::sync::broadcast;

/// Fans backend events out to every connected `/api/events` client.
#[derive(Debug, Clone)]
pub struct EventBus {
    sender: broadcast::Sender<String>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self {
            sender: broadcast::channel(256).0,
        }
    }
}

impl EventBus {
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.sender.subscribe()
    }
}

impl EventSink for EventBus {
    fn emit(&self, event: &AppEvent) {
        if let Ok(json) = serde_json::to_string(event) {
            // No subscribers is fine: nobody is listening.
            let _ = self.sender.send(json);
        }
    }
}

/// What the server runs commands on: an [`Api`], or a [`Host`] that
/// switches between folder sessions.
pub trait Backend: Send + Sync + 'static {
    fn call(&self, command: &str, args: Value) -> Result<Value, ApiError>;
    fn image_request(&self, path: &str) -> Result<PathBuf, ApiError>;
}

impl Backend for Api {
    fn call(&self, command: &str, args: Value) -> Result<Value, ApiError> {
        Api::call(self, command, args)
    }

    fn image_request(&self, path: &str) -> Result<PathBuf, ApiError> {
        Api::image_request(self, path)
    }
}

impl Backend for Host {
    fn call(&self, command: &str, args: Value) -> Result<Value, ApiError> {
        Host::call(self, command, args)
    }

    fn image_request(&self, path: &str) -> Result<PathBuf, ApiError> {
        self.api().image_request(path)
    }
}

#[derive(Clone)]
struct AppState {
    api: Arc<dyn Backend>,
    events: EventBus,
}

/// Builds the HTTP routes:
///
/// - `GET /api/health` returns `ok`.
/// - `POST /api/commands/{name}` runs a command; the body is its JSON
///   arguments (empty for none). Success returns the JSON result; failure
///   returns an [`ApiError`] with a matching status code.
/// - `GET /api/events` streams backend events as server-sent events, one
///   JSON [`AppEvent`] per `data:` line.
/// - `GET /api/images/{size}/{photo_id}` returns a photo's cached JPEG
///   (`size` is `grid` or `preview`), with an `ETag` for revalidation.
pub fn router(api: Arc<dyn Backend>, events: EventBus) -> Router {
    Router::new()
        .route("/api/health", get(|| async { "ok" }))
        .route("/api/commands/{name}", post(command))
        .route("/api/events", get(events_stream))
        .route("/api/images/{size}/{id}", get(image))
        .with_state(AppState { api, events })
}

async fn command(State(state): State<AppState>, Path(name): Path<String>, body: Bytes) -> Response {
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

    let api = state.api;
    let result = tokio::task::spawn_blocking(move || api.call(&name, args))
        .await
        .unwrap_or_else(|e| Err(ApiError::internal(format!("command task failed: {e}"))));

    match result {
        Ok(value) => Json(value).into_response(),
        Err(err) => error_response(err),
    }
}

async fn image(
    State(state): State<AppState>,
    Path((size, id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let api = state.api;
    let path = format!("{size}/{id}");
    let result = tokio::task::spawn_blocking(move || {
        let file = api.image_request(&path)?;
        let bytes = std::fs::read(&file)
            .map_err(|e| ApiError::internal(format!("read {}: {e}", file.display())))?;
        Ok::<_, ApiError>((file, bytes))
    })
    .await
    .unwrap_or_else(|e| Err(ApiError::internal(format!("image task failed: {e}"))));

    let (file, bytes) = match result {
        Ok(found) => found,
        Err(err) => return error_response(err),
    };
    // Cached files are named after the content hash, which makes a strong ETag.
    let etag = format!(
        "\"{}\"",
        file.file_stem()
            .map(|s| s.to_string_lossy())
            .unwrap_or_default()
    );
    let cache = [
        (header::ETAG, etag.clone()),
        (header::CACHE_CONTROL, "private, no-cache".to_owned()),
    ];
    if headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        == Some(etag.as_str())
    {
        return (StatusCode::NOT_MODIFIED, cache).into_response();
    }
    (cache, [(header::CONTENT_TYPE, "image/jpeg")], bytes).into_response()
}

async fn events_stream(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = unfold(state.events.subscribe(), |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(json) => return Some((Ok(Event::default().data(json)), rx)),
                // A slow client missed some events; progress events are
                // snapshots, so continue with the next one.
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

fn error_response(err: ApiError) -> Response {
    let status = match err.code {
        ApiErrorCode::UnknownCommand | ApiErrorCode::NotFound => StatusCode::NOT_FOUND,
        ApiErrorCode::InvalidArguments => StatusCode::BAD_REQUEST,
        ApiErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Json(err)).into_response()
}
