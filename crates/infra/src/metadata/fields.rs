//! Raw EXIF values as read from a file, and their conversion to
//! [`CaptureMetadata`]. Format adapters fill [`ExifFields`]; everything
//! here is pure and unit-tested.

use chrono::{FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta};
use picturesque_core::metadata::{
    Camera, CaptureMetadata, CaptureTime, Exposure, ExposureTime, GeoPoint, Orientation,
};

/// An unsigned EXIF rational, `numerator / denominator`.
pub type Ratio = (u32, u32);

/// EXIF values relevant to Picturesque, still in EXIF notation.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExifFields {
    /// `DateTimeOriginal`, `DateTimeDigitized` and `DateTime`
    /// (`YYYY:MM:DD HH:MM:SS`), with matching `OffsetTime*` (`±HH:MM`)
    /// and `SubSecTime*` (digits).
    pub date_time_original: Option<String>,
    pub date_time_digitized: Option<String>,
    pub date_time: Option<String>,
    pub offset_original: Option<String>,
    pub offset_digitized: Option<String>,
    pub offset: Option<String>,
    pub subsec_original: Option<String>,
    pub subsec_digitized: Option<String>,
    pub subsec: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub lens: Option<String>,
    pub iso: Option<u32>,
    pub f_number: Option<Ratio>,
    pub exposure_time: Option<Ratio>,
    pub focal_length: Option<Ratio>,
    pub orientation: Option<u16>,
    /// Degrees, minutes, seconds and the `N`/`S` or `E`/`W` reference.
    pub gps_latitude: Option<([Ratio; 3], String)>,
    pub gps_longitude: Option<([Ratio; 3], String)>,
    /// `GPSDateStamp` (`YYYY:MM:DD`) and `GPSTimeStamp` (UTC h, m, s).
    pub gps_date: Option<String>,
    pub gps_time: Option<[Ratio; 3]>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

impl ExifFields {
    /// Converts to domain metadata. Capture time falls back from
    /// `DateTimeOriginal` to `DateTimeDigitized` to `DateTime`. When no
    /// UTC offset is recorded but GPS time is, the offset is derived from
    /// the difference (rounded to 15 minutes).
    pub fn to_metadata(&self) -> CaptureMetadata {
        let captured_at = [
            (
                &self.date_time_original,
                &self.offset_original,
                &self.subsec_original,
            ),
            (
                &self.date_time_digitized,
                &self.offset_digitized,
                &self.subsec_digitized,
            ),
            (&self.date_time, &self.offset, &self.subsec),
        ]
        .into_iter()
        .find_map(|(dt, off, sub)| {
            let local = parse_datetime(dt.as_deref()?)?;
            let local = add_subsec(local, sub.as_deref());
            let offset = off
                .as_deref()
                .and_then(parse_offset)
                .or_else(|| self.offset_from_gps(local));
            Some(CaptureTime::new(local, offset))
        });

        let camera = match (clean(&self.make), clean(&self.model)) {
            (None, None) => None,
            (make, model) => Some(Camera::new(
                make.unwrap_or_default(),
                model.unwrap_or_default(),
            )),
        };

        let gps = match (&self.gps_latitude, &self.gps_longitude) {
            (Some((lat, lat_ref)), Some((lon, lon_ref))) => {
                match (dms_to_degrees(lat), dms_to_degrees(lon)) {
                    // 0,0 is what cameras write when they have no fix.
                    (Some(lat), Some(lon)) if lat != 0.0 || lon != 0.0 => GeoPoint::new(
                        lat * hemisphere(lat_ref, 'S'),
                        lon * hemisphere(lon_ref, 'W'),
                    ),
                    _ => None,
                }
            }
            _ => None,
        };

        let dimensions = match (self.width, self.height) {
            (Some(w), Some(h)) if w > 0 && h > 0 => Some((w, h)),
            _ => None,
        };

        CaptureMetadata {
            captured_at,
            camera,
            lens: clean(&self.lens),
            exposure: Exposure {
                iso: self.iso.filter(|&iso| iso > 0),
                f_number: self.f_number.and_then(ratio_f32),
                exposure_time: self.exposure_time.filter(|&(n, d)| n > 0 && d > 0).map(
                    |(numerator, denominator)| ExposureTime {
                        numerator,
                        denominator,
                    },
                ),
                focal_length_mm: self.focal_length.and_then(ratio_f32),
            },
            gps,
            place: None,
            orientation: self
                .orientation
                .map(Orientation::from_exif)
                .unwrap_or_default(),
            dimensions,
        }
    }

