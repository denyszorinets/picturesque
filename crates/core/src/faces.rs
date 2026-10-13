//! Faces: decoding the YuNet detector's output, and judging eyes and
//! focus from face-mesh landmarks (`docs/features/ai.rst`).

use crate::pixels::RgbImage;

/// Edge of the square image YuNet takes.
pub const DETECTOR_SIZE: u32 = 640;
/// Edge of the square face crop the face-mesh model takes.
pub const MESH_SIZE: u32 = 256;
/// Faces less certain than this are dropped.
pub const MIN_SCORE: f32 = 0.6;
/// Overlapping detections above this IoU are merged.
const NMS_IOU: f32 = 0.3;
/// The face-mesh crop is this much larger than the detected box's longer
/// side (1.25 gave the face mesh the most confidence on the fixtures).
const CROP_SCALE: f32 = 1.25;

/// A face found by the detector, in pixels of the analyzed image.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Detection {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub score: f32,
}

impl Detection {
    fn iou(&self, other: &Self) -> f32 {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.width).min(other.x + other.width);
        let y2 = (self.y + self.height).min(other.y + other.height);
        let inter = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);
        let union = self.width * self.height + other.width * other.height - inter;
        if union > 0.0 { inter / union } else { 0.0 }
    }
}

/// The detector input for `image`: scaled so its longest side is 640,
/// padded with black to 640 × 640 at the bottom and right, BGR channels
/// first with values 0–255 (as YuNet was trained). Also returns the scale
/// from image to input pixels.
pub fn detector_input(image: &RgbImage) -> (Vec<f32>, f32) {
    let longest = image.width().max(image.height()).max(1);
    let scale = DETECTOR_SIZE as f32 / longest as f32;
    let w = ((image.width() as f32 * scale).round() as u32).clamp(1, DETECTOR_SIZE);
    let h = ((image.height() as f32 * scale).round() as u32).clamp(1, DETECTOR_SIZE);
    let scaled = image.resize_area(w, h);
    let size = DETECTOR_SIZE as usize;
    let plane = size * size;
    let mut out = vec![0.0f32; 3 * plane];
    for y in 0..h as usize {
        for x in 0..w as usize {
            let [r, g, b] = scaled.pixel(x as u32, y as u32);
            let i = y * size + x;
            out[i] = f32::from(b);
            out[plane + i] = f32::from(g);
            out[2 * plane + i] = f32::from(r);
        }
    }
    (out, scale)
}

/// One stride level of YuNet output: per grid cell, class and objectness
/// scores and box offsets `[dx, dy, log w, log h]`.
#[derive(Debug, Clone, Copy)]
pub struct DetectorLevel<'a> {
    pub stride: u32,
    pub cls: &'a [f32],
    pub obj: &'a [f32],
    pub bbox: &'a [f32],
}

