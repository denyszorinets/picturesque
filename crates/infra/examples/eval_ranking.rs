//! Measures how often the best-in-series ranking agrees with your own
//! choice, with technical scores only (as in M3) and with the AI signals
//! (`spikes/ranking/README.md`).
//!
//!     cargo run --release -p picturesque-infra --example eval_ranking -- \
//!         <shoot folder> <clip visual model.onnx> <yunet.onnx> <face mesh.onnx>
//!
//! The shoot folder holds one subfolder per series with its JPEGs; the
//! photo you consider best has a file name starting with `best`. Every
//! photo is analyzed like the app does (technical scores, CLIP aesthetic
//! score, faces), then each series is ranked both ways.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use picturesque_app::analysis::clip::{ClipAnalyzer, aesthetic_score};
use picturesque_app::analysis::faces::{FaceAnalyzer, faces, signal};
use picturesque_app::analysis::technical::TechnicalAnalyzer;
use picturesque_app::api::{Api, AppContext};
use picturesque_app::models::{
    CLIP_IMAGE, CLIP_IMAGE_FILE, FACE_DETECTOR_FILE, FACE_MESH_FILE, FACES,
};
use picturesque_app::ports::catalog::{AnalyzerId, PhotoCatalog};
use picturesque_app::ports::events::NoEvents;
use picturesque_app::ports::models::{ModelFile, ModelSpec};
use picturesque_app::ports::settings::SettingsStore;
use picturesque_app::settings::Settings;
use picturesque_core::hash::ContentHash;
use picturesque_core::media::MediaFormat;
use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets, PhotoId};
use picturesque_core::ranking::{Candidate, RankWeights, rank};
use picturesque_infra::catalog::SqliteCatalog;
use picturesque_infra::fs::StdFileSystem;
use picturesque_infra::geocode::GeoNamesGeocoder;
use picturesque_infra::images::{VipsDecoder, VipsPixels};
use picturesque_infra::metadata::ExifMetadataReader;

