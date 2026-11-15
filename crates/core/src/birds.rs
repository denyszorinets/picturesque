//! Bird identification (`docs/features/bird-id.rst`): ranking species for
//! an image embedding against a matrix of species text embeddings, as
//! WingCLIP does, optionally limited to the species of a region.

use std::collections::HashSet;

use crate::clip::Embedding;

/// CLIP's logit scale: similarities are multiplied by it before the
/// softmax that turns them into confidences.
const LOGIT_SCALE: f32 = 100.0;

/// One species the classifier knows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Species {
    pub common: String,
    pub scientific: String,
    /// eBird species code, for example `amerob` (American Robin).
    pub code: String,
}

/// A species ranked for an image.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// Row in the classifier and the labels.
    pub index: usize,
    /// Cosine similarity of the image and the species' text, −1 to 1.
    pub similarity: f32,
    /// Share of the softmax among the species considered, 0–1.
    pub confidence: f32,
}

/// Species text embeddings: one unit-length row per species.
#[derive(Debug, Clone, PartialEq)]
pub struct Classifier {
    dim: usize,
    rows: Vec<f32>,
}

impl Classifier {
    /// From a NumPy `.npy` file of little-endian `f32`s, `(species, dim)`,
    /// in C order.
    pub fn from_npy(bytes: &[u8]) -> Result<Self, String> {
        let header_err = |what: &str| format!("not a species matrix: {what}");
        let rest = bytes
            .strip_prefix(b"\x93NUMPY")
            .ok_or_else(|| header_err("no .npy magic"))?;
        let (&major, rest) = rest.split_first().ok_or_else(|| header_err("truncated"))?;
        let rest = rest.get(1..).ok_or_else(|| header_err("truncated"))?;
        let (len, rest) = match major {
            1 => {
                let (n, rest) = rest
                    .split_at_checked(2)
                    .ok_or_else(|| header_err("truncated"))?;
                (usize::from(u16::from_le_bytes([n[0], n[1]])), rest)
            }
            2 | 3 => {
                let (n, rest) = rest
                    .split_at_checked(4)
                    .ok_or_else(|| header_err("truncated"))?;
                (u32::from_le_bytes([n[0], n[1], n[2], n[3]]) as usize, rest)
            }
            _ => return Err(header_err("unknown version")),
        };
        let (header, data) = rest
            .split_at_checked(len)
            .ok_or_else(|| header_err("truncated header"))?;
        let header = std::str::from_utf8(header).map_err(|_| header_err("header is not text"))?;
        if !header.contains("'descr': '<f4'") {
            return Err(header_err("values are not little-endian f32"));
        }
        if !header.contains("'fortran_order': False") {
            return Err(header_err("not in C order"));
        }
        let shape = header
            .split("'shape': (")
            .nth(1)
            .and_then(|s| s.split(')').next())
            .ok_or_else(|| header_err("no shape"))?;
        let dims: Vec<usize> = shape
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.parse().map_err(|_| header_err("bad shape")))
            .collect::<Result<_, _>>()?;
        let [species, dim] = dims[..] else {
            return Err(header_err("not two-dimensional"));
        };
        let (values, tail) = data.as_chunks::<4>();
        if values.len() != species * dim || !tail.is_empty() || dim == 0 {
            return Err(header_err("size does not match the shape"));
        }
        Ok(Self {
            dim,
            rows: values.iter().map(|c| f32::from_le_bytes(*c)).collect(),
        })
    }

    pub fn species(&self) -> usize {
        self.rows.len() / self.dim
    }

    pub fn dim(&self) -> usize {
        self.dim
    }

    /// The `k` most likely species for `image`, best first. With `allowed`,
    /// only those rows are considered (the species of a region), and the
    /// confidences are shares among them.
    pub fn rank(
        &self,
        image: &Embedding,
        k: usize,
        allowed: Option<&HashSet<usize>>,
    ) -> Vec<Candidate> {
        if image.values().len() != self.dim {
            return Vec::new();
        }
        let mut scored: Vec<(usize, f32)> = self
            .rows
            .chunks_exact(self.dim)
            .enumerate()
            .filter(|(i, _)| allowed.is_none_or(|a| a.contains(i)))
            .map(|(i, row)| (i, row.iter().zip(image.values()).map(|(a, b)| a * b).sum()))
            .collect();
        if scored.is_empty() {
            return Vec::new();
        }
        let max = scored.iter().map(|(_, s)| *s).fold(f32::MIN, f32::max);
        let total: f32 = scored
            .iter()
            .map(|(_, s)| ((s - max) * LOGIT_SCALE).exp())
            .sum();
        scored.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        scored
            .into_iter()
            .take(k)
            .map(|(index, similarity)| Candidate {
                index,
                similarity,
                confidence: ((similarity - max) * LOGIT_SCALE).exp() / total,
            })
            .collect()
    }
}

/// The species, in classifier row order, from the rows of WingCLIP's
/// `labels.json`: `[[common name, scientific name, eBird code], ...]`.
pub fn labels_from_rows(rows: Vec<Vec<String>>) -> Result<Vec<Species>, String> {
    rows.into_iter()
        .map(|row| match <[String; 3]>::try_from(row) {
            Ok([common, scientific, code]) => Ok(Species {
                common,
                scientific,
                code,
            }),
            Err(_) => Err("not a species list: each row needs three names".to_owned()),
        })
        .collect()
}

