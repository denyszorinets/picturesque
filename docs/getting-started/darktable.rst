.. _getting-started-darktable:

Using Picturesque with darktable
================================

Picturesque writes its decisions to XMP sidecars, which darktable reads:
star ratings, rejects, and colour labels for picks (darktable has no pick
flag). There are two ways to work, depending on whether darktable already
knows the photos.

Cull first, then import into darktable
--------------------------------------

The simplest way, because darktable reads sidecars when it imports.

1. Open the folder in Picturesque: the **folder menu** at the top left →
   **Open folder…** (:doc:`/features/folders`). A memory card works too.
2. Cull: **P** or **↑** picks, **X** or **↓** rejects, **1**–**5** rate.
3. Click **Finish folder…**. Auto-picked photos get a yellow label;
   every sidecar is written.
4. Import the folder into darktable. Picks show a **green** label,
   auto-picks a **yellow** one, rejects are rejected, and stars are
   stars. Filter by colour label in darktable's collections module to
   see only the picks.

.. figure:: /images/screenshots/folder.webp
   :alt: A folder culled in place: the "Using darktable?" note, bursts framed as series with the suggested keepers ringed, and the folder actions in the toolbar.

   A folder culled in place, with the note for darktable users.

To copy only the keepers somewhere else first (for example off a memory
card), use **Copy keepers…**: :doc:`offload-card`.

Photos darktable already knows
------------------------------

When the folder is already in darktable's library (you imported it, maybe
geotagged it, and cull afterwards), darktable keeps its own copy of
ratings and labels in its database, and only looks at sidecars again if
asked.

1. In darktable, open **preferences → storage** and turn on **look for
   updated XMP files on startup**.
2. **Close darktable** while you cull. darktable writes its sidecars
   whenever you change something, and could overwrite decisions made
   meanwhile.
3. Open the folder in Picturesque, cull, and click **Finish folder…**.
   Ratings, tags, geotags and edits darktable stored in the sidecars are
   kept: Picturesque only changes its own fields.
4. Start darktable. It lists the images whose sidecars changed since it
   last wrote them; choose to take the values from the **XMP files**
   rather than from its database, so Picturesque's decisions win.

What darktable shows
--------------------

.. list-table::
   :header-rows: 1

   * - In Picturesque
     - In darktable
   * - Pick
     - Green colour label (:ref:`reference-settings-labels` changes it)
   * - Auto-picked, after **Finish folder…**
     - Yellow colour label
   * - Reject
     - Rejected
   * - 1–5 stars
     - 1–5 stars

Colour labels you set in darktable stay: Picturesque only adds or removes
its own. Each file of a RAW+JPEG pair gets its own sidecar
(``IMG_0042.CR3.xmp`` and ``IMG_0042.JPG.xmp``), because darktable shows
both as separate images.

Troubleshooting
---------------

darktable does not show the decisions
   Check that the sidecars exist next to the photos (the toolbar says
   **Saved to XMP ✓**; in the photo view, **Show in folder**). For photos
   darktable knew already, check the preference above and restart
   darktable.

Decisions disappeared after using darktable
   darktable was running while you culled and wrote its own sidecars over
   them. Close darktable, open the folder in Picturesque again, and
   change any decision (or **Finish folder…**) to write them again:
   Picturesque still has them.
