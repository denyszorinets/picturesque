//! Where Bird ID learns which species live in a region
//! (`docs/features/bird-id.rst`): eBird, with the user's own API key. This
//! is the only network access besides model downloads, and only when the
//! user asks for a region's list.

/// Lists the species of a region.
pub trait SpeciesLists: Send + Sync {
    /// eBird species codes ever seen in `region` (an eBird region code such
    /// as `US`, `US-WA` or `US-WA-033`), asked with the user's `api_key`.
    fn species_in_region(&self, region: &str, api_key: &str) -> Result<Vec<String>, String>;
}

/// Knows no region (tests, tools).
#[derive(Debug, Default, Clone, Copy)]
pub struct NoSpeciesLists;

impl SpeciesLists for NoSpeciesLists {
    fn species_in_region(&self, _region: &str, _api_key: &str) -> Result<Vec<String>, String> {
        Err("species lists are not available here".into())
    }
}
