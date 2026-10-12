//! Reading and writing decisions in XMP sidecars (ADR-0009).

use std::path::Path;

use picturesque_core::decision::Decision;
use picturesque_core::hash::ContentHash;

pub trait SidecarStore: Send + Sync {
    /// The decision recorded in a sidecar by any tool; `None` if it has none.
    fn read(&self, path: &Path) -> Result<Option<Decision>, String>;

    /// Records `decision` in the sidecar at `path`, creating it or merging
    /// into it (other tools' data is kept). Returns the new file's hash and
    /// size. A sidecar that cannot be parsed is left untouched (error).
    fn write(&self, path: &Path, decision: Decision) -> Result<(ContentHash, u64), String>;
}
