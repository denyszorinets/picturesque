ADR-0006: Local AI with ONNX Runtime
====================================

:Status: Accepted
:Date: 2026-10-12

Context
-------

Picturesque uses machine-learning models to judge photos: an aesthetic
score, face detection with closed-eye classification, and CLIP embeddings
for semantic search and visual similarity. Requirements:

- Run entirely on the user's machine: photos never leave it.
- Free to use, with no API keys.
- Work on CPU everywhere; use the GPU when one is available, including
  integrated AMD GPUs.
- Model licenses compatible with distributing a GPL-3.0 application.
- Libraries of fewer than 10,000 photos, analyzed in the background.

Decision
--------

Run ONNX models with **ONNX Runtime** through the **ort** crate.

- Inference sits behind an inference port in ``picturesque-app``; ``ort``
  types stay in ``picturesque-infra``.
- Execution providers: CPU everywhere (baseline); DirectML on Windows;
  CoreML on macOS; on Linux, the WebGPU provider (Vulkan through Dawn) if a
  benchmark shows it is stable and faster than CPU. The app falls back to
  CPU when a provider fails to initialize.
- Models are downloaded once by a model manager with checksums and
  versions, not bundled in installers.
- Scores record the model version, so they can be recomputed when a model
  changes.

Alternatives Considered
-----------------------

Burn with the wgpu backend
~~~~~~~~~~~~~~~~~~~~~~~~~~

Pure-Rust ML framework running on Vulkan, Metal and DirectX 12.

Advantages:

- Native Vulkan support across GPU vendors, AMD integrated GPUs included.
- No C++ runtime to bundle.

Disadvantages:

- ONNX import does not support every operator; some models fail to load.

candle
~~~~~~

Advantages:

- Pure Rust, active development.

Disadvantages:

- GPU support is CUDA and Metal; no Vulkan or DirectML.

tract
~~~~~

Advantages:

- Pure Rust ONNX inference, small.

Disadvantages:

- CPU only.

Cloud vision APIs
~~~~~~~~~~~~~~~~~

Advantages:

- No local compute.

Disadvantages:

- Sends photos off the machine, costs money, needs keys and network.

Decision Criteria
-----------------

Model coverage, GPU support on the target hardware, privacy, maturity,
bundling effort.

Rationale
---------

ONNX Runtime runs virtually any exported model and is the most mature
option. Its execution providers cover GPUs on Windows and macOS well. Linux
integrated-GPU support is the weak spot: ROCm largely targets discrete
cards, and there is no Vulkan provider; WebGPU is the candidate. Keeping
inference behind a port means a Burn/wgpu backend can replace ``ort`` later
if Vulkan support matters more than model coverage.

Consequences
------------

Positive
~~~~~~~~

- Any ONNX model can be used.
- Good GPU acceleration on Windows and macOS.

Negative
~~~~~~~~

- The ONNX Runtime shared library must be bundled per platform.
- Linux users may get CPU-only inference.

Risks
~~~~~

- The WebGPU provider may be unavailable in ``ort`` builds or unstable on
  integrated AMD GPUs.
- Some candidate models have non-commercial licenses; each model's license
  must be checked before it is offered.

Implementation
~~~~~~~~~~~~~~

:issue:`40` integrated ``ort`` 2.0.0-rc.13 (ONNX Runtime 1.28):

- The port is ``InferenceEngine``/``Model`` in ``picturesque-app``, with plain
  ``f32``/``i64`` tensors; ``OrtEngine`` in ``picturesque-infra`` implements
  it.
- ONNX Runtime is linked statically from pyke's prebuilt binaries, chosen
  per target: WebGPU on Linux x86-64, DirectML on Windows, Core ML and
  WebGPU on Apple Silicon. Intel macOS has no prebuilt binaries and is not
  supported for now.
- Each provider is registered with ``error_on_failure``, so a provider that
  cannot start is detected and the session is rebuilt on CPU, recording why.
  ``ai_status`` probes the chosen provider with a tiny built-in model.
- A first check in the devcontainer (no GPU passed through) found that
  WebGPU initializes on a software Vulkan adapter and gives correct results;
  :issue:`42` measures real GPU performance.

:issue:`41` added the model manager: a manifest lists each model's files
with URL, size and SHA-256, and its license. Files are downloaded on
request to ``<data>/models/<id>/v<version>/``, verified while streaming,
and renamed into place only when they match; nothing else touches the
network.

:issue:`43` added CLIP image embeddings as a background analyzer that
becomes active once the model is downloaded, starting with OpenCLIP
ViT-B/32 (LAION-2B, MIT). :issue:`44` measured aesthetic predictors on 150
Wikimedia Commons *Featured pictures* against 150 random Commons photos
(:repo:`spikes/aesthetic/results.md`): LAION's V1 head on ViT-B/32 reached
an AUC of only 0.62 (below the technical score's 0.66), the improved LAION
predictor (Apache-2.0) on **OpenAI CLIP ViT-L/14** (MIT) 0.80. The app
therefore uses ViT-L/14, exported to ONNX by the Immich project and pinned
to a commit: a 1.2 GB download and about 0.3 s per photo on a laptop CPU.
Its 768-dimensional embeddings still fit in memory for the whole library.
:issue:`45` added the matching text encoder (496 MB) for text search, with
CLIP's byte-pair tokenizer reimplemented in ``picturesque-core`` (its
vocabulary is rebuilt from the merge list, as in OpenAI's reference code).
:issue:`46` added faces: the **YuNet** detector from OpenCV Zoo (MIT,
233 KB) and the **MediaPipe Face Landmarker** mesh (Apache-2.0, converted to
ONNX, 5 MB), from whose eyelid landmarks the eye aspect ratio tells open
from closed eyes. Eye-state classifiers found on Hugging Face were licensed
non-commercially (CC BY-NC-ND) and could not be used.

Follow-up
~~~~~~~~~

- Benchmark WebGPU against CPU on an integrated AMD GPU (:issue:`42`).

Validation
----------

:issue:`42` measures throughput and stability of WebGPU versus CPU on the
maintainer's integrated AMD GPU (Radeon 780M) under Linux
(:doc:`/development/benchmarks`). First findings, without the GPU passed
into the devcontainer:

- The CPU runs the CLIP ViT-B/32 image encoder in 18 ms per photo and
  ViT-L/14, which the app uses, in 283 ms (Ryzen 7 PRO 7840HS, 16
  threads): a 10,000-photo library in about an hour.
- WebGPU initializes on Mesa's software Vulkan driver and matches CPU
  results (largest difference 1.3 × 10⁻⁶) over repeated runs, though far
  slower. The measurement on the real iGPU is still to be done.

References
----------

- `ONNX Runtime execution providers <https://onnxruntime.ai/docs/execution-providers/>`_
- `ort <https://ort.pyke.io/>`_
