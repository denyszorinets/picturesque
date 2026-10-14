//! Decoding spike for ADR-0005 (issue #10).
//!
//! Compares three stacks on the corpus fetched by `scripts/fetch_corpus.py`:
//!
//! - A: libvips for everything (RAW through its LibRaw-based `dcrawload`)
//! - B: libvips + LibRaw embedded-preview extraction (hand-written FFI)
//! - C: pure Rust (`image`, `rawler`) + `libheif-rs` for HEIC
//!
//! Usage: `cargo run --release -- <corpus-dir>`; prints Markdown tables.

mod libraw;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use image::{DynamicImage, GenericImageView, ImageDecoder, ImageReader};
use libvips::{VipsApp, VipsImage, ops};

const THUMB: u32 = 512;
const RUNS: usize = 3;

fn main() {
    let corpus = PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("usage: decoding-spike <corpus-dir>"),
    );
    let app = VipsApp::new("decoding-spike", false).expect("init libvips");
    // Without this, repeat runs are served from libvips' operation cache.
    app.cache_set_max(0);
    app.cache_set_max_mem(0);
    println!(
        "libvips {} · LibRaw {}\n",
        app.version_string().unwrap_or("?"),
        libraw::version()
    );

    prepare_formats(&corpus, &app);
    bench_raw(&corpus, &app);
    bench_formats(&corpus, &app);
    check_orientation(&corpus, &app);
    check_heic(&corpus, &app);
}

/// Times `f` `RUNS` times after one warm-up and returns the median.
fn time<T>(mut f: impl FnMut() -> Result<T, String>) -> Result<(Duration, T), String> {
    f()?;
    let mut samples = Vec::new();
    let mut last = None;
    for _ in 0..RUNS {
        let start = Instant::now();
        let v = f()?;
        samples.push(start.elapsed());
        last = Some(v);
    }
    samples.sort();
    Ok((samples[RUNS / 2], last.expect("at least one run")))
}

fn ms(d: Duration) -> String {
    format!("{:.0} ms", d.as_secs_f64() * 1000.0)
}

fn vips_err(app: &VipsApp, what: &str) -> String {
    let msg = app
        .error_buffer()
        .unwrap_or("")
        .lines()
        .next()
        .unwrap_or("")
        .to_owned();
    app.error_clear();
    format!("{what}: {msg}")
}

fn vips_thumb_file(app: &VipsApp, path: &Path) -> Result<(u32, u32), String> {
    let img =
        ops::thumbnail(&path.to_string_lossy(), THUMB as i32).map_err(|_| vips_err(app, "vips"))?;
    materialize(app, &img)?;
    Ok((img.get_width() as u32, img.get_height() as u32))
}

fn vips_thumb_buffer(app: &VipsApp, bytes: &[u8]) -> Result<VipsImage, String> {
    let img =
        ops::thumbnail_buffer(bytes, THUMB as i32).map_err(|_| vips_err(app, "vips buffer"))?;
    materialize(app, &img)?;
    Ok(img)
}

/// libvips is lazy: computing the pixels is what actually decodes the file.
fn materialize(app: &VipsApp, img: &VipsImage) -> Result<(), String> {
    // The average needs every pixel, with no encoder cost.
    ops::avg(img)
        .map(|_| ())
        .map_err(|_| vips_err(app, "vips decode"))
}

fn rust_thumb(img: &DynamicImage) -> DynamicImage {
    img.thumbnail(THUMB, THUMB)
}

fn files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.is_file())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn short(path: &Path) -> String {
    path.file_name()
        .unwrap()
        .to_string_lossy()
        .chars()
        .take(48)
        .collect()
}

fn cell<T>(r: &Result<(Duration, T), String>, extra: impl Fn(&T) -> String) -> String {
    match r {
        Ok((d, v)) => format!("{} {}", ms(*d), extra(v)),
        Err(e) => format!("❌ {}", e.chars().take(60).collect::<String>()),
    }
}

/// Creates JPEG/PNG/TIFF/WebP samples from a full-size embedded RAW preview.
fn prepare_formats(corpus: &Path, app: &VipsApp) {
    let out = corpus.join("formats");
    if out.join("photo.jpg").exists() {
        return;
    }
    std::fs::create_dir_all(&out).unwrap();
    let source = files(&corpus.join("raw"))
        .into_iter()
        .filter_map(|p| libraw::thumb(&p).ok().filter(|t| t.is_jpeg))
        .max_by_key(|t| t.bytes.len())
        .expect("a RAW with a JPEG preview");
    std::fs::write(out.join("photo.jpg"), &source.bytes).unwrap();
    let img = VipsImage::new_from_file(&out.join("photo.jpg").to_string_lossy()).unwrap();
    for ext in ["png", "tif", "webp"] {
        img.image_write_to_file(&out.join(format!("photo.{ext}")).to_string_lossy())
            .unwrap_or_else(|_| panic!("{}", vips_err(app, ext)));
    }
}

