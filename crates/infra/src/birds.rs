//! eBird's regional species lists for Bird ID (`docs/features/bird-id.rst`),
//! asked with the user's own eBird API key.

use std::time::Duration;

use picturesque_app::ports::birds::SpeciesLists;

/// eBird API 2.0.
const EBIRD_API: &str = "https://api.ebird.org/v2";

/// [`SpeciesLists`] from eBird's `product/spplist` endpoint.
#[derive(Debug)]
pub struct EbirdSpeciesLists {
    agent: ureq::Agent,
    base: String,
}

impl Default for EbirdSpeciesLists {
    fn default() -> Self {
        Self::with_base(EBIRD_API)
    }
}

impl EbirdSpeciesLists {
    /// Against another server (tests).
    pub fn with_base(base: &str) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .user_agent(concat!("picturesque/", env!("CARGO_PKG_VERSION")))
            .http_status_as_error(false)
            .build()
            .into();
        Self {
            agent,
            base: base.trim_end_matches('/').to_owned(),
        }
    }
}

/// Whether `region` looks like an eBird region code (`US`, `US-WA`,
/// `US-WA-033`, `L123456`), so it is safe in a URL path.
pub fn valid_region(region: &str) -> bool {
    !region.is_empty()
        && region.len() <= 32
        && region
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
}

impl SpeciesLists for EbirdSpeciesLists {
    fn species_in_region(&self, region: &str, api_key: &str) -> Result<Vec<String>, String> {
        if !valid_region(region) {
            return Err(format!("`{region}` is not an eBird region code"));
        }
        let mut response = self
            .agent
            .get(&format!("{}/product/spplist/{region}", self.base))
            .header("X-eBirdApiToken", api_key)
            .call()
            .map_err(|e| format!("eBird could not be reached: {e}"))?;
        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("eBird's answer could not be read: {e}"))?;
        match status {
            200 => serde_json::from_str::<Vec<String>>(&body)
                .map_err(|e| format!("eBird's answer is not a species list: {e}")),
            401 | 403 => Err("eBird did not accept the API key".into()),
            400 | 404 => Err(format!("eBird does not know the region `{region}`")),
            other => Err(format!("eBird answered with HTTP {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_region_codes_go_into_the_url() {
        for ok in ["US", "US-WA", "US-WA-033", "L123456", "world"] {
            assert!(valid_region(ok), "{ok}");
        }
        for bad in ["", "US/WA", "../x", "US WA", "US?x=1", &"A".repeat(33)] {
            assert!(!valid_region(bad), "{bad}");
        }
        let err = EbirdSpeciesLists::with_base("http://127.0.0.1:9")
            .species_in_region("../etc", "key")
            .unwrap_err();
        assert!(err.contains("not an eBird region code"), "{err}");
    }
}
