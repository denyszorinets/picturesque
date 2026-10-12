Quality scores
==============

Background analysis
(:doc:`/architecture/decisions/0011-resumable-background-analysis`) gives
every photo technical scores from 0 to 1 (higher is better). They are
computed on the photo's preview shrunk to 1024 px on the long edge, so
they describe how the photo looks at screen size, not at 100 %. The code
is in :repo:`crates/core/src/quality.rs`.

The loupe shows them as percentages under **Quality**.

Sharpness
---------

The variance of the Laplacian (a measure of fine edge contrast) in each
tile of an 8 × 8 grid.

- **Subject-weighted:** the subject is usually the sharpest region, so the
  best sixteenth of tiles is used, not the average. A sharp portrait on a
  blurred background scores as sharp.
- **Centre-weighted:** tiles count from 100 % at the centre down to 50 %
  in the corners.
- **Noise-corrected:** noise also raises the Laplacian variance. The
  estimated noise contribution is subtracted, so noisy photos do not look
  sharp.

The score rises with the logarithm of the variance, from 0 at 20 to 1 at
1000. On camera samples, sharp photos measure about 850–16,000, the same
photos slightly soft (1 px blur) 75–650, and clearly blurred (3 px blur)
5–70.

Exposure
--------

Starts at 1 and is reduced for:

- **Blown highlights:** pixels with any channel at 250 or above, beyond
  0.5 % of the image (−0.6 at 10.5 %).
- **Crushed shadows:** pixels whose brightest channel is 5 or below,
  beyond 2 % of the image (−0.4 at 22 %).
- **Overall brightness:** mean luma outside 20–70 % (up to −0.5).

Dark or bright photos taken on purpose (night scenes, high key) score
lower. Use the score to compare photos of the same scene, not across
scenes.

Noise
-----

The noise level is estimated with Immerkær's method in the flattest tenth
of the image, as a standard deviation in brightness levels (0–255). The
score is 0.5 at 5 levels. Clean camera samples measure below 1.5, and
high-ISO samples 3–5.

Overall
-------

The three combined, weighted towards sharpness:
sharpness\ :sup:`0.6` × exposure\ :sup:`0.3` × noise\ :sup:`0.1`.

Versions
--------

The scores are stored with the analyzer version (``technical``, version
1). When the algorithm changes, the version is raised and photos are
rescored in the background.
