//! Model inference with ONNX Runtime through `ort` (ADR-0006).
//!
//! The prebuilt runtime is linked statically; which execution providers it
//! has depends on the target (see `Cargo.toml`). Sessions are created per
//! model with the user's provider choice and fall back to CPU.
//!
//! GPU stacks (Dawn and Vulkan drivers) are not reliably thread-safe while
//! devices are created: creating two sessions at once crashed or hung in
//! tests. Sessions are therefore created one at a time, and models on GPU
//! providers run one at a time (the GPU is shared anyway).

use std::sync::Mutex;

use ort::ep::{self, ExecutionProviderDispatch};
use ort::session::Session;
use ort::session::builder::SessionBuilder;
use ort::value::{DynValue, Tensor as OrtTensor, TensorElementType, ValueType};
use picturesque_app::ports::inference::{
    InferenceEngine, InferenceError, Model, ModelSource, Probe, Provider, ProviderChoice, Tensor,
    TensorData,
};

/// `y = 2x + 1` over a `[n, 3]` float tensor
/// (`tests/fixtures/onnx/make_affine.py`).
const PROBE_MODEL: &[u8] = include_bytes!("probe.onnx");

/// GPU providers compiled into this build, preferred first.
const GPU_PROVIDERS: &[Provider] = if cfg!(windows) {
    &[Provider::DirectMl]
} else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
    &[Provider::CoreMl, Provider::WebGpu]
} else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
    &[Provider::WebGpu]
} else {
    &[]
};

/// GPU providers "Automatic" tries, preferred first. WebGPU is left out: it
/// works on real GPUs (about 1.4× the CPU's speed on a Radeon 780M, #42),
/// but without one it silently runs on a software Vulkan driver, more than
/// ten times slower than the CPU, so it is used only when chosen.
const AUTO_PROVIDERS: &[Provider] = if cfg!(windows) {
    &[Provider::DirectMl]
} else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
    &[Provider::CoreMl]
} else {
    &[]
};

/// Held while creating any session.
static CREATING: Mutex<()> = Mutex::new(());
/// Held while creating, running or dropping a session on a GPU provider.
/// ONNX Runtime's WebGPU provider shares one device between sessions and
/// fails ("CommandEncoder is already finished") when one session uploads
/// or frees its weights while another runs (#42).
static GPU: Mutex<()> = Mutex::new(());

fn gpu_lock(provider: Provider) -> Option<std::sync::MutexGuard<'static, ()>> {
    (provider != Provider::Cpu).then(|| GPU.lock().unwrap_or_else(|e| e.into_inner()))
}

/// ONNX Runtime with the providers of this build.
#[derive(Debug, Clone)]
pub struct OrtEngine {
    /// Threads per session for CPU work.
    threads: usize,
}

impl OrtEngine {
    /// `threads`: how many cores one model may use on CPU.
    pub fn new(threads: usize) -> Self {
        Self {
            threads: threads.max(1),
        }
    }

    /// Half the cores, like background analysis, so the UI stays responsive.
    pub fn for_background() -> Self {
        Self::new(std::thread::available_parallelism().map_or(1, |n| n.get() / 2))
    }

    fn session(&self, model: ModelSource<'_>, provider: Provider) -> Result<Session, String> {
        let _creating = CREATING.lock().unwrap_or_else(|e| e.into_inner());
        let _gpu = gpu_lock(provider);
        let mut builder = Session::builder()
            .and_then(|b| b.with_intra_threads(self.threads).map_err(Into::into))
            .map_err(|e| e.to_string())?;
        if let Some(dispatch) = dispatch(provider) {
            builder = builder
                .with_execution_providers([dispatch.error_on_failure()])
                .map_err(|e| e.to_string())?;
        }
        commit(&mut builder, model)
    }
}

fn commit(builder: &mut SessionBuilder, model: ModelSource<'_>) -> Result<Session, String> {
    match model {
        ModelSource::File(path) => builder.commit_from_file(path),
        ModelSource::Bytes(bytes) => builder.commit_from_memory(bytes),
    }
    .map_err(|e| e.to_string())
}

/// The execution provider for `provider`; `None` for plain CPU and for
/// providers this build lacks (callers check `GPU_PROVIDERS` first).
fn dispatch(provider: Provider) -> Option<ExecutionProviderDispatch> {
    match provider {
        Provider::Cpu => None,
        #[cfg(windows)]
        Provider::DirectMl => Some(ep::DirectML::default().build()),
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        Provider::CoreMl => Some(ep::CoreML::default().build()),
        #[cfg(any(
            all(target_os = "linux", target_arch = "x86_64"),
            all(target_os = "macos", target_arch = "aarch64")
        ))]
        Provider::WebGpu => Some(ep::WebGPU::default().build()),
        #[allow(unreachable_patterns)]
        _ => None,
    }
}

