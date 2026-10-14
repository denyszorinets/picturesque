Import
======

For a step-by-step walkthrough, see :doc:`/getting-started/first-import`.
The same operations are available to developers through the API
(:ref:`import commands <reference-api-import>`).

Import copies photos from your existing folders (**import folders**, or
*sources*) into the Clean library, organized by a path template. Import
folders are never modified
(:doc:`ADR-0008 </architecture/decisions/0008-non-destructive-verified-import>`).
Importing runs in the background, photo by photo
(:doc:`ADR-0012 </architecture/decisions/0012-continuous-import-pipeline>`).

.. _features-import-steps:

How importing works
-------------------

Add a folder in **Activity** (in the desktop app with a folder picker; in a
browser during development, by typing the path). Picturesque remembers it
and scans it: every photo found is queued. One photo at a time then goes
through these steps, shown for each photo in **Activity → Photos**:

.. list-table::
   :header-rows: 1

   * - Step
     - What happens
   * - Found
     - The photo was found in the folder and is waiting.
   * - Read
     - Date, camera, lens, GPS and place are read
       (:ref:`features-import-metadata`), and any decision in an existing
       sidecar. The photo is checked for duplicates.
   * - Copied, Verified
     - Each file is copied to a temporary file next to its destination,
       the copy is read back and compared by hash with the original, and
       only then given its real name. A RAW+JPEG pair and its sidecars are
       imported together: if any file fails, the copies already made for
       that photo are removed.
   * - Catalogued
     - The photo is in the catalog and appears in the **Library** right
       away.
   * - Thumbnail
     - Its grid image has been rendered.
   * - Analyzed
     - Background analysis (quality, similarity, AI models when downloaded)
       has run; see :ref:`features-library-analysis`.

A photo that fails says why (for example, a file that cannot be read) and
the rest continue. Fix the cause and click **Retry failed**. The **Log**
records every photo imported, skipped or failed, folders found and
problems, newest first; the newest 20,000 entries are kept.

Importing waits while no library folder is set, and continues once one is
chosen.

Duplicates
~~~~~~~~~~

A photo is skipped as a duplicate if one of its RAW or image files has
exactly the same content as a file already in the library, for example
the same card copied to two folders, or a backup of photos already
imported. Content is compared with a BLAKE3 hash, but only for files whose
size matches a file in the library, so most photos are read only once.

Stopping and resuming
~~~~~~~~~~~~~~~~~~~~~

Quitting the app, or a crash, stops importing between steps. Folders and
the queue are kept in the catalog: when the app starts again, the photo
that was being copied starts over and the rest follow. A copy left at its
destination by the interrupted attempt is recognized by its content and
reused rather than copied again.

Safety
~~~~~~

- Source files are only ever read.
- An existing file in the library is never overwritten. If another file
  already uses a name, the photo gets a free name such as
  ``IMG_0042 (2).CR3``.
- A photo whose files changed after they were found, or while they were
  being copied, is not kept; it is imported again with its new content
  when the folder is next scanned.
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
- Files in unsupported formats (videos, documents).

If you add a folder and also one of its subfolders, each photo is still
imported once.

A folder that cannot be read (missing, or no permission) is reported in
the log with the reason, and for an import folder itself also under
**Import folders**; the rest of the scan continues. Sidecars that match no
photo are not copied.

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
A photo whose metadata cannot be read at all fails at *Read*, with the
reason.

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