/// Edge of the square image the detector (RT-DETR) takes.
pub const DETECT_SIZE: u32 = 640;

/// The detector's input for `image`: stretched to 640 × 640 (the boxes it
/// returns are shares of the image, so the aspect ratio comes back), values
/// 0–1, channels first.
pub fn detector_input(image: &crate::pixels::RgbImage) -> Vec<f32> {
    let square = image.resize_area(DETECT_SIZE, DETECT_SIZE);
    let plane = (DETECT_SIZE * DETECT_SIZE) as usize;
    let mut out = vec![0.0f32; 3 * plane];
    for (i, px) in square.data().as_chunks::<3>().0.iter().enumerate() {
        for c in 0..3 {
            out[c * plane + i] = f32::from(px[c]) / 255.0;
        }
    }
    out
}

/// Something the detector found.
#[derive(Debug, Clone, PartialEq)]
pub struct Detection {
    pub rect: Rect,
    /// 0–1.
    pub score: f32,
}

/// Detections of `class` from a DETR-style detector's outputs: per query,
/// `classes` logits (a sigmoid gives each class's score) and a box
/// `(centre x, centre y, width, height)` as shares of the image. Keeps
/// those scoring at least `threshold`, drops boxes mostly covered by a
/// better one, and orders them left to right.
pub fn decode_detections(
    logits: &[f32],
    boxes: &[f32],
    classes: usize,
    class: usize,
    threshold: f32,
) -> Vec<Detection> {
    if classes == 0 || class >= classes {
        return Vec::new();
    }
    let queries = (logits.len() / classes).min(boxes.len() / 4);
    let mut found: Vec<Detection> = (0..queries)
        .filter_map(|q| {
            let score = 1.0 / (1.0 + (-logits[q * classes + class]).exp());
            if score.is_nan() || score < threshold {
                return None;
            }
            let [cx, cy, w, h] = [0, 1, 2, 3].map(|k| boxes[q * 4 + k]);
            let x0 = (cx - w / 2.0).clamp(0.0, 1.0);
            let y0 = (cy - h / 2.0).clamp(0.0, 1.0);
            let x1 = (cx + w / 2.0).clamp(0.0, 1.0);
            let y1 = (cy + h / 2.0).clamp(0.0, 1.0);
            (x1 > x0 && y1 > y0).then_some(Detection {
                rect: Rect {
                    x: x0,
                    y: y0,
                    width: x1 - x0,
                    height: y1 - y0,
                },
                score,
            })
        })
        .collect();
    found.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut kept: Vec<Detection> = Vec::new();
    for d in found {
        if kept.iter().all(|k| overlap(&k.rect, &d.rect) < 0.6) {
            kept.push(d);
        }
    }
    kept.sort_by(|a, b| a.rect.x.total_cmp(&b.rect.x));
    kept
}

/// Intersection over union of two rectangles.
fn overlap(a: &Rect, b: &Rect) -> f32 {
    let w = (a.x + a.width).min(b.x + b.width) - a.x.max(b.x);
    let h = (a.y + a.height).min(b.y + b.height) - a.y.max(b.y);
    if w <= 0.0 || h <= 0.0 {
        return 0.0;
    }
    let inter = w * h;
    inter / (a.width * a.height + b.width * b.height - inter)
}

/// A rectangle in a picture, as shares (0–1) of its width and height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Context kept around the box the user drew, as a share of its longer
/// side on each side (WingCLIP was trained on crops with some margin).
const MARGIN: f32 = 0.1;

/// The square region of a `width` × `height` image to classify for `rect`:
/// centred on it, its longer side plus a margin, shifted (and if need be
/// shrunk) to stay inside the image. In pixels: `(x, y, side)`.
pub fn crop_square(rect: Rect, width: u32, height: u32) -> Option<(u32, u32, u32)> {
    let clamp = |v: f32| {
        if v.is_finite() {
            v.clamp(0.0, 1.0)
        } else {
            0.0
        }
    };
    let (x0, y0) = (clamp(rect.x), clamp(rect.y));
    let (x1, y1) = (clamp(rect.x + rect.width), clamp(rect.y + rect.height));
    let (w, h) = (width as f32, height as f32);
    let (bw, bh) = ((x1 - x0) * w, (y1 - y0) * h);
    if bw < 1.0 || bh < 1.0 || width == 0 || height == 0 {
        return None;
    }
    let side = (bw.max(bh) * (1.0 + 2.0 * MARGIN)).min(w.min(h)).max(1.0);
    let (cx, cy) = ((x0 + x1) / 2.0 * w, (y0 + y1) / 2.0 * h);
    let left = (cx - side / 2.0).clamp(0.0, w - side);
    let top = (cy - side / 2.0).clamp(0.0, h - side);
    Some((left.round() as u32, top.round() as u32, side.round() as u32))
}

#[cfg(test)]
mod tests;
