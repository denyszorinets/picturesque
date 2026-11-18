//! ONNX Runtime adapter: loading, running, provider fallback.

use std::path::Path;

use picturesque_app::ports::inference::{
    InferenceEngine, ModelSource, Provider, ProviderChoice, Tensor, TensorData,
};
use picturesque_infra::inference::OrtEngine;

fn affine() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/onnx/affine.onnx")
}

#[test]
fn runs_a_model_on_cpu_from_a_file() {
    let engine = OrtEngine::new(2);
    let model = engine
        .load(ModelSource::File(&affine()), ProviderChoice::Cpu)
        .unwrap();
    assert_eq!(model.provider(), Provider::Cpu);
    assert_eq!(model.fallback(), None);

    let out = model
        .run(vec![Tensor::f32(
            "x",
            &[2, 3],
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        )])
        .unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].name, "y");
    assert_eq!(out[0].shape, [2, 3]);
    assert_eq!(
        out[0].data,
        TensorData::F32(vec![3.0, 5.0, 7.0, 9.0, 11.0, 13.0])
    );
}

/// The affine probe alone misses failures while Core ML compiles a dense
/// image-model layer. In the legacy NeuralNetwork format, this fixture
/// crashes in Espresso's inner-product compiler on macOS 27.2 / Apple M4.
/// Require Core ML here: a silent CPU fallback must not make this pass.
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn coreml_image_projection_matches_cpu_without_fallback() {
    let engine = OrtEngine::new(2);
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/onnx/tiny_clip.onnx");
    let bytes = std::fs::read(&path).unwrap();
    let input = || {
        let plane = 224 * 224;
        let pixels = [0.25, 0.5, 0.75]
            .into_iter()
            .flat_map(|value| std::iter::repeat_n(value, plane))
            .collect();
        vec![Tensor::f32("image", &[1, 3, 224, 224], pixels)]
    };
    let cpu = engine
        .load(ModelSource::File(&path), ProviderChoice::Cpu)
        .unwrap();
    let expected = cpu.run(input()).unwrap();
    assert_eq!(expected[0].shape, [1, 768]);
    for choice in [ProviderChoice::CoreMl, ProviderChoice::Auto] {
        for source in [ModelSource::File(&path), ModelSource::Bytes(&bytes)] {
            let model = engine.load(source, choice).unwrap();
            assert_eq!(model.provider(), Provider::CoreMl);
            assert_eq!(model.fallback(), None);
            for _ in 0..2 {
                let actual = model.run(input()).unwrap();
                assert_eq!(actual.len(), expected.len());
                assert_eq!(actual[0].shape, expected[0].shape);
                let actual = actual[0].as_f32().unwrap();
                let expected = expected[0].as_f32().unwrap();
                assert_eq!(actual.len(), expected.len());
                for (&got, &want) in actual.iter().zip(expected) {
                    assert!(
                        got.is_finite() && (got - want).abs() <= 1e-3 * want.abs().max(1.0),
                        "Core ML {got} differs from CPU {want}"
                    );
                }
            }
        }
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
#[ignore = "needs the real CLIP model (PICTURESQUE_CLIP_MODEL) and Core ML hardware"]
fn real_clip_coreml_matches_cpu_without_fallback() {
    let path = std::env::var_os("PICTURESQUE_CLIP_MODEL").expect("PICTURESQUE_CLIP_MODEL");
    let path = Path::new(&path);
    let engine = OrtEngine::new(2);
    let input = || {
        vec![Tensor::f32(
            "image",
            &[1, 3, 224, 224],
            vec![0.5; 3 * 224 * 224],
        )]
    };
    let cpu = engine
        .load(ModelSource::File(path), ProviderChoice::Cpu)
        .unwrap();
    let expected = cpu.run(input()).unwrap();
    drop(cpu);
    let coreml = engine
        .load(ModelSource::File(path), ProviderChoice::CoreMl)
        .unwrap();
    assert_eq!(coreml.provider(), Provider::CoreMl);
    assert_eq!(coreml.fallback(), None);
    let actual = coreml.run(input()).unwrap();
    assert_eq!(actual[0].shape, [1, 768]);
    let actual = actual[0].as_f32().unwrap();
    let expected = expected[0].as_f32().unwrap();
    assert_eq!(actual.len(), expected.len());
    assert!(actual.iter().chain(expected).all(|v| v.is_finite()));
    let dot: f32 = actual.iter().zip(expected).map(|(a, b)| a * b).sum();
    let norm = |v: &[f32]| v.iter().map(|x| x * x).sum::<f32>().sqrt();
    let cosine = dot / (norm(actual) * norm(expected));
    assert!(cosine > 0.999, "Core ML/CPU embedding cosine: {cosine}");
}

#[test]
fn an_unavailable_provider_falls_back_to_cpu() {
    let engine = OrtEngine::new(1);
    let missing = [Provider::DirectMl, Provider::CoreMl, Provider::WebGpu]
        .into_iter()
        .find(|p| !engine.available().contains(p))
        .expect("no build has every GPU provider");
    let choice = match missing {
        Provider::DirectMl => ProviderChoice::DirectMl,
        Provider::CoreMl => ProviderChoice::CoreMl,
        _ => ProviderChoice::WebGpu,
    };
    let bytes = std::fs::read(affine()).unwrap();
    let model = engine.load(ModelSource::Bytes(&bytes), choice).unwrap();
    assert_eq!(model.provider(), Provider::Cpu);
    assert!(
        model.fallback().unwrap().contains("not available"),
        "{:?}",
        model.fallback()
    );
    let out = model
        .run(vec![Tensor::f32("x", &[1, 3], vec![0.0, 0.5, -1.0])])
        .unwrap();
    assert_eq!(out[0].as_f32().unwrap(), [1.0, 2.0, -1.0]);
}

#[test]
fn every_choice_probes_to_a_working_provider() {
    let engine = OrtEngine::new(1);
    assert_eq!(engine.available().last(), Some(&Provider::Cpu));
    for choice in [
        ProviderChoice::Auto,
        ProviderChoice::Cpu,
        ProviderChoice::DirectMl,
        ProviderChoice::CoreMl,
        ProviderChoice::WebGpu,
    ] {
        // A GPU provider may work (a real or software adapter) or fall
        // back; either way the probe model gives the right answer.
        let probe = engine.probe(choice).unwrap();
        assert!(engine.available().contains(&probe.provider), "{choice:?}");
        if choice == ProviderChoice::Cpu {
            assert_eq!(probe.provider, Provider::Cpu);
        }
        // An explicitly chosen GPU provider that ends up on CPU says why.
        // (Automatic may simply prefer the CPU, as it does on Linux.)
        if probe.provider == Provider::Cpu
            && !matches!(choice, ProviderChoice::Cpu | ProviderChoice::Auto)
        {
            assert!(probe.fallback.is_some(), "{choice:?}");
        }
    }
}

#[test]
fn automatic_leaves_webgpu_to_an_explicit_choice() {
    let engine = OrtEngine::new(1);
    if cfg!(target_os = "linux") {
        assert_eq!(
            engine.probe(ProviderChoice::Auto).unwrap().provider,
            Provider::Cpu
        );
    }
}

#[test]
fn bad_models_and_inputs_are_errors() {
    let engine = OrtEngine::new(1);
    let err = engine
        .load(ModelSource::Bytes(b"not a model"), ProviderChoice::Cpu)
        .unwrap_err();
    assert!(err.0.contains("CPU"), "{err}");
    let model = engine
        .load(ModelSource::File(&affine()), ProviderChoice::Cpu)
        .unwrap();
    assert!(
        model
            .run(vec![Tensor::f32("x", &[1, 4], vec![0.0; 4])])
            .is_err()
    );
    assert!(
        model
            .run(vec![Tensor::i64("x", &[1, 3], vec![1, 2, 3])])
            .is_err()
    );
}
