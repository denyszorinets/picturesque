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

In **Settings → AI acceleration**, choose where models run:

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Choice
     - Runs on
   * - Automatic (default)
     - DirectML on Windows, Core ML on Apple Silicon, the CPU on Linux.
       WebGPU is used only when chosen: on a computer without a working
       Vulkan driver for its GPU it runs on a software driver, more than
       ten times slower than the CPU.
   * - CPU only
     - The CPU. Works on every processor with AVX2 (2013 and later); on
       older ones AI is unavailable and Settings says why
       (:doc:`/architecture/decisions/0015-onnx-runtime-loaded-at-first-use`).
   * - DirectML
     - The GPU on Windows (DirectX 12)
   * - Core ML
     - The Neural Engine and GPU on Apple Silicon Macs
   * - WebGPU
     - The GPU through Vulkan on Linux, or Metal on Apple Silicon Macs.
       On Linux with a supported GPU, analysis is faster and the CPU stays
       free: on an AMD Radeon 780M about 1.4 times as fast as the CPU
       (:doc:`/development/benchmarks`).

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

.. _features-ai-models:

Models
------

Models are not bundled with the app. **Settings → AI models**
lists the models Picturesque can use, what each is for, its download size
and its **license** (with a link to the full text) and source. Every model
offered has a license that allows its use in a GPL-3.0 application.

- **Download** fetches the model once. Each file is checked against the
  size and SHA-256 checksum Picturesque expects; a corrupt or tampered
  download is rejected, nothing of it is kept, and **Retry** starts over
  (files that were already complete are kept). Each file may take up to
  an hour to download, enough for slow connections; a download that
  takes longer fails and can be retried.
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
       photos, text search
   * - CLIP ViT-L/14 text encoder (OpenAI; MIT)
     - 496 MB
     - Text search (with the image encoder)
   * - YuNet face detector (MIT) and MediaPipe face mesh (Apache-2.0)
     - 5 MB
     - Faces, closed eyes and face sharpness
   * - Qwen3-VL 4B, built-in critic (Apache-2.0)
     - 2.9 GB
     - Critiques of photos (:ref:`features-ai-critique`)

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

.. _features-ai-taste:

Your taste
----------

The aesthetic score knows what people in general like. Picturesque also
learns what **you** like, from your own decisions: once at least 10
analyzed photos are picks and 10 are rejects, it learns a **personal
taste model** and gives every photo a **Your taste** score, how much it
looks like the photos you pick rather than the ones you reject. Nothing is
downloaded and nothing leaves your computer: the model is a small linear
model on the CLIP embeddings background analysis already computed.

- **It keeps learning.** Whenever picks or rejects change, the model is
  retrained in the background with the series, in well under a second for
  a few hundred decisions and about a second at most (it uses at most
  1,000 picks and 1,000 rejects). Only flags count: stars, auto-picks and
  best-photo choices do not.
- **Blended into the aesthetic score.** The aesthetic score used for
  ranking, sorting, auto-pick and the **Aesthetic** filter is the general
  score blended with yours, half and half by default. **Settings → Your
  taste** (:ref:`reference-settings-taste`) changes the share, from 0 %
  (general score only) to 100 % (your taste only).
  The loupe shows the general **Aesthetic** score and **Your taste**
  separately.
- **Why this score** says *Like the photos you pick* above 70 % and *Like
  the photos you reject* below 30 %.
- **Settings → Your taste** says how many decisions it learned from and
  how well it **agrees** with you: how often it scores one of your picks
  above one of your rejects, measured on decisions it did not learn from
  (cross-validation); before that, how many more it needs. The API has it
  as ``taste_status`` (:doc:`/reference/api`).

It learns what distinguishes your picks from your rejects, so it is most
useful when you cull consistently. Rejecting the weaker frames of a burst
also teaches it; picking only in one genre and rejecting only in another
teaches it to prefer that genre.

On the Commons sample of the general score (featured as "picks", ordinary
as "rejects"), 40 picks and 40 rejects were enough to rank held-out photos
better than the general score (AUC 0.89 against 0.79), and blending both
reached 0.91. With the opposite taste, which the general score gets
backwards (0.21), it learned that just as well
(:repo:`spikes/taste/results.md`).

.. _features-ai-critique:

Critiques
---------

A score says how good a photo is; a **critique** says why. Picturesque can
ask a vision-language model running on your computer for one: a rating
from 1 to 10, a one-sentence summary, and what works and what does not
(:doc:`ADR-0016 </architecture/decisions/0016-local-vision-language-critic>`).

Critiques are **off by default**. Two ways to get them:

- **Built-in** (default): Picturesque runs Qwen3-VL 4B itself.
  Download *Qwen3-VL 4B (built-in critic)* (2.9 GB, Apache-2.0) in
  **Settings → AI models**, then turn on *Critique photos with a local
  vision-language model* in **Settings → AI critic**. Nothing else to
  install.
- **A model server you run**: choose it in **Settings → AI critic**, for
  example `Ollama <https://ollama.com>`_ with ``ollama pull
  qwen3-vl:4b``. Useful if you already run one or want another model.

Then the loupe's info panel has a **Critique this photo** button. The
critique is kept with the photo and shown whenever you come back to it;
**Ask again** asks for a new one. Choosing another model or rubric shows
only critiques written with them.

