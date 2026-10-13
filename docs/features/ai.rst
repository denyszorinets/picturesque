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

Models
------

Models are not bundled with the app. **Import → Library → AI models**
lists the models Picturesque can use, what each is for, its download size
and its **license** (with a link to the full text) and source. Every model
offered has a license that allows its use in a GPL-3.0 application.

- **Download** fetches the model once. Each file is checked against the
  size and SHA-256 checksum Picturesque expects; a corrupt or tampered
  download is rejected, nothing of it is kept, and **Retry** starts over
  (files that were already complete are kept).
- After that, models are used **offline**: Picturesque contacts the
  network only when you press Download.
- **Cancel** stops a download; **Remove** deletes a model's files.
- When a model is updated, the new version is downloaded next to the old
  one, which is then deleted. Results computed with the old version are
  recomputed.

Available models:

.. list-table::
   :header-rows: 1
   :widths: 30 15 55

   * - Model
     - Size
     - Used for
   * - CLIP ViT-L/14 image encoder (OpenAI; MIT)
     - 1.2 GB
     - Understanding what photos show: the aesthetic score, similar
       photos, and later text search

Models are stored in the ``models`` folder of the application data folder
(:doc:`/reference/data-locations`), one folder per model and version.

.. _features-ai-embeddings:

Image embeddings
----------------

Once the CLIP model is downloaded, background analysis
(:ref:`features-library-analysis`) also runs it on every photo, photos on
screen first. It turns each photo into an **embedding**: 768 numbers
describing what the photo shows, so photos of similar subjects get similar
embeddings. Photos analyzed before the download are processed then; until
the model is downloaded, analysis does not wait for it.

The photo's preview is scaled so its shorter side is 224 pixels and the
centre square is used, as CLIP was trained. Embeddings are stored in the
catalog with the model version and recomputed when the model changes.

**Similar photos** are found by comparing embeddings (cosine similarity)
across the whole library in memory, which takes milliseconds for 10,000
photos (``similar_photos``, :doc:`/reference/api`). The library shows them
from :issue:`48`.

On a laptop CPU (Ryzen 7 PRO 7840HS) a photo takes about a third of a
second, so a 10,000-photo library is analyzed in about an hour in the
background; a supported GPU is faster (:doc:`/development/benchmarks`).

.. _features-ai-aesthetic:

Aesthetic score
---------------

The **aesthetic score** estimates how good a photo looks to people:
composition, light, subject and mood, not only technical quality. It comes
from the improved LAION aesthetic predictor (Apache-2.0), a linear model on
the CLIP embedding trained on hundreds of thousands of human ratings from
1 to 10.

The loupe shows it as **Aesthetic** in percent: the rating mapped linearly
from 4.5 (0%) to 6.5 (100%), which covers almost all photos; ordinary
snapshots typically land around 30% and outstanding photos around 60%.
The rating itself is shown when you point at the score.

On a sample of 150 Wikimedia Commons *Featured pictures* and 150 random
Commons photos, a featured photo scores higher than an ordinary one 80%
of the time, against 66% for the technical score
(:repo:`spikes/aesthetic/results.md`). It judges looks only: it does not
know which shot of a series you prefer or which moment matters to you.
