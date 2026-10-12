mod support;

use std::path::{Path, PathBuf};

use chrono::{FixedOffset, NaiveDateTime};
use exif::Tag;
use picturesque_app::ports::metadata::MetadataReader;
use picturesque_core::media::MediaFormat;
use picturesque_core::metadata::Orientation;
use picturesque_infra::metadata::ExifMetadataReader;
use support::{ExifValue::*, write_jpeg};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn local(s: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f").unwrap()
}

#[test]
fn reads_a_jpeg_with_full_exif() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.jpg");
    write_jpeg(
        &path,
        32,
        24,
        &[
            (Tag::Make, Text("Canon")),
            (Tag::Model, Text("Canon EOS R5")),
            (Tag::Orientation, Short(6)),
            (Tag::DateTimeOriginal, Text("2026:10:12 14:30:05")),
            (Tag::OffsetTimeOriginal, Text("+01:00")),
            (Tag::SubSecTimeOriginal, Text("5")),
            (Tag::LensModel, Text("RF24-70mm F2.8 L IS USM")),
            (Tag::PhotographicSensitivity, Short(400)),
            (Tag::FNumber, Rationals(vec![(28, 10)])),
            (Tag::ExposureTime, Rationals(vec![(1, 250)])),
            (Tag::FocalLength, Rationals(vec![(35, 1)])),
            (Tag::GPSLatitudeRef, Text("N")),
            (
                Tag::GPSLatitude,
                Rationals(vec![(38, 1), (43, 1), (2028, 100)]),
            ),
            (Tag::GPSLongitudeRef, Text("W")),
            (
                Tag::GPSLongitude,
                Rationals(vec![(9, 1), (8, 1), (2148, 100)]),
            ),
        ],
    );

    let m = ExifMetadataReader.read(&path, MediaFormat::Jpeg).unwrap();

    let t = m.captured_at.unwrap();
    assert_eq!(t.local, local("2026-10-12 14:30:05.5"));
    assert_eq!(t.offset, FixedOffset::east_opt(3600));
    assert_eq!(m.camera.as_ref().unwrap().display_name(), "Canon EOS R5");
    assert_eq!(m.lens.as_deref(), Some("RF24-70mm F2.8 L IS USM"));
    assert_eq!(m.exposure.iso, Some(400));
    assert_eq!(m.exposure.f_number, Some(2.8));
    assert_eq!(m.orientation, Orientation::Rotate90);
    assert_eq!(m.dimensions, Some((32, 24)));
    assert_eq!(m.display_dimensions(), Some((24, 32)));
    let gps = m.gps.unwrap();
    assert!((gps.latitude() - 38.7223).abs() < 1e-4);
    assert!((gps.longitude() + 9.1393).abs() < 1e-4);
}

#[test]
fn jpeg_without_exif_uses_file_time_and_header_size() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plain.jpg");
    write_jpeg(&path, 40, 30, &[]);

    let m = ExifMetadataReader.read(&path, MediaFormat::Jpeg).unwrap();

    assert_eq!(m.dimensions, Some((40, 30)));
    assert_eq!(m.camera, None);
    let t = m.captured_at.expect("file time fallback");
    assert!(t.offset.is_some());
    let age = chrono::Local::now().naive_local() - t.local;
    assert!(age.num_seconds().abs() < 120, "{age}");
}

#[test]
fn reads_exif_from_heic() {
    let m = ExifMetadataReader
        .read(&fixture("exif.heic"), MediaFormat::Heif)
        .unwrap();

    assert_eq!(
        m.camera.as_ref().unwrap().display_name(),
        "Apple iPhone 15 Pro"
    );
    assert_eq!(m.captured_at.unwrap().local, local("2026-10-12 09:15:00"));
    assert_eq!(
        m.captured_at.unwrap().offset,
        FixedOffset::east_opt(-4 * 3600)
    );
    // HEIF dimensions come from the decoder (coded size != displayed size).
    assert_eq!(m.dimensions, None);
}

#[test]
fn unreadable_files_are_errors() {
    let dir = tempfile::tempdir().unwrap();
    let garbage = dir.path().join("broken.jpg");
    std::fs::write(&garbage, b"not an image at all").unwrap();
    let fake_raw = dir.path().join("broken.CR3");
    std::fs::write(&fake_raw, b"not a raw file").unwrap();

    assert!(
        ExifMetadataReader
            .read(&garbage, MediaFormat::Jpeg)
            .is_err()
    );
    assert!(
        ExifMetadataReader
            .read(&fake_raw, MediaFormat::Raw)
            .is_err()
    );
    assert!(
        ExifMetadataReader
            .read(&dir.path().join("missing.jpg"), MediaFormat::Jpeg)
            .is_err()
    );
}

#[test]
fn sidecars_have_no_capture_metadata() {
    assert!(
        ExifMetadataReader
            .read(Path::new("a.xmp"), MediaFormat::Xmp)
            .is_err()
    );
}

/// Runs against the ADR-0005 RAW corpus. Fetch it with
/// `python3 -I spikes/decoding/scripts/fetch_corpus.py`, then run
/// `cargo test -p picturesque-infra --test metadata -- --ignored`.
#[test]
#[ignore = "needs the spike corpus (~630 MB)"]
fn reads_every_raw_in_the_corpus() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spikes/decoding/corpus/raw");
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .expect("corpus present")
        .map(|e| e.unwrap().path())
        .collect();
    paths.sort();
    assert_eq!(paths.len(), 16);
    for path in paths {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let m = ExifMetadataReader
            .read(&path, MediaFormat::Raw)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let t = m
            .captured_at
            .unwrap_or_else(|| panic!("{name}: no capture time"));
        assert!(
            t.local.and_utc().timestamp() > 1_000_000_000,
            "{name}: {t:?}"
        );
        assert!(m.camera.is_some(), "{name}: no camera");
        assert!(m.exposure.iso.is_some(), "{name}: no ISO");
        println!(
            "{name:60} {} {:>7} {:30} iso={:?} f={:?} gps={}",
            t.local,
            t.offset.map(|o| o.to_string()).unwrap_or_default(),
            m.camera.unwrap().display_name(),
            m.exposure.iso,
            m.exposure.f_number,
            m.gps.is_some()
        );
    }
}
