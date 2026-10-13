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
    /// Highest technical quality (`overall` score) first; photos not
    /// analyzed yet last.
    QualityDesc,
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
    /// Show one photo per series: the best one that matches the filters.
    #[ts(optional)]
    pub collapse_series: Option<bool>,
    /// Series (by id) shown whole although `collapse_series` is set.
    #[ts(optional, type = "number[]")]
    pub expanded_series: Option<Vec<i64>>,
    /// Minimum technical scores, 0–1 (`docs/reference/quality-scores.rst`).
    /// Photos not analyzed yet never match a threshold.
    #[ts(optional)]
    pub min_sharpness: Option<f32>,
    #[ts(optional)]
    pub min_exposure: Option<f32>,
    #[ts(optional)]
    pub min_noise: Option<f32>,
    /// Hide photos that are in a series but not its best photo.
    #[ts(optional)]
    pub best_only: Option<bool>,
    /// Only these photos (at most 1000).
    #[ts(optional, type = "number[]")]
    pub photo_ids: Option<Vec<i64>>,
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
        for (name, value) in [
            ("min_sharpness", self.min_sharpness),
            ("min_exposure", self.min_exposure),
            ("min_noise", self.min_noise),
        ] {
            if value.is_some_and(|v| !(0.0..=1.0).contains(&v)) {
                return Err(format!("`{name}` must be between 0 and 1."));
            }
        }
        if self
            .photo_ids
            .as_ref()
            .is_some_and(|ids| ids.len() > MAX_PAGE as usize)
        {
            return Err(format!("`photo_ids` may list at most {MAX_PAGE} photos."));
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
    /// The series the photo is in (its first photo's id), if any.
    #[ts(type = "number | null")]
    pub series_id: Option<i64>,
    /// Whether this is the best photo of its series.
    pub best_in_series: bool,
    /// Photos in the photo's series (0 when in none).
    pub series_size: u32,
    /// Overall technical quality, 0–1; `null` until analyzed.
    pub quality: Option<f32>,
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

/// One file of a photo, for the info panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct PhotoFileView {
    /// Relative to the library folder, `/`-separated.
    pub path: String,
    /// `raw`, `jpeg`, `heif`, `png`, `tiff`, `webp` or `xmp`.
    pub format: String,
    #[ts(type = "number")]
    pub size: u64,
}

/// Everything known about a photo, formatted for display.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct PhotoDetail {
    #[ts(type = "number")]
    pub id: i64,
    /// Local capture time, ISO 8601 without offset.
    pub captured_at: Option<String>,
    /// UTC offset of the capture time, for example `+01:00`.
    pub utc_offset: Option<String>,
    pub camera: Option<String>,
    pub lens: Option<String>,
    pub iso: Option<u32>,
    /// For example `f/2.8`.
    pub aperture: Option<String>,
    /// For example `1/250 s` or `2 s`.
    pub shutter: Option<String>,
    /// For example `35 mm`.
    pub focal_length: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    /// "City, Region, Country".
    pub place: Option<String>,
    /// Size as displayed (after orientation).
    pub width: Option<u32>,
    pub height: Option<u32>,
    #[ts(type = "\"unflagged\" | \"pick\" | \"reject\"")]
    pub flag: picturesque_core::decision::Flag,
    pub rating: u8,
    pub files: Vec<PhotoFileView>,
    /// Technical scores, once analyzed.
    pub quality: Option<crate::analysis::technical::QualityView>,
    /// How good the photo looks, once the CLIP model analyzed it.
    pub aesthetic: Option<crate::analysis::clip::AestheticView>,
    /// Faces found, largest first; `null` until the face models analyzed
    /// the photo.
    pub faces: Option<Vec<crate::analysis::faces::FaceView>>,
    /// Combined quality, 0–1, from every analysis so far
    /// (`core::ranking::photo_quality`); `null` before technical analysis.
    pub score: Option<f32>,
}

/// Arguments of commands that act on one photo.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct PhotoArgs {
    #[ts(type = "number")]
    pub photo_id: i64,
}

/// Arguments of `photo_facets`.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(default, deny_unknown_fields)]
#[ts(export)]
pub struct FacetsArgs {
    /// Count `total` and `days` among photos matching this filter.
    #[ts(optional)]
    pub filter: Option<PhotoQuery>,
}

/// A change to one photo's decision; absent fields stay as they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct DecisionChange {
    #[ts(type = "number")]
    pub photo_id: i64,
    #[ts(optional, type = "\"unflagged\" | \"pick\" | \"reject\"")]
    pub flag: Option<picturesque_core::decision::Flag>,
    #[ts(optional)]
    pub rating: Option<u8>,
}

/// Arguments of `set_decisions`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct SetDecisionsArgs {
    pub changes: Vec<DecisionChange>,
}

/// A photo's decision after a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct DecisionView {
    #[ts(type = "number")]
    pub photo_id: i64,
    #[ts(type = "\"unflagged\" | \"pick\" | \"reject\"")]
    pub flag: picturesque_core::decision::Flag,
    pub rating: u8,
}

/// Formats an exposure time: `1/250 s` for fractions of a second, `2 s`,
/// `2.5 s` or `30 s` otherwise.
pub fn format_shutter(numerator: u32, denominator: u32) -> String {
    let seconds = f64::from(numerator) / f64::from(denominator.max(1));
    if seconds < 1.0 {
        let inverse = (1.0 / seconds).round() as u64;
        format!("1/{inverse} s")
    } else if (seconds - seconds.round()).abs() < 0.05 {
        format!("{} s", seconds.round() as u64)
    } else {
        format!("{seconds:.1} s")
    }
}

