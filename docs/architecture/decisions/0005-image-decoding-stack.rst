ADR-0005: Image Decoding Stack
==============================

:Status: Accepted
:Date: 2026-10-12

Context
-------

Picturesque must read JPEG, HEIC/HEIF, camera RAW (CR2, CR3, NEF, ARW, RAF,
ORF, RW2, PEF, DNG and others), PNG, TIFF and WebP on Linux, macOS and
Windows. It needs:

- Fast thumbnails and previews for thousands of photos.
- Correct EXIF orientation.
- Pixel data at reduced resolution for technical analysis and AI models.
- Broad RAW camera coverage. Most RAW files embed a large JPEG preview,
  which is good enough for culling and far faster than demosaicing.

The app is licensed GPL-3.0; libraries must be compatible. Native libraries
must be bundled in installers for all three platforms.

Decision
--------

Decode with **libvips** and **LibRaw**, both wrapped in
``picturesque-infra`` behind an image-decoder port.

RAW files
   Extract the embedded preview with **LibRaw** through a small FFI wrapper
   of our own (``libraw_r``, about a dozen functions), then let libvips
   shrink it. When a file has no usable preview, fall back to libvips'
   ``dcrawload`` (LibRaw-based full decode).

JPEG, PNG, TIFF, WebP, HEIC
   **libvips** ``thumbnail``, which uses shrink-on-load and applies EXIF
   orientation.

HEIC
   Requires libheif with the **libde265** HEVC decoder plugin on every
   platform; it is not part of default builds (see Consequences).

Rust bindings
   Our own FFI for both, each a small module and the only places in
   ``picturesque-infra`` that allow ``unsafe`` code:

   - **LibRaw** (``libraw_r``, about a dozen functions), because the
     published LibRaw crates are unmaintained or little used.
   - **libvips** (the dozen operations the app uses, through libvips' C API
     with name/value option lists). This replaced the ``libvips`` crate
     (2.3) in October 2026: its bindings are generated once on 64-bit Linux
     and do not compile on Windows, where C ``long`` and the C library's
     structures differ. An image read from memory keeps that memory alive,
     so no caller has to.

Alternatives Considered
-----------------------

A: libvips for everything, RAW included
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

libvips 8.18 includes ``dcrawload``, a LibRaw-based RAW loader.

Advantages:

- One library and one code path.

Disadvantages:

- ``dcrawload`` always demosaics the full sensor image: 0.3–3.2 s per RAW
  thumbnail, 5–30× slower than extracting the embedded preview.

C: Pure Rust (``image``, ``rawler``) with ``libheif-rs``
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Advantages:

- No libvips or LibRaw to bundle.

Disadvantages:

- 3–5× slower than libvips on large JPEG, TIFF and WebP files (no
  shrink-on-load).
- ``rawler``'s preview choice is inconsistent: sometimes a small preview,
  sometimes a slow full decode (about 800 ms for Olympus files).
- HEIC still needs native libheif and libde265.

Decision Criteria
-----------------

Thumbnail speed, RAW coverage, orientation correctness, licensing, bundling
effort on Windows and macOS.

Rationale
---------

Embedded-preview extraction makes RAW thumbnails as cheap as JPEG
thumbnails, and the previews are mostly full resolution, so the loupe and
the analysis models can use them too. libvips is the fastest option for
every other format. Both libraries are LGPL (LibRaw is LGPL-2.1 or CDDL),
compatible with GPL-3.0. The official libvips Windows build already bundles
LibRaw and libheif, which reduces bundling to adding libde265.

Consequences
------------

Positive
~~~~~~~~

- RAW thumbnails in 20–106 ms from previews of up to 9504×6336.
- Every camera in the test corpus is covered for culling, including
  formats whose sensor data cannot be decoded.

Negative
~~~~~~~~

- Three native libraries (libvips, LibRaw, libheif with libde265) to ship
  per platform.
- An ``unsafe`` FFI module to maintain.
- ``libvips`` crate caveats: ``VipsImage`` is not thread-safe (keep images
  on one thread per job), error details live in the libvips error buffer
  rather than the returned error, and libvips' operation cache must be
  sized deliberately. Found while implementing :issue:`22`: images made
  with ``new_from_buffer`` do not copy the buffer and are decoded lazily,
  so the buffer must outlive every use of the image (a dropped buffer
  produced corrupt output); and ``thumbnail_image`` auto-rotates from EXIF
  carried by the image unless ``no_rotate`` is set, which rotated RAW
  previews twice.

Risks
~~~~~

- **HEVC.** Fedora and the default libvips Windows build ship no HEVC
  decoder, for patent reasons. Picturesque must ship libde265 itself
  (installers) or depend on the system providing it (RPM Fusion's
  ``libheif-freeworld`` on Fedora). HEVC patent licensing may matter for
  distribution in some jurisdictions; review before the first release
  (:issue:`52`).
