.. _features-export:

Exporting
=========

**Export…** above the timeline copies photos to a folder, to share,
print or edit them elsewhere. The library is not changed.

What to export
--------------

- **Picks** (default): every photo flagged as a pick.
- **Photos in view**: the photos the timeline shows with the current
  filters, or the results of a text search.
- **Selected**: the photos selected in the timeline.

Photos are exported in capture order.

Files
-----

- **Originals, unchanged**: the photo's files exactly as imported. With
  **RAW files too**, both files of a RAW+JPEG pair are copied; with **XMP
  sidecars**, the sidecars holding your ratings and flags come along, so
  other tools see them.
- **JPEG**: one JPEG per photo, rendered from the JPEG or RAW file,
  upright, at **full size** or with its longest edge at most 4096, 2048 or
  1024 pixels (never enlarged), at the chosen quality (default 90). The
  photo's metadata (EXIF capture details, XMP, colour profile) is kept.

Names
-----

File names follow a path template, like imports
(:doc:`/reference/path-templates`): **Original names** (``{name}.{ext}``),
**Date and number** (``{date}_{seq}.{ext}``, numbered in export order),
**Folders by date** (``{year}/{date}/{name}.{ext}``) or your own.
Existing files are never overwritten: a name already taken gets
" (2)", " (3)" and so on, the same for every file of a photo.

Verification
------------

Every exported file is checked:

- Originals are hashed while copied and must match the hash recorded at
  import, so a file that changed or got damaged in the library is
  reported instead of exported; the written copy is then read back and
  hashed again.
- JPEGs are decoded again after writing.

A photo that fails leaves no files behind and is listed after the export
with the reason. The dialog shows progress and can cancel; the options are
remembered for next time. The export folder must exist and lie outside
the library.
