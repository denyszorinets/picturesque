//! Image decoding with libvips and LibRaw (ADR-0005).

#[expect(
    unsafe_code,
    reason = "LibRaw has no maintained Rust bindings; see ADR-0005"
)]
mod libraw;

use std::path::Path;
use std::sync::OnceLock;

use libvips::ops::{self, Angle, Direction, ForeignKeep, JpegsaveOptions};
use libvips::{VipsApp, VipsImage};
use picturesque_app::ports::images::{ImageDecoder, Rendered};
use picturesque_app::ports::pixels::PixelLoader;
use picturesque_core::media::MediaFormat;
use picturesque_core::metadata::Orientation;
use picturesque_core::pixels::RgbImage;

use libraw::Preview;

const JPEG_QUALITY: i32 = 85;

/// libvips must be initialized once per process.
fn vips() -> &'static VipsApp {
    static APP: OnceLock<VipsApp> = OnceLock::new();
    APP.get_or_init(|| {
        let app = VipsApp::new("picturesque", false).expect("libvips initializes");
        // Each image is rendered once; the operation cache would only use memory.
        app.cache_set_max(0);
        app.cache_set_max_mem(0);
        app
    })
}

fn vips_error(what: &str) -> String {
    let app = vips();
    let msg = app
        .error_buffer()
        .unwrap_or("")
        .lines()
        .next()
        .unwrap_or("")
        .to_owned();
    app.error_clear();
    if msg.is_empty() {
        what.to_owned()
    } else {
        format!("{what}: {msg}")
    }
}

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
        vips();
        let edge = i32::try_from(max_edge).map_err(|e| e.to_string())?;
        // libvips reads lazily from buffers it does not copy: `_buffer` must
        // outlive every use of `image`, including the save below.
        let mut _buffer: Option<Vec<u8>> = None;
        let (image, source_size) = match format {
            MediaFormat::Raw => render_raw(source, orientation, edge, &mut _buffer)?,
            MediaFormat::Xmp => return Err("XMP sidecars are not images".into()),
            _ => render_file(source, edge)?,
        };
        let options = JpegsaveOptions {
            q: JPEG_QUALITY,
            keep: ForeignKeep::None,
            ..JpegsaveOptions::default()
        };
        ops::jpegsave_with_opts(&image, &dest.to_string_lossy(), &options)
            .map_err(|_| vips_error("save"))?;
        Ok(Rendered {
            width: image.get_width() as u32,
            height: image.get_height() as u32,
            source_size,
        })
    }
}

/// [`PixelLoader`] decoding with libvips (used on cached preview JPEGs).
#[derive(Debug, Default, Clone, Copy)]
pub struct VipsPixels;

impl PixelLoader for VipsPixels {
    fn load(&self, path: &Path, max_edge: u32) -> Result<RgbImage, String> {
        vips();
        let edge = i32::try_from(max_edge).map_err(|e| e.to_string())?;
        let (mut image, _) = render_file(path, edge)?;
        if image.get_bands() == 4 || image.get_bands() == 2 {
            image = ops::flatten(&image).map_err(|_| vips_error("flatten"))?;
        }
        if image.get_bands() != 3 {
            image = ops::colourspace(&image, ops::Interpretation::Srgb)
                .map_err(|_| vips_error("colourspace"))?;
        }
        let image = ops::cast(&image, ops::BandFormat::Uchar).map_err(|_| vips_error("cast"))?;
        let (width, height) = (image.get_width() as u32, image.get_height() as u32);
        RgbImage::new(width, height, image.image_write_to_memory())
            .ok_or_else(|| "unexpected pixel layout".to_owned())
    }
}

