//! Browsing the library: querying photos and the values to filter by.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A camera, as stored (make and model) and as shown.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CameraKey {
    pub make: String,
    pub model: String,
}

/// Order of query results. Undated photos always come last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PhotoSort {
    /// Newest capture first.
    #[default]
    CapturedDesc,
    CapturedAsc,
    /// Most recently imported first.
    ImportedDesc,
}

/// Which photos to list. Every filter is optional; filters combine with AND.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, TS)]
#[serde(default, deny_unknown_fields)]
#[ts(export)]
pub struct PhotoQuery {
    /// First capture day included, `YYYY-MM-DD`.
    #[ts(optional)]
    pub from: Option<String>,
    /// Last capture day included, `YYYY-MM-DD`.
    #[ts(optional)]
    pub to: Option<String>,
    /// ISO country code, for example `PT`.
    #[ts(optional)]
    pub country_code: Option<String>,
    #[ts(optional)]
    pub city: Option<String>,
    #[ts(optional)]
    pub camera: Option<CameraKey>,
    #[ts(optional)]
    pub lens: Option<String>,
    /// Photos with any of these flags.
    #[ts(optional, type = "Array<\"unflagged\" | \"pick\" | \"reject\">")]
    pub flags: Option<Vec<picturesque_core::decision::Flag>>,
    #[ts(optional)]
    pub min_rating: Option<u8>,
    #[ts(optional)]
    pub sort: Option<PhotoSort>,
    /// From a previous page's `next_cursor`.
    #[ts(optional)]
    pub cursor: Option<String>,
    /// Page size, 1–1000 (default 200).
    #[ts(optional)]
    pub limit: Option<u32>,
}

pub const DEFAULT_PAGE: u32 = 200;
pub const MAX_PAGE: u32 = 1000;

impl PhotoQuery {
    /// Validates dates, rating and page size; the message is for users.
    pub fn validate(&self) -> Result<(), String> {
        for (name, value) in [("from", &self.from), ("to", &self.to)] {
            if let Some(v) = value
                && chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d").is_err()
            {
                return Err(format!(
                    "`{name}` must be a date like 2026-10-12, got `{v}`."
                ));
            }
        }
        if self.min_rating.is_some_and(|r| r > 5) {
            return Err("`min_rating` must be 0-5.".into());
        }
        if self.limit.is_some_and(|l| l == 0 || l > MAX_PAGE) {
            return Err(format!("`limit` must be 1-{MAX_PAGE}."));
        }
        Ok(())
    }

    pub fn page_size(&self) -> u32 {
        self.limit.unwrap_or(DEFAULT_PAGE)
    }
}

/// One photo in a result list: what the grid needs to show it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct PhotoSummary {
    #[ts(type = "number")]
    pub id: i64,
    /// Local capture time, ISO 8601 without offset.
    pub captured_at: Option<String>,
    /// Capture day, `YYYY-MM-DD`; for grouping.
    pub day: Option<String>,
    pub camera: Option<String>,
    pub lens: Option<String>,
    pub city: Option<String>,
    pub country: Option<String>,
    #[ts(type = "\"unflagged\" | \"pick\" | \"reject\"")]
    pub flag: picturesque_core::decision::Flag,
    pub rating: u8,
    /// Size as displayed (after orientation), when known.
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub has_raw: bool,
    /// The file the grid shows, relative to the library folder.
    pub file_name: String,
}

/// A page of results.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct PhotoPage {
    pub photos: Vec<PhotoSummary>,
    /// Pass back as `cursor` for the next page; `null` on the last page.
    pub next_cursor: Option<String>,
    /// Photos matching the filters, across all pages.
    pub total: usize,
}

/// A value with how many photos have it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Counted<T: TS> {
    pub value: T,
    pub count: usize,
}

/// A camera with its display name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct CameraFacet {
    pub key: CameraKey,
    pub label: String,
}

/// A place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct PlaceFacet {
    pub city: String,
    pub country: String,
    pub country_code: String,
}

/// How many photos have a flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct FlagCount {
    #[ts(type = "\"unflagged\" | \"pick\" | \"reject\"")]
    pub flag: picturesque_core::decision::Flag,
    pub count: usize,
}

/// Values present in the library, for building filters.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct PhotoFacets {
    pub total: usize,
    pub first_day: Option<String>,
    pub last_day: Option<String>,
    /// Days with photos, newest first.
    pub days: Vec<Counted<String>>,
    pub cameras: Vec<Counted<CameraFacet>>,
    pub lenses: Vec<Counted<String>>,
    pub places: Vec<Counted<PlaceFacet>>,
    pub flags: Vec<FlagCount>,
    /// Photos per star rating, index 0–5.
    pub ratings: Vec<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_dates_rating_and_limit() {
        assert!(PhotoQuery::default().validate().is_ok());
        let q = |f: fn(&mut PhotoQuery)| {
            let mut q = PhotoQuery::default();
            f(&mut q);
            q.validate()
        };
        assert!(
            q(|q| q.from = Some("2026-13-01".into()))
                .unwrap_err()
                .contains("from")
        );
        assert!(
            q(|q| q.to = Some("yesterday".into()))
                .unwrap_err()
                .contains("to")
        );
        assert!(q(|q| q.min_rating = Some(6)).is_err());
        assert!(q(|q| q.limit = Some(0)).is_err());
        assert!(q(|q| q.limit = Some(1001)).is_err());
        assert!(q(|q| q.limit = Some(1000)).is_ok());
    }

    #[test]
    fn rejects_unknown_query_fields() {
        assert!(serde_json::from_str::<PhotoQuery>(r#"{"colour":"red"}"#).is_err());
        let q: PhotoQuery = serde_json::from_str(
            r#"{"flags":["pick"],"camera":{"make":"Canon","model":"EOS R5"}}"#,
        )
        .unwrap();
        assert_eq!(q.flags.unwrap(), [picturesque_core::decision::Flag::Pick]);
    }
}
