//! Capture metadata: when, where and how a photo was taken.

use chrono::{DateTime, FixedOffset, NaiveDateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};

/// When a photo was taken. Cameras record local wall-clock time; the UTC
/// offset is only known when the file records it (EXIF `OffsetTime*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureTime {
    pub local: NaiveDateTime,
    /// Serialized as seconds east of UTC.
    #[serde(with = "offset_seconds")]
    pub offset: Option<FixedOffset>,
}

mod offset_seconds {
    use chrono::FixedOffset;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(offset: &Option<FixedOffset>, s: S) -> Result<S::Ok, S::Error> {
        match offset {
            Some(o) => s.serialize_some(&o.local_minus_utc()),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<FixedOffset>, D::Error> {
        Option::<i32>::deserialize(d)?
            .map(|secs| {
                FixedOffset::east_opt(secs)
                    .ok_or_else(|| serde::de::Error::custom("UTC offset out of range"))
            })
            .transpose()
    }
}

impl CaptureTime {
    pub fn new(local: NaiveDateTime, offset: Option<FixedOffset>) -> Self {
        Self { local, offset }
    }

    /// The absolute instant, when the offset is known.
    pub fn utc(&self) -> Option<DateTime<Utc>> {
        let offset = self.offset?;
        offset
            .from_local_datetime(&self.local)
            .single()
            .map(|t| t.with_timezone(&Utc))
    }
}

/// A position on Earth in decimal degrees (WGS 84).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeoPoint {
    latitude: f64,
    longitude: f64,
}

impl GeoPoint {
    /// Returns `None` for coordinates outside the valid range or not finite.
    pub fn new(latitude: f64, longitude: f64) -> Option<Self> {
        let valid = latitude.is_finite()
            && longitude.is_finite()
            && (-90.0..=90.0).contains(&latitude)
            && (-180.0..=180.0).contains(&longitude);
        valid.then_some(Self {
            latitude,
            longitude,
        })
    }

    pub fn latitude(&self) -> f64 {
        self.latitude
    }

    pub fn longitude(&self) -> f64 {
        self.longitude
    }
}

/// A named place resolved from GPS coordinates (ADR-0007).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Place {
    pub city: String,
    /// First-level administrative region (state, province), when known.
    pub region: Option<String>,
    pub country: String,
    /// ISO 3166-1 alpha-2 code, for example `PT`.
    pub country_code: String,
}

/// The camera body, as recorded in EXIF `Make` and `Model`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Camera {
    pub make: String,
    pub model: String,
}

impl Camera {
    pub fn new(make: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            make: make.into().trim().to_owned(),
            model: model.into().trim().to_owned(),
        }
    }

    /// Short manufacturer name: `NIKON CORPORATION` becomes `Nikon`.
    pub fn brand(&self) -> String {
        const BRANDS: &[(&str, &str)] = &[
            ("nikon", "Nikon"),
            ("canon", "Canon"),
            ("sony", "Sony"),
            ("fujifilm", "Fujifilm"),
            ("olympus", "Olympus"),
            ("om digital", "OM System"),
            ("panasonic", "Panasonic"),
            ("pentax", "Pentax"),
            ("ricoh", "Ricoh"),
            ("leica", "Leica"),
            ("hasselblad", "Hasselblad"),
            ("apple", "Apple"),
            ("google", "Google"),
            ("samsung", "Samsung"),
            ("dji", "DJI"),
        ];
        let make = self.make.to_lowercase();
        BRANDS
            .iter()
            .find(|(prefix, _)| make.starts_with(prefix))
            .map(|(_, brand)| (*brand).to_owned())
            .unwrap_or_else(|| self.make.clone())
    }

    /// Human-readable name without repeating the brand. For example, make
    /// `NIKON CORPORATION` with model `NIKON Z 8` gives `Nikon Z 8`, and
    /// make `SONY` with model `ILCE-7M4` gives `Sony ILCE-7M4`.
    pub fn display_name(&self) -> String {
        let brand = self.brand();
        let model = self.model.trim();
        let rest = [brand.as_str(), self.make.as_str()]
            .iter()
            .find_map(|prefix| strip_prefix_ignore_case(model, prefix))
            .unwrap_or(model)
            .trim();
        if rest.is_empty() {
            brand
        } else {
            format!("{brand} {rest}")
        }
    }
}

fn strip_prefix_ignore_case<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &s[prefix.len()..])
}

