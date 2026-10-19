Library
=======

The **Library** tab shows every imported photo as a timeline, newest
first, grouped by the day it was taken. Each day starts with a header
showing the date, the places photos were taken that day, and how many
photos it has. Photos without a capture date come last, under *No date*.

The timeline stays fast with tens of thousands of photos: its full height
is known from the number of photos per day, only the rows on screen (and
a margin around them) are drawn, and photo details are loaded page by
page as you scroll. While importing, new photos appear as each one is
copied.

Tiles
-----

Each tile shows the whole photo (never cropped), with:

- **RAW** when the photo has a RAW file (the JPEG of a RAW+JPEG pair is
  shown),
- a green **✓** for picks and a red **✕** for rejects (rejected photos are
  dimmed),
- its star rating,
- a green border and **★** on the best photo of a series; the photos of a
  series are framed together (:ref:`features-series`).

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
   * - The round checkbox on a tile (shown on hover, and on every tile
       while several are selected)
     - Adds or removes that photo, without a modifier key.

With two or more photos selected, **Compare** (or **C**) shows them side
by side (:doc:`/features/compare`).

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
   * - P, X, U, 0 – 5
     - Pick, reject, unflag, rate (:ref:`features-library-culling`)
   * - Shift+P, Shift+X, Shift+U
     - Decide, then show the next photo
   * - Esc
     - Leave 100% zoom, then close the loupe

The bar at the top of the loupe shows the photo's decision and does the
same with the mouse: **Pick**, **Reject**, **Unflag** and the stars
(click the current rating again to clear it). With **Auto-advance** ticked,
every decision (button or key) moves on to the next photo, so you can go
through a shoot with P and X alone; the choice is remembered.

The information panel shows when and where the photo was taken (with the
UTC offset when known), camera, lens, exposure (aperture, shutter speed,
ISO, focal length), size, GPS position and the photo's files with their
sizes. Closing the loupe leaves the last viewed photo selected in the
timeline.

The fitted view uses a 2048-pixel preview. 100% zoom uses a full-size
image, created the first time you zoom into a photo and then cached
(:doc:`/reference/data-locations`).

.. _features-library-culling:

Culling
-------

Mark photos as **picks** or **rejects** and give them **star ratings**
with the keyboard (:doc:`/reference/keyboard-shortcuts`): P, X and U for
pick, reject and unflag, 1–5 for stars and 0 to clear. In the timeline the
keys apply to every selected photo; in the loupe, to the photo shown.
Picked photos have a green ring in the timeline. Hold
**Shift** (Shift+P, Shift+X, Shift+U) to move on to the next photo
afterwards, in the loupe or with one photo selected in the timeline.
In the loupe and in compare, **↑** picks and **↓** rejects too (with
**Shift** to move on), so a whole cull fits on the arrow keys.

**Picks** counts only photos you marked. The app's suggestions are
separate: see :ref:`features-library-auto-pick`.

.. _features-library-auto-pick:

Auto-pick
~~~~~~~~~

Once photos are analyzed, the app marks the ones it thinks are keepers as
**Auto-picked** (a green *✓ Auto* badge and a light-green ring on the
tile, *Auto-picked* in the loupe; picks get a solid green ring). A photo is auto-picked when all of these hold:

- it is the best photo of its series (:ref:`features-series`), or in no
  series;
- its combined quality score (:ref:`features-series-score`) is at least
  50% (:ref:`reference-settings-auto-pick`);
- nobody important has closed eyes (:ref:`features-ai-faces`).

A best photo you chose yourself (**B**) is always auto-picked. When you
reject the best photo of a series, the next best one takes its place.

Auto-pick is only a suggestion and never overrides you: picking or
rejecting a photo replaces it, and **U** (unflag) brings it back. It is
not written to XMP sidecars and not exported as a pick. To keep the
suggestions, click **Pick all N auto-picked** under **Flag** in the
filters: every auto-picked photo becomes a pick, in one step that
**Undo** reverses. Filter by **Auto-picked** to go through them first.

