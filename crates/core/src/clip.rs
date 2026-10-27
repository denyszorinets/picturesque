//! CLIP image embeddings: preprocessing, normalized vectors and their
//! storage form (`docs/features/ai.rst`).

use crate::base64;
use crate::pixels::RgbImage;

/// Edge of the square image CLIP ViT-B/32 takes.
pub const INPUT_SIZE: u32 = 224;
/// Per-channel mean and standard deviation CLIP was trained with.
const MEAN: [f32; 3] = [0.481_454_66, 0.457_827_5, 0.408_210_73];
const STD: [f32; 3] = [0.268_629_54, 0.261_302_6, 0.275_777_1];

/// The model input for `image`: the shortest side scaled to 224 pixels,
/// the centre 224 × 224 cropped, normalized, channels first
/// (`[3, 224, 224]`).
pub fn preprocess(image: &RgbImage) -> Vec<f32> {
    let (w, h) = (image.width().max(1), image.height().max(1));
    let scale = INPUT_SIZE as f32 / w.min(h) as f32;
    let sw = ((w as f32 * scale).round() as u32).max(INPUT_SIZE);
    let sh = ((h as f32 * scale).round() as u32).max(INPUT_SIZE);
    let square = image.resize_area(sw, sh).crop(
        (sw - INPUT_SIZE) / 2,
        (sh - INPUT_SIZE) / 2,
        INPUT_SIZE,
        INPUT_SIZE,
    );
    let plane = (INPUT_SIZE * INPUT_SIZE) as usize;
    let mut out = vec![0.0f32; 3 * plane];
    for (i, px) in square.data().as_chunks::<3>().0.iter().enumerate() {
        for c in 0..3 {
            out[c * plane + i] = (f32::from(px[c]) / 255.0 - MEAN[c]) / STD[c];
        }
    }
    out
}

/// A unit-length embedding; similarity is the dot product.
#[derive(Debug, Clone, PartialEq)]
pub struct Embedding(Vec<f32>);

impl Embedding {
    /// Normalizes `raw`; `None` if it is empty, zero or not finite.
    pub fn new(raw: &[f32]) -> Option<Self> {
        let norm = raw.iter().map(|v| v * v).sum::<f32>().sqrt();
        (norm.is_finite() && norm > 0.0).then(|| Self(raw.iter().map(|v| v / norm).collect()))
    }

    pub fn values(&self) -> &[f32] {
        &self.0
    }

    /// Cosine similarity, −1 to 1; 0 for different dimensions.
    pub fn similarity(&self, other: &Self) -> f32 {
        if self.0.len() != other.0.len() {
            return 0.0;
        }
        self.0.iter().zip(&other.0).map(|(a, b)| a * b).sum()
    }

    /// Little-endian `f32`s in base64, for storing.
    pub fn encode(&self) -> String {
        let bytes: Vec<u8> = self.0.iter().flat_map(|v| v.to_le_bytes()).collect();
        base64::encode(&bytes)
    }

    pub fn decode(text: &str) -> Option<Self> {
        let bytes = base64::decode(text)?;
        let (chunks, rest) = bytes.as_chunks::<4>();
        if !rest.is_empty() {
            return None;
        }
        let values: Vec<f32> = chunks.iter().map(|c| f32::from_le_bytes(*c)).collect();
        // Stored vectors are already unit length; keep them bit for bit.
        (!values.is_empty() && values.iter().all(|v| v.is_finite())).then_some(Self(values))
    }
}

/// A linear model on top of embeddings, like LAION's aesthetic predictor.
#[derive(Debug, Clone, PartialEq)]
pub struct LinearHead {
    weights: Vec<f32>,
    bias: f32,
}

impl LinearHead {
    /// From little-endian `f32`s: the weights, then the bias.
    pub fn from_le_bytes(bytes: &[u8]) -> Option<Self> {
        let (chunks, rest) = bytes.as_chunks::<4>();
        let (bias, weights) = chunks.split_last()?;
        (rest.is_empty() && !weights.is_empty()).then(|| Self {
            weights: weights.iter().map(|c| f32::from_le_bytes(*c)).collect(),
            bias: f32::from_le_bytes(*bias),
        })
    }

