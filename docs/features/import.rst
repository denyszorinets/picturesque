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
