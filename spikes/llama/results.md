# Built-in critic spike: results

2026-10-27. Qwen3-VL-4B Instruct, Q4_K_M (2.50 GB) with the Q8_0 vision
projector (0.45 GB), from Qwen's official GGUF repository (Apache-2.0),
run by `llama-cpp-2` 0.1.160 (MIT or Apache-2.0; llama.cpp is MIT) with
the app's critique prompt and rubric. Five 768 px photos (Commons sample
and a face fixture). Ryzen 7 PRO 7840HS (16 threads), Radeon 780M through
Mesa RADV, in the devcontainer.

## Speed and memory

| | CPU | Vulkan, Radeon 780M |
|---|---|---|
| Load model and projector | 1.5 s | 1.0 s |
| Image + prompt (588–636 tokens) | 8.8–10.5 s | 2.5 s |
| Generation (105–129 tokens) | 8.4–10.2 s, 12.5 tok/s | 7.2 s, 15.1 tok/s |
| **Total per critique** | **19.6–21.8 s** | **10.7 s** |
| Peak process memory (2048-token context) | 5.4 GB (5.7 GB with 4096) | 0.3 GB (weights in GPU memory) |

The image itself becomes 432 tokens (18 × 24 patches) at 768 px.
Generation is memory-bandwidth bound, so the integrated GPU helps the
image and prompt (4×) much more than the writing.

## Quality

Constrained by a grammar generated from the critique's JSON Schema
(`json_schema_to_grammar`), every reply was valid JSON with the expected
fields. Critiques are specific (it noticed a traffic sign spoiling an
archway, blown ceiling lights in an industrial interior). Ratings are not
calibrated: an ordinary Commons photo got 8 and a featured one 7. This
confirms ADR-0016's choice to show the rating, not rank by it.

## Build and packaging

| | CPU | + Vulkan |
|---|---|---|
| Clean build (16 cores) | 1 m 16 s | 1 m 29 s |
| Binary size added | 8.6 MB | 44 MB (compiled shaders) |
| Linked at load time | `libgomp`, `libstdc++` | + `libvulkan.so.1` |

- Build machines need CMake; Vulkan builds also need `glslc`, the Vulkan
  headers/loader and **SPIRV-Headers** (the configure step fails without
  them).
- Linking `libvulkan.so.1` directly would stop the app from starting
  without a Vulkan loader, the problem ADR-0015 solved for ONNX Runtime.
  The crate's `dynamic-backends` feature loads GPU backends (and the best
  CPU variant) at run time instead; the integration must use it, and the
  `openmp` default feature should be off to avoid shipping `libgomp`.
- Not checked here: Windows and macOS builds (Metal), Intel macOS, and
  coexistence with ONNX Runtime's WebGPU provider in one process. The
  implementation task builds all three platforms in CI first.

## Pitfalls found

- `LlamaSampler::sample` already accepts the token. Accepting it again
  corrupts the grammar state, and llama.cpp **aborts the whole process**
  (`GGML_ASSERT(!stacks.empty())`). Native asserts cannot be caught; the
  built-in critic must keep its use of the API minimal and well tested,
  or run in a child process.
- `void_logs` does not silence the vision encoder; `send_logs_to_tracing`
  with logs disabled does.

## Decision

Go: in-process llama.cpp is fast enough on CPU (about 20 s) and twice as
fast on the Radeon 780M, needs no server, and the files and licenses fit
the model manager (3 GB, Apache-2.0). Build with `mtmd`,
`dynamic-backends`, Vulkan (Linux, Windows) and Metal (macOS), without
OpenMP, and keep the local-server critic as an alternative.
