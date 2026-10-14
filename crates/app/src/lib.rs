//! Picturesque use cases and the ports they depend on.
//!
//! Commands exposed to the UI are defined here so that every transport
//! (Tauri IPC, development HTTP) shares one definition.

#![forbid(unsafe_code)]

pub mod ai;
pub mod analysis;
pub mod api;
pub mod embeddings;
pub mod explain;
pub mod export;
pub mod hooks;
pub mod info;
pub mod library;
pub mod models;
pub mod pipeline;
pub mod ports;
pub mod rejects;
pub mod scan;
pub mod search;
pub mod series;
pub mod settings;
pub mod sidecars;
pub mod templates;
pub mod thumbnails;

#[cfg(test)]
mod testing;
