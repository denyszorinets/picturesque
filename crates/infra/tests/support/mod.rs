//! Test fixtures built at test time.

#![allow(dead_code)]

use std::io::Cursor;
use std::path::Path;

use exif::experimental::Writer;
use exif::{Field, In, Rational, Tag, Value};

/// An EXIF field to write into a test JPEG.
pub enum ExifValue {
    Text(&'static str),
    Short(u16),
    Rationals(Vec<(u32, u32)>),
}

/// Writes a small JPEG (`width`×`height`, gray) with the given EXIF fields.
pub fn write_jpeg(path: &Path, width: u32, height: u32, fields: &[(Tag, ExifValue)]) {
    let mut jpeg = Vec::new();
    image::DynamicImage::new_luma8(width, height)
        .write_to(&mut Cursor::new(&mut jpeg), image::ImageFormat::Jpeg)
        .unwrap();
    if fields.is_empty() {
        std::fs::write(path, jpeg).unwrap();
        return;
    }

    let fields: Vec<Field> = fields
        .iter()
        .map(|(tag, value)| Field {
            tag: *tag,
            ifd_num: In::PRIMARY,
            value: match value {
                ExifValue::Text(s) => Value::Ascii(vec![s.as_bytes().to_vec()]),
                ExifValue::Short(v) => Value::Short(vec![*v]),
                ExifValue::Rationals(rs) => Value::Rational(
                    rs.iter()
                        .map(|&(num, denom)| Rational { num, denom })
                        .collect(),
                ),
            },
        })
        .collect();
    let mut writer = Writer::new();
    for f in &fields {
        writer.push_field(f);
    }
    let mut tiff = Cursor::new(Vec::new());
    writer.write(&mut tiff, false).unwrap();
    let tiff = tiff.into_inner();

    // SOI, then an APP1 "Exif" segment, then the rest of the encoded JPEG.
    let mut out = vec![0xFF, 0xD8, 0xFF, 0xE1];
    let len = u16::try_from(2 + 6 + tiff.len()).unwrap();
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(b"Exif\0\0");
    out.extend_from_slice(&tiff);
    out.extend_from_slice(&jpeg[2..]);
    std::fs::write(path, out).unwrap();
}
