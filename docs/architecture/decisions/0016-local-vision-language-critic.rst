ADR-0016: Photo Critiques from a Local Vision-Language Model Server
===================================================================

:Status: Accepted
:Date: 2026-10-27

Context
-------

The aesthetic score (:doc:`ADR-0006 <0006-local-ai-with-onnx-runtime>`)
and the personal taste score (:issue:`218`) say *how good* a photo looks,
but not *why*. Amateur photographers learn more from a short critique:
what works, what does not. Vision-language models (VLMs) can write one
from a rubric.

Requirements:

- Photos never leave the machine unless the user points the app at another
  one.
- Every model the app offers or suggests is licensed compatibly with a
  GPL-3.0 application (ADR-0006).
- The app stays small: VLMs are several gigabytes and need a GPU to be
  quick.
- Works with the local LLM servers people already run.

Licenses of the candidates were checked on 2026-10-27 (:issue:`217`):

- **Q-Align / OneAlign** (aesthetic and quality scores from a VLM): the
  code is under the S-Lab License, for non-commercial use only; the
  weights are derived from LLaMA-2 through mPLUG-Owl2. Not offered.
- **ArtiMuse** (CVPR 2026, critique and score): no license on the code
  or weights (all rights reserved), and its dataset is for research only.
  Not offered.
- **Qwen3-VL** 2B, 4B and 8B: Apache-2.0. Offered.
- **Qwen2.5-VL 3B** uses the Qwen Research License (non-commercial), unlike
  the Apache-2.0 7B; it must not be suggested.

Decision
--------

