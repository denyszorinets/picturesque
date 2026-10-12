//! Picturesque use cases and the ports they depend on.
//!
//! Commands exposed to the UI are defined here so that every transport
//! (Tauri IPC, development HTTP) shares one definition.

#![forbid(unsafe_code)]

pub mod api;
pub mod info;
pub mod ports;
pub mod scan;
pub mod settings;
