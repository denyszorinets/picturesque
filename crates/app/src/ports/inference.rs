//! Running machine-learning models (ADR-0006). The ONNX Runtime adapter
//! lives in `picturesque-infra`; no runtime types cross this port.

use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Hardware a model runs on (an ONNX Runtime execution provider).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Provider {
    Cpu,
    /// GPUs on Windows (DirectX 12).
    #[serde(rename = "directml")]
    DirectMl,
    /// Apple Neural Engine and GPU.
    #[serde(rename = "coreml")]
    CoreMl,
    /// GPUs through WebGPU (Vulkan on Linux, Metal on macOS).
    #[serde(rename = "webgpu")]
    WebGpu,
}

impl Provider {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cpu => "CPU",
            Self::DirectMl => "DirectML",
            Self::CoreMl => "Core ML",
            Self::WebGpu => "WebGPU",
        }
    }
}

/// Which provider the user wants; `Auto` picks the best available one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ProviderChoice {
    #[default]
    Auto,
    Cpu,
    #[serde(rename = "directml")]
    DirectMl,
    #[serde(rename = "coreml")]
    CoreMl,
    #[serde(rename = "webgpu")]
    WebGpu,
}

impl ProviderChoice {
    /// Providers to try, in order, given what this build supports. CPU is
    /// always last, so loading falls back to it.
    pub fn candidates(self, available: &[Provider]) -> Vec<Provider> {
        let wanted: Vec<Provider> = match self {
            Self::Auto => available.to_vec(),
            Self::Cpu => Vec::new(),
            Self::DirectMl => vec![Provider::DirectMl],
            Self::CoreMl => vec![Provider::CoreMl],
            Self::WebGpu => vec![Provider::WebGpu],
        };
        let mut order: Vec<Provider> = wanted.into_iter().filter(|p| *p != Provider::Cpu).collect();
        order.push(Provider::Cpu);
        order
    }
}

/// Input or output data of a model.
#[derive(Debug, Clone, PartialEq)]
pub enum TensorData {
    F32(Vec<f32>),
    I64(Vec<i64>),
}

/// A named, shaped tensor.
#[derive(Debug, Clone, PartialEq)]
pub struct Tensor {
    pub name: String,
    pub shape: Vec<usize>,
    pub data: TensorData,
}

impl Tensor {
    pub fn f32(name: &str, shape: &[usize], data: Vec<f32>) -> Self {
        Self {
            name: name.to_owned(),
            shape: shape.to_vec(),
            data: TensorData::F32(data),
        }
    }

    pub fn i64(name: &str, shape: &[usize], data: Vec<i64>) -> Self {
        Self {
            name: name.to_owned(),
            shape: shape.to_vec(),
            data: TensorData::I64(data),
        }
    }

    /// The data as `f32`, if it is.
    pub fn as_f32(&self) -> Option<&[f32]> {
        match &self.data {
            TensorData::F32(v) => Some(v),
            TensorData::I64(_) => None,
        }
    }
}

/// Why a model could not be loaded or run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferenceError(pub String);

impl fmt::Display for InferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for InferenceError {}

/// Where a model comes from.
#[derive(Debug, Clone, Copy)]
pub enum ModelSource<'a> {
    File(&'a Path),
    Bytes(&'a [u8]),
}

/// Which provider a model got, from `InferenceEngine::probe`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probe {
    pub provider: Provider,
    pub fallback: Option<String>,
}

/// A model ready to run.
pub trait Model: Send + Sync + fmt::Debug {
    /// The provider the model actually runs on.
    fn provider(&self) -> Provider;
    /// Why a preferred provider was not used (`None` when the first choice
    /// worked).
    fn fallback(&self) -> Option<&str>;
    /// Runs the model; returns its outputs in declaration order.
    fn run(&self, inputs: Vec<Tensor>) -> Result<Vec<Tensor>, InferenceError>;
}

pub trait InferenceEngine: Send + Sync + fmt::Debug {
    /// Providers this build supports, preferred first; always ends with CPU.
    fn available(&self) -> Vec<Provider>;
    /// Loads an ONNX model, trying `choice`'s candidates in order and
    /// falling back to CPU when a provider fails to initialize.
    fn load(
        &self,
        model: ModelSource<'_>,
        choice: ProviderChoice,
    ) -> Result<Box<dyn Model>, InferenceError>;
    /// Loads and runs a tiny built-in model with `choice`, to find out
    /// which provider really works on this machine.
    fn probe(&self, choice: ProviderChoice) -> Result<Probe, InferenceError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_end_with_cpu_and_follow_the_choice() {
        let all = [Provider::WebGpu, Provider::Cpu];
        assert_eq!(
            ProviderChoice::Auto.candidates(&all),
            [Provider::WebGpu, Provider::Cpu]
        );
        assert_eq!(ProviderChoice::Cpu.candidates(&all), [Provider::Cpu]);
        // A provider this build lacks is still tried, then CPU.
        assert_eq!(
            ProviderChoice::DirectMl.candidates(&all),
            [Provider::DirectMl, Provider::Cpu]
        );
        assert_eq!(ProviderChoice::Auto.candidates(&[]), [Provider::Cpu]);
    }

    #[test]
    fn names_match_the_settings_format() {
        for (choice, name) in [
            (ProviderChoice::Auto, "\"auto\""),
            (ProviderChoice::DirectMl, "\"directml\""),
            (ProviderChoice::CoreMl, "\"coreml\""),
            (ProviderChoice::WebGpu, "\"webgpu\""),
        ] {
            assert_eq!(serde_json::to_string(&choice).unwrap(), name);
        }
        assert_eq!(
            serde_json::to_string(&Provider::DirectMl).unwrap(),
            "\"directml\""
        );
    }
}