Ask a **local server with an OpenAI-compatible chat API** for critiques:
Ollama (the default, ``http://127.0.0.1:11434/v1``), llama.cpp's
``llama-server`` or LM Studio. The user installs the server and the model;
the app sends one preview (longest edge 768 px, JPEG) with a rubric and
asks for JSON (a rating from 1 to 10, a summary, strengths and weaknesses)
through ``response_format``.

- Off by default; a critique runs only when the user asks for one, one at
  a time, and is cached with the photo's analysis results together with the
  model and a hash of the rubric. Changing either makes old critiques
  stale.
- The rubric is editable; the default covers composition, light, subject
  and moment, colour and technique.
- The app suggests only models it checked (Qwen3-VL, Apache-2.0); any
  model the server has can be chosen, at the user's responsibility.
- The server address is a setting. Settings warn when it is not on this
  computer, since photos are then sent to it.
- The critique's rating is shown, not used for ranking: it is not
  calibrated like the aesthetic score.
- A ``CriticServer`` port in ``picturesque-app`` keeps HTTP and image
  encoding in ``picturesque-infra``.

Alternatives Considered
-----------------------

Bundle llama.cpp
~~~~~~~~~~~~~~~~

Advantages:

- Nothing else to install.

Disadvantages:

- A large native dependency per platform and GPU backend, on top of ONNX
  Runtime; multi-gigabyte model downloads in the model manager.

VLM through ONNX Runtime
~~~~~~~~~~~~~~~~~~~~~~~~

Advantages:

- Reuses ADR-0006's runtime and providers.

Disadvantages:

- Generative VLM exports and their token loops are immature and slow on
  the providers the app uses; quantized models are hard to get.

Ollama's native API
~~~~~~~~~~~~~~~~~~~

Advantages:

- Can list models with sizes and pull them.

Disadvantages:

- Only Ollama speaks it; the OpenAI-compatible API also covers llama.cpp
  and LM Studio.

Dedicated aesthetic VLMs (Q-Align, ArtiMuse)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Advantages:

- Better calibrated scores than a general VLM with a rubric.

Disadvantages:

- Their licenses do not allow use in the app (above).

Amendment: the built-in critic
------------------------------

:Date: 2026-10-28

Requiring users to install Ollama is a hurdle for the amateurs the
critique is for. A spike (:issue:`224`, :repo:`spikes/llama/results.md`)
ran Qwen3-VL 4B Instruct (Qwen's official GGUF files, Q4_K_M with the
Q8_0 vision projector, Apache-2.0) with llama.cpp from Rust
(``llama-cpp-2``, MIT or Apache-2.0): about 20 s per critique on a
16-thread CPU and 10 s on a Radeon 780M through Vulkan, with every reply
valid JSON under a grammar made from the schema.

The app now has a **built-in critic** (:issue:`225`), the default:

- The model files (2.9 GB) are offered in the model manager like the
  other models, with checksums and license, and downloaded on request.
- A helper process, ``picturesque-critic`` (``crates/critic``), runs them
  with llama.cpp. It is started on the first critique, keeps the model
  loaded while critiques follow, and is stopped after five idle minutes.
  It speaks one JSON line per request and reply on stdin and stdout
  (``picturesque_core::critic_protocol``).
- A separate process, because a misuse of llama.cpp's API or running out
  of memory aborts the process with a native assert that Rust cannot
  catch (the spike hit one): it ends one critique, not the app; and the
  model's memory is returned when the helper exits.
- llama.cpp is built with dynamic backends: GPU backends (Vulkan on Linux,
  Metal on macOS) and the best of 14 CPU variants are libraries loaded at
  run time from the helper's folder, so it starts without a Vulkan loader.
  No OpenMP. Software Vulkan drivers are skipped, and the AI acceleration
  setting *CPU only* keeps it on the CPU. On Windows it runs on the CPU for
  now (Vulkan there needs the Vulkan SDK on the build machine).
- The server option stays for users who already run one or want another
  model.

Alternatives for running the model, considered after the spike:

- **llama.cpp in the app's process**: no helper to ship, but a native
  assert or out-of-memory would close the app, and llama.cpp would share
  the process and GPU device with ONNX Runtime.
- **ONNX Runtime**: Qwen3-VL has no native support in onnxruntime-genai,
  which has no Rust binding; ``ort`` would need a hand-written generation
  loop, tokenizer and image preprocessing.

Amendment: background critique
------------------------------

:Date: 2026-10-28

The app can critique the whole library in the background (:issue:`228`,
:ref:`features-ai-critique-background`). Decisions:

- **A separate, slow pipeline**, off by default (``critic.background``). It
  critiques one photo at a time, every photo except rejects, with the same
  model, rubric and backend as on-demand critiques. Its state is in the
  catalog, so it resumes after a restart.
- **Percentile calibration.** Ratings are not calibrated (a model rated an
  ordinary photo 8 and a featured one 7). A photo's critique score is the
  share of the library's critiqued photos rated below it, ties counting
  half; with fewer than 20 critiques, ratings map linearly. The score is
  blended into the aesthetic score by ``critic.score_weight`` (default
  0.25). This amends the Decision: the critique's rating is still not used
  for ranking, but the critique score is.
- **Intensity levels.** ``gentle`` (default: low operating-system priority,
  half the processor threads, waits during imports, analysis and on battery),
  ``when_idle`` (full speed after three minutes without using the app) and
  ``full``.
- **User critiques interrupt it.** A critique asked for in the loupe
  interrupts the background one, which is retried later. Servers cannot be
  interrupted, so the background critique finishes first there.
- **Key order in the schema.** The critic's JSON Schema is sent with its
  keys in generation order (flaws, strengths, summary, rating): llama.cpp's
  grammar writes them in that order, and the rating is written last so that
  it follows the flaws the model just listed. To keep the order, the
  workspace enables serde_json's ``preserve_order`` feature (adds the
  ``indexmap`` crate, already in the lockfile through schemars; MIT or
  Apache-2.0).
- **Failures.** A critique that fails is skipped until the critic settings
  change. After three failures in a row, or when the server or helper cannot
  be reached, the background critic pauses and tells why instead of trying
  every photo.

Alternatives considered:

- **Blending the absolute rating linearly** into the score: the ratings
  compress to 7 to 8.
- **Replacing the aesthetic score** with the critique: too exposed to the
  model's bias.
- **Running at full speed always**: slows the computer.

Amendment: critique v2
----------------------

:Date: 2026-10-28

The critique also advises (:issue:`231`). Decisions:

- **New fields**, written after the rating: ``issues`` (categories of the
  weaknesses), ``potential`` (the rating after the best RAW editing, at
  least the rating), ``fixable`` (``not_needed``, ``easy``, ``needs_skill``,
  ``not_fixable``), ``fix_steps``, ``improvements`` and ``camera_tips``. The
  schema lists them in that order, after ``rating``, so the verdict follows
  from the flaws already written. The grade anchors are unchanged.
