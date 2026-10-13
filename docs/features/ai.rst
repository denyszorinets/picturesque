.. _features-ai:

Local AI
========

Picturesque runs machine-learning models on your computer to judge and
search photos; photos never leave it
(:doc:`ADR-0006 </architecture/decisions/0006-local-ai-with-onnx-runtime>`).
Models run with ONNX Runtime, on the CPU everywhere and on the GPU where
one is supported.

Acceleration
------------

In **Import → Library → AI acceleration**, choose where models run:

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Choice
     - Runs on
   * - Automatic (default)
     - The best provider this computer supports, else the CPU
   * - CPU only
     - The CPU. Always works.
   * - DirectML
     - The GPU on Windows (DirectX 12)
   * - Core ML
     - The Neural Engine and GPU on Apple Silicon Macs
   * - WebGPU
     - The GPU through Vulkan on Linux, or Metal on Apple Silicon Macs

Only the providers your build supports are offered:

- **Linux** (x86-64): WebGPU, CPU
- **Windows**: DirectML, CPU
- **macOS** (Apple Silicon): Core ML, WebGPU, CPU. Intel Macs are not
  supported yet: ONNX Runtime has no prebuilt binaries for them.

Below the choice, Picturesque shows where models really run. It checks by
running a tiny test model, so a provider that is compiled in but does not
work on your hardware (for example, no Vulkan driver) is detected. If the
chosen provider fails, models **fall back to the CPU** and the reason is
shown. Changing the choice takes effect for models loaded afterwards.
