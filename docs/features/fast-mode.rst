Fast mode
=========

Fast mode shows photos as fast as possible, for culling a memory card right
away. Turn it on in **Settings → Fast mode**, then open the card from the
folder menu to cull it in place (see :doc:`folders`): nothing is copied.

What fast mode does:

- Thumbnails come from the JPEG preview inside each RAW file, decoded at
  the size needed (this shrink-on-load applies in every mode).
- On a card culled in place, each file is identified by a fingerprint of
  its size and first and last mebibyte instead of being read in full.
  **Copy keepers…** still copies with a full checksum and records it, and
  **Clear folder…** checks copies against that checksum before deleting.
- Only the perceptual hash that series need is computed, from the grid
  thumbnail, so photos still group into series (stacks) by time and look.
  No preview is rendered until a photo is opened.
- There are no quality or AI scores, so nothing is auto-picked, and the
  best photo of a series is not chosen by score.

The library bar shows **Fast mode**; **Score photos** turns it off, and
everything fast mode skipped is analyzed in the background.
