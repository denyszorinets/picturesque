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

Consequences
------------

Positive
~~~~~~~~

- Readable reasons for amateurs, with no new native dependency.
- Users choose the model size that fits their hardware.

Negative
~~~~~~~~

- Users must install and run a server and a model themselves.
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
