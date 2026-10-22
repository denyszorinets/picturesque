Offload a Memory Card
=====================

From a full card to your keepers in another folder, and an empty card,
without importing the card into a library. This uses a folder session
(:doc:`/features/folders`); nothing on the card changes until you clear
it at the end.

1. Open the card
----------------

Insert the card, click the **folder menu** at the top left of the window
and choose **Open folder…**, then pick the card (or its ``DCIM`` folder).
Picturesque finds every photo on it, reads its date, camera and place,
and starts analyzing. You can cull right away; photos on screen are
analyzed first.

.. figure:: /images/screenshots/folder.webp
   :alt: A card opened as a folder: fox bursts framed as series, the suggested keepers ringed, and the folder actions Copy keepers, Finish folder and Clear folder in the toolbar.

   A card opened as a folder. **Copy keepers…** is the highlighted button.

2. Cull
-------

Bursts are framed as series, and the best frame of each is marked with a
star. Photos the app suggests keeping are **Auto-picked**. Go through the
shoot with :kbd:`P` (pick), :kbd:`X` (reject) and the arrow keys
(:doc:`first-cull`); **Pick all N auto-picked** under **Flag** in the
filters accepts the suggestions at once. Decisions are saved to XMP
sidecars on the card as you go.

3. Copy the keepers
-------------------

Click **Copy keepers…**. It copies your picks and the auto-picked photos,
the original files (RAW too), each with an XMP sidecar holding its
decision and colour label, and verifies every copy against the original.
Choose the folder they go to, for example the one darktable imports from
(:doc:`darktable`); it is remembered for next time.

.. figure:: /images/screenshots/copy-keepers.webp
   :alt: The Copy keepers dialog: keepers (picks and auto-picked) selected, original files with RAW files and XMP sidecars, copied under their original names.

   Copy keepers: picks and auto-picked photos, originals with sidecars.

4. Clear the card
-----------------

Click **Clear folder…**. It deletes the originals that have a verified
copy, hashing each copy again first, and lists what it will delete before
asking. With **Settings → After copying keepers** set to *verified copies
and rejects*, it also offers to delete the rejected photos, with their own
confirmation. Deleting is permanent, so the card is really empty
afterwards; photos that were not copied stay.

Prefer to clear cards yourself? Set **After copying keepers** to **Never
delete originals**, and the action is not offered
(:ref:`features-folders-clear`).

Next card
---------

Open the next card from the folder menu. Recent folders are listed there
too, and **Library** takes you back to your library.
