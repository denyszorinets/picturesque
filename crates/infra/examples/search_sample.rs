//! Tries text search on a folder of photos with the real CLIP models
//! (`spikes/aesthetic/README.md` downloads a sample).
//!
//!     cargo run --release -p picturesque-infra --example search_sample -- \
//!         <visual model.onnx> <textual model.onnx> <merges.txt> <folder> "query" ...
//!
//! Prints each query's best matches with their similarity, and the
//! similarity of the median photo, to choose a relevance cut-off.

use std::path::Path;

use picturesque_app::ports::inference::{InferenceEngine, ModelSource, ProviderChoice, Tensor};
use picturesque_app::ports::pixels::PixelLoader;
use picturesque_core::clip::{Embedding, preprocess};
use picturesque_core::clip_tokenizer::{CONTEXT_LENGTH, ClipTokenizer};
use picturesque_infra::images::VipsPixels;
use picturesque_infra::inference::OrtEngine;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [visual, textual, merges, folder, queries @ ..] = &args[..] else {
        panic!("usage: search_sample <visual.onnx> <textual.onnx> <merges.txt> <folder> query...");
    };
    let engine = OrtEngine::new(std::thread::available_parallelism().map_or(1, |n| n.get()));
    let load = |p: &str| {
        engine
            .load(ModelSource::File(Path::new(p)), ProviderChoice::Cpu)
            .unwrap()
    };
    let (visual, textual) = (load(visual), load(textual));
    let tokenizer = ClipTokenizer::from_merges(&std::fs::read_to_string(merges).unwrap()).unwrap();
    let titles: std::collections::HashMap<String, String> =
        std::fs::read_to_string(Path::new(folder).join("sources.txt"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| l.split_once('\t'))
            .map(|(f, t)| (f.to_owned(), t.to_owned()))
            .collect();
    let mut photos: Vec<(String, Embedding)> = Vec::new();
    let mut files: Vec<_> = std::fs::read_dir(folder)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    files.sort();
    for path in files
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "jpg"))
    {
        let image = VipsPixels.load(path, 1024).unwrap();
        let out = visual
            .run(vec![Tensor::f32(
                "image",
                &[1, 3, 224, 224],
                preprocess(&image),
            )])
            .unwrap();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        photos.push((name, Embedding::new(out[0].as_f32().unwrap()).unwrap()));
    }
    for query in queries {
        let ids = tokenizer.encode(query);
        let out = textual
            .run(vec![Tensor::i32("text", &[1, CONTEXT_LENGTH], ids)])
            .unwrap();
        let q = Embedding::new(out[0].as_f32().unwrap()).unwrap();
        let mut scored: Vec<(f32, &str)> = photos
            .iter()
            .map(|(n, e)| (q.similarity(e), n.as_str()))
            .collect();
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        println!(
            "\n\"{query}\" (median photo {:.3})",
            scored[scored.len() / 2].0
        );
        for (s, name) in scored.iter().take(5) {
            println!(
                "  {s:.3}  {name}  {}",
                titles.get(*name).map_or("", String::as_str)
            );
        }
    }
}
