//! Perceptual hashes for visual similarity.
//!
//! The pHash of an image compares the low-frequency DCT coefficients of a
//! 32 × 32 brightness thumbnail with their median, giving 64 bits that
//! follow the overall layout. It is robust to resizing, brightness changes,
//! small shifts, noise and recompression. Near-duplicates are a few bits
//! apart; unrelated photos about 32 (half the bits).
//!
//! dHash (comparing neighbouring cells) was tried too, but flips many bits
//! in flat areas such as skies: resizing alone changed 20 of 64 bits.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::pixels::RgbImage;

/// Perceptual signature of one image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Signature {
    pub phash: Hash64,
}

/// Bit distance at or below which two photos count as near-duplicates
/// (the same shot, or nearly). Series detection uses a looser limit.
pub const NEAR_DUPLICATE: u32 = 10;

impl Signature {
    pub fn of(image: &RgbImage) -> Self {
        let luma = image.luma();
        let (w, h) = (image.width() as usize, image.height() as usize);
        Self {
            phash: phash(&luma, w, h),
        }
    }

    /// Bits that differ (0–64).
    pub fn distance(&self, other: &Self) -> u32 {
        self.phash.distance(other.phash)
    }

    /// 1 for identical hashes, about 0.5 for unrelated images.
    pub fn similarity(&self, other: &Self) -> f32 {
        1.0 - self.distance(other) as f32 / 64.0
    }

    pub fn is_near_duplicate(&self, other: &Self) -> bool {
        self.distance(other) <= NEAR_DUPLICATE
    }
}

/// A 64-bit hash, serialized as 16 hex digits (JSON numbers cannot hold
/// 64 bits exactly).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Hash64(pub u64);

impl Hash64 {
    pub fn distance(self, other: Self) -> u32 {
        (self.0 ^ other.0).count_ones()
    }
}

impl fmt::Debug for Hash64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

impl fmt::Display for Hash64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

impl Serialize for Hash64 {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Hash64 {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        if s.len() != 16 {
            return Err(serde::de::Error::custom("expected 16 hex digits"));
        }
        u64::from_str_radix(&s, 16)
            .map(Hash64)
            .map_err(serde::de::Error::custom)
    }
}

/// Shrinks a luma plane to `tw × th` by averaging the source pixels each
/// target cell covers (at least one).
fn shrink(luma: &[f32], w: usize, h: usize, tw: usize, th: usize) -> Vec<f32> {
    let mut out = vec![0.0; tw * th];
    if w == 0 || h == 0 {
        return out;
    }
    for (ty, row) in out.chunks_exact_mut(tw).enumerate() {
        let (y0, y1) = (ty * h / th, ((ty + 1) * h / th).max(ty * h / th + 1).min(h));
        for (tx, cell) in row.iter_mut().enumerate() {
            let (x0, x1) = (tx * w / tw, ((tx + 1) * w / tw).max(tx * w / tw + 1).min(w));
            let mut sum = 0.0;
            for y in y0..y1 {
                sum += luma[y * w + x0..y * w + x1].iter().sum::<f32>();
            }
            *cell = sum / ((y1 - y0) * (x1 - x0)) as f32;
        }
    }
    out
}

const DCT_SIZE: usize = 32;
const DCT_KEEP: usize = 8;

