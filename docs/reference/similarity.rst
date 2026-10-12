Visual similarity
=================

Background analysis
(:doc:`/architecture/decisions/0011-resumable-background-analysis`) stores
a perceptual hash of every photo (analyzer ``similarity``, version 1).
Series detection uses it to tell whether photos taken close together show
the same scene. The code is in :repo:`crates/core/src/similarity.rs`.

pHash
-----

The photo's preview is shrunk to a 32 × 32 brightness thumbnail and
transformed with a DCT. The 8 × 8 lowest frequencies are compared with
their median (leaving out the average brightness), giving 64 bits that
describe the photo's overall layout. Stored as 16 hex digits:
``{"phash": "c3a1f0e07c3c1e0f"}``.

The **distance** between two photos is the number of differing bits
(0–64). **Similarity** is ``1 − distance / 64``: 1 for identical layouts,
about 0.5 for unrelated photos.

.. list-table::
   :header-rows: 1

   * - Distance
     - Meaning
   * - 0–10
     - Near-duplicates: the same shot, possibly resized, re-encoded,
       brightened, slightly shifted or noisier.
   * - 11–16
     - Similar: often the same scene with some movement.
   * - over 16
     - Different scenes. Unrelated camera samples measure 18 bits apart
       or more; random photos about 32.

Measured on test images: resizing, +25 brightness and added noise change
0 bits, an 8 px shift 2 bits. All eight EXIF orientations of a photo hash
alike once decoded upright. The same photo saved as JPEG, PNG, TIFF, WebP
and RAW hashes identically.

Limits
------

- The hash ignores colour; two layouts with similar brightness but
  different colours look alike.
- It compares layout, not content: the same subject from a different
  angle is not similar. AI embeddings (:issue:`43`) will cover that.
- dHash, which compares neighbouring cells, was considered and dropped:
  it flips many bits in flat areas such as skies (resizing alone changed
  20 of 64 bits on a test scene).
