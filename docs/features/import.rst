Import
======

For a step-by-step walkthrough of the import screen, see
:doc:`/getting-started/first-import`. The same operations are available
to developers through the API (:doc:`/reference/api`).

Import copies photos from your existing folders (**sources**) into the
Clean library, organized by a path template. Sources are never modified
(:doc:`ADR-0008 </architecture/decisions/0008-non-destructive-verified-import>`).

.. _features-import-steps:

How an import works
-------------------

1. **Plan.** Choose one or more source folders (in the desktop app with a
   folder picker; in a browser during development, by typing the path). Picturesque scans them,
   reads each photo's metadata, finds duplicates and works out where every
   file will go. This runs in the background with progress, and nothing is
   copied yet.
2. **Review.** The plan lists every photo with its destination, the
   duplicates that will be skipped and any problems (unreadable folders or
   files, sidecars without a photo).
3. **Copy.** Start the import. Each file is copied to a temporary file
   next to its destination, the copy is read back and compared by hash
   with what was read from the source, and only then is it given its real
   name. A RAW+JPEG pair and its sidecars are imported together: if any of
   its files fails, the copies already made for that photo are removed, and
   the photo is reported as failed while the rest of the import continues.
4. **Done.** Imported photos are in the catalog and the library; the plan
   now shows what was imported, skipped and failed.

Duplicates
~~~~~~~~~~

A photo is skipped as a duplicate if one of its RAW or image files has
exactly the same content as a file already in the library, or as a file
earlier in the same import (for example the same card copied to two
folders). When the same photo is in several of the chosen folders, the
copy in the folder listed first is imported. Content is compared with a
BLAKE3 hash, but only for files whose size matches another file, so
planning does not read every file twice.

Cancelling and resuming
~~~~~~~~~~~~~~~~~~~~~~~

Planning and copying can be cancelled. Copying stops between photos, so a
photo is never left half-copied. A cancelled import, or one interrupted
because the app was closed or crashed, can be started again: it continues
with the photos not yet imported, and a file already copied with the same
content is recognized instead of being copied again.

Safety
~~~~~~

- Source files are only ever read.
- An existing file in the library is never overwritten. If a file with
  different content already sits at a destination, that photo fails rather
  than replacing it.
- If a source file changes after planning (or while it is being copied),
  it is not imported; plan the import again.
- A copy that does not match the original is retried once; if it fails
  again, the destination disk may be failing and the photo is reported.

.. _features-import-watching:

Watched folders
---------------

An import folder is *watched* unless you turn it off. Picturesque then
imports new and changed photos in it without being asked:

- When something in the folder changes, it is scanned once the folder has
  been quiet for 2 seconds, so files still being copied in are complete.
- Every 5 minutes (:ref:`reference-settings-import`), in case a change was
  missed, for example on a network share that does not report changes.
- When the app starts.

A scan only queues photos that are new, or whose files changed size or
modification time since they were imported, skipped or failed; the rest are
not read again. A folder that cannot be watched (for example when the
system limit on watches is reached) is still scanned periodically, and the
pipeline log says so.

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

.. _features-import-places:

Place names
-----------

Photos with GPS coordinates get a place name (city, region and country)
from a dataset bundled with Picturesque. No network access is used and
locations never leave your computer
(:doc:`ADR-0007 </architecture/decisions/0007-offline-reverse-geocoding>`).

The place is chosen like this:

- Every town and city "claims" the area around it, larger for bigger
  places (about 2 km for a small town, 7 km for Paris, 16 km for Tokyo,
  at most 20 km).
- If the photo is inside one or more claims, the most populous of those
  places names it. A photo taken in Shibuya is named *Tokyo*, and one taken
  in Brooklyn *New York City*, rather than the district.
- Otherwise the nearest town is used, if it is within 50 km.
- Farther than that (at sea, in wilderness), the photo gets no place.

Names are the local names used by GeoNames, with accents (*Reykjavík*,
*Oía*). Nearest-town matching can be wrong close to borders and in
sparsely populated areas.

The dataset is GeoNames ``cities500``: about 225,000 places with at least
500 inhabitants. See :doc:`/reference/third-party-data`.