    fn offset_from_gps(&self, local: NaiveDateTime) -> Option<FixedOffset> {
        let date = NaiveDate::parse_from_str(self.gps_date.as_deref()?.trim(), "%Y:%m:%d").ok()?;
        let [h, m, s] = self.gps_time?;
        let secs = ratio_f64(h)? * 3600.0 + ratio_f64(m)? * 60.0 + ratio_f64(s)?;
        let utc =
            date.and_time(NaiveTime::MIN) + TimeDelta::milliseconds((secs * 1000.0).round() as i64);
        let diff = (local - utc).num_seconds();
        let quarter_hours = (diff as f64 / 900.0).round() as i32;
        // Real offsets are within -12:00..+14:00; a larger difference means
        // the clocks disagree, not a time zone.
        if !(-48..=56).contains(&quarter_hours) {
            return None;
        }
        FixedOffset::east_opt(quarter_hours * 900)
    }
}

/// Parses EXIF `YYYY:MM:DD HH:MM:SS`. Also accepts `-` as the date
/// separator and `T` between date and time, which some writers use.
/// Blank or zeroed values (`0000:00:00 00:00:00`) give `None`.
pub fn parse_datetime(s: &str) -> Option<NaiveDateTime> {
    let s = s.trim().trim_end_matches('\0');
    let normalized: String = s
        .char_indices()
        .map(|(i, c)| match (i, c) {
            (4 | 7, '-') => ':',
            (10, 'T') => ' ',
            _ => c,
        })
        .collect();
    NaiveDateTime::parse_from_str(normalized.get(..19)?, "%Y:%m:%d %H:%M:%S").ok()
}

/// Parses `±HH:MM` (also `±HHMM`, `Z`).
pub fn parse_offset(s: &str) -> Option<FixedOffset> {
    let s = s.trim().trim_end_matches('\0');
    if s == "Z" {
        return FixedOffset::east_opt(0);
    }
    let (sign, rest) = match s.as_bytes().first()? {
        b'+' => (1, &s[1..]),
        b'-' => (-1, &s[1..]),
        _ => return None,
    };
    let digits: String = rest.chars().filter(|c| *c != ':').collect();
    if digits.len() != 4 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let hours: i32 = digits[..2].parse().ok()?;
    let minutes: i32 = digits[2..].parse().ok()?;
    if hours > 14 || minutes > 59 {
        return None;
    }
    FixedOffset::east_opt(sign * (hours * 3600 + minutes * 60))
}

/// Adds `SubSecTime` digits as a fraction of a second: `"5"` is 0.5 s,
/// `"250"` is 0.25 s.
fn add_subsec(local: NaiveDateTime, subsec: Option<&str>) -> NaiveDateTime {
    let Some(digits) = subsec.map(|s| s.trim().trim_end_matches('\0')) else {
        return local;
    };
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return local;
    }
    let nanos_str: String = digits
        .chars()
        .chain(std::iter::repeat('0'))
        .take(9)
        .collect();
    let nanos: i64 = nanos_str.parse().unwrap_or(0);
    local + TimeDelta::nanoseconds(nanos)
}

fn dms_to_degrees(dms: &[Ratio; 3]) -> Option<f64> {
    Some(ratio_f64(dms[0])? + ratio_f64(dms[1])? / 60.0 + ratio_f64(dms[2])? / 3600.0)
}

