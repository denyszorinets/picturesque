Installing
==========

Picturesque runs on Linux, macOS (Apple Silicon) and Windows.

Installers are attached to each
`release <https://github.com/denyszorinets/picturesque/releases>`_:

- **Fedora** and other rpm-based distributions: ``Picturesque-*.x86_64.rpm``
  (``sudo dnf install ./Picturesque-*.rpm`` installs libvips, LibRaw and
  libheif as well).
- **Ubuntu, Debian**: ``Picturesque_*_amd64.deb``
  (``sudo apt install ./Picturesque_*.deb``).
- **Other Linux**: ``Picturesque_*_amd64.AppImage``: make it executable and
  run it. For HEIC photos, prefer the rpm or deb.
- **macOS** (Apple Silicon): ``Picturesque_*.dmg``. The app is not signed
  yet: the first time, right-click it and choose **Open**.
- **Windows**: not available yet.

Until the first release is published, build the app from source
(:doc:`/development/building`, :doc:`/development/packaging`).

What you need
-------------

- A disk with room for your photos: imports **copy** photos into the
  library, so they take as much space again as the originals.
- For the AI features, room for the models (about 1.7 GB for all of
  them) and, on first use, an internet connection to download them. Every
  other feature works offline from the start.
- Any recent CPU. AI analysis of a 10,000-photo library takes about an
  hour on a laptop CPU in the background; a supported GPU is faster
  (:doc:`/features/ai`).

Where Picturesque keeps its data
--------------------------------

Your photos go into the library folder you choose. The catalog,
thumbnails and AI models live in the application's data and cache
folders (:doc:`/reference/data-locations`). Removing the app does not
touch your library.

Next: :doc:`first-import`.
