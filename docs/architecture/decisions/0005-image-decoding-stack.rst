ADR-0005: Image Decoding Stack
==============================

:Status: Proposed
:Date: 2026-10-12

Context
-------

Picturesque must read JPEG, HEIC/HEIF, camera RAW (CR2, CR3, NEF, ARW, RAF,
DNG and others), PNG, TIFF and WebP on Linux, macOS and Windows. It needs:

- Fast thumbnails and previews for thousands of photos.
- Correct orientation.
- Pixel data for technical analysis and AI models, at reduced resolution.
- Broad RAW camera coverage. Most RAW files embed a full-size JPEG preview,
  which is good enough for culling and far faster than demosaicing.

The app is licensed GPL-3.0; libraries must be compatible. Native libraries
must be bundled in installers for all three platforms.

Decision
--------

Proposed, pending the decoding spike (:issue:`10`):

- **libvips** for JPEG, HEIC (via libheif), PNG, TIFF and WebP: decoding,
  shrink-on-load thumbnails, orientation.
- **LibRaw** for RAW files: extract the embedded preview first; demosaic
  only when no usable preview exists.
- Both are wrapped in ``picturesque-infra`` behind an image-decoder port.

Alternatives Considered
-----------------------

Pure Rust (image, rawler, libheif-rs)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Advantages:

- Mostly pure Rust; simpler builds and bundling (HEIC still needs libheif).
- No large native dependency tree.

Disadvantages:

- ``rawler`` supports fewer cameras than LibRaw.
- ``image`` is slower for thumbnails than libvips' shrink-on-load.

libvips for everything, RAW included
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Advantages:

- One library.

Disadvantages:

- libvips reads RAW only through optional loaders that are not reliably
  present across platform builds.

Decision Criteria
-----------------

Format coverage (RAW cameras especially), thumbnail speed, licensing,
bundling effort on Windows and macOS, build complexity.

Rationale
---------

To be completed with spike findings. The working assumption is that LibRaw's
camera coverage and libvips' speed outweigh the cost of bundling native
libraries, which the spike must confirm is manageable on Windows.

Consequences
------------

Positive
~~~~~~~~

- Broad format coverage and fast thumbnails.

Negative
~~~~~~~~

- Native libraries (LGPL) must be built or bundled per platform.

Risks
~~~~~

- Fedora's libheif decodes HEVC through ffmpeg, which may lack an HEVC
  decoder in Fedora's build; HEIC support must be verified on real iPhone
  files.
- Bundling libvips' dependency tree on Windows may be heavy.

Follow-up
~~~~~~~~~

- Run the spike (:issue:`10`) and accept, revise or reject this ADR.

Validation
----------

The spike measures thumbnail time per format, embedded-preview extraction
rate, camera coverage on raw.pixls.us samples, and bundling steps per
platform. Findings will be recorded here.

References
----------

- `libvips <https://www.libvips.org/>`_
- `LibRaw <https://www.libraw.org/>`_
- `raw.pixls.us <https://raw.pixls.us/>`_