/// Exposure time as a fraction of a second, for example 1/250.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExposureTime {
    pub numerator: u32,
    pub denominator: u32,
}

impl ExposureTime {
    pub fn seconds(&self) -> f64 {
        f64::from(self.numerator) / f64::from(self.denominator.max(1))
    }
}

/// Exposure settings.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Exposure {
    pub iso: Option<u32>,
    pub f_number: Option<f32>,
    pub exposure_time: Option<ExposureTime>,
    pub focal_length_mm: Option<f32>,
}

/// EXIF orientation: how the stored pixels must be transformed for display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    #[default]
    Normal,
    FlipHorizontal,
    Rotate180,
    FlipVertical,
    Transpose,
    Rotate90,
    Transverse,
    Rotate270,
}

impl Orientation {
    /// Maps EXIF tag values 1–8; anything else is treated as normal.
    pub fn from_exif(value: u16) -> Self {
        match value {
            2 => Self::FlipHorizontal,
            3 => Self::Rotate180,
            4 => Self::FlipVertical,
            5 => Self::Transpose,
            6 => Self::Rotate90,
            7 => Self::Transverse,
            8 => Self::Rotate270,
            _ => Self::Normal,
        }
    }

    /// The EXIF tag value, 1–8.
    pub fn to_exif(self) -> u16 {
        match self {
            Self::Normal => 1,
            Self::FlipHorizontal => 2,
            Self::Rotate180 => 3,
            Self::FlipVertical => 4,
            Self::Transpose => 5,
            Self::Rotate90 => 6,
            Self::Transverse => 7,
            Self::Rotate270 => 8,
        }
    }

    /// Whether displaying the image swaps its width and height.
    pub fn swaps_dimensions(self) -> bool {
        matches!(
            self,
            Self::Transpose | Self::Rotate90 | Self::Transverse | Self::Rotate270
        )
    }
}

/// Everything known about how and where a photo was taken. Every field is
/// optional because files vary widely in what they record.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CaptureMetadata {
    pub captured_at: Option<CaptureTime>,
    pub camera: Option<Camera>,
    pub lens: Option<String>,
    pub exposure: Exposure,
    pub gps: Option<GeoPoint>,
    pub place: Option<Place>,
    pub orientation: Orientation,
    /// Stored pixel dimensions, before orientation is applied.
    pub dimensions: Option<(u32, u32)>,
}

impl CaptureMetadata {
    /// Fills fields that are missing here from `other`, field by field.
    /// Used for RAW+JPEG pairs: the RAW is read first, and the JPEG can
    /// supply what the RAW lacks (for example GPS added by a phone app).
    pub fn fill_missing_from(&mut self, other: &CaptureMetadata) {
        fn fill<T: Clone>(slot: &mut Option<T>, other: &Option<T>) {
            if slot.is_none() {
                slot.clone_from(other);
            }
        }
        fill(&mut self.captured_at, &other.captured_at);
        fill(&mut self.camera, &other.camera);
        fill(&mut self.lens, &other.lens);
        fill(&mut self.exposure.iso, &other.exposure.iso);
        fill(&mut self.exposure.f_number, &other.exposure.f_number);
        fill(
            &mut self.exposure.exposure_time,
            &other.exposure.exposure_time,
        );
        fill(
            &mut self.exposure.focal_length_mm,
            &other.exposure.focal_length_mm,
        );
        fill(&mut self.gps, &other.gps);
        fill(&mut self.place, &other.place);
        fill(&mut self.dimensions, &other.dimensions);
    }

