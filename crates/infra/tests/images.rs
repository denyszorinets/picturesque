mod support;

use std::path::{Path, PathBuf};
use std::time::Instant;

use picturesque_app::ports::images::ImageDecoder;
use picturesque_app::ports::metadata::MetadataReader;
use picturesque_app::ports::pixels::PixelLoader;
use picturesque_core::media::MediaFormat;
use picturesque_core::metadata::Orientation;
use picturesque_infra::images::{VipsDecoder, VipsPixels};
use picturesque_infra::metadata::ExifMetadataReader;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn gray_signature(path: &Path) -> Vec<u8> {
    image::open(path)
        .unwrap()
        .resize_exact(8, 8, image::imageops::FilterType::Triangle)
        .to_luma8()
        .into_raw()
}

fn diff(a: &[u8], b: &[u8]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (f64::from(*x) - f64::from(*y)).abs())
        .sum::<f64>()
        / a.len() as f64
}

#[test]
fn renders_every_exif_orientation_upright() {
    let dir = tempfile::tempdir().unwrap();
    let reference = dir.path().join("ref.jpg");
    VipsDecoder
        .render(
            &fixture("orient_1.jpg"),
            MediaFormat::Jpeg,
            Orientation::Normal,
            64,
            &reference,
        )
        .unwrap();
    let reference = gray_signature(&reference);

    for value in 1..=8 {
        let out = dir.path().join(format!("{value}.jpg"));
        // Orientation is read from the file for JPEGs; the argument is ignored.
        let r = VipsDecoder
            .render(
                &fixture(&format!("orient_{value}.jpg")),
                MediaFormat::Jpeg,
                Orientation::Normal,
                64,
                &out,
            )
            .unwrap();

        assert!(r.width > r.height, "orientation {value}: {r:?}");
        assert_eq!(r.source_size, Some((144, 96)), "orientation {value}");
        let d = diff(&gray_signature(&out), &reference);
        assert!(d < 12.0, "orientation {value}: difference {d}");
    }
}

#[test]
fn shrinks_but_never_enlarges() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("a.jpg");
    support::write_jpeg(&src, 32, 24, &[]);

    let small = VipsDecoder
        .render(
            &src,
            MediaFormat::Jpeg,
            Orientation::Normal,
            16,
            &dir.path().join("s.jpg"),
        )
        .unwrap();
    let large = VipsDecoder
        .render(
            &src,
            MediaFormat::Jpeg,
            Orientation::Normal,
            512,
            &dir.path().join("l.jpg"),
        )
        .unwrap();

    assert_eq!((small.width, small.height), (16, 12));
    assert_eq!((large.width, large.height), (32, 24));
    assert_eq!(
        image::image_dimensions(dir.path().join("s.jpg")).unwrap(),
        (16, 12)
    );
}

#[test]
#[cfg_attr(
    windows,
    ignore = "the libvips build for Windows has no HEVC decoder (issue #135)"
)]
fn decodes_heic_and_reports_its_displayed_size() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("h.jpg");

    let r = VipsDecoder
        .render(
            &fixture("exif.heic"),
            MediaFormat::Heif,
            Orientation::Normal,
            512,
            &out,
        )
        .unwrap();

    // The HEVC data is padded to 64×64; the displayed image is 64×48.
    assert_eq!((r.width, r.height), (64, 48));
    assert_eq!(r.source_size, Some((64, 48)));
}

#[test]
fn unreadable_files_are_errors() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.jpg");
    std::fs::write(&bad, b"not an image").unwrap();
    let fake_raw = dir.path().join("bad.CR3");
    std::fs::write(&fake_raw, b"not a raw").unwrap();

    assert!(
        VipsDecoder
            .render(
                &bad,
                MediaFormat::Jpeg,
                Orientation::Normal,
                64,
                &dir.path().join("o.jpg")
            )
            .is_err()
    );
    assert!(
        VipsDecoder
            .render(
                &fake_raw,
                MediaFormat::Raw,
                Orientation::Normal,
                64,
                &dir.path().join("o.jpg")
            )
            .is_err()
    );
    assert!(!dir.path().join("o.jpg").exists());
}

/// `cargo test -p picturesque-infra --test images -- --ignored --nocapture`
/// after fetching the corpus (see docs/development/building.rst).
#[test]
#[ignore = "needs the spike corpus (~630 MB)"]
fn renders_every_raw_in_the_corpus() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spikes/decoding/corpus/raw");
    let dir = tempfile::tempdir().unwrap();
    let mut paths: Vec<_> = std::fs::read_dir(&corpus)
        .expect("corpus present")
        .map(|e| e.unwrap().path())
        .collect();
    paths.sort();
    for path in paths {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let out = dir.path().join("out.jpg");
        let orientation = ExifMetadataReader
            .read(&path, MediaFormat::Raw)
            .unwrap()
            .orientation;
        let start = Instant::now();
        let r = VipsDecoder
            .render(&path, MediaFormat::Raw, orientation, 512, &out)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        // Portrait photos (orientation 5–8) come out taller than wide.
        assert_eq!(
            r.height > r.width,
            orientation.swaps_dimensions(),
            "{name}: {r:?} {orientation:?}"
        );
        let ms = start.elapsed().as_millis();
        assert_eq!(r.width.max(r.height), 512, "{name}: {r:?}");
        println!(
            "{name:60} {:>4}x{:<4} source {:?} {ms} ms",
            r.width, r.height, r.source_size
        );
    }
}

fn mean_rgb(data: &[u8]) -> [f64; 3] {
    let mut sum = [0.0; 3];
    for px in data.as_chunks::<3>().0 {
        for c in 0..3 {
            sum[c] += f64::from(px[c]);
        }
    }
    let n = (data.len() / 3) as f64;
    sum.map(|s| s / n)
}

#[test]
fn loads_shrunken_rgb_pixels() {
    let path = fixture("orient_1.jpg");
    let reference = image::open(&path).unwrap().to_rgb8();

    let pixels = VipsPixels.load(&path, 64).unwrap();

    let (w, h) = reference.dimensions();
    assert_eq!(pixels.width().max(pixels.height()), 64);
    let ratio = f64::from(pixels.width()) / f64::from(pixels.height());
    assert!(
        (ratio - f64::from(w) / f64::from(h)).abs() < 0.05,
        "{ratio}"
    );
    let (a, b) = (mean_rgb(pixels.data()), mean_rgb(reference.as_raw()));
    for c in 0..3 {
        assert!((a[c] - b[c]).abs() < 4.0, "{a:?} vs {b:?}");
    }
}

#[test]
fn loads_greyscale_and_transparent_images_as_rgb() {
    let dir = tempfile::tempdir().unwrap();
    let grey = dir.path().join("grey.png");
    image::GrayImage::from_pixel(40, 20, image::Luma([90]))
        .save(&grey)
        .unwrap();
    let alpha = dir.path().join("alpha.png");
    image::RgbaImage::from_pixel(20, 40, image::Rgba([200, 10, 10, 255]))
        .save(&alpha)
        .unwrap();

    let grey = VipsPixels.load(&grey, 100).unwrap();
    assert_eq!((grey.width(), grey.height()), (40, 20), "never enlarged");
    assert_eq!(grey.pixel(5, 5), [90, 90, 90]);

    let alpha = VipsPixels.load(&alpha, 100).unwrap();
    assert_eq!((alpha.width(), alpha.height()), (20, 40));
    assert_eq!(alpha.pixel(3, 3), [200, 10, 10]);

    assert!(
        VipsPixels
            .load(&dir.path().join("missing.jpg"), 64)
            .is_err()
    );
}
