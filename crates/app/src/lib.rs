//! Picturesque use cases and the ports they depend on.
//!
//! Commands exposed to the UI are defined here so that every transport
//! (Tauri IPC, development HTTP) shares one definition.

#![forbid(unsafe_code)]

pub mod ai;
pub mod analysis;
pub mod api;
pub mod hooks;
pub mod import;
pub mod info;
pub mod library;
pub mod ports;
pub mod scan;
pub mod series;
pub mod settings;
pub mod sidecars;
pub mod templates;
pub mod thumbnails;

#[cfg(test)]
mod testing;
