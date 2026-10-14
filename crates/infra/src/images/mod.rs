//! Image decoding with libvips and LibRaw (ADR-0005).

#[expect(
    unsafe_code,
    reason = "LibRaw has no maintained Rust bindings; see ADR-0005"
)]
mod libraw;
#[expect(
    unsafe_code,
    reason = "the libvips crate does not build on Windows; see ADR-0005"
)]
mod vips;

use std::path::Path;

use picturesque_app::ports::images::{ImageDecoder, Rendered};
use picturesque_app::ports::pixels::PixelLoader;
use picturesque_core::media::MediaFormat;
use picturesque_core::metadata::Orientation;
use picturesque_core::pixels::RgbImage;

use libraw::Preview;
use vips::{Angle, Image};

const JPEG_QUALITY: i32 = 85;

/// [`ImageDecoder`] using LibRaw's embedded previews for RAW files and
/// libvips for everything else.
#[derive(Debug, Default, Clone, Copy)]
pub struct VipsDecoder;

impl ImageDecoder for VipsDecoder {
    fn render(
        &self,
        source: &Path,
        format: MediaFormat,
        orientation: Orientation,
        max_edge: u32,
        dest: &Path,
    ) -> Result<Rendered, String> {
        save_jpeg(
            source,
            format,
            orientation,
            max_edge,
            JPEG_QUALITY,
            false,
            dest,
        )
    }

    fn export_jpeg(
        &self,
        source: &Path,
        format: MediaFormat,
        orientation: Orientation,
        max_edge: u32,
        quality: u8,
        dest: &Path,
    ) -> Result<Rendered, String> {
        save_jpeg(
            source,
            format,
            orientation,
            max_edge,
            i32::from(quality.clamp(1, 100)),
            true,
            dest,
        )
    }
}

fn save_jpeg(
    source: &Path,
    format: MediaFormat,
    orientation: Orientation,
    max_edge: u32,
    quality: i32,
    keep_metadata: bool,
    dest: &Path,
) -> Result<Rendered, String> {
    let edge = i32::try_from(max_edge).map_err(|e| e.to_string())?;
    let (image, source_size) = match format {
        MediaFormat::Raw => render_raw(source, orientation, edge)?,
        MediaFormat::Xmp => return Err("XMP sidecars are not images".into()),
        _ => render_file(source, edge)?,
    };
    image.save_jpeg(dest, quality, keep_metadata)?;
    Ok(Rendered {
        width: image.width(),
        height: image.height(),
        source_size,
    })
}

/// [`PixelLoader`] decoding with libvips (used on cached preview JPEGs).
#[derive(Debug, Default, Clone, Copy)]
pub struct VipsPixels;

impl PixelLoader for VipsPixels {
    fn load(&self, path: &Path, max_edge: u32) -> Result<RgbImage, String> {
        let edge = i32::try_from(max_edge).map_err(|e| e.to_string())?;
        let (mut image, _) = render_file(path, edge)?;
        if image.bands() == 4 || image.bands() == 2 {
            image = image.flatten()?;
        }
        if image.bands() != 3 {
            image = image.to_srgb()?;
        }
        let image = image.to_uchar()?;
        RgbImage::new(image.width(), image.height(), image.pixels()?)
            .ok_or_else(|| "unexpected pixel layout".to_owned())
    }
}

/// JPEG, HEIC, PNG, TIFF, WebP: shrink-on-load, auto-rotated from EXIF.
fn render_file(source: &Path, edge: i32) -> Result<(Image, Option<(u32, u32)>), String> {
    let image = Image::thumbnail(source, edge)?;
    // The header gives the stored size; arrange it to match the upright output.
    let header = Image::from_file(source).ok();
    let source_size = header.map(|h| {
        let (w, h) = (h.width(), h.height());
        let landscape = image.width() >= image.height();
        if landscape == (w >= h) {
            (w, h)
        } else {
            (h, w)
        }
    });
    Ok((image, source_size))
}

/// RAW: the embedded preview (fast), rotated by the photo's orientation;
/// files without one are fully decoded by libvips (LibRaw, upright).
fn render_raw(
    source: &Path,
    orientation: Orientation,
    edge: i32,
) -> Result<(Image, Option<(u32, u32)>), String> {
    // Previews may carry their own EXIF orientation; `shrink` ignores it and
    // the photo's orientation is applied exactly once, below.
    let full = match libraw::preview(source)? {
        Some(Preview::Jpeg(bytes)) => Image::from_buffer(bytes)?,
        Some(Preview::Bitmap {
            width,
            height,
            bands,
            pixels,
        }) => Image::from_pixels(&pixels, width, height, bands)?,
        None => return render_file(source, edge),
    };
    let size = (full.width(), full.height());
    let preview = full.shrink(edge, false)?;
    let upright = orient(&preview, orientation)?;
    let size = if orientation.swaps_dimensions() {
        (size.1, size.0)
    } else {
        size
    };
    Ok((upright, Some(size)))
}

/// Applies an EXIF orientation, the same way libvips' `autorot` does.
fn orient(image: &Image, orientation: Orientation) -> Result<Image, String> {
    let (angle, flip) = match orientation {
        Orientation::Normal => return image.copy(),
        Orientation::FlipHorizontal => (Angle::D0, true),
        Orientation::Rotate180 => (Angle::D180, false),
        Orientation::FlipVertical => (Angle::D180, true),
        Orientation::Transpose => (Angle::D90, true),
        Orientation::Rotate90 => (Angle::D90, false),
        Orientation::Transverse => (Angle::D270, true),
        Orientation::Rotate270 => (Angle::D270, false),
    };
    let rotated = image.rotate(angle)?;
    if flip { rotated.flip() } else { Ok(rotated) }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    /// 8×8 grayscale signature of an image.
    fn signature(image: &Image) -> Vec<u8> {
        image
            .shrink(8, true)
            .unwrap()
            .to_grey()
            .unwrap()
            .pixels()
            .unwrap()
    }

    fn diff(a: &[u8], b: &[u8]) -> f64 {
        a.iter()
            .zip(b)
            .map(|(x, y)| (f64::from(*x) - f64::from(*y)).abs())
            .sum::<f64>()
            / a.len() as f64
    }

    #[test]
    fn manual_orientation_matches_libvips_autorot() {
        let reference = signature(&Image::from_file(&fixture("orient_1.jpg")).unwrap());
        for value in 1..=8u16 {
            // Stored pixels, not auto-rotated.
            let stored = Image::from_file(&fixture(&format!("orient_{value}.jpg"))).unwrap();
            let upright = orient(&stored, Orientation::from_exif(value)).unwrap();

            let d = diff(&signature(&upright), &reference);
            assert!(d < 12.0, "orientation {value}: difference {d}");
        }
    }
}
