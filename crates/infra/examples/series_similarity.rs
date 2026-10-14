//! Measures CLIP similarity between variants of one scene (as in a burst
//! or retakes) and between different photos, to choose when embeddings
//! say two photos belong to a series.
//!
//!     cargo run --release -p picturesque-infra --example series_similarity -- \
//!         <visual model.onnx> <folder of photos>
//!
//! Variants of each photo: a 75% centre crop (zooming in), a crop shifted
//! sideways (reframing) and a darker copy (exposure change).

use std::path::Path;

use picturesque_app::ports::inference::{InferenceEngine, ModelSource, ProviderChoice, Tensor};
use picturesque_app::ports::pixels::PixelLoader;
use picturesque_core::clip::{Embedding, preprocess};
use picturesque_core::pixels::RgbImage;
use picturesque_core::similarity::Signature;
use picturesque_infra::images::VipsPixels;
use picturesque_infra::inference::OrtEngine;

fn darker(image: &RgbImage) -> RgbImage {
    RgbImage::from_fn(image.width(), image.height(), |x, y| {
        image.pixel(x, y).map(|v| (f32::from(v) * 0.6) as u8)
    })
}

fn quantiles(name: &str, values: &mut [f32]) {
    values.sort_by(f32::total_cmp);
    let q = |p: f64| values[((values.len() - 1) as f64 * p).round() as usize];
    println!(
        "{name:<28} n={:<5} min {:.3}  p5 {:.3}  median {:.3}  p95 {:.3}  max {:.3}",
        values.len(),
        q(0.0),
        q(0.05),
        q(0.5),
        q(0.95),
        q(1.0)
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [model, folder] = &args[..] else {
        panic!("usage: series_similarity <model.onnx> <folder>");
    };
    let engine = OrtEngine::new(std::thread::available_parallelism().map_or(1, |n| n.get()));
    let model = engine
        .load(ModelSource::File(Path::new(model)), ProviderChoice::Cpu)
        .unwrap();
    let embed = |image: &RgbImage| {
        let out = model
            .run(vec![Tensor::f32(
                "image",
                &[1, 3, 224, 224],
                preprocess(image),
            )])
            .unwrap();
        Embedding::new(out[0].as_f32().unwrap()).unwrap()
    };
    let mut files: Vec<_> = std::fs::read_dir(folder)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    files.retain(|p| p.extension().is_some_and(|e| e == "jpg"));
    files.sort();
    let (mut zoom, mut shift, mut dark, mut other) = (vec![], vec![], vec![], vec![]);
    let (mut hzoom, mut hshift, mut hdark, mut hother) = (vec![], vec![], vec![], vec![]);
    let mut originals: Vec<(Embedding, Signature)> = Vec::new();
    for path in &files {
        let image = VipsPixels.load(path, 1024).unwrap();
        let (w, h) = (image.width(), image.height());
        let variants = [
            image.crop(w / 8, h / 8, w * 3 / 4, h * 3 / 4),
            image.crop(w / 5, 0, w * 4 / 5, h),
            darker(&image),
        ];
        let e = embed(&image);
        let s = Signature::of(&image);
        for (i, v) in variants.iter().enumerate() {
            let sim = e.similarity(&embed(v));
            let bits = s.distance(&Signature::of(v)) as f32;
            [&mut zoom, &mut shift, &mut dark][i].push(sim);
            [&mut hzoom, &mut hshift, &mut hdark][i].push(bits);
        }
        originals.push((e, s));
    }
    for i in 0..originals.len() {
        for j in i + 1..originals.len() {
            other.push(originals[i].0.similarity(&originals[j].0));
            hother.push(originals[i].1.distance(&originals[j].1) as f32);
        }
    }
    println!("CLIP cosine similarity");
    quantiles("same scene, zoomed in", &mut zoom);
    quantiles("same scene, reframed", &mut shift);
    quantiles("same scene, darker", &mut dark);
    quantiles("different photos", &mut other);
    println!("perceptual hash distance (bits)");
    quantiles("same scene, zoomed in", &mut hzoom);
    quantiles("same scene, reframed", &mut hshift);
    quantiles("same scene, darker", &mut hdark);
    quantiles("different photos", &mut hother);
}
