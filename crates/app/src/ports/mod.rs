//! Capabilities use cases need from the outside world. Implementations
//! live in `picturesque-infra` (ADR-0003).

pub mod catalog;
pub mod critic;
pub mod events;
pub mod fs;
pub mod geocoder;
pub mod images;
pub mod inference;
pub mod metadata;
pub mod models;
pub mod pipeline;
pub mod pixels;
pub mod settings;
pub mod sidecars;
pub mod watcher;
