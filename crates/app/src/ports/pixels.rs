//! Loading pixels for analysis.

use std::path::Path;

use picturesque_core::pixels::RgbImage;

pub trait PixelLoader: Send + Sync {
    /// Decodes an image file (a cached preview JPEG) to RGB, shrunk so its
    /// longest edge is at most `max_edge`.
    fn load(&self, path: &Path, max_edge: u32) -> Result<RgbImage, String>;
}
