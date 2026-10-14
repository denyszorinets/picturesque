//! Technical quality scores: sharpness, exposure and noise.
//!
//! Scores are computed on a downscaled preview (about 1024 px on the long
//! edge), so the constants below are calibrated for that size. Every score
//! is in `0.0..=1.0`, higher is better; the raw measurements are kept so
//! scores can be re-derived.

use serde::{Deserialize, Serialize};

use crate::pixels::RgbImage;

/// Tiles per side for local measurements.
const GRID: usize = 8;
/// Smallest tile side worth measuring; smaller images use fewer tiles.
const MIN_TILE: usize = 8;
/// Sharpness rises with the logarithm of the Laplacian variance from 0 at
/// `BLURRED` to 1 at `SHARP`. Calibrated on 1024 px previews of camera
/// samples: sharp photos measure about 850–16,000, the same photos slightly
/// soft (1 px blur) 75–650, and clearly blurred (3 px blur) 5–70.
const BLURRED: f32 = 20.0;
const SHARP: f32 = 1000.0;
/// Noise level (luma standard deviation, 0–255) at which the noise score
/// is 0.5. Clean camera samples measure below 1.5; high-ISO ones 3–5.
const NOISE_HALF: f32 = 5.0;
/// Channel values at or above this count as blown highlights.
const HIGHLIGHT: u8 = 250;
/// Pixels whose brightest channel is at or below this are crushed shadows.
const SHADOW: u8 = 5;

/// Technical measurements and scores of one image.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TechnicalScores {
    /// How sharp the sharpest, most central part of the image is.
    pub sharpness: f32,
    /// How well exposed: few clipped highlights or shadows, not too dark or
    /// bright overall.
    pub exposure: f32,
    /// How clean: 1 for no visible noise.
    pub noise: f32,
    /// The three combined, weighted towards sharpness.
    pub overall: f32,
    /// Noise-corrected Laplacian variance of the subject area.
    pub laplacian_variance: f32,
    /// Share of pixels with a blown channel.
    pub highlights_clipped: f32,
    /// Share of pixels that are black.
    pub shadows_clipped: f32,
    /// Mean luma, 0–1.
    pub mean_brightness: f32,
    /// Estimated noise standard deviation in luma levels (0–255).
    pub noise_sigma: f32,
}

/// Measures `image`. Images smaller than 3×3 pixels score 0 for sharpness.
pub fn technical_scores(image: &RgbImage) -> TechnicalScores {
    let (w, h) = (image.width() as usize, image.height() as usize);
    let luma = image.luma();
    let tiles = measure_tiles(&luma, w, h);

    let noise_sigma = noise_sigma(&tiles);
    let laplacian_variance = subject_laplacian_variance(&tiles, noise_sigma);
    let sharpness = ((laplacian_variance.max(1.0).ln() - BLURRED.ln())
        / (SHARP.ln() - BLURRED.ln()))
    .clamp(0.0, 1.0);

    let (highlights_clipped, shadows_clipped) = clipping(image);
    let mean_brightness = if luma.is_empty() {
        0.0
    } else {
        luma.iter().sum::<f32>() / luma.len() as f32 / 255.0
    };
    let exposure = exposure_score(highlights_clipped, shadows_clipped, mean_brightness);
    let noise = 1.0 / (1.0 + (noise_sigma / NOISE_HALF).powi(2));

    let overall =
        sharpness.max(0.01).powf(0.6) * exposure.max(0.01).powf(0.3) * noise.max(0.01).powf(0.1);

    TechnicalScores {
        sharpness,
        exposure,
        noise,
        overall,
        laplacian_variance,
        highlights_clipped,
        shadows_clipped,
        mean_brightness,
        noise_sigma,
    }
}

/// Sums over one tile's interior pixels.
#[derive(Debug, Default, Clone, Copy)]
struct Tile {
    /// Centre of the tile, −1..1 on both axes.
    cx: f32,
    cy: f32,
    count: f32,
    lap_sum: f64,
    lap_sq: f64,
    /// Sum of |Immerkær noise operator|.
    noise_abs: f64,
}

impl Tile {
    fn laplacian_variance(&self) -> f32 {
        if self.count == 0.0 {
            return 0.0;
        }
        let n = f64::from(self.count);
        let mean = self.lap_sum / n;
        (self.lap_sq / n - mean * mean).max(0.0) as f32
    }

    /// Immerkær (1996): σ = √(π/2) · Σ|I ∗ N| / (6 · pixels).
    fn noise_sigma(&self) -> f32 {
        if self.count == 0.0 {
            return 0.0;
        }
        ((std::f64::consts::PI / 2.0).sqrt() * self.noise_abs / (6.0 * f64::from(self.count)))
            as f32
    }
}