    /// `weights · embedding + bias`; `None` when the dimensions differ.
    pub fn apply(&self, embedding: &Embedding) -> Option<f32> {
        (self.weights.len() == embedding.0.len()).then(|| {
            self.weights
                .iter()
                .zip(&embedding.0)
                .map(|(w, e)| w * e)
                .sum::<f32>()
                + self.bias
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preprocessing_crops_the_centre_and_normalizes() {
        // Left third red, middle green, right third blue: the centre crop
        // of a 3:2 image sees mostly green, with red and blue edges.
        let img = RgbImage::from_fn(900, 600, |x, _| match x / 300 {
            0 => [255, 0, 0],
            1 => [0, 255, 0],
            _ => [0, 0, 255],
        });
        let input = preprocess(&img);
        assert_eq!(input.len(), 3 * 224 * 224);
        let plane = 224 * 224;
        let at = |c: usize, x: usize, y: usize| input[c * plane + y * 224 + x];
        let green = (1.0 - MEAN[1]) / STD[1];
        assert!((at(1, 112, 112) - green).abs() < 1e-4);
        assert!((at(0, 112, 112) - (0.0 - MEAN[0]) / STD[0]).abs() < 1e-4);
        // The crop spans x = 75..525 of the source: red at the left edge.
        assert!(at(0, 0, 0) > 1.0);
        assert!(at(2, 223, 0) > 1.0);
    }

    #[test]
    fn small_and_tall_images_still_give_224_squares() {
        assert_eq!(
            preprocess(&RgbImage::filled(10, 300, [9, 9, 9])).len(),
            3 * 224 * 224
        );
        assert_eq!(
            preprocess(&RgbImage::filled(1, 1, [9, 9, 9])).len(),
            3 * 224 * 224
        );
    }

    #[test]
    fn embeddings_are_unit_length_and_compare_by_angle() {
        let a = Embedding::new(&[3.0, 4.0]).unwrap();
        assert_eq!(a.values(), [0.6, 0.8]);
        let b = Embedding::new(&[6.0, 8.0]).unwrap();
        let c = Embedding::new(&[-4.0, 3.0]).unwrap();
        assert!((a.similarity(&b) - 1.0).abs() < 1e-6);
        assert!(a.similarity(&c).abs() < 1e-6);
        assert_eq!(
            a.similarity(&Embedding::new(&[1.0, 0.0, 0.0]).unwrap()),
            0.0
        );
        assert!(Embedding::new(&[0.0, 0.0]).is_none());
        assert!(Embedding::new(&[f32::NAN]).is_none());
        assert!(Embedding::new(&[]).is_none());
    }

    #[test]
    fn linear_heads_score_embeddings() {
        let bytes: Vec<u8> = [2.0f32, -1.0, 0.5]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let head = LinearHead::from_le_bytes(&bytes).unwrap();
        let e = Embedding::new(&[3.0, 4.0]).unwrap();
        assert!((head.apply(&e).unwrap() - (2.0 * 0.6 - 0.8 + 0.5)).abs() < 1e-6);
        assert_eq!(head.apply(&Embedding::new(&[1.0]).unwrap()), None);
        assert!(LinearHead::from_le_bytes(&bytes[..4]).is_none());
        assert!(LinearHead::from_le_bytes(&bytes[..5]).is_none());
    }

    #[test]
    fn embeddings_round_trip_through_base64() {
        for raw in [
            vec![1.0f32],
            vec![0.1, -0.2, 0.3],
            (0..512).map(|i| (i as f32).sin()).collect(),
        ] {
            let e = Embedding::new(&raw).unwrap();
            assert_eq!(Embedding::decode(&e.encode()), Some(e));
        }
        assert!(Embedding::decode("not base64!").is_none());
        assert!(Embedding::decode("TWE=").is_none()); // 2 bytes: not f32s
    }
}
