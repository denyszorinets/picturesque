//! EXIF from image containers (JPEG, HEIF, PNG, TIFF, WebP) via
//! `kamadak-exif`.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use exif::{Exif, Field, In, Tag, Value};

use super::fields::{ExifFields, Ratio};

/// Reads EXIF fields. A file without EXIF, or whose EXIF cannot be
/// parsed, gives empty fields; only failing to open the file is an error.
pub fn read(path: &Path) -> Result<ExifFields, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let Ok(exif) = exif::Reader::new().read_from_container(&mut BufReader::new(file)) else {
        return Ok(ExifFields::default());
    };
    Ok(fields(&exif))
}

fn fields(exif: &Exif) -> ExifFields {
    let field = |tag| exif.get_field(tag, In::PRIMARY);
    let text = |tag| field(tag).and_then(ascii);
    let uint = |tag| field(tag).and_then(|f| f.value.get_uint(0));
    let ratio = |tag| field(tag).and_then(|f| rationals(f).and_then(|r| r.first().copied()));
    let triple = |tag| {
        field(tag)
            .and_then(rationals)
            .and_then(|r| <[Ratio; 3]>::try_from(r.get(..3)?).ok())
    };
    let gps = |value_tag, ref_tag| Some((triple(value_tag)?, text(ref_tag)?));

    ExifFields {
        date_time_original: text(Tag::DateTimeOriginal),
        date_time_digitized: text(Tag::DateTimeDigitized),
        date_time: text(Tag::DateTime),
        offset_original: text(Tag::OffsetTimeOriginal),
        offset_digitized: text(Tag::OffsetTimeDigitized),
        offset: text(Tag::OffsetTime),
        subsec_original: text(Tag::SubSecTimeOriginal),
        subsec_digitized: text(Tag::SubSecTimeDigitized),
        subsec: text(Tag::SubSecTime),
        make: text(Tag::Make),
        model: text(Tag::Model),
        lens: text(Tag::LensModel),
        iso: uint(Tag::PhotographicSensitivity),
        f_number: ratio(Tag::FNumber),
        exposure_time: ratio(Tag::ExposureTime),
        focal_length: ratio(Tag::FocalLength),
        orientation: uint(Tag::Orientation).and_then(|v| u16::try_from(v).ok()),
        gps_latitude: gps(Tag::GPSLatitude, Tag::GPSLatitudeRef),
        gps_longitude: gps(Tag::GPSLongitude, Tag::GPSLongitudeRef),
        gps_date: text(Tag::GPSDateStamp),
        gps_time: triple(Tag::GPSTimeStamp),
        width: uint(Tag::PixelXDimension),
        height: uint(Tag::PixelYDimension),
    }
}

fn ascii(field: &Field) -> Option<String> {
    match &field.value {
        Value::Ascii(parts) => parts
            .first()
            .map(|bytes| String::from_utf8_lossy(bytes).into_owned()),
        _ => None,
    }
}

fn rationals(field: &Field) -> Option<Vec<Ratio>> {
    match &field.value {
        Value::Rational(values) => Some(values.iter().map(|r| (r.num, r.denom)).collect()),
        _ => None,
    }
}