fn measure_tiles(luma: &[f32], w: usize, h: usize) -> Vec<Tile> {
    if w < 3 || h < 3 {
        return Vec::new();
    }
    let gx = GRID.min((w / MIN_TILE).max(1));
    let gy = GRID.min((h / MIN_TILE).max(1));
    let mut tiles = vec![Tile::default(); gx * gy];
    for (i, tile) in tiles.iter_mut().enumerate() {
        let (tx, ty) = (i % gx, i / gx);
        tile.cx = (tx as f32 + 0.5) / gx as f32 * 2.0 - 1.0;
        tile.cy = (ty as f32 + 0.5) / gy as f32 * 2.0 - 1.0;
    }
    let at = |x: usize, y: usize| luma[y * w + x];
    for y in 1..h - 1 {
        let ty = (y * gy / h).min(gy - 1);
        for x in 1..w - 1 {
            let tx = (x * gx / w).min(gx - 1);
            let c = at(x, y);
            let (n, s, e, west) = (at(x, y - 1), at(x, y + 1), at(x + 1, y), at(x - 1, y));
            let lap = n + s + e + west - 4.0 * c;
            let diag = at(x - 1, y - 1) + at(x + 1, y - 1) + at(x - 1, y + 1) + at(x + 1, y + 1);
            let noise = diag - 2.0 * (n + s + e + west) + 4.0 * c;
            let tile = &mut tiles[ty * gx + tx];
            tile.count += 1.0;
            tile.lap_sum += f64::from(lap);
            tile.lap_sq += f64::from(lap) * f64::from(lap);
            tile.noise_abs += f64::from(noise.abs());
        }
    }
    tiles
}

/// Noise is measured where the image is flattest: edges inflate the
/// estimate, so the quietest tenth of tiles is used.
fn noise_sigma(tiles: &[Tile]) -> f32 {
    let mut sigmas: Vec<f32> = tiles.iter().map(Tile::noise_sigma).collect();
    if sigmas.is_empty() {
        return 0.0;
    }
    sigmas.sort_by(f32::total_cmp);
    let n = (sigmas.len() / 10).max(1);
    sigmas[..n].iter().sum::<f32>() / n as f32
}

/// The subject is usually the sharpest region and near the centre: tiles
/// are weighted from 1 (centre) to 0.5 (corners), and the best sixteenth
/// is averaged. Noise adds 20σ² to the Laplacian variance of the 4-neighbour
/// kernel; that is subtracted, with a 25 % margin because σ is slightly
/// underestimated and the noisiest tiles are the ones selected, so noisy
/// photos do not look sharp.
fn subject_laplacian_variance(tiles: &[Tile], noise_sigma: f32) -> f32 {
    let noise = 25.0 * noise_sigma * noise_sigma;
    let mut values: Vec<f32> = tiles
        .iter()
        .map(|t| {
            let d2 = (t.cx * t.cx + t.cy * t.cy) / 2.0;
            (t.laplacian_variance() - noise).max(0.0) * (1.0 - 0.5 * d2)
        })
        .collect();
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| b.total_cmp(a));
    let n = (values.len() / 16).max(1);
    values[..n].iter().sum::<f32>() / n as f32
}

fn clipping(image: &RgbImage) -> (f32, f32) {
    let pixels = image.data().as_chunks::<3>().0;
    if pixels.is_empty() {
        return (0.0, 0.0);
    }
    let (mut high, mut low) = (0usize, 0usize);
    for px in pixels {
        let max = px[0].max(px[1]).max(px[2]);
        if max >= HIGHLIGHT {
            high += 1;
        } else if max <= SHADOW {
            low += 1;
        }
    }
    let n = pixels.len() as f32;
    (high as f32 / n, low as f32 / n)
}

