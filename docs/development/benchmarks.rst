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
       ~/.local/share/io.github.denyszorinets.picturesque/models/clip-vit-b32-image/v1/visual/model.onnx 50

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

CLIP ViT-B/32 image encoder (:issue:`43`), 50 runs, ONNX Runtime 1.28.

.. list-table::
   :header-rows: 1

   * - Machine
     - Provider
     - Load
     - Mean
     - p95
     - Max Δ vs CPU
     - Stable
   * - Ryzen 7 PRO 7840HS, 16 threads, devcontainer
     - CPU
     - 0.6 s
     - 18 ms
     - 19 ms
     - —
     - yes
   * - same, no GPU passed in (software Vulkan)
     - WebGPU
     - 0.3 s
     - 258 ms
     - 269 ms
     - 7 × 10⁻⁷
     - yes
   * - same, Radeon 780M passed in
     - WebGPU
     - —
     - —
     - —
     - —
     - pending (:issue:`42`)

On this laptop the CPU embeds a 10,000-photo library in about three
minutes, so the GPU is not needed for CLIP; it matters for larger models.