/// Faces from YuNet's outputs, best first, after non-maximum suppression,
/// in pixels of the image `detector_input` was given (`scale` from it).
pub fn decode_detections(levels: &[DetectorLevel<'_>], scale: f32) -> Vec<Detection> {
    let mut found = Vec::new();
    for level in levels {
        let cols = (DETECTOR_SIZE / level.stride) as usize;
        let s = level.stride as f32;
        for (i, (cls, obj)) in level.cls.iter().zip(level.obj).enumerate() {
            let score = (cls.clamp(0.0, 1.0) * obj.clamp(0.0, 1.0)).sqrt();
            if score < MIN_SCORE {
                continue;
            }
            let Some(b) = level.bbox.get(i * 4..i * 4 + 4) else {
                continue;
            };
            let (col, row) = ((i % cols) as f32, (i / cols) as f32);
            let cx = (col + b[0]) * s;
            let cy = (row + b[1]) * s;
            let w = b[2].exp() * s;
            let h = b[3].exp() * s;
            found.push(Detection {
                x: (cx - w / 2.0) / scale,
                y: (cy - h / 2.0) / scale,
                width: w / scale,
                height: h / scale,
                score,
            });
        }
    }
    found.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut kept: Vec<Detection> = Vec::new();
    for d in found {
        if kept.iter().all(|k| k.iou(&d) <= NMS_IOU) {
            kept.push(d);
        }
    }
    kept
}

/// The square region around a face that the face-mesh model looks at, in
/// image pixels: `(x, y, side)`, possibly reaching outside the image.
pub fn mesh_region(face: &Detection) -> (f32, f32, f32) {
    let side = face.width.max(face.height) * CROP_SCALE;
    let cx = face.x + face.width / 2.0;
    let cy = face.y + face.height / 2.0;
    (cx - side / 2.0, cy - side / 2.0, side)
}

/// The face-mesh input for a face: its region scaled to 256 × 256 (black
/// outside the image), RGB channels last with values 0–1.
pub fn mesh_input(image: &RgbImage, face: &Detection) -> Vec<f32> {
    let (x0, y0, side) = mesh_region(face);
    let n = MESH_SIZE as usize;
    let step = side / MESH_SIZE as f32;
    let mut out = vec![0.0f32; n * n * 3];
    for y in 0..n {
        for x in 0..n {
            // Average the source pixels this output pixel covers (sampled
            // on a small grid, enough for shrinking face crops).
            let mut sum = [0.0f32; 3];
            let mut count = 0.0;
            for sy in 0..2 {
                for sx in 0..2 {
                    let px = x0 + (x as f32 + 0.25 + 0.5 * sx as f32) * step;
                    let py = y0 + (y as f32 + 0.25 + 0.5 * sy as f32) * step;
                    if px >= 0.0
                        && py >= 0.0
                        && (px as u32) < image.width()
                        && (py as u32) < image.height()
                    {
                        let p = image.pixel(px as u32, py as u32);
                        for c in 0..3 {
                            sum[c] += f32::from(p[c]);
                        }
                    }
                    count += 1.0;
                }
            }
            for c in 0..3 {
                out[(y * n + x) * 3 + c] = sum[c] / count / 255.0;
            }
        }
    }
    out
}

/// Landmarks of the face mesh used for each eye, in the order of the eye
/// aspect ratio formula: outer corner, two upper-lid points, inner corner,
/// two lower-lid points (MediaPipe face mesh indices).
const RIGHT_EYE: [usize; 6] = [33, 160, 158, 133, 153, 144];
const LEFT_EYE: [usize; 6] = [362, 385, 387, 263, 373, 380];

/// Eye aspect ratio of one eye: lid opening relative to eye width; about
/// 0.25–0.35 for open eyes, under 0.15 for closed ones.
fn eye_aspect_ratio(landmarks: &[[f32; 3]], eye: [usize; 6]) -> Option<f32> {
    let p = |i: usize| landmarks.get(eye[i]).map(|l| (l[0], l[1]));
    let d = |a: (f32, f32), b: (f32, f32)| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
    let (p1, p2, p3, p4, p5, p6) = (p(0)?, p(1)?, p(2)?, p(3)?, p(4)?, p(5)?);
    let width = d(p1, p4);
    (width > 0.0).then(|| (d(p2, p6) + d(p3, p5)) / (2.0 * width))
}

/// Eye aspect ratios `(right, left)` from the face mesh's flat output
/// (x, y, z per landmark).
pub fn eye_openness(mesh: &[f32]) -> Option<(f32, f32)> {
    let (landmarks, _) = mesh.as_chunks::<3>();
    Some((
        eye_aspect_ratio(landmarks, RIGHT_EYE)?,
        eye_aspect_ratio(landmarks, LEFT_EYE)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detector_input_letterboxes_bgr() {
        let img = RgbImage::filled(1000, 500, [10, 20, 30]);
        let (input, scale) = detector_input(&img);
        assert_eq!(input.len(), 3 * 640 * 640);
        assert!((scale - 0.64).abs() < 1e-6);
        let plane = 640 * 640;
        // Top left: the image, as B, G, R planes.
        assert_eq!(
            (input[0], input[plane], input[2 * plane]),
            (30.0, 20.0, 10.0)
        );
        // Below the scaled image (320 rows): black padding.
        assert_eq!(input[400 * 640], 0.0);
    }

    #[test]
    fn decodes_and_suppresses_overlapping_detections() {
        // Stride 32 has a 20 × 20 grid. Cell 21 (row 1, column 1) holds a
        // confident face, cell 22 a weaker overlapping one, cell 300 a
        // face too uncertain to keep.
        let n = 400;
        let mut cls = vec![0.0; n];
        let mut obj = vec![0.0; n];
        let mut bbox = vec![0.0; n * 4];
        for (cell, c, o, dx) in [
            (21, 0.95, 0.95, 0.5),
            (22, 0.8, 0.8, -0.3),
            (300, 0.3, 0.3, 0.0),
        ] {
            cls[cell] = c;
            obj[cell] = o;
            bbox[cell * 4] = dx;
            bbox[cell * 4 + 1] = 0.5;
            bbox[cell * 4 + 2] = (2.0f32).ln(); // 64 px wide
            bbox[cell * 4 + 3] = (2.0f32).ln();
        }
        let faces = decode_detections(
            &[DetectorLevel {
                stride: 32,
                cls: &cls,
                obj: &obj,
                bbox: &bbox,
            }],
            0.5,
        );
        assert_eq!(faces.len(), 1);
        let f = faces[0];
        // Centre (1.5 cells, 1.5 cells) * 32 = 48 px; 64 px box; scale 0.5.
        assert!((f.x - (48.0 - 32.0) / 0.5).abs() < 1e-3, "{f:?}");
        assert!((f.width - 128.0).abs() < 1e-3);
        assert!((f.score - 0.95).abs() < 1e-6);
    }

    #[test]
    fn mesh_input_crops_around_the_face() {
        let img = RgbImage::from_fn(
            400,
            400,
            |x, _| if x < 200 { [255, 255, 255] } else { [0, 0, 0] },
        );
        let face = Detection {
            x: 150.0,
            y: 150.0,
            width: 100.0,
            height: 100.0,
            score: 0.9,
        };
        assert_eq!(mesh_region(&face), (137.5, 137.5, 125.0));
        let input = mesh_input(&img, &face);
        assert_eq!(input.len(), 256 * 256 * 3);
        // Left half of the crop is white, right half black.
        assert!(input[(128 * 256 + 10) * 3] > 0.99);
        assert!(input[(128 * 256 + 245) * 3] < 0.01);
        // A face at the edge reads black outside the image.
        let edge = Detection {
            x: -50.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            score: 0.9,
        };
        assert_eq!(mesh_input(&img, &edge)[0], 0.0);
    }

    #[test]
    fn eye_aspect_ratio_tells_open_from_closed() {
        let mut mesh = vec![0.0f32; 478 * 3];
        let mut set = |i: usize, x: f32, y: f32| {
            mesh[i * 3] = x;
            mesh[i * 3 + 1] = y;
        };
        // Right eye 30 px wide, lids 9 px apart: open (0.3).
        for (i, (x, y)) in RIGHT_EYE.iter().zip([
            (0.0, 0.0),
            (10.0, -4.5),
            (20.0, -4.5),
            (30.0, 0.0),
            (20.0, 4.5),
            (10.0, 4.5),
        ]) {
            set(*i, x, y);
        }
        // Left eye 30 px wide, lids 1.5 px apart: closed (0.05).
        for (i, (x, y)) in LEFT_EYE.iter().zip([
            (0.0, 0.0),
            (10.0, -0.75),
            (20.0, -0.75),
            (30.0, 0.0),
            (20.0, 0.75),
            (10.0, 0.75),
        ]) {
            set(*i, x + 100.0, y);
        }
        let (right, left) = eye_openness(&mesh).unwrap();
        assert!((right - 0.3).abs() < 1e-5);
        assert!((left - 0.05).abs() < 1e-5);
        assert!(eye_openness(&mesh[..300]).is_none());
    }
}
