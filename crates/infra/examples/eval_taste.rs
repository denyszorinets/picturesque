//! Measures how well the personal taste model learns a preference from
//! picks and rejects (`spikes/taste/results.md`).
//!
//!     cargo run --release -p picturesque-infra --example eval_taste -- \
//!         <clip visual model.onnx> <liked folder> <disliked folder> [cache]
//!
//! Embeds every JPEG like the app does, then treats the first folder as
//! picks and the second as rejects. Prints, for growing numbers of
//! examples, the model's cross-validated agreement and its AUC on a
//! held-out half, next to LAION's general score and their blend; then the
//! same with the folders swapped (a taste the general score cannot know),
//! and the training time for the largest library size. `cache` keeps the
//! embeddings between runs.

use std::path::{Path, PathBuf};
use std::time::Instant;

use picturesque_app::analysis::clip::{aesthetic_head, aesthetic_score};
use picturesque_app::ports::inference::{InferenceEngine, ModelSource, ProviderChoice, Tensor};
use picturesque_app::ports::pixels::PixelLoader;
use picturesque_core::clip::{Embedding, preprocess};
use picturesque_core::taste::{MAX_EACH, auc, blend, train};
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

fn embed_all(model_path: &str, dirs: [&str; 2]) -> [Vec<Embedding>; 2] {
    let engine = OrtEngine::new(std::thread::available_parallelism().map_or(1, |n| n.get()));
    let model = engine
        .load(
            ModelSource::File(Path::new(model_path)),
            ProviderChoice::Cpu,
        )
        .expect("model");
    dirs.map(|dir| {
        photos(Path::new(dir))
            .iter()
            .map(|path| {
                let image = VipsPixels.load(path, 1024).expect("decode");
                let out = model
                    .run(vec![Tensor::f32(
                        "image",
                        &[1, 3, 224, 224],
                        preprocess(&image),
                    )])
                    .expect("inference");
                Embedding::new(out[0].as_f32().unwrap()).unwrap()
            })
            .collect()
    })
}

fn load_or_embed(model: &str, dirs: [&str; 2], cache: Option<&String>) -> [Vec<Embedding>; 2] {
    if let Some(text) = cache.and_then(|c| std::fs::read_to_string(c).ok()) {
        let mut groups = [Vec::new(), Vec::new()];
        for line in text.lines() {
            let (group, encoded) = line.split_once(' ').expect("cache line");
            groups[usize::from(group == "1")].push(Embedding::decode(encoded).expect("cached"));
        }
        return groups;
    }
    let groups = embed_all(model, dirs);
    if let Some(cache) = cache {
        let text: String = groups
            .iter()
            .enumerate()
            .flat_map(|(g, es)| es.iter().map(move |e| format!("{g} {}\n", e.encode())))
            .collect();
        std::fs::write(cache, text).expect("write cache");
    }
    groups
}

/// Liked and disliked photos interleaved, as decisions are over a shoot.
fn interleave<'a>(liked: &'a [Embedding], disliked: &'a [Embedding]) -> Vec<(&'a Embedding, bool)> {
    let mut out = Vec::new();
    for i in 0..liked.len().max(disliked.len()) {
        if let Some(e) = liked.get(i) {
            out.push((e, true));
        }
        if let Some(e) = disliked.get(i) {
            out.push((e, false));
        }
    }
    out
}

fn report(name: &str, liked: &[Embedding], disliked: &[Embedding]) {
    let head = aesthetic_head();
    let general = |e: &Embedding| aesthetic_score(head.apply(e).unwrap());
    // The second half of each group is held out.
    let (train_liked, test_liked) = liked.split_at(liked.len() / 2);
    let (train_disliked, test_disliked) = disliked.split_at(disliked.len() / 2);
    let test = interleave(test_liked, test_disliked);
    let general_auc = auc(&test
        .iter()
        .map(|(e, p)| (general(e), *p))
        .collect::<Vec<_>>());
    println!("\n{name}: held-out {} photos", test.len());
    println!(
        "  general score (LAION)          AUC {:.3}",
        general_auc.unwrap()
    );
    println!("  per class  cross-val  held-out  blend 50%");
    let most = train_liked.len().min(train_disliked.len());
    for n in [10, 20, 40, 80, most].into_iter().filter(|n| *n <= most) {
        let examples = interleave(&train_liked[..n], &train_disliked[..n]);
        let Some(model) = train(&examples) else {
            println!("  {n:>9}  (not enough)");
            continue;
        };
        let personal: Vec<(f32, bool)> = test
            .iter()
            .map(|(e, p)| (model.score(e).unwrap(), *p))
            .collect();
        let blended: Vec<(f32, bool)> = test
            .iter()
            .map(|(e, p)| {
                let b = blend(Some(general(e)), model.score(e), 0.5).unwrap();
                (b, *p)
            })
            .collect();
        println!(
            "  {n:>9}  {:>9.3}  {:>8.3}  {:>9.3}",
            model.agreement,
            auc(&personal).unwrap(),
            auc(&blended).unwrap()
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [model, liked, disliked, ..] = &args[..] else {
        panic!("usage: eval_taste <model.onnx> <liked folder> <disliked folder> [cache]");
    };
    let [liked, disliked] = load_or_embed(model, [liked, disliked], args.get(3));
    report("Picks are the first folder", &liked, &disliked);
    report(
        "Picks are the second folder (reversed taste)",
        &disliked,
        &liked,
    );

    // The largest training set: MAX_EACH of each class, cycling the sample.
    let big: Vec<(&Embedding, bool)> = (0..2 * MAX_EACH)
        .map(|i| {
            let group = if i.is_multiple_of(2) {
                &liked
            } else {
                &disliked
            };
            (&group[(i / 2) % group.len()], i.is_multiple_of(2))
        })
        .collect();
    let start = Instant::now();
    train(&big).expect("enough examples");
    println!(
        "\nTraining on {} examples: {:.2} s",
        big.len(),
        start.elapsed().as_secs_f64()
    );
}
