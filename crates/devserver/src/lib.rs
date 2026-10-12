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
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use futures_util::Stream;
use futures_util::stream::unfold;
use picturesque_app::api::{Api, ApiError, ApiErrorCode};
use picturesque_app::ports::events::{AppEvent, EventSink};
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

#[derive(Clone)]
struct AppState {
    api: Arc<Api>,
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
pub fn router(api: Arc<Api>, events: EventBus) -> Router {
    Router::new()
        .route("/api/health", get(|| async { "ok" }))
        .route("/api/commands/{name}", post(command))
        .route("/api/events", get(events_stream))
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
        ApiErrorCode::UnknownCommand => StatusCode::NOT_FOUND,
        ApiErrorCode::InvalidArguments => StatusCode::BAD_REQUEST,
        ApiErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Json(err)).into_response()
}
