//! Tauri adapter over the shared command API.

use std::sync::Arc;

use picturesque_app::api::ApiError;
use picturesque_app::sessions::Host;
use serde_json::Value;

/// Forwards a UI command to [`Host::call`] on a blocking thread.
#[tauri::command]
pub async fn api(
    host: tauri::State<'_, Arc<Host>>,
    command: String,
    args: Option<Value>,
) -> Result<Value, ApiError> {
    let host = Arc::clone(&host);
    tauri::async_runtime::spawn_blocking(move || host.call(&command, args.unwrap_or(Value::Null)))
        .await
        .map_err(|e| ApiError::internal(format!("command task failed: {e}")))?
}