fn phash(luma: &[f32], w: usize, h: usize) -> Hash64 {
    let cells = shrink(luma, w, h, DCT_SIZE, DCT_SIZE);
    // cos[k][n] for the DCT-II, only the frequencies kept.
    let cos: Vec<[f32; DCT_SIZE]> = (0..DCT_KEEP)
        .map(|k| {
            std::array::from_fn(|n| {
                (std::f32::consts::PI / DCT_SIZE as f32 * (n as f32 + 0.5) * k as f32).cos()
            })
        })
        .collect();
    // Rows, then columns (separable).
    let mut rows = vec![[0.0f32; DCT_KEEP]; DCT_SIZE];
    for (y, row) in rows.iter_mut().enumerate() {
        let line = &cells[y * DCT_SIZE..(y + 1) * DCT_SIZE];
        for (k, c) in cos.iter().enumerate() {
            row[k] = line.iter().zip(c).map(|(v, c)| v * c).sum();
        }
    }
    let mut coeffs = [0.0f32; DCT_KEEP * DCT_KEEP];
    for (ky, c) in cos.iter().enumerate() {
        for kx in 0..DCT_KEEP {
            coeffs[ky * DCT_KEEP + kx] = rows.iter().zip(c).map(|(r, c)| r[kx] * c).sum();
        }
    }
    // The DC term (average brightness) is left out of the median.
    let mut ac: Vec<f32> = coeffs[1..].to_vec();
    ac.sort_by(f32::total_cmp);
    let median = (ac[ac.len() / 2 - 1] + ac[ac.len() / 2]) / 2.0;
    let mut bits = 0u64;
    for c in coeffs {
        bits = (bits << 1) | u64::from(c > median);
    }
    Hash64(bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scene: blobs at given positions on a gradient, with fine texture.
    fn scene(seed: u32, w: u32, h: u32, shift: i32, light: i32) -> RgbImage {
        let blobs: Vec<(f32, f32, f32)> = (0..5)
            .map(|i| {
                let v = seed.wrapping_mul(2_654_435_761).rotate_left(i * 7);
                (
                    (v % 1000) as f32 / 1000.0,
                    ((v / 1000) % 1000) as f32 / 1000.0,
                    0.08 + ((v / 1_000_000) % 100) as f32 / 800.0,
                )
            })
            .collect();
        RgbImage::from_fn(w, h, |x, y| {
            let fx = (x as i32 - shift) as f32 / w as f32;
            let fy = y as f32 / h as f32;
            let mut v = 40.0 + 80.0 * fy;
            for (bx, by, r) in &blobs {
                let d = ((fx - bx).powi(2) + (fy - by).powi(2)).sqrt();
                if d < *r {
                    v += 110.0;
                }
            }
            v += if (x + y) % 4 == 0 { 6.0 } else { 0.0 };
            let v = (v as i32 + light).clamp(0, 255) as u8;
            [v, v, v]
        })
    }

    fn add_noise(image: &RgbImage, amount: i32) -> RgbImage {
        RgbImage::from_fn(image.width(), image.height(), |x, y| {
            let n = ((x.wrapping_mul(7919) ^ y.wrapping_mul(104_729)) % (2 * amount as u32 + 1))
                as i32
                - amount;
            image
                .pixel(x, y)
                .map(|v| (i32::from(v) + n).clamp(0, 255) as u8)
        })
    }

    #[test]
    fn identical_images_have_identical_hashes() {
        let a = Signature::of(&scene(1, 300, 200, 0, 0));
        assert_eq!(a, Signature::of(&scene(1, 300, 200, 0, 0)));
        assert_eq!(a.similarity(&a), 1.0);
    }

    #[test]
    fn near_duplicates_are_close_despite_edits() {
        let original = Signature::of(&scene(7, 600, 400, 0, 0));
        let variants = [
            ("smaller", Signature::of(&scene(7, 300, 200, 0, 0))),
            ("brighter", Signature::of(&scene(7, 600, 400, 0, 25))),
            ("shifted", Signature::of(&scene(7, 600, 400, 8, 0))),
            (
                "noisy",
                Signature::of(&add_noise(&scene(7, 600, 400, 0, 0), 12)),
            ),
        ];
        for (what, v) in variants {
            let d = original.distance(&v);
            assert!(d <= 4, "{what}: {d}");
            assert!(original.is_near_duplicate(&v));
            assert!(original.similarity(&v) > 0.85, "{what}");
        }
    }

    #[test]
    fn different_scenes_are_far_apart() {
        let scenes: Vec<Signature> = (1..=6)
            .map(|seed| Signature::of(&scene(seed * 31, 400, 300, 0, 0)))
            .collect();
        for (i, a) in scenes.iter().enumerate() {
            for b in &scenes[i + 1..] {
                let d = a.distance(b);
                assert!(d > 16, "{d}");
                assert!(!a.is_near_duplicate(b));
                assert!(a.similarity(b) < 0.75);
            }
        }
    }

    #[test]
    fn hashes_serialize_as_hex() {
        let s = Signature {
            phash: Hash64(0x00ff_0000_0000_0001),
        };
        let json = serde_json::to_value(s).unwrap();
        assert_eq!(json, serde_json::json!({ "phash": "00ff000000000001" }));
        assert_eq!(serde_json::from_value::<Signature>(json).unwrap(), s);
        assert!(serde_json::from_str::<Hash64>("\"abc\"").is_err());
    }

    #[test]
    fn tiny_and_flat_images_do_not_panic() {
        for (w, h) in [(0, 0), (1, 1), (3, 2), (9, 8), (2000, 1)] {
            let s = Signature::of(&RgbImage::filled(w, h, [90, 90, 90]));
            assert_eq!(s.distance(&s), 0);
        }
    }
}
