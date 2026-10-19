//! Reading and writing decisions in XMP sidecars (ADR-0009).

use std::path::Path;

use picturesque_core::decision::Decision;
use picturesque_core::hash::ContentHash;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A colour label, as darktable, Lightroom and digiKam show them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ColorLabel {
    Red,
    Yellow,
    Green,
    Blue,
    Purple,
}

impl ColorLabel {
    pub const ALL: [Self; 5] = [
        Self::Red,
        Self::Yellow,
        Self::Green,
        Self::Blue,
        Self::Purple,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Yellow => "yellow",
            Self::Green => "green",
            Self::Blue => "blue",
            Self::Purple => "purple",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.as_str() == name)
    }
}

/// What Picturesque records in a sidecar: the decision, and the colour
/// label it sets (`None` removes a label it set before; labels set in
/// other tools are kept).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SidecarValues {
    pub decision: Decision,
    pub label: Option<ColorLabel>,
}

pub trait SidecarStore: Send + Sync {
    /// The decision recorded in a sidecar by any tool; `None` if it has none.
    fn read(&self, path: &Path) -> Result<Option<Decision>, String>;

    /// Records `values` in the sidecar at `path`, creating it or merging
    /// into it (other tools' data is kept). Returns the new file's hash and
    /// size. A sidecar that cannot be parsed is left untouched (error).
    fn write(&self, path: &Path, values: SidecarValues) -> Result<(ContentHash, u64), String>;
}
