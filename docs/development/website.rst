Website
=======

The website, `denyszorinets.github.io/picturesque
<https://denyszorinets.github.io/picturesque/>`_, has two parts:

.. list-table::
   :header-rows: 1

   * - Path
     - What
     - Source
   * - ``/``
     - Landing page
     - :repo:`site/index.html` and its images in :repo:`site/assets`
   * - ``/docs/``
     - This documentation
     - ``docs/``, built with ``make docs``

Publishing
----------

The *Pages* workflow (:repo:`.github/workflows/pages.yml`) builds both and
deploys them to GitHub Pages whenever ``main`` changes, that is with each
release (:doc:`releasing`), so the site describes the released version.
To publish a fix between releases, run the workflow by hand (Actions →
Pages → Run workflow).

The repository's Pages source must be set to **GitHub Actions**
(Settings → Pages).

Previewing
----------

The landing page is one static HTML file without scripts or external
fonts: open ``site/index.html`` in a browser. For the documentation, run
``make docs-serve`` (:doc:`building`).

The landing page
----------------

- It follows the system's light or dark mode, and so do its screenshots
  (``*-light.webp`` and ``*-dark.webp``).
- It loads nothing from other sites: no fonts, scripts or analytics, in
  line with an app that keeps photos private.
- Claims about other products stay at the level of categories and say
  when they were checked; update the comparison table when they change.

Screenshots
~~~~~~~~~~~

The screenshots show the real app (the web UI against the development
server, :doc:`building`) with a demo library made from CC0 photos from
Wikimedia Commons:

1. Download photos and note their authors (the page credits the ones on
   screen).
2. Make bursts from them: crops of each photo, all but one blurred, a
   second apart, with camera, lens, focal length, capture time and GPS
   written to their EXIF.
3. Import them into a fresh library (``PICTURESQUE_DATA_DIR`` pointing to
   an empty folder), let analysis finish, and make some decisions.
4. Capture at 1600 × 1000 with a device scale factor of 2, in light and
   dark mode, and save as WebP (quality 80, 2400 pixels wide).
