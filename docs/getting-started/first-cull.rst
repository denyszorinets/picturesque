Your First Cull
===============

After :doc:`first-import`, this tour goes from a full library to the
photos worth keeping. Every step works with the keyboard
(:doc:`/reference/keyboard-shortcuts`).

1. Look through the timeline
----------------------------

The **Library** tab shows every photo, newest day first. Scroll, or move
with the arrow keys; **Enter** opens a photo full screen in the loupe,
where **Z** zooms to 100% to check focus (:doc:`/features/library`).

Photos taken in a burst are framed together as a **series**; the green
border and ★ mark the best photo of each series (:doc:`/features/series`).
**Stack series** shows only the best of each, to see the shoot at a
glance.

2. Let the AI help (optional)
-----------------------------

Under **Settings → AI models**, download the CLIP image encoder
and the face models. Picturesque then scores every photo in the
background: how good it looks, whether eyes are closed, whether faces
are sharp (:doc:`/features/ai`). Series pick their best photo with these
signals, and **Best quality first** sorts the whole library by score.

3. Pick and reject
------------------

- **P** picks, **X** rejects, **U** unflags; **1** to **5** give stars.
- In the loupe, the keys apply to the photo shown; in the timeline, to
  every selected photo (select with **Shift+click**, **Ctrl+click** or
  **Shift+arrows**).
- **Ctrl+Z** undoes.

Decisions are saved right away, and written to XMP sidecars so
Lightroom, darktable and digiKam see them.

4. Compare close calls
----------------------

Select a photo of a series and press **C**: the series opens side by side,
best-ranked first (:doc:`/features/compare`). **Z** zooms every photo to
the same spot, **Shift+arrows** pan them together; pick the keeper with
**P**, reject the rest with **X**, or press **B** to make another photo the
series' best.

5. Narrow down
--------------

The **Filters** panel narrows the timeline: only unflagged photos, three
stars or more, a date, a place, a camera, sharp photos only, no closed
eyes. The search box finds photos by description (*kids on the beach*)
once the text model is downloaded (:ref:`features-ai-search`).

6. Deliver and tidy up
----------------------

- **Export…** copies the picks to a folder, as originals or resized
  JPEGs, every file verified (:doc:`/features/export`).
- **Move rejects…** moves the rejected photos into ``_Rejected`` inside the
  library, out of the way and still undoable (:doc:`/features/rejects`).
