Library
=======

The **Library** tab shows every imported photo as a timeline, newest
first, grouped by the day it was taken. Each day starts with a header
showing the date, the places photos were taken that day, and how many
photos it has. Photos without a capture date come last, under *No date*.

The timeline stays fast with tens of thousands of photos: its full height
is known from the number of photos per day, only the rows on screen (and
a margin around them) are drawn, and photo details are loaded page by
page as you scroll. New photos appear as soon as an import completes.

Tiles
-----

Each tile shows the whole photo (never cropped), with:

- **RAW** when the photo has a RAW file (the JPEG of a RAW+JPEG pair is
  shown),
- a green **✓** for picks and a red **✕** for rejects (rejected photos are
  dimmed),
- its star rating.

Hover a tile for its file name, capture time, camera and place.

Selecting photos
----------------

.. list-table::
   :header-rows: 1

   * - Action
     - Selection
   * - Click
     - Only that photo.
   * - Shift+click
     - Every photo from the last clicked one to this one, across days.
   * - Ctrl+click (⌘+click on macOS)
     - Adds or removes that photo; Shift+Ctrl/⌘+click adds a range.

The number of selected photos is shown above the timeline.

.. _features-library-loupe:

Loupe
-----

Double-click a photo, or press **Enter** on the selected one, to view it
full screen. The photo is fitted to the window; zoom to 100% to check
focus.

.. list-table::
   :header-rows: 1

   * - Key
     - Action
   * - ← / →
     - Previous / next photo
   * - Z, or click the photo
     - Toggle 100% zoom. Clicking zooms in on the spot you clicked; drag to
       move around, click again to fit.
   * - I
     - Show or hide the information panel
   * - Esc
     - Leave 100% zoom, then close the loupe

The information panel shows when and where the photo was taken (with the
UTC offset when known), camera, lens, exposure (aperture, shutter speed,
ISO, focal length), size, GPS position and the photo's files with their
sizes. Closing the loupe leaves the last viewed photo selected in the
timeline.

The fitted view uses a 2048-pixel preview. 100% zoom uses a full-size
image, created the first time you zoom into a photo and then cached
(:doc:`/reference/data-locations`).

Culling
-------

Mark photos as **picks** or **rejects** and give them **star ratings**
with the keyboard (:doc:`/reference/keyboard-shortcuts`): P, X and U for
pick, reject and unflag, 1–5 for stars and 0 to clear. In the timeline the
keys apply to every selected photo; in the loupe, to the photo shown.

Decisions are saved immediately. **Undo** and **Redo** (above the
timeline, or Ctrl+Z / Ctrl+Shift+Z) step back and forward through the
last 200 changes; undoing restores each photo's previous flag and rating,
even when a change covered photos that had different ones.

Filters
-------

The **Filters** panel beside the timeline (toggle it with the
**Filters** button) narrows the timeline to matching photos:

.. list-table::
   :header-rows: 1

   * - Filter
     - Shows
   * - Flag
     - Picks, unflagged and/or rejects (any of the chosen ones)
   * - Rating
     - Photos with at least the chosen number of stars
   * - Date
     - Photos taken from / until a day (both included)
   * - Place
     - A country, or one city in it
   * - Camera, Lens
     - Photos taken with that camera or lens

Filters combine: *Picks* with *3★+* in *Lisbon* shows picked photos of
three stars or more taken in Lisbon. Every option shows how many photos
in the library have it. Active filters appear as chips above the
timeline; remove one with its ✕, or all with **Clear all**. Filters are
kept while the app is open.

The timeline, day headers and counts all reflect the filters. A photo you
change while filtering (for example rejecting it while showing only
picks) stays in view until the filters change, so it does not jump away
mid-cull.
