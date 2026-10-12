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
