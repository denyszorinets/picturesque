//! Picturesque domain model and pure logic.
//!
//! This crate has no I/O and no framework dependencies. See
//! `docs/architecture/components.rst` for the layer rules.

#![forbid(unsafe_code)]

pub mod base64;
pub mod clip;
pub mod clip_tokenizer;
pub mod critic_protocol;
pub mod critique;
pub mod decision;
pub mod faces;
pub mod grouping;
pub mod hash;
pub mod insights;
pub mod media;
pub mod metadata;
pub mod photo;
pub mod pixels;
pub mod quality;
pub mod ranking;
pub mod series;
pub mod similarity;
pub mod taste;
pub mod template;
