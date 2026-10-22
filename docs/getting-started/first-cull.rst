Your First Cull
===============

After :doc:`first-import`, this tour goes from a full library to the
photos worth keeping. Every step works with the keyboard
(:doc:`/reference/keyboard-shortcuts`).

The first time the library has photos, a short **culling tour** in the
app points at each of these controls. Use **Next** or the arrow keys,
or **Skip tour**; **Show the culling tour** at the top of **Settings**
plays it again.

1. Look through the timeline
----------------------------

The **Library** tab shows every photo, newest day first. Scroll, or move
with the arrow keys; **Enter** opens a photo full screen in the loupe,
where **Z** zooms to 100% to check focus (:doc:`/features/library`).

Photos taken in a burst are framed together as a **series**; the green
border and ★ mark the best photo of each series (:doc:`/features/series`).
**Stack series** shows only the best of each, to see the shoot at a
glance.

.. figure:: /images/screenshots/library.webp
   :alt: The library: a day in the Dolomites, bursts of a misty forest and two mountain lakes framed as series, the best frame of each picked and starred, the weakest frames rejected and dimmed, and the filters on the left.

   The timeline with series, picks (green), rejects (dimmed) and the
   filters.

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
  In the loupe, **↑** picks and **↓** rejects, next to **← →**.
- In the loupe, the keys apply to the photo shown; in the timeline, to
  every selected photo (select with **Shift+click**, **Ctrl+click** or
  **Shift+arrows**).
- **Ctrl+Z** undoes.
- **?** shows every shortcut (:doc:`/reference/keyboard-shortcuts`).

Decisions are saved right away, and written to XMP sidecars so
Lightroom, darktable and digiKam see them.

.. figure:: /images/screenshots/loupe.webp
   :alt: The loupe: a portrait rated five stars, with the pick, reject and star controls on top and the info panel explaining its score: best of its series of 6, eyes open, face in focus, sharp, well exposed, looks appealing.

   The loupe, with the info panel (**I**) explaining the score.

4. Compare close calls
----------------------

Select a photo of a series and press **C**: the series opens side by side,
best-ranked first (:doc:`/features/compare`). **Z** zooms every photo to
the same spot, **Shift+arrows** pan them together; pick the keeper with
**P**, reject the rest with **X**, or press **B** to make another photo the
series' best.

.. figure:: /images/screenshots/compare.webp
   :alt: Compare: four frames of a fox burst side by side, each with its score, the sharpest marked Best.

   Compare: four frames of a series, best-ranked first.

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
