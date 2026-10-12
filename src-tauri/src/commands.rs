//! Tauri command adapters over `picturesque-app` use cases.

use picturesque_app::info::AppInfo;

#[tauri::command]
pub fn app_info() -> AppInfo {
    picturesque_app::info::app_info()
}
