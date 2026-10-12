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
then they are available through the ``edit_series`` command
(:doc:`/reference/api`).