fn installed(models: &Path, id: &str, files: &[(&str, &str)]) -> ModelSpec {
    let files = files
        .iter()
        .map(|(name, source)| {
            let dest = models.join(id).join("v1").join(name);
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::copy(source, &dest).unwrap();
            ModelFile {
                name: (*name).into(),
                url: String::new(),
                sha256: String::new(),
                size: std::fs::metadata(source).unwrap().len(),
            }
        })
        .collect();
    ModelSpec {
        id: id.into(),
        name: id.into(),
        purpose: String::new(),
        version: 1,
        license: String::new(),
        license_url: String::new(),
        homepage: String::new(),
        files,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [shoot, clip, detector, mesh] = &args[..] else {
        panic!("usage: eval_ranking <shoot folder> <clip.onnx> <yunet.onnx> <mesh.onnx>");
    };
    let dir = tempfile::tempdir().unwrap();
    let models = dir.path().join("models");
    let specs = vec![
        installed(&models, CLIP_IMAGE, &[(CLIP_IMAGE_FILE, clip)]),
        installed(
            &models,
            FACES,
            &[(FACE_DETECTOR_FILE, detector), (FACE_MESH_FILE, mesh)],
        ),
    ];
    let catalog = Arc::new(SqliteCatalog::open_in_memory().unwrap());
    let shoot = PathBuf::from(shoot).canonicalize().unwrap();
    catalog
        .save_settings(&Settings {
            library_root: Some(shoot.clone()),
            ..Default::default()
        })
        .unwrap();
    // Series: (name, [(photo, is the user's pick)]).
    let mut series: Vec<(String, Vec<(PhotoId, bool)>)> = Vec::new();
    let mut folders: Vec<_> = std::fs::read_dir(&shoot)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    folders.retain(|p| p.is_dir());
    folders.sort();
    let mut n = 0u64;
    for folder in folders {
        let mut files: Vec<_> = std::fs::read_dir(&folder)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        files.retain(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("jpg")));
        files.sort();
        let mut members = Vec::new();
        for file in files {
            n += 1;
            let mut hash = [0u8; 32];
            hash[..8].copy_from_slice(&n.to_le_bytes());
            let id = catalog
                .insert_photo(&NewPhoto {
                    metadata: Default::default(),
                    assets: PhotoAssets::new(vec![Asset {
                        format: MediaFormat::Jpeg,
                        path: file.strip_prefix(&shoot).unwrap().to_path_buf(),
                        size: std::fs::metadata(&file).unwrap().len(),
                        hash: ContentHash::from_bytes(hash),
                    }])
                    .unwrap(),
                })
                .unwrap();
            let pick = file
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_lowercase()
                .starts_with("best");
            members.push((id, pick));
        }
        if members.len() >= 2 && members.iter().filter(|m| m.1).count() == 1 {
            series.push((
                folder.file_name().unwrap().to_string_lossy().into_owned(),
                members,
            ));
        } else {
            eprintln!(
                "skipping {}: needs 2+ photos and exactly one `best…`",
                folder.display()
            );
        }
    }
    let api = Api::new(AppContext {
        pipeline_store: catalog.clone(),
        watcher: Arc::new(picturesque_app::ports::watcher::NoWatcher),
        critic: Arc::new(picturesque_infra::critic::CriticAdapter::new()),
        settings_store: catalog.clone(),
        catalog: catalog.clone(),
        fs: Arc::new(StdFileSystem),
        metadata: Arc::new(ExifMetadataReader),
        geocoder: Arc::new(GeoNamesGeocoder::default()),
        events: Arc::new(NoEvents),
        decoder: Arc::new(VipsDecoder),
        sidecars: Arc::new(picturesque_infra::xmp::XmpSidecars),
        pixels: Arc::new(VipsPixels),
        analyzers: picturesque_app::analysis::default_analyzers(),
        inference: Arc::new(picturesque_infra::inference::OrtEngine::for_background()),
        models: specs,
        model_store: Arc::new(picturesque_infra::models::HttpModelStore::new(models)),
        cache_dir: dir.path().join("cache"),
    })
    .unwrap();
    eprintln!("analyzing {n} photos…");
    api.run_analysis();

    let result = |id: PhotoId, analyzer: AnalyzerId| {
        catalog
            .analysis(id)
            .unwrap()
            .into_iter()
            .find(|(name, version, _)| *name == analyzer.name && *version == analyzer.version)
            .map(|(_, _, r)| r)
    };
    let candidate = |id: PhotoId, ai: bool| Candidate {
        id,
        scores: result(id, TechnicalAnalyzer::ID).and_then(|r| serde_json::from_value(r).ok()),
        rejected: false,
        aesthetic: ai
            .then(|| result(id, ClipAnalyzer::ID))
            .flatten()
            .and_then(|r| ClipAnalyzer::aesthetic(&r))
            .map(aesthetic_score),
        face: ai
            .then(|| result(id, FaceAnalyzer::ID))
            .flatten()
            .and_then(|r| faces(&r))
            .and_then(|f| signal(&f)),
    };
    let technical_only = RankWeights {
        aesthetic: 0.0,
        ..Default::default()
    };
    let (mut m3_hits, mut ai_hits) = (0, 0);
    println!(
        "{:<30} {:>6} {:>10} {:>10}",
        "series", "photos", "M3 rank", "AI rank"
    );
    for (name, members) in &series {
        let pick = members.iter().find(|m| m.1).unwrap().0;
        let place = |ai: bool, weights| {
            let candidates: Vec<Candidate> = members.iter().map(|m| candidate(m.0, ai)).collect();
            rank(&candidates, weights)
                .iter()
                .position(|r| r.id == pick)
                .unwrap()
                + 1
        };
        let (m3, ai) = (
            place(false, technical_only),
            place(true, RankWeights::default()),
        );
        m3_hits += usize::from(m3 == 1);
        ai_hits += usize::from(ai == 1);
        println!("{name:<30} {:>6} {m3:>10} {ai:>10}", members.len());
    }
    let total = series.len().max(1) as f64;
    println!(
        "\nyour pick ranked first: M3 {m3_hits}/{} ({:.0}%), AI {ai_hits}/{} ({:.0}%)",
        series.len(),
        100.0 * m3_hits as f64 / total,
        series.len(),
        100.0 * ai_hits as f64 / total
    );
}
