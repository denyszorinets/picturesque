//! Picturesque desktop shell: wires the application layer into Tauri.
//!
//! Command handlers only adapt arguments and results; behaviour lives in
//! `picturesque-app`.

mod commands;

/// Builds and runs the Tauri application.
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::app_info])
        .run(tauri::generate_context!())
        .expect("failed to start Picturesque");
}
