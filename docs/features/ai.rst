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
from 1 to 10, a one-sentence summary, what works and what does not, and
advice (:doc:`ADR-0016 </architecture/decisions/0016-local-vision-language-critic>`).

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
**Ask again** asks for a new one. After choosing another model, preset or
rubric, earlier critiques stay visible, marked as written by an earlier
model, preset or rubric, until you ask again.

The advice has four parts. Each flaw has a **category** (exposure, focus,
composition, noise, colour, horizon, light, timing or other). For a RAW
file the critic estimates the **potential**, the rating the photo could
reach after the best editing (exposure, white balance, crop, noise
reduction), says whether the flaws are **fixable** (not needed, easy,
needs skill, not fixable) and lists up to three **fix steps**. It also
suggests up to three **improvements** to the photo itself (framing, light,
moment) and up to three **camera tips** for the next shot. The critic is
told the photo's ISO, shutter speed, aperture and focal length when the
file records them, and whether the original is a RAW file or a JPEG, so
the tips can fit. The model judges from a small preview, so the fixability
and the potential are advice, not a promise. The fixability does not change
the score; the potential does for RAW originals, with a small weight
(:ref:`features-ai-critique-score`).

The built-in critic runs in a helper process that starts with the first
critique, keeps the model loaded while you keep asking, and stops after
five idle minutes, returning its memory (about 5 GB on the CPU). It uses
the GPU through Vulkan (Linux) or Metal (macOS) when there is one, unless
**AI acceleration** is *CPU only*; on Windows it uses the CPU. On a Ryzen
7 PRO 7840HS a critique takes about 16 s on its Radeon 780M; the time on
the CPU has not been measured. The first critique also loads the model (a
few seconds). If the helper fails, only that critique fails.

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
  Settings warn about it. With a server, the photo's shot data (ISO,
  shutter speed, aperture, focal length, and whether the original is a RAW
  file or a JPEG) is sent as text together with the image; no GPS, file
  path or file name is.
- **The rubric** tells the model what to judge: by default composition,
  light, subject and moment, colour and tone, and technique. Write your own
  in Settings to focus on what you are learning; *Use the default rubric*
  goes back.
- Settings suggest checked models with their size and license; click one
  to use it. Any model the server has can be typed in.
- Settings warn when the server address is not on this computer.
- **Speed.** One critique at a time. With a server, the time depends on
  the server and model.
- The rating is a second opinion in words. Ratings are not calibrated like
  the aesthetic score and models differ from each other, so ratings are not
  compared directly. Ranking uses the **critique score** instead
  (:ref:`features-ai-critique-score`).

How strict it is
~~~~~~~~~~~~~~~~

With the Professional preset (default), the critic is told to judge like a
juror of a selective competition: it
lists the flaws first, then the strengths, and only then rates. Ordinary
snapshots get 3 to 5; 8 or more is rare. On a sample of 50 Wikimedia
Commons photos, 25 featured pictures and 25 ordinary ones, the mean rating
was 7.6 for the featured pictures and 4.2 for the ordinary photos, and it
tells them apart as well as the milder prompt it replaced (AUC 0.90). The
measurements are in ``spikes/llama/results.md``.

Critiques written by an earlier prompt version are still counted until
they are redone; the background critic redoes them last. Prompt version 4
added the advice; critiques written before show without it, marked as
outdated.

.. _features-ai-critique-presets:

Presets: who the critique is for
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**Settings → AI critic → Critique style** chooses the audience. Every
preset asks for the same reply (flaws first, then the rating, then the
advice) and differs in voice, in how forgiving the rating is and in how
the advice is worded.

- **Professional** (default): a strict juror. A typical photo gets 5,
  one clear strength 6, 7 is portfolio quality. Formal vocabulary. Use it to pick the best
  photos from a large library.
- **Amateur**: a demanding but fair coach for an enthusiast. A typical
  photo gets 5, 6 needs one clear strength and 7 is a photo to be proud of.
  One clear weakness caps the rating at 6, two or more at 5. Constructive,
  practical wording, and concrete improvements and camera settings.
