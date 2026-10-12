//! Picturesque domain model and pure logic.
//!
//! This crate has no I/O and no framework dependencies. See
//! `docs/architecture/components.rst` for the layer rules.

#![forbid(unsafe_code)]

pub mod decision;
pub mod grouping;
pub mod hash;
pub mod media;
pub mod metadata;
pub mod photo;
pub mod pixels;
pub mod quality;
pub mod similarity;
pub mod template;
