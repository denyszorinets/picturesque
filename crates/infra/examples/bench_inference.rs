//! Measures ONNX model speed and stability on each execution provider
//! (`docs/development/benchmarks.rst`).
//!
//!     cargo run --release -p picturesque-infra --example bench_inference -- \
//!         <model.onnx> [runs] [input-name] [shape, e.g. 1x3x224x224]
//!
//! Defaults fit the CLIP image encoder (`image`, `1x3x224x224`). For each
//! provider this build has, it reports which provider really ran, load
//! time, latency (mean, p50, p95 after warm-up) and the largest difference
//! from the CPU output, then repeats the run to check results stay the
//! same.

use std::path::Path;
use std::time::Instant;

use picturesque_app::ports::inference::{
    InferenceEngine, Model, ModelSource, Provider, ProviderChoice, Tensor,
};
use picturesque_infra::inference::OrtEngine;

const WARMUP: usize = 3;

fn choice(p: Provider) -> ProviderChoice {
    match p {
        Provider::Cpu => ProviderChoice::Cpu,
        Provider::DirectMl => ProviderChoice::DirectMl,
        Provider::CoreMl => ProviderChoice::CoreMl,
        Provider::WebGpu => ProviderChoice::WebGpu,
    }
}

/// Deterministic pseudo-random input in roughly the normalized range.
fn input(len: usize, seed: u64) -> Vec<f32> {
    let mut x = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    (0..len)
        .map(|_| {
            x = x
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((x >> 33) as f32 / (1u64 << 31) as f32) * 4.0 - 2.0
        })
        .collect()
}

fn run(model: &dyn Model, name: &str, shape: &[usize], data: &[f32]) -> Vec<f32> {
    let out = model
        .run(vec![Tensor::f32(name, shape, data.to_vec())])
        .expect("inference");
    out[0].as_f32().expect("f32 output").to_vec()
}

fn max_diff(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f32::max)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args
        .first()
        .expect("usage: bench_inference <model.onnx> [runs] [input] [shape]");
    let runs: usize = args.get(1).map_or(50, |r| r.parse().expect("runs"));
    let name = args.get(2).map_or("image", String::as_str);
    let shape: Vec<usize> = args
        .get(3)
        .map_or("1x3x224x224", String::as_str)
        .split('x')
        .map(|d| d.parse().expect("shape"))
        .collect();
    let len = shape.iter().product();
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let engine = OrtEngine::new(threads);
    let inputs: Vec<Vec<f32>> = (0..runs.min(8) as u64).map(|s| input(len, s)).collect();

    println!(
        "model {} · input {name} {shape:?} · {runs} runs · {threads} CPU threads",
        Path::new(path).display()
    );
    println!(
        "{:<10} {:<10} {:>9} {:>9} {:>9} {:>9} {:>12} {:>8}",
        "asked", "ran on", "load ms", "mean ms", "p50 ms", "p95 ms", "max |Δ| CPU", "stable"
    );
    let mut reference: Option<Vec<Vec<f32>>> = None;
    for provider in engine.available().into_iter().rev() {
        let start = Instant::now();
        let model = match engine.load(ModelSource::File(Path::new(path)), choice(provider)) {
            Ok(m) => m,
            Err(e) => {
                println!("{:<10} failed to load: {e}", provider.label());
                continue;
            }
        };
        let load = start.elapsed().as_secs_f64() * 1e3;
        for data in inputs.iter().take(WARMUP) {
            run(model.as_ref(), name, &shape, data);
        }
        let mut times = Vec::with_capacity(runs);
        let mut first: Vec<Vec<f32>> = Vec::new();
        let mut stable = true;
        for i in 0..runs {
            let data = &inputs[i % inputs.len()];
            let start = Instant::now();
            let out = run(model.as_ref(), name, &shape, data);
            times.push(start.elapsed().as_secs_f64() * 1e3);
            if i < inputs.len() {
                first.push(out);
            } else if max_diff(&first[i % inputs.len()], &out) > 1e-4 {
                stable = false;
            }
        }
        times.sort_by(f64::total_cmp);
        let mean = times.iter().sum::<f64>() / times.len() as f64;
        let pct = |p: f64| times[((times.len() - 1) as f64 * p).round() as usize];
        let diff = reference.as_ref().map_or(0.0, |r| {
            r.iter()
                .zip(&first)
                .map(|(a, b)| max_diff(a, b))
                .fold(0.0, f32::max)
        });
        println!(
            "{:<10} {:<10} {load:>9.0} {mean:>9.1} {:>9.1} {:>9.1} {diff:>12.2e} {:>8}",
            provider.label(),
            model.provider().label(),
            pct(0.5),
            pct(0.95),
            if stable { "yes" } else { "NO" },
        );
        if let Some(reason) = model.fallback() {
            println!("           fallback: {reason}");
        }
        if provider == Provider::Cpu {
            reference = Some(first);
        }
    }
}