impl InferenceEngine for OrtEngine {
    fn available(&self) -> Vec<Provider> {
        let mut providers = GPU_PROVIDERS.to_vec();
        providers.push(Provider::Cpu);
        providers
    }

    fn load(
        &self,
        model: ModelSource<'_>,
        choice: ProviderChoice,
    ) -> Result<Box<dyn Model>, InferenceError> {
        let mut failures = Vec::new();
        for provider in choice.candidates(AUTO_PROVIDERS) {
            if provider != Provider::Cpu && !GPU_PROVIDERS.contains(&provider) {
                failures.push(format!(
                    "{} is not available in this build",
                    provider.label()
                ));
                continue;
            }
            match self.session(model, provider) {
                Ok(session) => {
                    return Ok(Box::new(OrtModel {
                        session: Mutex::new(Some(session)),
                        provider,
                        fallback: (!failures.is_empty()).then(|| failures.join("; ")),
                    }));
                }
                Err(e) => failures.push(format!("{}: {e}", provider.label())),
            }
        }
        Err(InferenceError(failures.join("; ")))
    }

    fn probe(&self, choice: ProviderChoice) -> Result<Probe, InferenceError> {
        let model = self.load(ModelSource::Bytes(PROBE_MODEL), choice)?;
        let outputs = model.run(vec![Tensor::f32("x", &[1, 3], vec![0.0, 1.0, 2.0])])?;
        let y = outputs.first().and_then(Tensor::as_f32);
        if y != Some(&[1.0, 3.0, 5.0][..]) {
            return Err(InferenceError(format!(
                "{} returned wrong results: {y:?}",
                model.provider().label()
            )));
        }
        Ok(Probe {
            provider: model.provider(),
            fallback: model.fallback().map(str::to_owned),
        })
    }
}

/// A loaded ONNX model. ONNX Runtime sessions run one call at a time here.
#[derive(Debug)]
struct OrtModel {
    /// `None` only while dropping.
    session: Mutex<Option<Session>>,
    provider: Provider,
    fallback: Option<String>,
}

impl Drop for OrtModel {
    /// Frees the session's GPU memory while no other session uses the device.
    fn drop(&mut self) {
        let _gpu = gpu_lock(self.provider);
        let session = self.session.get_mut().unwrap_or_else(|e| e.into_inner());
        drop(session.take());
    }
}

fn error(e: impl std::fmt::Display) -> InferenceError {
    InferenceError(e.to_string())
}

fn to_value(tensor: Tensor) -> Result<(String, DynValue), InferenceError> {
    let shape = tensor.shape.clone();
    let value = match tensor.data {
        TensorData::F32(data) => OrtTensor::from_array((shape, data))
            .map_err(error)?
            .into_dyn(),
        TensorData::I32(data) => OrtTensor::from_array((shape, data))
            .map_err(error)?
            .into_dyn(),
        TensorData::I64(data) => OrtTensor::from_array((shape, data))
            .map_err(error)?
            .into_dyn(),
    };
    Ok((tensor.name, value))
}

impl Model for OrtModel {
    fn provider(&self) -> Provider {
        self.provider
    }

    fn fallback(&self) -> Option<&str> {
        self.fallback.as_deref()
    }

    fn run(&self, inputs: Vec<Tensor>) -> Result<Vec<Tensor>, InferenceError> {
        let inputs = inputs
            .into_iter()
            .map(to_value)
            .collect::<Result<Vec<_>, _>>()?;
        let _gpu = gpu_lock(self.provider);
        let mut session = self.session.lock().expect("session lock");
        let session = session.as_mut().expect("a session until dropped");
        let outputs = session.run(inputs).map_err(error)?;
        outputs
            .iter()
            .map(|(name, value)| {
                let element = match value.dtype() {
                    ValueType::Tensor { ty, .. } => *ty,
                    other => return Err(InferenceError(format!("output `{name}` is a {other}"))),
                };
                let (shape, data) = match element {
                    TensorElementType::Float32 => {
                        let (shape, data) = value.try_extract_tensor::<f32>().map_err(error)?;
                        (shape.to_vec(), TensorData::F32(data.to_vec()))
                    }
                    TensorElementType::Int32 => {
                        let (shape, data) = value.try_extract_tensor::<i32>().map_err(error)?;
                        (shape.to_vec(), TensorData::I32(data.to_vec()))
                    }
                    TensorElementType::Int64 => {
                        let (shape, data) = value.try_extract_tensor::<i64>().map_err(error)?;
                        (shape.to_vec(), TensorData::I64(data.to_vec()))
                    }
                    other => {
                        return Err(InferenceError(format!(
                            "output `{name}` has unsupported type {other}"
                        )));
                    }
                };
                Ok(Tensor {
                    name: name.to_owned(),
                    shape: shape.iter().map(|d| (*d).max(0) as usize).collect(),
                    data,
                })
            })
            .collect()
    }
}