- **Nikon high-efficiency NEF** (Z 8, Z 9, Z f): neither LibRaw 0.22 nor
  rawler decodes the sensor data. The embedded preview works, so culling
  works; only a future full-resolution development path is affected.
- **CI on Ubuntu 24.04** ships older libvips and LibRaw than Fedora 44;
  infrastructure tests may need newer builds there. In particular
  ``dcrawload`` (the fallback for RAW files without a preview) needs
  libvips 8.17 or newer.
- **Windows builds** need libvips and LibRaw import libraries; until that
  is set up, Windows CI checks only the crates without native libraries.

Follow-up
~~~~~~~~~

- Implement the decoder port, LibRaw FFI and preview selection (choose the
  smallest embedded preview at least as large as the requested size) with
  the thumbnail cache (:issue:`22`).
- Add the HEVC decoder to the devcontainer.
- Bundle libvips (built with libde265), LibRaw and libheif in installers
  (:issue:`52`). Done; macOS dylib bundling is verified on a real Mac, and
  Windows ships without an HEVC decoder (:issue:`135`).

Validation
----------

Spike: :repo:`spikes/decoding/` (``scripts/fetch_corpus.py`` downloads the
corpus; ``cargo run --release -- corpus`` reproduces the tables). Measured
on an AMD Ryzen 7 PRO 7840HS (16 threads), Fedora 44, libvips 8.18.3,
LibRaw 0.22.2, median of three warm runs, libvips cache disabled, 512 px
thumbnails.

Corpus: 16 RAW files from raw.pixls.us (CC0) covering Canon CR2/CR3 (incl.
dual-pixel), Nikon NEF (incl. high efficiency), Sony ARW (uncompressed and
lossless), Fujifilm RAF, Olympus/OM ORF, Panasonic RW2, Pentax PEF, Leica,
iPhone and Pixel DNG; a 9504×6336 photo in JPEG, PNG, TIFF and WebP; the
eight EXIF orientations; libheif's example HEIC.

.. list-table:: RAW thumbnail time (ms)
   :header-rows: 1

   * - Camera
     - A: vips ``dcrawload``
     - B: LibRaw preview + vips
     - C: rawler + ``image``
   * - Canon EOS 5D Mark IV (CR2)
     - 979
     - 49
     - 128
   * - Canon EOS R6 Mark III (CR3)
     - 680
     - 61
     - 149
   * - Nikon D850 (NEF)
     - 1408
     - 67
     - 201
   * - Nikon Z 8 (high-efficiency NEF)
     - fails
     - 66
     - 206
   * - Sony A7R V (ARW, lossless)
     - 1830
     - 106
     - 16 (small preview)
   * - Fujifilm X-T5 (RAF)
     - 3195
     - 52
     - 88
   * - OM System OM-3 (ORF)
     - 817
     - 26
     - 809
   * - Google Pixel 2 XL (DNG, no preview)
     - 304
     - no preview
     - 349

All 16 files: B extracted a JPEG preview from 15 (all but the Pixel DNG,
which A decodes). LibRaw and rawler both decode the sensor data of 15
files; both fail on the Nikon Z 8 high-efficiency NEF.

.. list-table:: Other formats, 9504×6336 source (ms)
   :header-rows: 1

   * - Format
     - libvips ``thumbnail``
     - ``image`` decode + thumbnail
   * - JPEG (11.5 MB)
     - 99
     - 293
   * - TIFF (181 MB)
     - 55
     - 248
   * - PNG (147 MB)
     - 479
     - 534
   * - WebP (6.3 MB)
     - 236
     - 1146

Orientation: both libvips and ``image`` (with ``apply_orientation``)
rendered all eight EXIF orientations upright.

HEIC: with Fedora's packages, libheif reports no HEVC decoder and both
libvips and ``libheif-rs`` fail. With RPM Fusion's ``libheif-freeworld``
(libde265 1.0.18) both succeed: libvips 75 ms, ``libheif-rs`` 55 ms
(1280×854).

Bundling: the official libvips Windows build (``vips-dev-x64-all``)
includes LibRaw 0.22.2 and libheif, with libde265 available as a build
option. vcpkg's libvips port has no HEIF or RAW support, so it is not a
shortcut.

References
----------

- `libvips <https://www.libvips.org/>`_
- `LibRaw <https://www.libraw.org/>`_
- `libheif <https://github.com/strukturag/libheif>`_
- `libvips Windows builds <https://github.com/libvips/build-win64-mxe>`_
- `raw.pixls.us <https://raw.pixls.us/>`_
- :doc:`0006-local-ai-with-onnx-runtime` (models consume the previews)
