//! Measures how long grid thumbnails and loupe previews take to render
//! from photos (RAW files read through their embedded preview), which is
//! what fast mode waits on (`docs/features/fast-mode.rst`).
//!
//!     cargo run --release -p picturesque-infra --example bench_thumbnails -- \
//!         <photo>... [--runs N]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use picturesque_app::ports::images::ImageDecoder;
use picturesque_core::media::MediaFormat;
use picturesque_core::metadata::Orientation;
use picturesque_infra::images::VipsDecoder;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let runs = match args.iter().position(|a| a == "--runs") {
        Some(i) => {
            let n = args.get(i + 1).and_then(|n| n.parse().ok()).unwrap_or(5);
            args.drain(i..=(i + 1).min(args.len() - 1));
            n
        }
        None => 5,
    };
    if args.is_empty() {
        eprintln!("usage: bench_thumbnails <photo>... [--runs N]");
        std::process::exit(2);
    }
    let out = tempfile::tempdir().expect("temp dir");
    for path in args.iter().map(PathBuf::from) {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
        let Some(format) = name.as_deref().and_then(MediaFormat::from_file_name) else {
            eprintln!("{}: not a photo", path.display());
            continue;
        };
        for (label, edge) in [("grid 512", 512), ("preview 2048", 2048)] {
            let mut times = Vec::new();
            let mut size = None;
            for run in 0..runs {
                let dest = out.path().join(format!("{label}-{run}.jpg"));
                let started = Instant::now();
                match VipsDecoder.render(&path, format, Orientation::Normal, edge, &dest) {
                    Ok(r) => size = Some((r.width, r.height, r.source_size)),
                    Err(e) => {
                        eprintln!("{}: {e}", path.display());
                        break;
                    }
                }
                times.push(started.elapsed());
            }
            times.sort();
            let median = times
                .get(times.len() / 2)
                .copied()
                .unwrap_or(Duration::ZERO);
            println!(
                "{:<40} {label:<13} median {:>7.1} ms  {:?}",
                name.as_deref().unwrap_or("?"),
                median.as_secs_f64() * 1000.0,
                size
            );
        }
    }
}
