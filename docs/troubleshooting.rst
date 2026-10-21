Troubleshooting
===============

Importing
---------

HEIC photos are skipped as unreadable
   The HEVC decoder for libheif is missing; many Linux distributions leave
   it out for patent reasons. Install ``libde265`` support (on Fedora,
   ``libheif-freeworld`` from RPM Fusion; see :doc:`/development/building`),
   then click **Retry failed** in **Activity**.

A RAW file cannot be read
   Very new cameras may not be supported by LibRaw yet. **Activity** lists
   the photo as failed with the reason; if the camera also wrote a JPEG,
   the photo is imported using the JPEG's metadata.

Photos are reported as duplicates
   A photo whose image file is byte-for-byte identical to one already in
   the library is not imported twice, even under another name
   (:doc:`/features/import`).

Importing stopped
   Photos already copied stay in the library. When the app starts again,
   importing continues with the photos that were waiting; the one being
   copied when it stopped starts over. If **Activity** says importing is
   paused, it explains why (for example, no library folder is set).

Importing or analysis is paused because a disk is full
   Free space on the disk named in **Activity** (the library disk, or the
   disk holding the catalog and thumbnail cache, see
   :doc:`/reference/data-locations`); work continues on its own within a
   few seconds (:ref:`features-import-disk-full`). Deleting the thumbnail
   cache is safe and frees space on the system disk.

New photos in a watched folder are imported late
   The pipeline log says *Cannot watch … for new photos* when the system
   refuses to watch a folder, usually because a large folder tree exceeds
   Linux's limit on watches. The folder is still scanned every few minutes
   (:ref:`reference-settings-import`). To raise the limit:
   ``sudo sysctl fs.inotify.max_user_watches=524288`` (add it to
   ``/etc/sysctl.d/`` to keep it). Network shares often report no changes
   at all and rely on the periodic scan.

"Choosing a different library folder is not supported yet"
   The catalog stores paths relative to the library folder, so it cannot
   point at another folder once it has photos. Keep the library where it
   is for now; moving a library is planned.

The library
-----------

Dates or places are missing
   Photos without capture time in their metadata appear under **No date**;
   photos without GPS have no place. Places come from GPS coordinates via
   an offline database (:doc:`/reference/third-party-data`).

Thumbnails take a while
   Thumbnails are made in the background after an import; RAW files
   without an embedded preview must be fully decoded, which is slower.

Ratings do not show up in another program
   Picturesque writes decisions to XMP sidecars next to the photos.
   Choose the sidecar naming your other program expects (Lightroom reads
   ``IMG_0042.xmp``, darktable and digiKam ``IMG_0042.CR3.xmp``) under
   **Settings** (:ref:`reference-settings-sidecar-naming`).

Series look wrong
   Adjust the longest pause and similarity limit under **Settings →
   Series**, or correct single photos with **S** (split before)
   and **J** (join) (:doc:`/features/series`).

AI features
-----------

A model download fails
   Downloads need an internet connection to Hugging Face; a proxy is used
   when set in the environment (``HTTPS_PROXY``). A download that does not
   match its checksum is discarded with "download was corrupt"; press
   **Retry**. Files already complete are kept.

Analysis is slow or uses a lot of CPU
   Analysis uses half the cores so the app stays responsive; with the
   CLIP model a photo takes about a third of a second on a laptop CPU.
   Photos on screen are analyzed first. A GPU provider may help
   (:doc:`/features/ai`).

"Models run on CPU" although I chose a GPU
   The chosen provider could not start and models fell back to the CPU;
   the reason is shown under **AI acceleration**. On Linux, WebGPU needs a
   working Vulkan driver for your GPU (``vulkaninfo --summary`` should list
   your GPU, not ``llvmpipe``); without one it runs on a slow software
   driver. Choose **Automatic** (the CPU on Linux) if WebGPU misbehaves.

Text search finds nothing
   Search needs both CLIP models (image and text encoder) and the photos'
   embeddings: wait until **Analyzing** in the toolbar has finished.
   Unrelated photos are left out on purpose; try a more general
   description, in English.

Eyes are reported closed when they are not
   Squinting, deep shadows, sunglasses and very small faces can mislead
   the face mesh. Faces too small to judge are left undecided. Your own
   decisions always win; choose the best photo yourself with **B**.

Exporting and moving rejects
----------------------------

"changed since it was imported"
   An original in the library no longer matches the copy that was
   imported (edited in place, or damaged). It is not exported, so a
   damaged file is never passed on silently. Check the file.

A photo could not be moved to ``_Rejected``
   Usually a file of the same name is already there. The photo stays in
   place untouched; rename or remove the other file and move again
   (:doc:`/features/rejects`).

Still stuck?
------------

Open an issue on
`GitHub <https://github.com/denyszorinets/picturesque/issues>`_ with what
you did, what happened and your operating system.
