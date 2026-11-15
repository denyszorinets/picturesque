ADR-0018: Bird ID with WingCLIP, a Non-Commercial Model, and eBird Region Lists
==============================================================================

:Status: Accepted (this fork, for personal use); amends
   :doc:`ADR-0006 <0006-local-ai-with-onnx-runtime>`
:Date: 2026-11-14

Context
-------

Bird photographers want to know which species a photo shows, offline and
quickly, and to name stacks of a bird while culling. ADR-0006 requires every
downloadable model's license to allow use in a GPL-3.0 application.

Options considered:

- **WingCLIP-0.3** (int8 ONNX, 39 MB): an image encoder distilled from
  BioCLIP-2, ranked against 11,167 bird species' text embeddings. 86.8 %
  top-1 on NABirds. **CC BY-NC 4.0**: the weights inherit the
  non-commercial terms of their iNaturalist training photos.
- **BioCLIP-2** (MIT): 1.7 GB, no ONNX export, needs the text tower or
  precomputed species embeddings; slower and larger.
- Small single-dataset classifiers (MIT): a few hundred species.
- Automatic bird detection (D-FINE, RT-DETR; Apache-2.0): would need an
  ONNX export of our own and a second model.

Decision
--------

- This personal fork offers **WingCLIP-0.3** as a downloadable model, pinned
  to a commit with SHA-256 checksums, and shows its **CC BY-NC 4.0**
  license and "non-commercial use only" in the model list. This is an
  exception to ADR-0006 for this fork only; a distribution for commercial
  use must not ship or offer it.
- Birds are boxed by the user, or found by **RT-DETR R18** (COCO +
  Objects365, Apache-2.0, the onnx-community ONNX export pinned to a
  commit), keeping its "bird" class. Each box is then identified by
  WingCLIP.
- Regions come from **eBird**'s species list for a region code, asked with
  the user's own API key, only when they press the button, and stored in
  the settings. This is the only network access besides model downloads
  (an exception to the "offline" rule, opt-in and user-initiated).
- The chosen species, several per photo, are stored in the catalog
  (migrations ``0021_bird_species`` and ``0022_bird_species_many``), not
  in XMP sidecars.

Consequences
------------

- Bird ID works offline once the model (and optionally a region list) is
  downloaded; identification takes well under a second on a CPU.
- The model always answers with a bird, even for a box without one; the UI
  says so.
- Upstream changes to ADR-0006 or a new migration ``0021`` would conflict
  with this fork and need renumbering on merge.
