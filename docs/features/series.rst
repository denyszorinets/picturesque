.. _features-series:

Series
======

A **series** is a run of photos taken close together that show the same
scene: a burst, or several attempts at the same shot. Picturesque finds
series automatically, so you can compare their photos and keep the best.

How series are found
--------------------

Photos are taken in capture order. A photo joins the current series when
both are true:

- it was taken at most **30 seconds** after the previous photo, and
- it **looks like** one of the series' last three photos: their
  perceptual hashes differ by at most **16 bits**
  (:doc:`/reference/similarity`), or, once the CLIP model has analyzed
  them, their image embeddings are at least **0.87** similar
  (:ref:`features-ai-embeddings`). Embeddings recognize zoomed-in and
  reframed retakes that hashes miss
  (:repo:`spikes/series/results.md`).

Otherwise it starts a new series. A single odd frame (someone walking
through the shot) stays in the series when the next photo looks like the
series again. A series has at least two photos; photos without a capture
date are never in a series.

Until a photo has been analyzed, it is grouped by time alone. Series are
updated in the background as analysis progresses and after each import
(:ref:`features-library-analysis`).

Adjusting the thresholds
------------------------

In **Settings → Series**, set the longest pause between shots and
the similarity limit (:ref:`reference-settings-series`). Series are
regrouped as soon as you save.

The best photo
--------------

Each series has a **best photo**, ranked from the technical scores
(:doc:`/reference/quality-scores`) and, once the AI models have run, the
aesthetic score and faces (:doc:`/features/ai`):

.. code-block:: text

   score = (wₛ · sharpness + wₑ · exposure + wₙ · low noise + wₐ · aesthetic)
           / (wₛ + wₑ + wₙ + wₐ)
   score × 0.5 when someone important has closed eyes

- **Sharpness** is relative to the sharpest photo of the series: 1 for the
  sharpest, 0.25 less for each halving of detail. In a burst every frame
  may be "sharp"; the crispest one still wins. With faces, the face's
  sharpness relative to the sharpest face of the series counts as much as
  the whole photo's, so the frame where the face is in focus wins.
- The **aesthetic** term counts only for photos the CLIP model has
  analyzed (:ref:`features-ai-aesthetic`).
- **Closed eyes** of the largest face, or of any face at least half its
  size, halve the score: a sharp blink loses to a slightly softer frame
  with open eyes (:ref:`features-ai-faces`).
- The weights default to 0.6 (sharpness), 0.3 (exposure), 0.1 (noise) and
  0.4 (aesthetic) and can be changed in **Settings → Series**
  (:ref:`reference-settings-ranking`). Only their ratios matter; an
  aesthetic weight of 0 ranks by technical scores alone.
- **Rejected** photos rank last, and photos not analyzed yet just before
  them.
- Ties go to the earlier photo, so the ranking never changes by itself.

Whether this matches your own choices can be measured on a shoot of yours
with :repo:`spikes/ranking/README.md`.

You can **choose the best photo** yourself. Your choice wins over the
ranking and is kept when photos are re-analyzed; if a series contains
several photos you chose (after joining series), the latest choice wins.
Withdrawing a choice returns the series to the ranking. The best photo is
recomputed when scores arrive, when you reject a photo and when the
weights change.

.. _features-series-score:

Combined score
--------------

Every photo also gets a combined **score** (0–1) for comparing photos
across the library: the technical overall score, scaled by the face's
sharpness when there are faces, averaged with the aesthetic score when
there is one, and halved for closed eyes. The loupe shows it as
**Score**, and **Best quality first** sorts by it.

Correcting series
-----------------

When the automatic grouping is wrong, mark a photo:

- **Split:** the photo starts a new series.
- **Join:** the photo belongs to the series of the photo before it, even
  if it was taken later or looks different.

Corrections are saved per photo and applied every time series are
recomputed, so they survive re-analysis, new imports and threshold
changes. Removing a correction returns that photo to automatic grouping.

In the Library, select a photo and press **S** to split before it or
**J** to join it to the photo taken before it; pressing the same key again
removes the correction (:doc:`/reference/keyboard-shortcuts`).

Series in the grid
------------------

In the Library timeline:

- The photos of a series sit inside a **frame**. When a series continues
  on the next row, the frame's edge there is dashed and open.
- The best photo has a **green border** and a **★** badge. Rejected
  photos stay dimmed.
- Press **C** to compare the photos of the selected photo's series side by
  side, best first (:ref:`features-compare`).
- Press **B** to make the selected photo (or the photo in the loupe) the
  best of its series; press it again on that photo to withdraw the choice
  (:doc:`/reference/keyboard-shortcuts`).
- **Stack series** (above the timeline) shows each series as its best
  photo, with a **⧉ n** button giving the series' size. Click it to show
  all photos of that series; click **−** on its best photo to stack it
  again. Stacking works with filters: a stacked series shows its
  best-placed photo among those matching the filters. Whether series are
  stacked is kept for the session.

The grid updates by itself whenever series or their best photos change:
during analysis (at least every five seconds while photos are being
analyzed), after imports, culling and corrections.
