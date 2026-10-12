Photos and Files
================

A **photo** in Picturesque is one captured image. It can consist of
several **files** (assets) that share a base name in the same folder:

.. list-table::
   :header-rows: 1

   * - Role
     - Formats
     - Notes
   * - RAW
     - Camera RAW files: CR2, CR3, NEF, ARW, RAF, ORF, RW2, PEF, DNG and
       other vendor formats
     - At most one per photo.
   * - Image
     - JPEG, HEIC/HEIF/AVIF, PNG, TIFF, WebP
     - At most one per photo.
   * - Sidecar
     - XMP
     - Metadata written by editing tools; any number.

Every photo has at least one RAW or image file. Other files, such as
videos and text files, are ignored.

RAW+JPEG pairs
--------------

Cameras set to record RAW+JPEG write two files with the same base name,
for example ``IMG_0042.CR3`` and ``IMG_0042.JPG``. Picturesque treats them
as **one photo**: capture metadata is read from the RAW, which usually
records the most detail, and the JPEG is used for display. Names are
compared without regard to case, so ``img_0042.cr3`` pairs with
``IMG_0042.JPG``.

If a base name has more than one RAW or more than one image (for example
``IMG_0042.CR2`` and a converted ``IMG_0042.DNG``), the pairing is
ambiguous and each file becomes a separate photo.

Sidecars
--------

Editing tools name XMP sidecars in one of two ways, and both are
recognized:

- ``IMG_0042.xmp`` (Lightroom style) belongs to the photo with base name
  ``IMG_0042``.
- ``IMG_0042.CR3.xmp`` (darktable style) belongs to that exact file.

A sidecar that matches no photo is reported during import and not copied.

Identity
--------

Each file is identified by a BLAKE3 hash of its contents. Importing a file
whose contents are already in the library is skipped as a duplicate,
whatever its name
(:doc:`ADR-0008 </architecture/decisions/0008-non-destructive-verified-import>`).