Decisions are saved immediately. **Undo** and **Redo** (above the
timeline, or Ctrl+Z / Ctrl+Shift+Z) step back and forward through the
last 200 changes; undoing restores each photo's previous flag and rating,
even when a change covered photos that had different ones.

Filters
-------

The **search box** above the timeline finds photos by describing them
(:ref:`features-ai-search`).

The **Filters** panel beside the timeline (toggle it with the
**Filters** button) narrows the timeline to matching photos:

.. list-table::
   :header-rows: 1

   * - Filter
     - Shows
   * - Flag
     - Picks, auto-picked, unflagged and/or rejects (any of the chosen
       ones; auto-picked photos are also unflagged). A decision under a
       Flag or Rating filter updates the view right away, so a photo you
       reject leaves a *Picks* view.
   * - Rating
     - Photos with at least the chosen number of stars
   * - Camera
     - Photos taken with that camera: one button per camera, with its
       photo count, the most used first (the six most used, then **All N
       cameras**). Click the chosen camera again, or **Any**, to show
       every camera. Files that spell one camera differently (*Canon* /
       *EOS R5* and *Canon* / *Canon EOS R5*) are one camera.
   * - Lens
     - Photos taken with that lens
   * - Date
     - Photos taken from / until a day (both included)
   * - Place
     - A country, or one city in it
   * - Quality
     - Photos whose sharpness, exposure and/or low-noise score is at least
       25, 50 or 75% (:doc:`/reference/quality-scores`). Photos not analyzed
       yet are hidden while a quality filter is on.
   * - Aesthetic, Hide closed eyes, People
     - AI signals, once the models have run (:ref:`features-ai-library`)
   * - Best of each series only
     - Hides photos that are in a series but are not its best photo
       (:ref:`features-series`); photos in no series stay.

Filters combine: *Picks* with *3★+* in *Lisbon* shows picked photos of
three stars or more taken in Lisbon. Every option shows how many photos
in the library have it. Active filters appear as chips above the
timeline; remove one with its ✕, or all with **Clear all**. Filters are
kept while the app is open.

The timeline, day headers and counts all reflect the filters. A photo you
change while filtering (for example rejecting it while showing only
picks) stays in view until the filters change, so it does not jump away
mid-cull.

Sorting
-------

The menu above the timeline orders it:

- **Newest first** (default) and **Oldest first** group photos by day.
- **Recently imported** lists the latest imports first.
- **Best quality first** orders by the combined score
  (:ref:`features-series-score`: technical, aesthetic and faces) and shows
  each photo's score on its tile; photos not analyzed yet come last.

The last two show one list without day headers. Sorting works with every
filter and with **Stack series**, and is kept while the app is open.

.. _features-library-analysis:

Background analysis
-------------------

After an import, Picturesque analyzes every photo in the background to
score its sharpness, exposure and noise
(:doc:`/reference/quality-scores`) and to hash its visual layout for
finding similar photos (:doc:`/reference/similarity`), which groups photos
into :doc:`series </features/series>`. The loupe shows the scores under
**Quality**. While it works, the progress ring at the top right shows
**Analyzing 120 / 300** (:ref:`features-library-progress`). Photos on screen are analyzed first, so scrolling
to a day gets its results within seconds. Analysis continues where it left
off after the app restarts. With the CLIP model downloaded, analysis also
computes image embeddings (:ref:`features-ai-embeddings`).

AI analysis (aesthetic scores and similar photos, faces and closed eyes)
needs its models, which are not bundled with the app
(:ref:`features-ai-models`). Until they are downloaded, photos get only
the other analyses, and the toolbar shows **AI analysis needs models**:
click it to open Settings, where **AI models** downloads them. Photos are
then analyzed with the new models automatically.

.. _features-library-progress:

Progress ring
~~~~~~~~~~~~~

The ring at the top right of the window shows background work on every
tab, one thing at a time:

