Library, Catalog and Cache
==========================

Picturesque keeps three kinds of data apart, so it is always clear what is
precious and what can be rebuilt:

The library
   The folder you chose, holding your imported photos, organized by the
   folder structure you picked (:doc:`/reference/path-templates`), with
   the XMP sidecars that carry your ratings and flags. It is ordinary
   files: back it up, browse it, open it in other tools. Picturesque only
   adds files (imports, sidecars) and moves rejects into ``_Rejected`` when
   you ask; it never deletes a photo
   from the library.

The catalog
   A database in the application data folder with what Picturesque knows
   about each photo: capture details, place, files, decisions, analysis
   results, series. It makes browsing and filtering instant. Your
   decisions are also in the sidecars, so other tools see them.

The cache
   Thumbnails and previews, recreated whenever needed. Deleting the cache
   folder only costs time.

Where each lives, and what is safe to delete:
:doc:`/reference/data-locations`.

Imports copy
------------

Importing copies photos from their original folders (memory cards, old
dumps, backups) into the library, verifying every copy, and leaves the
originals untouched (:doc:`/features/import`). Once you have checked the
library, you can format the card or delete the old copies yourself.

Folders culled in place
-----------------------

A folder can also be culled where it is, without importing it
(:doc:`/features/folders`). It gets a catalog of its own, so the library's
catalog is never mixed with it; sidecars are written next to its photos.

One library per catalog
-----------------------

A catalog belongs to one library folder: photo paths are stored relative
to it. Choosing a different library folder is refused once the library
has photos; moving a library to another disk or path is not supported
yet (see :doc:`/troubleshooting`).
