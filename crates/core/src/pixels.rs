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
}
