Import
======

.. note::

   Import is being built in milestone M1. This page describes the parts
   that exist so far and grows with each task.

Import copies photos from your existing folders (**sources**) into the
Clean library, organized by a path template. Sources are never modified
(:doc:`ADR-0008 </architecture/decisions/0008-non-destructive-verified-import>`).

.. _features-import-scanning:

What gets scanned
-----------------

Each source folder is searched recursively. Files are grouped into photos
as described in :doc:`/concepts/photos-and-files`: RAW+JPEG pairs become one
photo, and XMP sidecars travel with their photo.

Skipped:

- Hidden files and folders (names starting with ``.``), including macOS
  ``._`` resource files.
- System, trash and NAS metadata folders: ``@eaDir`` (Synology),
  ``#recycle``, ``$RECYCLE.BIN``, ``System Volume Information``,
  ``__MACOSX`` and ``lost+found``. Matching ignores case.
- The Clean library itself, if it is inside a source folder, so it is
  never imported into itself.
- Links to folders, so a link cannot make the scan loop. Links to files
  are followed.
- Files in unsupported formats (videos, documents). They are counted in
  the import summary.

If you select a folder and also one of its subfolders, the subfolder is
scanned once.

A folder that cannot be read (missing, or no permission) is listed in the
import summary with the reason; the rest of the scan continues. Sidecars
that match no photo are listed too and are not copied.

.. _features-import-metadata:

What metadata is read
---------------------

For each photo, capture metadata is read from the RAW when there is one,
otherwise from the image. In a RAW+JPEG pair, anything the RAW lacks (for
example GPS added later by a phone app) is taken from the JPEG.

.. list-table::
   :header-rows: 1

   * - Field
     - Source
   * - Capture time
     - EXIF ``DateTimeOriginal``, falling back to ``DateTimeDigitized``,
       then ``DateTime``, then the file's modification time. Sub-seconds
       are included when recorded.
   * - UTC offset
     - EXIF ``OffsetTime*`` when recorded. Otherwise, if the photo has GPS
       time, the offset is derived from the difference between camera time
       and GPS time, rounded to 15 minutes. For the file-time fallback, the
       computer's time zone. Many cameras record no offset; the capture
       time is then kept as local time only.
   * - Camera
     - EXIF ``Make`` and ``Model``, shown without repeating the brand
       (``NIKON CORPORATION`` + ``NIKON Z 8`` becomes *Nikon Z 8*).
   * - Lens
     - EXIF ``LensModel``; for RAW files, completed from a lens database
       where the camera records only a lens ID.
   * - Exposure
     - ISO, aperture, shutter speed and focal length.
   * - GPS position
     - EXIF GPS latitude and longitude. ``0, 0`` (written by cameras without
       a fix) is ignored.
   * - Orientation
     - EXIF ``Orientation``.
   * - Dimensions
     - EXIF or the image header. For HEIC and RAW files they are filled in
       later, when the image is first decoded.

RAW files are read with `rawler <https://github.com/dnglab/dnglab>`_,
which understands every common format including CR3, RAF, ORF and RW2;
other images with `kamadak-exif <https://github.com/kamadak/exif-rs>`_.
A file whose metadata cannot be read at all is reported in the import
summary.
