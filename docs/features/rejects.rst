.. _features-rejects:

Moving Rejects
==============

Rejected photos stay in the library until you move them out of the way.
**Move rejects…** above the timeline (shown when there are rejected
photos) says how many photos and files would move and how much space they
take; **Move** then moves every rejected photo, with all its files (RAW,
JPEG and XMP sidecars), into the ``_Rejected`` folder of the library,
keeping its place in the folder structure::

   Clean/2026/2026-10-12/IMG_0042.CR3
   Clean/_Rejected/2026/2026-10-12/IMG_0042.CR3

The photos stay in the catalog, keep their decisions and still appear in
the timeline (filter by *Rejects* to see them). Delete the ``_Rejected``
folder yourself when you are sure; Picturesque never deletes photos.

Undo
----

After a move, **Undo** moves that batch back. Later, **Move rejects…** also
offers to undo the latest move. A photo you un-reject after moving it stays
in ``_Rejected`` until you undo the move.

Safety
------

Files are only renamed within the library, never copied or deleted, so a
failure cannot lose a photo:

- A photo's files move together. If one cannot be moved (for example a
  file with the same name is already in ``_Rejected``), the files already
  moved go back, the photo stays where it was, and the result lists it.
- The catalog is updated only after a photo's files moved; if that fails,
  the files go back.
- If the app stops between moving files and updating the catalog, the next
  move or undo finds the files at their new place and fixes the catalog.