/// JPEG, HEIC, PNG, TIFF, WebP: shrink-on-load, auto-rotated from EXIF.
fn render_file(source: &Path, edge: i32) -> Result<(VipsImage, Option<(u32, u32)>), String> {
    let path = source.to_string_lossy();
    let image = ops::thumbnail_with_opts(
        &path,
        edge,
        &ops::ThumbnailOptions {
            height: edge,
            size: ops::Size::Down,
            ..ops::ThumbnailOptions::default()
        },
    )
    .map_err(|_| vips_error("decode"))?;
    // The header gives the stored size; arrange it to match the upright output.
    let header = VipsImage::new_from_file(&path).ok();
    let source_size = header.map(|h| {
        let (w, h) = (h.get_width() as u32, h.get_height() as u32);
        let landscape = image.get_width() >= image.get_height();
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
    buffer: &mut Option<Vec<u8>>,
) -> Result<(VipsImage, Option<(u32, u32)>), String> {
    // Previews may carry their own EXIF orientation; ignore it and apply the
    // photo's orientation exactly once, below.
    let options = ops::ThumbnailImageOptions {
        height: edge,
        size: ops::Size::Down,
        no_rotate: true,
        ..Default::default()
    };
    let (preview, full) = match libraw::preview(source)? {
        Some(Preview::Jpeg(bytes)) => {
            // Kept alive by the caller until the image has been saved.
            let bytes = buffer.insert(bytes);
            let full =
                VipsImage::new_from_buffer(bytes, "").map_err(|_| vips_error("read preview"))?;
            let size = (full.get_width() as u32, full.get_height() as u32);
            (
                ops::thumbnail_image_with_opts(&full, edge, &options)
                    .map_err(|_| vips_error("shrink preview"))?,
                size,
            )
        }
        Some(Preview::Bitmap {
            width,
            height,
            bands,
            pixels,
        }) => {
            let full = VipsImage::new_from_memory_copy(
                &pixels,
                width as i32,
                height as i32,
                bands as i32,
                ops::BandFormat::Uchar,
            )
            .map_err(|_| vips_error("read preview"))?;
            (
                ops::thumbnail_image_with_opts(&full, edge, &options)
                    .map_err(|_| vips_error("shrink preview"))?,
                (width, height),
            )
        }
        None => {
            let (image, size) = render_file(source, edge)?;
            return Ok((image, size));
        }
    };
    let upright = orient(&preview, orientation)?;
    let size = if orientation.swaps_dimensions() {
        (full.1, full.0)
    } else {
        full
    };
    Ok((upright, Some(size)))
}

/// Applies an EXIF orientation, the same way libvips' `autorot` does.
fn orient(image: &VipsImage, orientation: Orientation) -> Result<VipsImage, String> {
    let (angle, flip) = match orientation {
        Orientation::Normal => return copy(image),
        Orientation::FlipHorizontal => (Angle::D0, true),
        Orientation::Rotate180 => (Angle::D180, false),
        Orientation::FlipVertical => (Angle::D180, true),
        Orientation::Transpose => (Angle::D90, true),
        Orientation::Rotate90 => (Angle::D90, false),
        Orientation::Transverse => (Angle::D270, true),
        Orientation::Rotate270 => (Angle::D270, false),
    };
    let rotated = ops::rot(image, angle).map_err(|_| vips_error("rotate"))?;
    if flip {
        ops::flip(&rotated, Direction::Horizontal).map_err(|_| vips_error("flip"))
    } else {
        Ok(rotated)
    }
}

fn copy(image: &VipsImage) -> Result<VipsImage, String> {
    ops::copy(image).map_err(|_| vips_error("copy"))
}

#[cfg(test)]
pub(crate) fn orient_for_test(
    image: &VipsImage,
    orientation: Orientation,
) -> Result<VipsImage, String> {
    vips();
    orient(image, orientation)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn fixture(name: &str) -> String {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
            .to_string_lossy()
            .into_owned()
    }

    /// 8×8 grayscale signature of an image.
    fn signature(image: &VipsImage) -> Vec<u8> {
        let small = ops::thumbnail_image_with_opts(
            image,
            8,
            &ops::ThumbnailImageOptions {
                height: 8,
                size: ops::Size::Force,
                no_rotate: true,
                ..Default::default()
            },
        )
        .unwrap();
        let gray = ops::colourspace(&small, ops::Interpretation::BW).unwrap();
        gray.image_write_to_memory()
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
        vips();
        let reference = signature(&VipsImage::new_from_file(&fixture("orient_1.jpg")).unwrap());
        for value in 1..=8u16 {
            // Stored pixels, not auto-rotated.
            let stored =
                VipsImage::new_from_file(&fixture(&format!("orient_{value}.jpg"))).unwrap();
            let upright = orient_for_test(&stored, Orientation::from_exif(value)).unwrap();

            let d = diff(&signature(&upright), &reference);
            assert!(d < 12.0, "orientation {value}: difference {d}");
        }
    }
}