The built-in critic runs in a helper process that starts with the first
critique, keeps the model loaded while you keep asking, and stops after
five idle minutes, returning its memory (about 5 GB on the CPU). It uses
the GPU through Vulkan (Linux) or Metal (macOS) when there is one, unless
**AI acceleration** is *CPU only*; on Windows it uses the CPU. On a Ryzen
7 PRO 7840HS a critique takes about 20 s on the CPU and 10 s on its
Radeon 780M; the first one also loads the model (a few seconds). If the
helper fails, only that critique fails.

.. list-table:: Suggested models for a server (all Apache-2.0)
   :header-rows: 1
   :widths: 25 15 60

   * - Model
     - Download
     - Notes
   * - ``qwen3-vl:2b``
     - 1.9 GB
     - Fastest; fine on a laptop CPU, shallower critiques.
   * - ``qwen3-vl:4b`` (default)
     - 3.3 GB
     - Good critiques, about 8 GB of memory.
   * - ``qwen3-vl:8b``
     - 6.2 GB
     - Best critiques; wants a GPU or 16 GB of memory.

- **Any OpenAI-compatible server works**: Ollama (default address
  ``http://127.0.0.1:11434/v1``), llama.cpp's ``llama-server`` (for example
  ``http://127.0.0.1:8080/v1``) or LM Studio. Any vision model the server
  has can be chosen; check its license yourself. Picturesque suggests only
  models whose license it checked: Q-Align/OneAlign (non-commercial code,
  LLaMA-2 weights), ArtiMuse (no license) and Qwen2.5-VL 3B (research
  license) are not suggested.
- **Privacy.** Only the photo's preview, shrunk to 768 pixels and stripped
  of all metadata (no EXIF, GPS or XMP), is given to the model, and only
  when you ask. The built-in critic never uses the network. If a server's
  address is not on this computer, the photo goes to that machine;
  Settings warn about it.
- **The rubric** tells the model what to judge: by default composition,
  light, subject and moment, colour and tone, and technique. Write your own
  in Settings to focus on what you are learning; *Use the default rubric*
  goes back.
- Settings suggest checked models with their size and license; click one
  to use it. Any model the server has can be typed in.
- Settings warn when the server address is not on this computer.
- **Speed.** One critique at a time. With a server, the time depends on
  the server and model.
- The rating is **not used for ranking**: models are not calibrated like the
  aesthetic score, and differ from each other. It is a second opinion in
  words.

.. _features-ai-search:

Text search
-----------

With both CLIP encoders downloaded, type a description in the **search
box** above the timeline (or press **/**) and press **Enter**: *dog on a
beach*, *red flowers*, *church interior*, *sunset over the sea*. The
description is turned into an embedding in the same space as the photos',
and the matching photos replace the timeline, **best match first**, under
*Matching "…"*. The search appears as a chip with the other filters and
combines with them (for example only picks, or one day); remove the chip,
clear the box or press **Esc** in it to return to the timeline.

Photos are ranked by how close their embedding is to the description's,
and clearly unrelated photos are left out. On a sample of 150 photos,
queries such as *a bird*, *flowers* or *a church interior* returned only
fitting photos in their top five (:repo:`spikes/search/results.md`).
Descriptions work best in English. Photos are found once background
analysis has computed their embeddings.

Text search also needs the image encoder; without either model the search
box says which one to download.

.. _features-ai-faces:

Faces and closed eyes
---------------------

With the face models downloaded, background analysis finds the faces in
every photo and judges each one:

- **Closed eyes.** The face mesh places landmarks on the eyelids; the
  *eye aspect ratio* (the lids' opening relative to the eye's width) is
  about 0.3 for open eyes and falls under 0.17 when they close. Eyes are
  judged only on faces at least 40 pixels tall in the 1024-pixel analysis
  image, and only when the face mesh is confident it sees a face; otherwise
  they are left undecided rather than guessed.
- **Face sharpness.** The technical sharpness measure
  (:doc:`/reference/quality-scores`) applied to the face alone, so a photo
  whose background is sharp but whose face is soft is caught.

The loupe lists the faces, how many have closed eyes and how sharp the
sharpest face is. Up to eight faces per photo are examined, largest first.
The test photos (two with closed eyes, two with open eyes, from Wikimedia
Commons) are all judged correctly
(:repo:`crates/infra/tests/fixtures/faces/SOURCES.md`). Squinting, deep
shadows and sunglasses can still fool it.

.. _features-ai-library:

AI in the library
-----------------

- **Tiles** of photos where someone important has closed eyes show an
  **Eyes closed** badge.
- **Filters** (Quality section): a minimum **Aesthetic** score, **Hide
  closed eyes**, and **People**: *With* (any face found, even a small one
  in the background) or *Without* (no face found, or not analyzed by the
  face models yet).
- **Best quality first** sorts by the combined score
  (:ref:`features-series-score`).
- The loupe's **Why this score** list says, in words, what raised or
  lowered the photo's score: its place in its series and why the best
  photo won (open eyes, sharper, looks better), closed eyes, a soft or
  sharp face, blur, exposure problems, noise and the aesthetic score. Only
  notable values are listed.