fn hemisphere(reference: &str, negative: char) -> f64 {
    if reference.trim().starts_with(negative) {
        -1.0
    } else {
        1.0
    }
}

fn ratio_f64((n, d): Ratio) -> Option<f64> {
    (d != 0).then(|| f64::from(n) / f64::from(d))
}

fn ratio_f32(r: Ratio) -> Option<f32> {
    ratio_f64(r).filter(|v| *v > 0.0).map(|v| v as f32)
}

/// Trims whitespace and NULs; blank becomes `None`.
fn clean(s: &Option<String>) -> Option<String> {
    let t = s
        .as_deref()?
        .trim_matches(|c: char| c.is_whitespace() || c == '\0');
    (!t.is_empty()).then(|| t.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f").unwrap()
    }

    fn some(s: &str) -> Option<String> {
        Some(s.to_owned())
    }

    #[test]
    fn parses_exif_datetimes_and_rejects_blank_ones() {
        assert_eq!(
            parse_datetime("2026:10:12 14:30:05"),
            Some(dt("2026-10-12 14:30:05"))
        );
        assert_eq!(
            parse_datetime("2026-10-12T14:30:05"),
            Some(dt("2026-10-12 14:30:05"))
        );
        assert_eq!(
            parse_datetime("2026:10:12 14:30:05\0"),
            Some(dt("2026-10-12 14:30:05"))
        );
        assert_eq!(parse_datetime("0000:00:00 00:00:00"), None);
        assert_eq!(parse_datetime("    :  :     :  :  "), None);
        assert_eq!(parse_datetime(""), None);
    }

    #[test]
    fn parses_offsets() {
        assert_eq!(parse_offset("+02:00"), FixedOffset::east_opt(7200));
        assert_eq!(parse_offset("-05:30"), FixedOffset::east_opt(-19800));
        assert_eq!(parse_offset("+0545"), FixedOffset::east_opt(20700));
        assert_eq!(parse_offset("Z"), FixedOffset::east_opt(0));
        assert_eq!(parse_offset("02:00"), None);
        assert_eq!(parse_offset("+25:00"), None);
        assert_eq!(parse_offset(""), None);
    }

    #[test]
    fn capture_time_uses_original_with_offset_and_subseconds() {
        let fields = ExifFields {
            date_time_original: some("2026:10:12 14:30:05"),
            offset_original: some("+01:00"),
            subsec_original: some("25"),
            date_time: some("2030:01:01 00:00:00"),
            ..Default::default()
        };

        let t = fields.to_metadata().captured_at.unwrap();

        assert_eq!(t.local, dt("2026-10-12 14:30:05.25"));
        assert_eq!(t.offset, FixedOffset::east_opt(3600));
    }

    #[test]
    fn capture_time_falls_back_to_digitized_then_modified() {
        let digitized = ExifFields {
            date_time_digitized: some("2026:10:12 10:00:00"),
            date_time: some("2026:10:13 10:00:00"),
            ..Default::default()
        };
        let modified = ExifFields {
            date_time: some("2026:10:13 10:00:00"),
            ..Default::default()
        };

        assert_eq!(
            digitized.to_metadata().captured_at.unwrap().local,
            dt("2026-10-12 10:00:00")
        );
        assert_eq!(
            modified.to_metadata().captured_at.unwrap().local,
            dt("2026-10-13 10:00:00")
        );
        assert_eq!(ExifFields::default().to_metadata().captured_at, None);
    }

    #[test]
    fn derives_offset_from_gps_time_when_missing() {
        let fields = ExifFields {
            date_time_original: some("2026:10:12 14:30:05"),
            gps_date: some("2026:10:12"),
            gps_time: Some([(12, 1), (29, 1), (58, 1)]),
            ..Default::default()
        };

        assert_eq!(
            fields.to_metadata().captured_at.unwrap().offset,
            FixedOffset::east_opt(7200)
        );
    }

    #[test]
    fn derives_offset_across_midnight_and_ignores_implausible_differences() {
        let across = ExifFields {
            date_time_original: some("2026:10:13 01:00:00"),
            gps_date: some("2026:10:12"),
            gps_time: Some([(20, 1), (0, 1), (0, 1)]),
            ..Default::default()
        };
        let wrong_clock = ExifFields {
            date_time_original: some("2026:10:12 14:30:05"),
            gps_date: some("2026:09:01"),
            gps_time: Some([(12, 1), (0, 1), (0, 1)]),
            ..Default::default()
        };

        assert_eq!(
            across.to_metadata().captured_at.unwrap().offset,
            FixedOffset::east_opt(5 * 3600)
        );
        assert_eq!(wrong_clock.to_metadata().captured_at.unwrap().offset, None);
    }

    #[test]
    fn recorded_offset_wins_over_gps() {
        let fields = ExifFields {
            date_time_original: some("2026:10:12 14:30:05"),
            offset_original: some("+03:00"),
            gps_date: some("2026:10:12"),
            gps_time: Some([(12, 1), (30, 1), (5, 1)]),
            ..Default::default()
        };

        assert_eq!(
            fields.to_metadata().captured_at.unwrap().offset,
            FixedOffset::east_opt(3 * 3600)
        );
    }

    #[test]
    fn converts_gps_with_hemispheres_and_ignores_null_island() {
        let fields = ExifFields {
            gps_latitude: Some(([(38, 1), (43, 1), (2028, 100)], "N".into())),
            gps_longitude: Some(([(9, 1), (8, 1), (2148, 100)], "W".into())),
            ..Default::default()
        };
        let gps = fields.to_metadata().gps.unwrap();

        assert!(
            (gps.latitude() - 38.7223).abs() < 1e-4,
            "{}",
            gps.latitude()
        );
        assert!(
            (gps.longitude() + 9.1393).abs() < 1e-4,
            "{}",
            gps.longitude()
        );

        let zero = ExifFields {
            gps_latitude: Some(([(0, 1), (0, 1), (0, 1)], "N".into())),
            gps_longitude: Some(([(0, 1), (0, 1), (0, 1)], "E".into())),
            ..Default::default()
        };
        assert_eq!(zero.to_metadata().gps, None);
    }

    #[test]
    fn converts_camera_lens_exposure_orientation_and_size() {
        let fields = ExifFields {
            make: some("NIKON CORPORATION\0"),
            model: some("NIKON Z 8   "),
            lens: some(" NIKKOR Z 24-70mm f/2.8 S "),
            iso: Some(64),
            f_number: Some((28, 10)),
            exposure_time: Some((1, 250)),
            focal_length: Some((350, 10)),
            orientation: Some(8),
            width: Some(8256),
            height: Some(5504),
            ..Default::default()
        };
        let m = fields.to_metadata();

        assert_eq!(m.camera.unwrap().display_name(), "Nikon Z 8");
        assert_eq!(m.lens.as_deref(), Some("NIKKOR Z 24-70mm f/2.8 S"));
        assert_eq!(m.exposure.iso, Some(64));
        assert_eq!(m.exposure.f_number, Some(2.8));
        assert_eq!(
            m.exposure.exposure_time,
            Some(ExposureTime {
                numerator: 1,
                denominator: 250
            })
        );
        assert_eq!(m.exposure.focal_length_mm, Some(35.0));
        assert_eq!(m.orientation, Orientation::Rotate270);
        assert_eq!(m.dimensions, Some((8256, 5504)));
    }

    #[test]
    fn drops_blank_and_zero_values() {
        let fields = ExifFields {
            make: some("  "),
            lens: some("\0\0"),
            iso: Some(0),
            f_number: Some((0, 1)),
            exposure_time: Some((1, 0)),
            focal_length: Some((10, 0)),
            width: Some(0),
            height: Some(100),
            ..Default::default()
        };
        let m = fields.to_metadata();

        assert_eq!(m.camera, None);
        assert_eq!(m.lens, None);
        assert_eq!(m.exposure, Exposure::default());
        assert_eq!(m.dimensions, None);
    }
}