- **Prompt version 4.** The bump makes the background critic redo the
  library once; critiques of version 3 and earlier still parse (the new
  fields default to empty or absent) and are shown as outdated.
- **Context for the prompt.** The prompt names the shot data the catalog
  holds (ISO, shutter speed, aperture, focal length) and whether the
  original is a RAW file or a JPEG.
- **Fixability is advisory.** The model sees a 768 px preview, not the
  RAW file; ``potential`` and ``fixable`` are never used for the score.
- **Larger limits.** The reply grew from about 120 to about 350 tokens:
  the helper's ``max_tokens`` is 768 and its context 3072 tokens.
- **Lenient reading.** An unknown issue or fixability value is dropped, not
  a failed critique.

Amendment: critique presets
---------------------------

:Date: 2026-10-28

The critique can be written for different audiences (:issue:`232`).
Decisions:

- **A ``preset`` setting**: ``professional`` (the existing strict prompt,
  unchanged and the default), ``amateur`` and ``beginner``. A preset
  changes the voice, the different grade anchors and the wording of
  the advice. The JSON schema, its key order and the flaws-first structure
  are the same, since they are what made the rating discriminate.
- **The rubric stays separate.** It remains the user's override of what is
  judged; there is no "custom" preset.
- **The preset is part of the stored rubric key**, so changing it marks
  critiques outdated and the background critic redoes them. For
  ``professional`` the key is computed as before presets, so existing
  critiques stay current and the prompt version is not bumped.
- **Ratings are not comparable across presets.** The critique score uses
  percentiles within the library, which absorbs a uniform shift; mixed
  presets blur it a little.
- **Not evaluated yet.** The presets have not been evaluated on a photo
  sample; measuring them is a follow-up.

Amendment: RAW potential in the score
-------------------------------------

:Date: 2026-10-28

The critic's ``potential`` now counts for RAW originals (:issue:`233`). It
replaces the v2 decision that ``potential`` and ``fixable`` are never used
for the score, for ``potential`` only; ``fixable`` and ``fix_steps`` stay
advisory. Decisions:

- **A ``critic.potential_weight`` setting**, 0–1, default 0.15; 0 turns it
  off. For a RAW original the rating moves that share of the way toward the
  potential: ``effective = rating + (max(potential, rating) - rating) *
  weight``, clamped to 1–10, not rounded. A RAW+JPEG pair counts as RAW
  because the RAW is still there to edit; a JPEG-only photo never does.
  There is no effect when the potential is missing, below the rating, or the
  weight is 0.
- **Blend before the percentiles.** The effective ratings are the
  population the library percentiles are computed over, so the score keeps
  one unit.
- **Nothing is stored.** Scores are rebuilt from the stored critiques on
  every series recompute, so changing the weight needs no schema change,
  never marks critiques outdated and never reruns the model.
- **A reason.** "Why this score" adds an Info reason only when the rating
  was raised, e.g. "Rated as a RAW file: the critic's 6 is raised toward the
  8 it could reach after editing".
- **Weakness.** The model sees a preview of about 768 px, not the RAW data,
  so it cannot judge clipped highlights, noise floor or dynamic range. Its
  potential is a guess, which is why the default is low and 0 disables it.

Consequences
------------

Positive
~~~~~~~~

- Readable reasons for amateurs, with no new native dependency.
- Users choose the model size that fits their hardware.

Negative
~~~~~~~~

- With the server option, users must install and run a server and a model
  themselves (the built-in critic, added later, needs neither).
- The built-in critic adds about 80 MB of llama.cpp libraries to every
  installer, a C++ build with CMake (and, for Vulkan, ``glslc`` and
  SPIRV-Headers) to every build machine, and minutes to a clean build.
- Critiques are slow on a CPU (tens of seconds) and vary between models.

Risks
~~~~~

- A server on another machine receives photos; mitigated by the warning
  and the default loopback address.
- Servers differ in their support for ``response_format``; replies are
  parsed leniently (the first JSON object in the text).

References
----------

- `Ollama OpenAI compatibility <https://docs.ollama.com/api/openai-compatibility>`_
- `Qwen3-VL <https://huggingface.co/Qwen/Qwen3-VL-4B-Instruct>`_