    /// Dimensions as displayed, after orientation.
    pub fn display_dimensions(&self) -> Option<(u32, u32)> {
        let (w, h) = self.dimensions?;
        Some(if self.orientation.swaps_dimensions() {
            (h, w)
        } else {
            (w, h)
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    fn local() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 12)
            .unwrap()
            .and_hms_opt(14, 30, 0)
            .unwrap()
    }

    #[test]
    fn capture_time_with_offset_has_utc_instant() {
        let t = CaptureTime::new(local(), FixedOffset::east_opt(2 * 3600));

        assert_eq!(t.utc().unwrap().to_rfc3339(), "2026-10-12T12:30:00+00:00");
    }

    #[test]
    fn capture_time_without_offset_has_no_utc_instant() {
        assert_eq!(CaptureTime::new(local(), None).utc(), None);
    }

    #[test]
    fn geo_point_accepts_valid_and_rejects_invalid_coordinates() {
        assert!(GeoPoint::new(38.72, -9.14).is_some());
        assert!(GeoPoint::new(90.0, 180.0).is_some());
        assert!(GeoPoint::new(90.1, 0.0).is_none());
        assert!(GeoPoint::new(0.0, -180.5).is_none());
        assert!(GeoPoint::new(f64::NAN, 0.0).is_none());
    }

    #[test]
    fn camera_display_name_does_not_repeat_the_brand() {
        assert_eq!(
            Camera::new("Canon", "Canon EOS R5").display_name(),
            "Canon EOS R5"
        );
        assert_eq!(
            Camera::new("NIKON CORPORATION", "NIKON Z 8").display_name(),
            "Nikon Z 8"
        );
        assert_eq!(
            Camera::new("SONY", "ILCE-7M4").display_name(),
            "Sony ILCE-7M4"
        );
        assert_eq!(
            Camera::new("OM Digital Solutions", "OM-1").display_name(),
            "OM System OM-1"
        );
        assert_eq!(
            Camera::new("Apple", "iPhone 15 Pro").display_name(),
            "Apple iPhone 15 Pro"
        );
        assert_eq!(Camera::new("Acme", "Acme").display_name(), "Acme");
    }

    #[test]
    fn camera_brand_falls_back_to_make() {
        assert_eq!(Camera::new("  Zeiss ", "ZX1").brand(), "Zeiss");
    }

    #[test]
    fn orientation_maps_exif_values() {
        assert_eq!(Orientation::from_exif(1), Orientation::Normal);
        assert_eq!(Orientation::from_exif(6), Orientation::Rotate90);
        assert_eq!(Orientation::from_exif(8), Orientation::Rotate270);
        assert_eq!(Orientation::from_exif(0), Orientation::Normal);
        assert_eq!(Orientation::from_exif(42), Orientation::Normal);
    }

    #[test]
    fn orientation_round_trips_exif_values() {
        for v in 1..=8 {
            assert_eq!(Orientation::from_exif(v).to_exif(), v);
        }
    }

    #[test]
    fn display_dimensions_apply_orientation() {
        let mut meta = CaptureMetadata {
            dimensions: Some((6000, 4000)),
            ..Default::default()
        };
        assert_eq!(meta.display_dimensions(), Some((6000, 4000)));

        meta.orientation = Orientation::Rotate90;
        assert_eq!(meta.display_dimensions(), Some((4000, 6000)));
    }

    #[test]
    fn fill_missing_keeps_existing_values_and_adds_missing_ones() {
        let mut raw = CaptureMetadata {
            camera: Some(Camera::new("Canon", "EOS R5")),
            exposure: Exposure {
                iso: Some(100),
                ..Default::default()
            },
            ..Default::default()
        };
        let jpeg = CaptureMetadata {
            camera: Some(Camera::new("Other", "Thing")),
            exposure: Exposure {
                iso: Some(800),
                f_number: Some(2.8),
                ..Default::default()
            },
            gps: GeoPoint::new(1.0, 2.0),
            orientation: Orientation::Rotate90,
            ..Default::default()
        };

        raw.fill_missing_from(&jpeg);

        assert_eq!(raw.camera, Some(Camera::new("Canon", "EOS R5")));
        assert_eq!(raw.exposure.iso, Some(100));
        assert_eq!(raw.exposure.f_number, Some(2.8));
        assert_eq!(raw.gps, GeoPoint::new(1.0, 2.0));
        // Orientation always has a value, so it is never taken from the other file.
        assert_eq!(raw.orientation, Orientation::Normal);
    }

    #[test]
    fn exposure_time_in_seconds() {
        let t = ExposureTime {
            numerator: 1,
            denominator: 250,
        };
        assert!((t.seconds() - 0.004).abs() < 1e-12);
    }

    #[test]
    fn capture_time_serializes_offset_as_seconds() {
        let t = CaptureTime::new(local(), FixedOffset::west_opt(5 * 3600));
        let json = serde_json::to_value(t).unwrap();

        assert_eq!(
            json,
            serde_json::json!({ "local": "2026-10-12T14:30:00", "offset": -18000 })
        );
        assert_eq!(serde_json::from_value::<CaptureTime>(json).unwrap(), t);
    }

    #[test]
    fn capture_time_rejects_out_of_range_offset() {
        let json = serde_json::json!({ "local": "2026-10-12T14:30:00", "offset": 999_999 });

        assert!(serde_json::from_value::<CaptureTime>(json).is_err());
    }
}
