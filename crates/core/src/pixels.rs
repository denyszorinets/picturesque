//! A minimal in-memory RGB image for analysis algorithms.

/// 8-bit RGB pixels, row-major, no padding.
#[derive(Clone, PartialEq, Eq)]
pub struct RgbImage {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl std::fmt::Debug for RgbImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RgbImage({}x{})", self.width, self.height)
    }
}

impl RgbImage {
    /// `None` if `data` is not exactly `width * height * 3` bytes.
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Option<Self> {
        (data.len() == width as usize * height as usize * 3).then_some(Self {
            width,
            height,
            data,
        })
    }

    /// An image where every pixel is `rgb`.
    pub fn filled(width: u32, height: u32, rgb: [u8; 3]) -> Self {
        let data = rgb
            .iter()
            .copied()
            .cycle()
            .take(width as usize * height as usize * 3)
            .collect();
        Self {
            width,
            height,
            data,
        }
    }

    /// Builds an image from a function of (x, y).
    pub fn from_fn(width: u32, height: u32, f: impl Fn(u32, u32) -> [u8; 3]) -> Self {
        let mut data = Vec::with_capacity(width as usize * height as usize * 3);
        for y in 0..height {
            for x in 0..width {
                data.extend_from_slice(&f(x, y));
            }
        }
        Self {
            width,
            height,
            data,
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 3] {
        let i = (y as usize * self.width as usize + x as usize) * 3;
        [self.data[i], self.data[i + 1], self.data[i + 2]]
    }

    /// The image scaled to `width` × `height` by averaging the source
    /// pixels each output pixel covers (good for shrinking; enlarging
    /// repeats pixels).
    pub fn resize_area(&self, width: u32, height: u32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        // Rows first, then columns; each pass averages with fractional
        // coverage at the edges of each output pixel.
        let rows = resample(
            &self.data,
            self.width as usize,
            self.height as usize,
            height as usize,
            true,
        );
        let data = resample(
            &rows,
            self.width as usize,
            height as usize,
            width as usize,
            false,
        );
        Self {
            width,
            height,
            data: data
                .into_iter()
                .map(|v| v.round().clamp(0.0, 255.0) as u8)
                .collect(),
        }
    }

    /// The `width` × `height` region starting at (`x`, `y`), clipped to the
    /// image.
    pub fn crop(&self, x: u32, y: u32, width: u32, height: u32) -> Self {
        let x = x.min(self.width);
        let y = y.min(self.height);
        let width = width.min(self.width - x);
        let height = height.min(self.height - y);
        Self::from_fn(width, height, |cx, cy| self.pixel(x + cx, y + cy))
    }

    /// Luma (Rec. 601) of every pixel, row-major.
    pub fn luma(&self) -> Vec<f32> {
        self.data
            .as_chunks::<3>()
            .0
            .iter()
            .map(|[r, g, b]| 0.299 * f32::from(*r) + 0.587 * f32::from(*g) + 0.114 * f32::from(*b))
            .collect()
    }
}

/// Resamples interleaved RGB along one axis. `vertical`: `src` is
/// `w` × `h` and the result `w` × `out`; otherwise `src` is `w` × `h` and
/// the result `out` × `h`. Takes `u8` or `f32` samples.
fn resample<T: Copy + Into<f32>>(
    src: &[T],
    w: usize,
    h: usize,
    out: usize,
    vertical: bool,
) -> Vec<f32> {
    let n = if vertical { h } else { w };
    let scale = n as f32 / out as f32;
    // For each output index: the source indices it covers, with weights.
    let taps: Vec<Vec<(usize, f32)>> = (0..out)
        .map(|o| {
            let start = o as f32 * scale;
            let end = ((o + 1) as f32 * scale).max(start + 1e-6);
            let mut taps = Vec::new();
            let mut i = start.floor() as usize;
            while (i as f32) < end && i < n {
                let cover = (end.min(i as f32 + 1.0) - start.max(i as f32)).max(0.0);
                if cover > 0.0 {
                    taps.push((i, cover));
                }
                i += 1;
            }
            if taps.is_empty() {
                taps.push((start.floor().min(n as f32 - 1.0) as usize, 1.0));
            }
            let total: f32 = taps.iter().map(|t| t.1).sum();
            taps.iter().map(|&(i, c)| (i, c / total)).collect()
        })
        .collect();
    let (ow, oh) = if vertical { (w, out) } else { (out, h) };
    let mut result = vec![0.0f32; ow * oh * 3];
    for y in 0..oh {
        for x in 0..ow {
            let o = (y * ow + x) * 3;
            let along = if vertical { y } else { x };
            for &(i, weight) in &taps[along] {
                let s = if vertical {
                    (i * w + x) * 3
                } else {
                    (y * w + i) * 3
                };
                for c in 0..3 {
                    result[o + c] += src[s + c].into() * weight;
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_buffer_size() {
        assert!(RgbImage::new(2, 2, vec![0; 12]).is_some());
        assert!(RgbImage::new(2, 2, vec![0; 11]).is_none());
    }

    #[test]
    fn reads_pixels_and_luma() {
        let img = RgbImage::from_fn(3, 2, |x, y| [x as u8 * 10, y as u8 * 20, 0]);

        assert_eq!(img.pixel(2, 1), [20, 20, 0]);
        let white = RgbImage::filled(1, 1, [255, 255, 255]);
        assert!((white.luma()[0] - 255.0).abs() < 0.01);
    }

    #[test]
    fn shrinking_averages_covered_pixels() {
        // Left half black, right half white: halving gives the halves;
        // shrinking to 3 columns blends the middle one.
        let img = RgbImage::from_fn(4, 2, |x, _| if x < 2 { [0, 0, 0] } else { [255, 255, 255] });
        let half = img.resize_area(2, 1);
        assert_eq!(half.data(), &[0, 0, 0, 255, 255, 255]);
        let three = img.resize_area(3, 1);
        assert_eq!(three.pixel(0, 0), [0, 0, 0]);
        assert_eq!(three.pixel(1, 0), [128, 128, 128]);
        assert_eq!(three.pixel(2, 0), [255, 255, 255]);
        // A flat image stays flat at any size.
        let flat = RgbImage::filled(1024, 683, [10, 200, 30]).resize_area(336, 224);
        assert_eq!((flat.width(), flat.height()), (336, 224));
        assert!(flat.data().chunks(3).all(|p| p == [10, 200, 30]));
    }

    #[test]
    fn crops_inside_the_image() {
        let img = RgbImage::from_fn(4, 3, |x, y| [x as u8, y as u8, 0]);
        let c = img.crop(1, 1, 2, 5);
        assert_eq!((c.width(), c.height()), (2, 2));
        assert_eq!(c.pixel(0, 0), [1, 1, 0]);
        assert_eq!(img.crop(9, 9, 2, 2).width(), 0);
    }
}
