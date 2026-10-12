Data Locations
==============

Picturesque keeps three kinds of data
(:doc:`/architecture/data`):

.. list-table::
   :header-rows: 1

   * - Data
     - Where
     - Safe to delete?
   * - Photos and sidecars
     - The library folder you chose (:ref:`reference-settings-library-root`)
     - No: these are your imported photos.
   * - Catalog (``catalog.sqlite``)
     - Application data folder (below)
     - No: it holds metadata, decisions and import history.
   * - Thumbnails and previews
     - Application cache folder (below), in ``v1/grid`` (512 px),
       ``v1/preview`` (2048 px) and ``v1/full`` (photos zoomed to 100%)
     - Yes: they are recreated when needed.

.. list-table::
   :header-rows: 1

   * - Platform
     - Data folder
     - Cache folder
   * - Linux
     - ``~/.local/share/io.github.denyszorinets.picturesque/``
     - ``~/.cache/io.github.denyszorinets.picturesque/``
   * - macOS
     - ``~/Library/Application Support/io.github.denyszorinets.picturesque/``
     - ``~/Library/Caches/io.github.denyszorinets.picturesque/``
   * - Windows
     - ``%APPDATA%\io.github.denyszorinets.picturesque\``
     - ``%LOCALAPPDATA%\io.github.denyszorinets.picturesque\``

The development server keeps both under ``target/devserver-data/``
(``cache/`` inside it), or under ``PICTURESQUE_DATA_DIR``.

Thumbnails
----------

Thumbnails are JPEGs named after the content hash of the file they are
made from, so they never go out of date. They are generated in the
background after every import and when the app starts, and on demand for
anything still missing. RAW files use the preview the camera embedded,
which is fast; a RAW without one is fully decoded, which takes longer.
Thumbnails are always upright: the photo's EXIF orientation is applied.