fn format_number(value: f32) -> String {
    let rounded = (value * 10.0).round() / 10.0;
    if rounded.fract() == 0.0 {
        format!("{rounded:.0}")
    } else {
        format!("{rounded:.1}")
    }
}

impl PhotoDetail {
    pub fn new(
        photo: &picturesque_core::photo::Photo,
        decision: picturesque_core::decision::Decision,
    ) -> Self {
        let m = &photo.metadata;
        let (width, height) = m
            .display_dimensions()
            .map_or((None, None), |(w, h)| (Some(w), Some(h)));
        Self {
            id: photo.id.0,
            captured_at: m
                .captured_at
                .map(|t| t.local.format("%Y-%m-%dT%H:%M:%S").to_string()),
            utc_offset: m.captured_at.and_then(|t| t.offset).map(|o| o.to_string()),
            camera: m.camera.as_ref().map(|c| c.display_name()),
            lens: m.lens.clone(),
            iso: m.exposure.iso,
            aperture: m
                .exposure
                .f_number
                .map(|f| format!("f/{}", format_number(f))),
            shutter: m
                .exposure
                .exposure_time
                .map(|t| format_shutter(t.numerator, t.denominator)),
            focal_length: m
                .exposure
                .focal_length_mm
                .map(|f| format!("{} mm", format_number(f))),
            latitude: m.gps.map(|g| g.latitude()),
            longitude: m.gps.map(|g| g.longitude()),
            place: m.place.as_ref().map(|p| {
                [
                    Some(p.city.as_str()),
                    p.region.as_deref().filter(|r| *r != p.city),
                    Some(p.country.as_str()),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(", ")
            }),
            width,
            height,
            flag: decision.flag,
            rating: decision.rating.stars(),
            quality: None,
            aesthetic: None,
            faces: None,
            score: None,
            files: photo
                .assets
                .iter()
                .map(|a| PhotoFileView {
                    path: a
                        .path
                        .iter()
                        .map(|c| c.to_string_lossy())
                        .collect::<Vec<_>>()
                        .join("/"),
                    format: a.format.as_str().to_owned(),
                    size: a.size,
                })
                .collect(),
        }
    }
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
        assert!(q(|q| q.photo_ids = Some(vec![1; 1001])).is_err());
        assert!(q(|q| q.photo_ids = Some(vec![1; 1000])).is_ok());
        assert!(q(|q| q.min_sharpness = Some(1.5)).is_err());
        assert!(q(|q| q.min_noise = Some(f32::NAN)).is_err());
        assert!(q(|q| q.min_exposure = Some(0.0)).is_ok());
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

    #[test]
    fn formats_shutter_speeds_like_photographers() {
        assert_eq!(format_shutter(1, 250), "1/250 s");
        assert_eq!(format_shutter(10, 1600), "1/160 s");
        assert_eq!(format_shutter(1, 3), "1/3 s");
        assert_eq!(format_shutter(2, 1), "2 s");
        assert_eq!(format_shutter(5, 2), "2.5 s");
        assert_eq!(format_shutter(30, 1), "30 s");
    }

    #[test]
    fn builds_details_from_a_photo() {
        use chrono::{FixedOffset, NaiveDateTime};
        use picturesque_core::decision::{Decision, Flag, Rating};
        use picturesque_core::hash::ContentHash;
        use picturesque_core::media::MediaFormat;
        use picturesque_core::metadata::*;
        use picturesque_core::photo::*;

        let photo = Photo {
            id: PhotoId(7),
            metadata: CaptureMetadata {
                captured_at: Some(CaptureTime::new(
                    NaiveDateTime::parse_from_str("2026-10-12 14:30:05", "%Y-%m-%d %H:%M:%S")
                        .unwrap(),
                    FixedOffset::east_opt(3600),
                )),
                camera: Some(Camera::new("Canon", "Canon EOS R5")),
                lens: Some("RF24-70mm F2.8 L IS USM".into()),
                exposure: Exposure {
                    iso: Some(400),
                    f_number: Some(2.8),
                    exposure_time: Some(ExposureTime {
                        numerator: 1,
                        denominator: 250,
                    }),
                    focal_length_mm: Some(35.0),
                },
                gps: GeoPoint::new(38.72, -9.14),
                place: Some(Place {
                    city: "Lisbon".into(),
                    region: Some("Lisbon".into()),
                    country: "Portugal".into(),
                    country_code: "PT".into(),
                }),
                orientation: Orientation::Rotate90,
                dimensions: Some((6000, 4000)),
            },
            assets: PhotoAssets::new(vec![Asset {
                format: MediaFormat::Raw,
                path: ["2026", "IMG_1.CR3"].iter().collect(),
                size: 30_000_000,
                hash: ContentHash::from_bytes([1; 32]),
            }])
            .unwrap(),
        };

        let d = PhotoDetail::new(
            &photo,
            Decision {
                flag: Flag::Pick,
                rating: Rating::new(4).unwrap(),
            },
        );

        assert_eq!(d.captured_at.as_deref(), Some("2026-10-12T14:30:05"));
        assert_eq!(d.utc_offset.as_deref(), Some("+01:00"));
        assert_eq!(d.aperture.as_deref(), Some("f/2.8"));
        assert_eq!(d.shutter.as_deref(), Some("1/250 s"));
        assert_eq!(d.focal_length.as_deref(), Some("35 mm"));
        // The region is skipped when it repeats the city.
        assert_eq!(d.place.as_deref(), Some("Lisbon, Portugal"));
        assert_eq!((d.width, d.height), (Some(4000), Some(6000)));
        assert_eq!(d.files[0].path, "2026/IMG_1.CR3");
        assert_eq!((d.flag, d.rating), (Flag::Pick, 4));
    }
}
