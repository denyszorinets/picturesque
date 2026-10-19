.. _features-folders:

Culling a Folder in Place
=========================

Besides the library, Picturesque can cull **a folder where it is**: a
folder darktable already knows, a dump of last weekend, a memory card.
Nothing is copied. Decisions are written to XMP sidecars next to the
photos, so the tools that read that folder see them
(:doc:`ADR-0013 </architecture/decisions/0013-folder-sessions>`).

One folder is open at a time: open it, cull, open the next one. The
library stays as it is meanwhile and is one click away.

Opening a folder
----------------

Click the **folder menu** at the top left of the window (it shows
**Library**, or the open folder's name) and choose **Open folder…**. On
first launch, the setup wizard also offers **Or cull a folder where it
is, without a library…**.

Picturesque then finds every photo in the folder and its subfolders,
reads its date, camera, place and any decision already in its sidecar
(ratings from darktable or Lightroom, for example), and analyzes it as
it would a library photo. The progress ring at the top right shows how
far it got (:ref:`features-library-progress`); you can start culling
right away.

Everything you know from the library works the same: the timeline,
filters, the loupe, compare, series and auto-pick, export and
statistics. The **Activity** tab shows the folder's photos and log.

What changes in the folder
--------------------------

- **Nothing is copied or renamed.** Picturesque reads the photos where
  they are.
- **Sidecars are written next to the photos** you decide on, as in the
  library (:ref:`features-library-sidecars`): ``IMG_0042.CR3.xmp`` by
  default. An existing sidecar is updated in place, keeping everything
  other tools stored in it. Photos you do not touch get no sidecar.
- **Move rejects…** moves rejected photos into ``_Rejected`` inside the
  folder, if you ask for it (:doc:`rejects`).

A folder that cannot be written to (a read-only card, a network share)
can still be opened and culled; sidecars that cannot be written are
reported in the toolbar (*XMP: N not saved*).

Switching folders and back to the library
-----------------------------------------

The folder menu lists **Library**, the open folder and the **recent
folders**. Choosing one closes the current folder: decisions not yet in
their sidecars are written first, then the other folder (or the library)
opens. Reopening a folder is quick: its photos, analysis and decisions
are kept.

When the app starts, it reopens the folder that was open when it was
closed, or the library if that folder is gone (a card taken out).

Folders not opened for 30 days are forgotten: their entry in the recent
list and their catalog are deleted. The folder and its sidecars are not
touched; opening it again reads its sidecars again.

Settings
--------

Settings apply everywhere, except the library folder and its folder
structure, which only apply to the library. While a folder is open,
**Settings** says so instead of showing them.

Limitations
-----------

- A folder inside the library, or a folder holding the library, cannot
  be opened: cull those photos in the library.
- One folder at a time; to cull two folders together, open the folder
  that holds both.
