//! Metadata from camera RAW files via `rawler`, which reads every RAW
//! format in the ADR-0005 corpus (including CR3, RAF, ORF and RW2, which
//! generic EXIF readers do not) and normalizes lens names from its
//! database.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use rawler::decoders::RawDecodeParams;
use rawler::formats::tiff::Rational;
use rawler::rawsource::RawSource;

use super::fields::{ExifFields, Ratio};

pub fn read(path: &Path) -> Result<ExifFields, String> {
    // A malformed file must not take the whole import down with it.
    catch_unwind(AssertUnwindSafe(|| read_inner(path)))
        .unwrap_or_else(|_| Err("the RAW decoder failed on this file".into()))
}

fn read_inner(path: &Path) -> Result<ExifFields, String> {
    let source = RawSource::new(path).map_err(|e| e.to_string())?;
    let decoder = rawler::get_decoder(&source).map_err(|e| e.to_string())?;
    let meta = decoder
        .raw_metadata(&source, &RawDecodeParams::default())
        .map_err(|e| e.to_string())?;
    let exif = meta.exif;
    let ratio = |r: Rational| -> Ratio { (r.n, r.d) };
    let gps = exif.gps.as_ref();
    let coordinate = |value: Option<&[Rational; 3]>, reference: Option<&String>| {
        Some((value?.map(ratio), reference?.clone()))
    };

    Ok(ExifFields {
        date_time_original: exif.date_time_original,
        date_time_digitized: exif.create_date,
        date_time: exif.modify_date,
        offset_original: exif.offset_time_original,
        offset_digitized: exif.offset_time_digitized,
        offset: exif.offset_time,
        subsec_original: exif.sub_sec_time_original,
        subsec_digitized: exif.sub_sec_time_digitized,
        subsec: exif.sub_sec_time,
        make: Some(meta.make),
        model: Some(meta.model),
        lens: exif.lens_model,
        iso: exif
            .iso_speed_ratings
            .map(u32::from)
            .or(exif.iso_speed)
            .or(exif.recommended_exposure_index),
        f_number: exif.fnumber.map(ratio),
        exposure_time: exif.exposure_time.map(ratio),
        focal_length: exif.focal_length.map(ratio),
        orientation: exif.orientation,
        gps_latitude: gps
            .and_then(|g| coordinate(g.gps_latitude.as_ref(), g.gps_latitude_ref.as_ref())),
        gps_longitude: gps
            .and_then(|g| coordinate(g.gps_longitude.as_ref(), g.gps_longitude_ref.as_ref())),
        gps_date: gps.and_then(|g| g.gps_date_stamp.clone()),
        gps_time: gps.and_then(|g| g.gps_timestamp.map(|t| t.map(ratio))),
        width: None,
        height: None,
    })
}
