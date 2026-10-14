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
