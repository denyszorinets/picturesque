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
  CPU variant) at run time instead (checked below); the integration must
  use it, and the `openmp` default feature should be off to avoid
  shipping `libgomp`.
- Not checked here: Windows and macOS builds (Metal), Intel macOS, and
  coexistence with ONNX Runtime's WebGPU provider in one process. The
  implementation task builds all three platforms in CI first.

## Dynamic backends

Built with `default-features = false` and `common`, `mtmd`,
`dynamic-backends` and `vulkan` (no OpenMP):

- The executable links only llama.cpp's own shared libraries
  (`libllama`, `libggml`, `libggml-base`, `libmtmd`, `libllama-common`,
  12 MB) and `libstdc++`; neither `libvulkan` nor `libgomp`.
- Backends are separate libraries loaded with `load_backends_from_path`:
  `libggml-vulkan.so` (44 MB) and 14 CPU variants from SSE4.2 to
  Sapphire Rapids (21 MB in all); the best one for the processor is
  chosen at run time (`libggml-cpu-zen4.so` here).
- With the Vulkan backend present: 10.0 s per critique on the 780M.
  Without it (as on a machine without a Vulkan loader): the app starts
  and runs on the CPU in 20.3 s.
- Ships as 77 MB of libraries next to the executable, like
  `libonnxruntime.so` (ADR-0015); the CPU variants can be trimmed.
- Clean build: 3 m 24 s on 16 cores (every CPU variant is compiled).

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

## Prompt evaluation (2026-10-28)

50 Wikimedia Commons photos: 25 featured pictures and 25 random photos.
Rating mean / median by class; AUC tells featured from ordinary (about
+-0.06 error with 25 photos per class). The scripts were throwaway and are
not kept.

| prompt | featured | ordinary | AUC | s/photo |
|---|---|---|---|---|
| first prompt (milder; "rate 5 for an ordinary snapshot, 7 for a good photo") | 8.48 / 9 | 7.12 / 7 | 0.894 | 11.7 |
| strict with a formal `analysis` written first | 8.07 / 8 (15+15 photos) | 6.60 / 6 | 0.789 | 14.4 |
| final: flaws first, grade anchors, no `analysis`, rating last | 7.64 / 8 | 4.24 / 4 | 0.902 | 9.5 |

Ratings of the final prompt, photos per rating:

- featured: 4: 2, 6: 3, 8: 15, 9: 5
- ordinary: 3: 11, 4: 9, 5: 1, 8: 4

Asking for a formal analysis before the rating did not make the model
stricter, and it cost time; listing the flaws first and anchoring the
scale did. The order of the JSON keys decides what the model writes
first, and the schema keeps its key order (serde_json `preserve_order`).
Four of 25 ordinary photos still got an 8, so the rating is a signal, not
a verdict; that is why the library score uses percentiles.
