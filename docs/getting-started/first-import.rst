Your First Import
=================

This walks through bringing photos from existing folders into a clean,
organized library. Picturesque **copies** every photo into the library
(the output folder); your original folders are only read: nothing in
them is ever moved, renamed, edited or deleted.

On first launch, a **setup wizard** leads through steps 1 to 3 and
offers the AI models (:ref:`features-ai-models`): choose the output
(library) folder and its layout, add the folders to import from, then
download models or skip them. Any step can be skipped, and **Run setup again** at
the top of **Settings** reopens it. Until an output folder is chosen, the
**Library**, **Activity** and **Statistics** tabs say so and offer **Set
up Picturesque**. The rest of this page does the same
by hand, and explains what happens next.

1. Choose the library folder
----------------------------

Open **Settings** and set **Library folder** to an empty folder (or an
existing Picturesque library) on a disk with enough free space. In the
desktop app, **Browse…** opens a folder picker; you can also type or paste
a path.

2. Choose how folders are named
-------------------------------

**Folder structure** offers ready-made layouts, such as *By date* or *By
date and place*. Below it, the template can be edited freely; the
examples underneath update as you type and show where three typical
photos would go. A template with a mistake is explained in red. See
:doc:`/reference/path-templates` for every option.

Click **Save**.

3. Add a folder to import from
------------------------------

Open **Activity** and add a folder under **Import folders**: a memory
card, an old photo dump, a backup drive. Subfolders are included.
Importing starts right away, in the background.

Photos appear in the **Library** one by one as soon as each is copied,
so you can start culling while the rest is still importing. The header
shows how many photos are left; click it to come back to **Activity**.

4. Follow each photo
--------------------

**Photos** in **Activity** lists every photo found, newest first, with a
row of steps: *Found*, *Read*, *Copied*, *Verified*, *Catalogued*,
*Thumbnail* and *Analyzed*. A green dot is done, a pulsing blue one is in
progress, a red one is where a photo failed. The filters show only the
photos in progress, imported, skipped as duplicates or failed. A photo
that failed says why; **Retry failed** tries again once the cause is
fixed.

The **Log** next to it records what happened and when, with warnings and
errors highlighted; choose **Errors** to see only problems.

.. figure:: /images/screenshots/activity.webp
   :alt: Activity: the import folder, every photo with its steps from Found to Analyzed as green dots, and the log of photos imported into dated folders.

   **Activity**: each photo's steps, and the log.

5. Keep importing
-----------------

Folders are *watched*: new photos copied into them later are imported
automatically, and the folder is checked again every few minutes and
when the app starts. Untick **Watch** to import a folder only when you
click **Check**. Quitting is safe at any time; importing continues where
it stopped when the app starts again.

What next
---------

- :doc:`/features/import` explains duplicates, safety, watching and
  resuming in detail.
- :doc:`/concepts/photos-and-files` explains how RAW+JPEG pairs and
  sidecars are handled.
