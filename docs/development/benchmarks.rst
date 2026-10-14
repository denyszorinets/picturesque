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

   cargo build --release -p picturesque-infra --example bench_inference
   LD_LIBRARY_PATH=target/release target/release/examples/bench_inference \
       ~/.local/share/io.github.denyszorinets.picturesque/models/clip-image/v1/visual/model.onnx 50

Arguments: the model, the number of runs (default 50), the input name
(default ``image``) and its shape (default ``1x3x224x224``, the CLIP image
encoder). ``LD_LIBRARY_PATH`` lets the example find ``libwebgpu_dawn.so``
on Linux.

Using the GPU in the devcontainer
---------------------------------

The devcontainer does not see the GPU unless the host passes it in. On a
Linux host, add to :repo:`.devcontainer/devcontainer.json` and rebuild the
container:

.. code-block:: json

   "runArgs": ["--device=/dev/dri", "--group-add=keep-groups"]

``--group-add=keep-groups`` is for rootless Podman; with Docker use
``"--group-add=video", "--group-add=render"`` instead. Check with
``ls /dev/dri`` (``renderD128`` should appear) and ``vulkaninfo --summary``.
Without a GPU, Mesa's software Vulkan driver runs WebGPU on the CPU.

Results
-------

ONNX Runtime 1.28, Ryzen 7 PRO 7840HS (16 threads) in the devcontainer.
Without the GPU passed in, WebGPU runs on Mesa's software Vulkan driver.

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
   * - Either, Radeon 780M passed in
     - WebGPU
     - —
     - —
     - —
     - —
     - pending (:issue:`42`)

With ViT-L/14 the CPU analyzes a 10,000-photo library in about an hour in
the background, so a working GPU provider is worth having.