- **Beginner**: an encouraging teacher. Plain language, photography terms
  explained, the single easiest fix first, camera settings explained in
  everyday words ("raise the ISO a little so the picture is less
  blurry"). A typical photo gets 5; a photo with clear flaws is never
  above 5, and one whose only weakness is small gets 7 at most.

The preset is separate from the **rubric**, which still decides what is
judged and works with every preset. Changing the preset marks the
critiques already written as outdated; they stay visible, and the
background critic redoes them slowly, after photos that have no critique
yet. The critique score is a percentile within your library, so a softer
or tougher preset does not distort the ranking; a library critiqued with
several presets at once blurs it a little, because their ratings are not
on the same scale. Let the background critic finish after a switch.

.. _features-ai-critique-background:

Background critique
~~~~~~~~~~~~~~~~~~~

Picturesque can critique the whole library in the background, so that
every photo has a critique to compare. It is **off by default**: turn on
*Critique the whole library in the background* in **Settings → AI critic**
(:ref:`reference-settings-critic`), once critiques are on. It works with
the built-in critic and with a model server, and uses the same model,
rubric and backend as the critiques you ask for. It critiques one photo at
a time, every photo of the library except rejects.

While it works, a second pill in the top right corner, next to the
analysis pill, shows ``Critique N / M`` (photos with a current critique out
of all photos but rejects). It says why it waits when paused, and the
**Activity** tab has a *Critique* section with the same count, the time
per photo, the time left and the photos that failed.

It is a separate, slow pipeline. The order is:

1. Photos on screen first; the library's ``prioritize_analysis`` call also
   prioritizes their critiques.
2. The best photo of each series, and the picks.
3. Photos in no series.
4. The rest of each series.

Within each group, newest photos come first. A photo whose critique was
written by another model, preset or rubric, or with an older prompt
version, keeps that critique and counts in the score until it is redone; such photos are
redone only after every photo without any critique.

All its state is in the catalog, where critiques are stored with the
analysis results, so after a restart it carries on where it stopped. A
photo whose critique fails is skipped until the critic settings change.

The intensity setting (``critic.intensity``) in **Settings → AI critic**
sets how hard it works:

- **Gentle** (default): runs the built-in helper at low operating-system
  priority and with half the processor's threads. It waits while photos
  are being imported or analyzed, and while the computer runs on battery.
  Battery is detected on Linux only for now; elsewhere, power counts as
  mains.
- **When idle**: full speed, but only after three minutes without using
  the app.
- **Full**: full speed whenever critiques are on.

Changing the intensity restarts the built-in helper; the model reloads in
a second or two. A critique you ask for in the loupe interrupts the
background one, which is retried later. A server cannot be interrupted, so
with a server your request waits until the background critique finishes
its current photo.

More about the background critic:

- Rejected photos are not critiqued and do not count in the percentiles.
- A critique you ask for in the loupe is shown at once and updates the
  scores.
- On shutdown, a critique in progress is cancelled and redone later.
- A critique you ask for while the background one works restarts the
  built-in helper once, because the background critique runs at low
  priority. The model reloads in a second or two.
- After three failed critiques in a row, or when the server or the
  built-in critic cannot be reached, it pauses and shows why in
  **Settings → AI critic**.

The speed depends on the hardware. On an AMD Radeon 780M with Vulkan a
critique took 16.4 s on average and 28 s at most (prompt version 4). A
library of 10,000 photos takes about 45 hours on that GPU. The CPU time
was not measured again for version 4 and is longer. That is why the
feature is off by default and gentle by default.

**Settings → AI critic** shows the progress: how many photos are critiqued
out of how many, how many are to redo, about how long is left, why it is
paused if it is, *Critiquing now* while it works, and *Done* when it is
finished. The API has the same figures as ``background_critique_status``
(:doc:`/reference/api`).

.. _features-ai-critique-score:

Critique score
~~~~~~~~~~~~~~

Ratings are not calibrated: a model may rate an ordinary photo 8 and a
featured one 7. So each photo's **critique score** is the share of the
library's critiqued photos that it is rated above, ties counting half. A
score of 80 % means the photo is better than 80 % of your critiqued photos.
With fewer than 20 critiques, the rating maps linearly instead, from 1 to
10 onto 0 to 100 %.

For a RAW original (a RAW+JPEG pair counts as RAW; a JPEG-only photo
never does) the rating first moves toward the critic's **potential**, the
rating it could reach after editing, by ``critic.potential_weight`` (default
15 %, 0 turns it off)::

   effective = rating + (max(potential, rating) - rating) * weight

The result is clamped to 1–10 and not rounded. This happens before the
percentiles are computed, so the library's population is all effective
ratings. It has no effect when the potential is missing (critiques from
older prompts), is below the rating, or the weight is 0. Nothing is stored:
scores are rebuilt from the stored critiques whenever series are
recomputed, so changing the weight never marks critiques outdated and never
reruns the model. The fixability and fix steps are not used. The weight is
low because the model sees a small preview (about 768 px), not the RAW data,
so it cannot judge clipped highlights, the noise floor or the dynamic
range; its potential is a guess.

The loupe shows it in a **Critic** row, and **Why this score** adds *The
critic ranks it among your best (top N%)* at 80 % or more, where N is 100
minus the critique score in percent (at least 1), and *The critic ranks it
below most of your photos (bottom N%)* at 20 % or less, where N is the
critique score in percent (at least 1). With fewer than 20 critiques the
score is the rating mapped linearly, so N then reflects the rating, not a
rank. When the potential raised a RAW photo's rating, **Why this score**
also says, for example, *Rated as a RAW file: the critic's 6 is raised toward
the 8 it could reach after editing*.

The critique score counts for ``critic.score_weight`` of the aesthetic
score; the default is 25 %, and **Settings → AI critic** shows it as
*Critique counts for N % of the aesthetic score* and, for RAW files,
*RAW potential counts for N % of the critique rating* (step 5). The aesthetic score used
for ranking, sorting, auto-pick and the **Aesthetic** filter is built in
steps: the general score, blended with **Your taste** (:ref:`features-ai-taste`),
then blended with the critique score. Photos without a critique are not
affected. Critique scores count only while critiques are on. See
:doc:`/concepts/scores`.

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
