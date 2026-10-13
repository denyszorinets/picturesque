Scores
======

Picturesque judges photos with several independent signals, and combines
them for ranking and sorting. Each comes with a detailed page.

.. list-table::
   :header-rows: 1
   :widths: 22 48 30

   * - Signal
     - What it measures
     - Needs
   * - Technical scores
     - Sharpness, exposure and noise, measured from the pixels
       (:doc:`/reference/quality-scores`)
     - Nothing; always computed
   * - Aesthetic score
     - How good the photo looks to people, from a model trained on human
       ratings (:ref:`features-ai-aesthetic`)
     - The CLIP image model
   * - Faces
     - Faces found, closed eyes and how sharp each face is
       (:ref:`features-ai-faces`)
     - The face models
   * - Similarity
     - Whether two photos show the same scene: perceptual hashes and CLIP
       embeddings (:doc:`/reference/similarity`)
     - Hashes always; embeddings with the CLIP model

How they are used:

- **Series** group photos taken close together that look alike
  (:doc:`/features/series`).
- The **best photo of a series** is ranked from sharpness (relative to
  the series, with faces counting), exposure, noise and the aesthetic
  score; closed eyes halve a photo's score. The weights are settings.
- Every photo gets a combined **score** (0–100%) to compare photos across
  the library; **Best quality first** sorts by it
  (:ref:`features-series-score`).
- The loupe's **Why this score** list explains a photo's score in words.

Scores are suggestions. Your own picks, rejects, stars and best-photo
choices always win, and are never changed by re-analysis.
