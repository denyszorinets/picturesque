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
  (:doc:`/reference/similarity`).

Otherwise it starts a new series. A single odd frame (someone walking
through the shot) stays in the series when the next photo looks like the
series again. A series has at least two photos; photos without a capture
date are never in a series.

Until a photo has been analyzed, it is grouped by time alone. Series are
updated in the background as analysis progresses and after each import
(:ref:`features-library-analysis`).

Adjusting the thresholds
------------------------

In **Import → Library → Series**, set the longest pause between shots and
the similarity limit (:ref:`reference-settings-series`). Series are
regrouped as soon as you save.

The best photo
--------------

Each series has a **best photo**, ranked from the technical scores
(:doc:`/reference/quality-scores`):

.. code-block:: text

   score = (wₛ · relative sharpness + wₑ · exposure + wₙ · low noise) / (wₛ + wₑ + wₙ)

- **Relative sharpness** compares detail with the sharpest photo of the
  series: 1 for the sharpest, 0.25 less for each halving of detail. In a
  burst every frame may be "sharp"; the crispest one still wins.
- The weights default to 0.6 (sharpness), 0.3 (exposure) and 0.1 (noise)
  and can be changed in **Import → Library → Series**
  (:ref:`reference-settings-ranking`). Only their ratios matter.
- **Rejected** photos rank last, and photos not analyzed yet just before
  them.
- Ties go to the earlier photo, so the ranking never changes by itself.

You can **choose the best photo** yourself. Your choice wins over the
ranking and is kept when photos are re-analyzed; if a series contains
several photos you chose (after joining series), the latest choice wins.
Withdrawing a choice returns the series to the ranking. The best photo is
recomputed when scores arrive, when you reject a photo and when the
weights change.

Correcting series
-----------------

When the automatic grouping is wrong, mark a photo:

- **Split:** the photo starts a new series.
- **Join:** the photo belongs to the series of the photo before it, even
  if it was taken later or looks different.

Corrections are saved per photo and applied every time series are
recomputed, so they survive re-analysis, new imports and threshold
changes. Removing a correction returns that photo to automatic grouping.
The grid shows series and offers these commands from :issue:`36`; until
then they are available through the ``edit_series`` and ``choose_best``
commands (:doc:`/reference/api`).
