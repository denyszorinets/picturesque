//! Tauri adapter over the shared command API.

use std::sync::Arc;

use picturesque_app::api::{Api, ApiError};
use serde_json::Value;

/// Forwards a UI command to [`Api::call`] on a blocking thread.
#[tauri::command]
pub async fn api(
    api: tauri::State<'_, Arc<Api>>,
    command: String,
    args: Option<Value>,
) -> Result<Value, ApiError> {
    let api = Arc::clone(&api);
    tauri::async_runtime::spawn_blocking(move || api.call(&command, args.unwrap_or(Value::Null)))
        .await
        .map_err(|e| ApiError::internal(format!("command task failed: {e}")))?
}
