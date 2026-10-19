//! Picturesque infrastructure: implementations of the ports defined in
//! `picturesque-app` (catalog, decoding, inference, geocoding, sidecars,
//! file system).
//!
//! Types from native libraries and frameworks stay inside this crate.

pub mod catalog;
pub mod fs;
pub mod geocode;
pub mod images;
pub mod inference;
pub mod metadata;
pub mod models;
pub mod sessions;
pub mod watcher;
pub mod xmp;