fn bench_raw(corpus: &Path, app: &VipsApp) {
    println!("## RAW: 512 px thumbnail (median of {RUNS} warm runs)\n");
    println!(
        "| File | A: vips dcrawload | B: LibRaw preview + vips | C: rawler preview + image | LibRaw unpack | rawler decode |"
    );
    println!("|---|---|---|---|---|---|");
    for path in files(&corpus.join("raw")) {
        let a = time(|| vips_thumb_file(app, &path));
        let b = time(|| {
            let t = libraw::thumb(&path)?;
            if t.is_jpeg {
                let img = vips_thumb_buffer(app, &t.bytes)?;
                // Header only: libvips does not decode pixels to answer this.
                let full = VipsImage::new_from_buffer(&t.bytes, "")
                    .map_err(|_| vips_err(app, "header"))?;
                Ok(format!(
                    "{}x{} jpeg → {}x{}",
                    full.get_width(),
                    full.get_height(),
                    img.get_width(),
                    img.get_height()
                ))
            } else {
                Ok(format!("{}x{} bitmap", t.width, t.height))
            }
        });
        let c = time(|| {
            let preview = rawler::analyze::extract_preview_pixels(&path, &Default::default())
                .map_err(|e| e.to_string())?;
            let (w, h) = preview.dimensions();
            let t = rust_thumb(&preview);
            Ok(format!("{w}x{h} → {}x{}", t.width(), t.height()))
        });
        let unpack = libraw::unpack(&path)
            .map(|_| "✅".to_owned())
            .unwrap_or_else(|e| format!("❌ {e}"));
        let decode = rawler::decode_file(&path)
            .map(|_| "✅".to_owned())
            .unwrap_or_else(|e| {
                format!("❌ {}", e.to_string().chars().take(50).collect::<String>())
            });
        println!(
            "| {} | {} | {} | {} | {} | {} |",
            short(&path),
            cell(&a, |(w, h)| format!("({w}x{h})")),
            cell(&b, |s| format!("({s})")),
            cell(&c, |s| format!("({s})")),
            unpack,
            decode
        );
    }
    println!();
}

fn bench_formats(corpus: &Path, app: &VipsApp) {
    println!("## JPEG / PNG / TIFF / WebP: 512 px thumbnail (median of {RUNS} warm runs)\n");
    println!("| File | Size | A/B: vips thumbnail | C: image decode + thumbnail |");
    println!("|---|---|---|---|");
    for path in files(&corpus.join("formats")) {
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let dims = image::image_dimensions(&path)
            .map(|(w, h)| format!("{w}x{h}"))
            .unwrap_or_default();
        let vips = time(|| vips_thumb_file(app, &path));
        let rust = time(|| {
            let img = image::open(&path).map_err(|e| e.to_string())?;
            let t = rust_thumb(&img);
            Ok((t.width(), t.height()))
        });
        println!(
            "| {} | {dims}, {:.1} MB | {} | {} |",
            short(&path),
            size as f64 / 1e6,
            cell(&vips, |(w, h)| format!("({w}x{h})")),
            cell(&rust, |(w, h)| format!("({w}x{h})"))
        );
    }
    println!();
}

/// Grayscale 32x32 signature for comparing thumbnails.
fn signature(img: &DynamicImage) -> Vec<u8> {
    img.resize_exact(32, 32, image::imageops::FilterType::Triangle)
        .to_luma8()
        .into_raw()
}

fn diff(a: &[u8], b: &[u8]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (*x as f64 - *y as f64).abs())
        .sum::<f64>()
        / a.len() as f64
}

fn vips_to_dynamic(img: &VipsImage) -> DynamicImage {
    let png = img.image_write_to_buffer(".png").expect("png encode");
    image::load_from_memory(&png).expect("decode png")
}

fn rust_oriented(path: &Path) -> Result<DynamicImage, String> {
    let mut decoder = ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .into_decoder()
        .map_err(|e| e.to_string())?;
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    img.apply_orientation(orientation);
    Ok(img)
}

fn check_orientation(corpus: &Path, app: &VipsApp) {
    println!("## EXIF orientation (difference from upright reference; < 10 is correct)\n");
    println!("| File | vips thumbnail | image + apply_orientation |");
    println!("|---|---|---|");
    let dir = corpus.join("orientation");
    let reference = signature(&image::open(dir.join("Landscape_1.jpg")).expect("reference"));
    for path in files(&dir) {
        let v = ops::thumbnail(&path.to_string_lossy(), THUMB as i32)
            .map(|i| diff(&signature(&vips_to_dynamic(&i)), &reference))
            .map_err(|_| vips_err(app, "vips"));
        let r = rust_oriented(&path).map(|i| diff(&signature(&i), &reference));
        let show = |x: Result<f64, String>| match x {
            Ok(d) => format!("{} {:.1}", if d < 10.0 { "✅" } else { "❌" }, d),
            Err(e) => format!("❌ {e}"),
        };
        println!("| {} | {} | {} |", short(&path), show(v), show(r));
    }
    println!();
}

fn check_heic(corpus: &Path, app: &VipsApp) {
    println!("## HEIC\n");
    println!("| File | vips thumbnail | libheif-rs decode |");
    println!("|---|---|---|");
    for path in files(&corpus.join("heic")) {
        let v = time(|| vips_thumb_file(app, &path));
        let h = time(|| {
            let lib = libheif_rs::LibHeif::new();
            let ctx = libheif_rs::HeifContext::read_from_file(&path.to_string_lossy())
                .map_err(|e| e.to_string())?;
            let handle = ctx.primary_image_handle().map_err(|e| e.to_string())?;
            let img = lib
                .decode(
                    &handle,
                    libheif_rs::ColorSpace::Rgb(libheif_rs::RgbChroma::Rgb),
                    None,
                )
                .map_err(|e| e.to_string())?;
            Ok((img.width(), img.height()))
        });
        println!(
            "| {} | {} | {} |",
            short(&path),
            cell(&v, |(w, hh)| format!("({w}x{hh})")),
            cell(&h, |(w, hh)| format!("({w}x{hh})"))
        );
    }
    println!();
}