/// Small amounts of clipping are normal (specular highlights, deep
/// shadows); penalties start above 0.5 % blown and 2 % black pixels, and
/// for mean brightness outside 20–70 %.
fn exposure_score(highlights: f32, shadows: f32, mean: f32) -> f32 {
    let high = ((highlights - 0.005) / 0.10).clamp(0.0, 1.0);
    let low = ((shadows - 0.02) / 0.20).clamp(0.0, 1.0);
    let off = ((mean - 0.45).abs() - 0.25).max(0.0) / 0.25;
    (1.0 - 0.6 * high - 0.4 * low - 0.5 * off.min(1.0)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A detailed test scene: fine stripes, circles and a gradient.
    fn scene(w: u32, h: u32) -> RgbImage {
        RgbImage::from_fn(w, h, |x, y| {
            let (fx, fy) = (x as f32, y as f32);
            let stripes = if (x / 3 + y / 7) % 2 == 0 { 60.0 } else { 0.0 };
            let (dx, dy) = (fx - w as f32 / 2.0, fy - h as f32 / 2.0);
            let rings = if ((dx * dx + dy * dy).sqrt() as u32 / 5).is_multiple_of(2) {
                50.0
            } else {
                0.0
            };
            let base = 60.0 + 60.0 * fx / w as f32;
            let v = (base + stripes + rings) as u8;
            [v, v.saturating_sub(10), v.saturating_add(10)]
        })
    }

    fn box_blur(image: &RgbImage, radius: u32) -> RgbImage {
        let (w, h) = (image.width(), image.height());
        RgbImage::from_fn(w, h, |x, y| {
            let mut sum = [0u32; 3];
            let mut n = 0;
            for yy in y.saturating_sub(radius)..(y + radius + 1).min(h) {
                for xx in x.saturating_sub(radius)..(x + radius + 1).min(w) {
                    let p = image.pixel(xx, yy);
                    for c in 0..3 {
                        sum[c] += u32::from(p[c]);
                    }
                    n += 1;
                }
            }
            sum.map(|s| (s / n) as u8)
        })
    }

    /// Adds deterministic pseudo-random noise of about `sigma` levels.
    fn add_noise(image: &RgbImage, sigma: f32) -> RgbImage {
        let mut state = 0x2545_f491_u32;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as f32 / u32::MAX as f32
        };
        // Sum of 12 uniforms − 6 ≈ standard normal.
        let noise: Vec<f32> = (0..image.width() * image.height())
            .map(|_| (0..12).map(|_| next()).sum::<f32>() - 6.0)
            .collect();
        RgbImage::from_fn(image.width(), image.height(), |x, y| {
            let n = noise[(y * image.width() + x) as usize];
            image
                .pixel(x, y)
                .map(|v| (f32::from(v) + n * sigma).round().clamp(0.0, 255.0) as u8)
        })
    }

    #[test]
    fn separates_sharp_from_blurred() {
        let sharp = scene(320, 213);
        let soft = box_blur(&sharp, 1);
        let blurred = box_blur(&sharp, 4);

        let (a, b, c) = (
            technical_scores(&sharp),
            technical_scores(&soft),
            technical_scores(&blurred),
        );

        assert!(a.sharpness > 0.9, "{a:?}");
        assert!(a.sharpness > b.sharpness && b.sharpness > c.sharpness);
        assert!(c.sharpness < 0.3, "{c:?}");
        assert!(a.overall > c.overall);
    }

    #[test]
    fn a_sharp_subject_on_a_blurred_background_is_sharp() {
        let sharp = scene(320, 213);
        let background = box_blur(&sharp, 6);
        let portrait = RgbImage::from_fn(320, 213, |x, y| {
            let inside = (120..200).contains(&x) && (70..150).contains(&y);
            if inside {
                sharp.pixel(x, y)
            } else {
                background.pixel(x, y)
            }
        });

        let s = technical_scores(&portrait);
        assert!(s.sharpness > 0.8, "{s:?}");
    }

    #[test]
    fn noise_is_estimated_and_not_mistaken_for_detail() {
        let soft = box_blur(&scene(640, 427), 4);
        let clean = technical_scores(&soft);
        let noisy = technical_scores(&add_noise(&soft, 8.0));

        assert!(clean.noise_sigma < 1.5, "{clean:?}");
        assert!((noisy.noise_sigma - 8.0).abs() < 2.5, "{noisy:?}");
        assert!(noisy.noise < 0.4 && clean.noise > 0.8);
        assert!(noisy.sharpness < 0.4, "noise is not sharpness: {noisy:?}");
    }

    #[test]
    fn flags_clipped_and_badly_exposed_images() {
        let good = technical_scores(&scene(160, 120));
        let blown = technical_scores(&RgbImage::from_fn(160, 120, |x, _| {
            if x < 60 {
                [255, 255, 255]
            } else {
                [180, 170, 160]
            }
        }));
        let dark = technical_scores(&RgbImage::from_fn(160, 120, |x, _| {
            if x < 80 { [0, 0, 0] } else { [20, 20, 25] }
        }));

        assert!(good.exposure > 0.95, "{good:?}");
        assert!((blown.highlights_clipped - 0.375).abs() < 0.01);
        assert!(blown.exposure < 0.5, "{blown:?}");
        assert!((dark.shadows_clipped - 0.5).abs() < 0.01);
        assert!(dark.mean_brightness < 0.05);
        assert!(dark.exposure < 0.3, "{dark:?}");
    }

    #[test]
    fn flat_and_tiny_images_do_not_panic() {
        let flat = technical_scores(&RgbImage::filled(64, 64, [128, 128, 128]));
        assert_eq!(flat.sharpness, 0.0);
        assert_eq!(flat.noise_sigma, 0.0);
        assert_eq!(flat.noise, 1.0);

        for (w, h) in [(0, 0), (1, 1), (2, 5), (3, 3), (9, 2000)] {
            let s = technical_scores(&RgbImage::filled(w, h, [10, 200, 30]));
            assert!(s.overall.is_finite() && (0.0..=1.0).contains(&s.overall));
        }
    }

    #[test]
    fn serializes_with_snake_case_fields() {
        let json = serde_json::to_value(technical_scores(&scene(64, 64))).unwrap();
        for field in ["sharpness", "exposure", "noise", "overall", "noise_sigma"] {
            assert!(json.get(field).is_some(), "{field}");
        }
    }
}
