//! Running the model with llama.cpp (`spikes/llama/results.md`).

use std::num::NonZeroU32;
use std::path::PathBuf;
use std::time::Instant;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{LlamaChatMessage, LlamaModel};
use llama_cpp_2::mtmd::{
    MtmdBitmap, MtmdContext, MtmdContextParams, MtmdInputText, mtmd_default_marker,
};
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::{LlamaBackendDeviceType, LogOptions, json_schema_to_grammar};
use picturesque_core::base64;
use picturesque_core::critic_protocol::{HelperReply, HelperRequest};

/// The prompt (about 200 tokens), the image (about 430 at 768 px) and the
/// reply fit with room to spare.
const CONTEXT: u32 = 2048;
const BATCH: u32 = 1024;
/// Low: critiques should be steady, not creative.
const TEMPERATURE: f32 = 0.2;

/// A loaded model and its vision projector.
struct Loaded {
    model: PathBuf,
    mmproj: PathBuf,
    gpu: bool,
    llama: LlamaModel,
    mtmd: MtmdContext,
    device: String,
}

pub struct Engine {
    backend: Option<LlamaBackend>,
    loaded: Option<Loaded>,
    threads: i32,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            backend: None,
            loaded: None,
            threads: std::thread::available_parallelism()
                .map_or(4, |n| n.get())
                .try_into()
                .unwrap_or(4),
        }
    }

    pub fn critique(&mut self, request: &HelperRequest) -> HelperReply {
        let start = Instant::now();
        match self.run(request) {
            Ok((text, device)) => HelperReply::Done {
                text,
                device,
                seconds: start.elapsed().as_secs_f32(),
            },
            Err(message) => HelperReply::Failed { message },
        }
    }

    fn backend(&mut self) -> Result<&LlamaBackend, String> {
        if self.backend.is_none() {
            // GPU backends and the best CPU variant are libraries beside
            // the helper.
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            if let Some(dir) = exe.parent() {
                llama_cpp_2::llama_backend::load_backends_from_path(dir);
            }
            let mut backend = LlamaBackend::init().map_err(|e| e.to_string())?;
            backend.void_logs();
            // The vision projector logs on its own.
            llama_cpp_2::send_logs_to_tracing(LogOptions::default().with_logs_enabled(false));
            self.backend = Some(backend);
        }
        Ok(self.backend.as_ref().expect("just set"))
    }

    /// Loads the request's model unless it is the loaded one.
    fn load(&mut self, request: &HelperRequest) -> Result<(), String> {
        if let Some(l) = &self.loaded
            && l.model == request.model
            && l.mmproj == request.mmproj
            && l.gpu == request.gpu
        {
            return Ok(());
        }
        self.loaded = None;
        let threads = self.threads;
        let backend = self.backend()?;
        let gpu = request.gpu.then(gpu_device).flatten();
        let params =
            LlamaModelParams::default().with_n_gpu_layers(if gpu.is_some() { 999 } else { 0 });
        let llama = LlamaModel::load_from_file(backend, &request.model, &params)
            .map_err(|e| format!("cannot load the model: {e}"))?;
        let mtmd = MtmdContext::init_from_file(
            request
                .mmproj
                .to_str()
                .ok_or("the projector's path is not UTF-8")?,
            &llama,
            &MtmdContextParams {
                use_gpu: gpu.is_some(),
                print_timings: false,
                n_threads: threads,
                media_marker: std::ffi::CString::new(mtmd_default_marker())
                    .expect("the marker has no NUL"),
                image_min_tokens: -1,
                image_max_tokens: -1,
            },
        )
        .map_err(|e| format!("cannot load the vision projector: {e}"))?;
        if !mtmd.support_vision() {
            return Err("the projector does not handle images".into());
        }
        self.loaded = Some(Loaded {
            model: request.model.clone(),
            mmproj: request.mmproj.clone(),
            gpu: request.gpu,
            llama,
            mtmd,
            device: gpu.unwrap_or_else(|| "CPU".into()),
        });
        Ok(())
    }

    fn run(&mut self, request: &HelperRequest) -> Result<(String, String), String> {
        self.load(request)?;
        let threads = match request.threads {
            0 => self.threads,
            n => i32::try_from(n).unwrap_or(self.threads).min(self.threads),
        };
        let backend = self.backend.as_ref().expect("loaded with the model");
        let loaded = self.loaded.as_mut().expect("just loaded");
        let llama = &loaded.llama;

        let template = llama
            .chat_template(None)
            .map_err(|e| format!("the model has no chat template: {e}"))?;
        let message = |role: &str, content: String| {
            LlamaChatMessage::new(role.into(), content).map_err(|e| e.to_string())
        };
        let prompt = llama
            .apply_chat_template(
                &template,
                &[
                    message("system", request.system.clone())?,
                    message(
                        "user",
                        format!("{}{}", mtmd_default_marker(), request.prompt),
                    )?,
                ],
                true,
            )
            .map_err(|e| e.to_string())?;

        let jpeg = base64::decode(&request.image).ok_or("the image is not base64")?;
        let bitmap = MtmdBitmap::from_buffer(&loaded.mtmd, &jpeg, false)
            .map_err(|e| format!("cannot read the image: {e}"))?;
        let chunks = loaded
            .mtmd
            .tokenize(
                MtmdInputText {
                    text: prompt,
                    add_special: true,
                    parse_special: true,
                },
                &[&bitmap],
            )
            .map_err(|e| format!("cannot prepare the prompt: {e}"))?;
        if chunks.total_tokens() + request.max_tokens as usize > CONTEXT as usize {
            return Err(format!(
                "the prompt is too long ({} tokens)",
                chunks.total_tokens()
            ));
        }

        let mut ctx = llama
            .new_context(
                backend,
                LlamaContextParams::default()
                    .with_n_ctx(NonZeroU32::new(CONTEXT))
                    .with_n_batch(BATCH)
                    .with_n_threads(threads)
                    .with_n_threads_batch(threads),
            )
            .map_err(|e| format!("cannot start the model: {e}"))?;
        let n_past = chunks
            .eval_chunks(&mut loaded.mtmd, &mut ctx, 0, 0, BATCH as i32, true)
            .map_err(|e| format!("cannot read the photo: {e}"))?;

        // A grammar from the schema makes the reply valid JSON.
        let grammar = json_schema_to_grammar(&request.schema)
            .map_err(|e| format!("invalid reply schema: {e}"))?;
        let mut sampler = LlamaSampler::chain_simple([
            LlamaSampler::grammar(llama, &grammar, "root").map_err(|e| e.to_string())?,
            LlamaSampler::temp(TEMPERATURE),
            LlamaSampler::dist(0x5EED),
        ]);
        let vocab = llama.vocab();
        let mut text = Vec::new();
        let mut batch = LlamaBatch::new(1, 1);
        let end = n_past + i32::try_from(request.max_tokens).unwrap_or(i32::MAX);
        for position in n_past..end {
            // `sample` also accepts the token: accepting it again breaks
            // the grammar, and llama.cpp aborts.
            let token = sampler.sample(&ctx, -1);
            if vocab.is_eog(token) {
                break;
            }
            text.extend(vocab.token_to_piece(token, false, None));
            batch.clear();
            batch
                .add(token, position, &[0], true)
                .map_err(|e| e.to_string())?;
            ctx.decode(&mut batch)
                .map_err(|e| format!("the model failed: {e}"))?;
        }
        Ok((
            String::from_utf8_lossy(&text).into_owned(),
            loaded.device.clone(),
        ))
    }
}

/// The GPU to use: a discrete or integrated GPU llama.cpp found (Vulkan or
/// Metal), by its description; `None` to use the CPU.
fn gpu_device() -> Option<String> {
    llama_cpp_2::list_llama_ggml_backend_devices()
        .into_iter()
        .find(|d| {
            matches!(
                d.device_type,
                LlamaBackendDeviceType::Gpu | LlamaBackendDeviceType::IntegratedGpu
            ) && !is_software(&d.description)
        })
        .map(|d| d.description)
}

/// Software Vulkan drivers are much slower than the CPU path (ADR-0006).
fn is_software(description: &str) -> bool {
    let d = description.to_lowercase();
    ["llvmpipe", "lavapipe", "swiftshader"]
        .iter()
        .any(|s| d.contains(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn software_renderers_are_not_gpus() {
        assert!(is_software("llvmpipe (LLVM 19.1.7, 256 bits)"));
        assert!(is_software("SwiftShader Device (Subzero)"));
        assert!(!is_software("AMD Radeon 780M Graphics (RADV PHOENIX)"));
    }
}