.. list-table::
   :header-rows: 1

   * - Shows
     - Meaning
   * - **Importing · 12 left**
     - Photos are being read and copied into the library; the ring spins.
   * - **Looking for photos**
     - An import folder is being scanned.
   * - **Analyzing 120 / 300**
     - Photos are being analyzed; the ring fills as they finish.
   * - **Import paused**, **Analysis paused**
     - Work waits, for example for disk space (amber).
   * - **Up to date**
     - Nothing is waiting.

Photos that could not be imported are counted in red next to it. Click
the ring for details (what is being copied, why work is paused) and
**Open Activity**.

.. _features-library-reanalyze:

Analyzing photos again
~~~~~~~~~~~~~~~~~~~~~~

Each photo is analyzed once. To analyze photos again, for example after
an analysis failed or after downloading a new model:

- select photos in the timeline and click **Re-analyze** above it, or
- click **Re-analyze all photos** in **Settings → Analysis** (and confirm).

The photos go back into the queue, the selected ones first, and the
progress ring shows **Analyzing …** again. Until a photo's new results are
ready, its earlier ones stay in use, so scores, series, auto-picks and
quality filters keep working meanwhile; when the new results are stored,
series and auto-picks are updated. Your decisions (picks, rejects,
ratings, a best photo you chose) are never changed. If AI models are not
downloaded yet, **Re-analyze all photos** says which analyses need them.

.. _features-library-sidecars:

XMP sidecars
------------

Picks, rejects and ratings are written to **XMP sidecar files** next to
each photo in the library, so other photo tools see them
(:doc:`ADR-0009 </architecture/decisions/0009-xmp-sidecars-for-culling-decisions>`).
Photo files themselves are never modified.

.. list-table::
   :header-rows: 1

   * - Written
     - Value
     - Read by
   * - ``xmp:Rating``
     - Stars 0–5, or −1 for a rejected photo
     - darktable, Lightroom, Bridge
   * - ``digiKam:PickLabel``
     - 3 for picks, 1 for rejects, 0 otherwise
     - digiKam
   * - ``picturesque:Flag``, ``picturesque:Stars``
     - The exact flag and stars (a rejected photo keeps its stars)
     - Picturesque

- If a photo already has a sidecar (for example from darktable or
  Lightroom), it is **updated in place**: only the properties above are
  changed; everything else other tools stored there is kept.
- Otherwise a new sidecar is created next to the photo's RAW file (or its
  only file), named ``IMG_0042.CR3.xmp`` by default (darktable,
  digiKam) or ``IMG_0042.xmp`` (Lightroom, Bridge); see
  :ref:`reference-settings-sidecar-naming`.
- Sidecars are written in the background about a second after you stop
  changing decisions, so rapid culling writes each file once, and again
  when the app closes. Anything that could not be written is retried after
  the next change and when the app starts.
- Sidecars live next to the photos **in the library**, not in the folders
  you imported from. The toolbar says **Saved to XMP ✓** once every
  decision is written (*Saving to XMP…* meanwhile, or how many could not
  be written and why); in the desktop app, clicking it opens the library
  folder. In the photo view, **Show in folder** next to each file opens
  its folder with the file selected.
- A sidecar that cannot be read as XMP is left untouched.

On import, decisions found in existing sidecars (ratings, darktable or
Lightroom rejects, digiKam pick labels) become the photo's starting
decision.

Checking with other tools
~~~~~~~~~~~~~~~~~~~~~~~~~

To confirm that another tool picks up decisions:

1. In Picturesque, rate a RAW photo 3 stars and pick another; reject a
   third.
2. **darktable**: import the library folder (or re-read sidecars with
   *selected images → metadata → refresh EXIF*). The first photo shows
   three stars and the rejected one shows the reject mark.
3. **digiKam**: with *Settings → Metadata → Sidecars → Read from sidecar
   files* enabled, the picked photo shows the green "Accepted" pick label
   and the rejected one the red "Rejected" label.
4. **Lightroom Classic**: set sidecar naming to ``IMG_0042.xmp``, then
   *Metadata → Read Metadata from Files*; ratings appear. Lightroom does
   not read pick/reject flags from XMP.
