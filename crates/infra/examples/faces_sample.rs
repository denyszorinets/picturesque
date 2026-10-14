//! Runs face detection and the face mesh on photos and prints each face's
//! eye aspect ratios, to check and tune closed-eye detection.
//!
//!     cargo run --release -p picturesque-infra --example faces_sample -- \
//!         <yunet.onnx> <face mesh.onnx> <photo.jpg> ...

use std::path::Path;

use picturesque_app::ports::inference::{InferenceEngine, ModelSource, ProviderChoice, Tensor};
use picturesque_app::ports::pixels::PixelLoader;
use picturesque_core::faces::{
    DETECTOR_SIZE, DetectorLevel, MESH_SIZE, decode_detections, detector_input, eye_openness,
    mesh_input,
};
use picturesque_infra::images::VipsPixels;
use picturesque_infra::inference::OrtEngine;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [detector, mesh, photos @ ..] = &args[..] else {
        panic!("usage: faces_sample <yunet.onnx> <mesh.onnx> <photo>...");
    };
    let engine = OrtEngine::new(4);
    let load = |p: &str| {
        engine
            .load(ModelSource::File(Path::new(p)), ProviderChoice::Cpu)
            .unwrap()
    };
    let (detector, mesh) = (load(detector), load(mesh));
    for photo in photos {
        let image = VipsPixels.load(Path::new(photo), 1024).unwrap();
        let (input, scale) = detector_input(&image);
        let size = DETECTOR_SIZE as usize;
        let out = detector
            .run(vec![Tensor::f32("input", &[1, 3, size, size], input)])
            .unwrap();
        let get = |name: &str| {
            out.iter()
                .find(|t| t.name == name)
                .unwrap()
                .as_f32()
                .unwrap()
        };
        let levels: Vec<DetectorLevel> = [8u32, 16, 32]
            .iter()
            .map(|s| DetectorLevel {
                stride: *s,
                cls: get(&format!("cls_{s}")),
                obj: get(&format!("obj_{s}")),
                bbox: get(&format!("bbox_{s}")),
            })
            .collect();
        let faces = decode_detections(&levels, scale);
        println!("{photo}: {} face(s)", faces.len());
        for f in faces {
            let n = MESH_SIZE as usize;
            let m = mesh
                .run(vec![Tensor::f32(
                    "input_12",
                    &[1, n, n, 3],
                    mesh_input(&image, &f),
                )])
                .unwrap();
            let presence = m[1].as_f32().unwrap()[0];
            let eyes = eye_openness(m[0].as_f32().unwrap());
            println!(
                "  box {:.0},{:.0} {:.0}x{:.0} score {:.2} presence {presence:.2} eyes {eyes:?}",
                f.x, f.y, f.width, f.height, f.score
            );
        }
    }
}
