//! Measures how well the aesthetic score separates photos people rated
//! highly from ordinary ones (`spikes/aesthetic/README.md`).
//!
//!     cargo run --release -p picturesque-infra --example eval_aesthetic -- \
//!         <clip visual model.onnx> <good folder> <ordinary folder> [head.bin]
//!
//! Scores every JPEG in both folders like the app does (preview at 1024
//! px, CLIP, LAION's aesthetic head) and prints each group's distribution
//! and the AUC: the chance that a random good photo outscores a random
//! ordinary one (0.5 is chance). The technical score is shown for
//! comparison. `head.bin` replaces the built-in aesthetic head (raw
//! little-endian f32 weights, then the bias), to compare predictors.

use std::path::{Path, PathBuf};

use picturesque_app::analysis::clip::aesthetic_head;
use picturesque_app::ports::inference::{InferenceEngine, ModelSource, ProviderChoice, Tensor};
use picturesque_app::ports::pixels::PixelLoader;
use picturesque_core::clip::{Embedding, LinearHead, preprocess};
use picturesque_core::quality::technical_scores;
use picturesque_infra::images::VipsPixels;
use picturesque_infra::inference::OrtEngine;

fn photos(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("folder")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("jpg")))
        .collect();
    files.sort();
    files
}

/// Chance that a random `good` value beats a random `bad` one.
fn auc(good: &[f32], bad: &[f32]) -> f64 {
    let mut wins = 0.0;
    for g in good {
        for b in bad {
            wins += if g > b {
                1.0
            } else if g == b {
                0.5
            } else {
                0.0
            };
        }
    }
    wins / (good.len() * bad.len()) as f64
}

fn summary(name: &str, values: &[f32]) {
    let mut v = values.to_vec();
    v.sort_by(f32::total_cmp);
    let q = |p: f64| v[((v.len() - 1) as f64 * p).round() as usize];
    let mean = v.iter().sum::<f32>() / v.len() as f32;
    println!(
        "{name:<26} n={:<4} mean {mean:6.3}  min {:6.3}  q1 {:6.3}  median {:6.3}  q3 {:6.3}  max {:6.3}",
        v.len(),
        q(0.0),
        q(0.25),
        q(0.5),
        q(0.75),
        q(1.0)
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (model, good, ordinary) = match &args[..] {
        [m, g, o] | [m, g, o, _] => (m, g, o),
        _ => {
            panic!("usage: eval_aesthetic <model.onnx> <good folder> <ordinary folder> [head.bin]")
        }
    };
    let engine = OrtEngine::new(std::thread::available_parallelism().map_or(1, |n| n.get()));
    let model = engine
        .load(ModelSource::File(Path::new(model)), ProviderChoice::Cpu)
        .expect("model");
    let head = match args.get(3) {
        Some(path) => LinearHead::from_le_bytes(&std::fs::read(path).expect("head")).expect("head"),
        None => aesthetic_head(),
    };
    let score = |path: &Path| -> (f32, f32) {
        let image = VipsPixels.load(path, 1024).expect("decode");
        let out = model
            .run(vec![Tensor::f32(
                "image",
                &[1, 3, 224, 224],
                preprocess(&image),
            )])
            .expect("inference");
        let embedding = Embedding::new(out[0].as_f32().unwrap()).unwrap();
        (
            head.apply(&embedding).unwrap(),
            technical_scores(&image).overall,
        )
    };
    let mut groups = Vec::new();
    for dir in [good, ordinary] {
        let scored: Vec<(f32, f32)> = photos(Path::new(dir)).iter().map(|p| score(p)).collect();
        groups.push(scored);
    }
    let pick = |g: &[(f32, f32)], f: fn(&(f32, f32)) -> f32| g.iter().map(f).collect::<Vec<_>>();
    let (aes_good, aes_bad) = (pick(&groups[0], |s| s.0), pick(&groups[1], |s| s.0));
    let (tech_good, tech_bad) = (pick(&groups[0], |s| s.1), pick(&groups[1], |s| s.1));
    summary("aesthetic, good", &aes_good);
    summary("aesthetic, ordinary", &aes_bad);
    summary("technical, good", &tech_good);
    summary("technical, ordinary", &tech_bad);
    println!("AUC aesthetic {:.3}", auc(&aes_good, &aes_bad));
    println!("AUC technical {:.3}", auc(&tech_good, &tech_bad));
}
