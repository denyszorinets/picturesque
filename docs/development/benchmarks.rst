Inference Benchmarks
====================

:repo:`crates/infra/examples/bench_inference.rs` measures how fast and how
stable an ONNX model runs on each execution provider of the build
(:doc:`/features/ai`). It loads the model once per provider, warms up, then
runs it repeatedly on fixed pseudo-random inputs and reports:

- which provider really ran (a provider that fails falls back to CPU, with
  the reason);
- load time, and mean, median and 95th-percentile latency;
- the largest difference from the CPU output (correctness);
- whether repeated runs give the same output (stability).

.. code-block:: bash

   make ort-dylib ORT_PROFILE=release
   cargo build --release -p picturesque-infra --example bench_inference
   target/release/examples/bench_inference \
       ~/.local/share/io.github.denyszorinets.picturesque/models/clip-image/v1/visual/model.onnx 50

Arguments: the model, the number of runs (default 50), the input name
(default ``image``) and its shape (default ``1x3x224x224``, the CLIP image
encoder). On Linux the example loads ``target/release/libonnxruntime.so``
(:doc:`/architecture/decisions/0015-onnx-runtime-loaded-at-first-use`).

Using the GPU in the devcontainer
---------------------------------

The devcontainer passes the host's GPU in
(:repo:`.devcontainer/devcontainer.json`):

.. code-block:: json

   "runArgs": ["--device=/dev/dri", "--group-add=keep-groups"]

``--group-add=keep-groups`` is for rootless Podman; with Docker use
``"--group-add=video", "--group-add=render"`` instead. Check with
``ls /dev/dri`` (``renderD128`` should appear); on AMD,
``/sys/class/drm/card1/device/gpu_busy_percent`` shows the GPU's load
while a benchmark runs.

Without a GPU, Mesa's software Vulkan driver runs WebGPU on the CPU.

Results
-------

ONNX Runtime 1.28, Ryzen 7 PRO 7840HS (16 threads) with Radeon 780M in
the devcontainer. "Radeon 780M" rows ran with the GPU passed in and only
Mesa's RADV driver enabled
(``VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/radeon_icd.x86_64.json``), so
WebGPU cannot fall back to the software driver; the GPU's load
(``/sys/class/drm/card1/device/gpu_busy_percent``) confirmed it was
used. "Software Vulkan" rows ran without the GPU passed in.

.. list-table::
   :header-rows: 1

   * - Model
     - Provider
     - Load
     - Mean
     - p95
     - Max Δ vs CPU
     - Stable
   * - CLIP ViT-L/14 image encoder (the app's, :issue:`44`)
     - CPU
     - 2.0 s
     - 283 ms
     - 290 ms
     - —
     - yes
   * -
     - WebGPU, software Vulkan
     - 1.7 s
     - 3.6 s
     - 3.7 s
     - 1.3 × 10⁻⁶
     - yes
   * - CLIP ViT-B/32 image encoder (:issue:`43`)
     - CPU
     - 0.6 s
     - 18 ms
     - 19 ms
     - —
     - yes
   * -
     - WebGPU, software Vulkan
     - 0.3 s
     - 258 ms
     - 269 ms
     - 7 × 10⁻⁷
     - yes
   * - CLIP ViT-L/14 image encoder, again
     - CPU
     - 2.2 s
     - 320 ms
     - 351 ms
     - —
     - yes
   * -
     - WebGPU, Radeon 780M
     - 1.1 s
     - 232 ms
     - 236 ms
     - 1.3 × 10⁻⁶
     - yes
   * - YuNet face detector (640 × 640)
     - CPU
     - 17 ms
     - 3.6 ms
     - 4.1 ms
     - —
     - yes
   * -
     - WebGPU, Radeon 780M
     - 43 ms
     - 9.5 ms
     - 12.4 ms
     - 2 × 10⁻⁷
     - yes
   * - Face mesh (256 × 256)
     - CPU
     - 44 ms
     - 9.3 ms
     - 11.0 ms
     - —
     - yes
   * -
     - WebGPU, Radeon 780M
     - 60 ms
     - 6.7 ms
     - 10.2 ms
     - 3.2 × 10⁻⁴
     - yes

With ViT-L/14 the CPU analyzes a 10,000-photo library in about an hour in
the background, so a working GPU provider is worth having. On the Radeon
780M, WebGPU is about 1.4 times as fast for CLIP; the small face detector
is faster on the CPU.

In the app
~~~~~~~~~~

Importing and fully analyzing 600 photos (technical scores, CLIP, faces
and closed eyes) through the dev server, with **Settings → AI
acceleration** set to each provider:

.. list-table::
   :header-rows: 1

   * - Provider
     - Time
     - GPU load
     - WebGPU device errors
   * - CPU
     - 222 s
     - about 2 %
     - —
   * - WebGPU, Radeon 780M
     - 157 s and 163 s (two runs)
     - about 92 %
     - none

Before :issue:`42`, the WebGPU runs failed within a minute with
*CommandEncoder is already finished* and took the server down: ONNX
Runtime's WebGPU provider shares one device between sessions, and a
session was created or freed while another ran. GPU sessions are now
created, run and dropped under one lock.
