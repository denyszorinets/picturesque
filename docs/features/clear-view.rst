Clearing and reloading the view
===============================

Reload folder
-------------

When a folder (a memory card) is culled in place, **Reload folder** in the
bar, or on the empty-folder screen, reads the folder again from scratch:
the view then shows exactly what is on the card now. Picks, ratings and
labels are written to their XMP sidecars first and read back from them, so
they are kept. No file is deleted.

Photos whose RAW and image files are gone, for example after the card was
formatted, also leave the view by themselves each time the folder is
checked. A card that is not inserted keeps its photos in view.

Clear view
----------

In the library, **Clear view…** removes the photos in view from Picturesque
without deleting any file. With filters, only the photos matching them are
removed (a stack counts with all its photos); without, every photo is.

- Their picks, ratings, scores and species in Picturesque are forgotten;
  what was already written to their sidecars stays.
- They do not come back while their files are unchanged; new photos in the
  import folders appear as usual.
- The copies stay in the library folder. Importing the same files again
  would copy them again.
