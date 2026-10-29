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
- The AI critic section is text only (no screenshot yet); add one with
  the other screenshots when it can be captured.
- It loads nothing from other sites: no fonts, scripts or analytics, in
  line with an app that keeps photos private.
- Claims about other products stay at the level of categories and say
  when they were checked; update the comparison table when they change.

Screenshots
~~~~~~~~~~~

The screenshots of the landing page (``site/assets/*-light.webp`` and
``*-dark.webp``) and of these docs (``docs/images/screenshots``) show the
real app, the web UI against the development server (:doc:`building`),
with a demo library made from CC0 photos. One command rebuilds them all::

   make screenshots

It runs three steps (:repo:`scripts/screenshots`):

1. ``build-demo.py`` downloads the photos listed in ``photos.json`` from
   Wikimedia Commons (cached in ``target/screenshots/originals``),
   refusing any whose licence on Commons is not CC0 or whose author is not
   the one listed, and makes a demo memory card: each photo becomes a burst
   of shifted crops a second apart, all but one blurred, with its trip's
   camera, lens, focal length, time and GPS position in the EXIF.
2. It starts the dev server with an empty catalog
   (``target/screenshots/data``; downloaded AI models are kept between
   runs) and Vite.
3. ``capture.mjs`` imports the card, waits for analysis, makes some
   decisions, and captures the library, the loupe, Compare, Statistics and
   Activity, then a folder session (the darktable note and **Copy
   keepers…**), each at 1600 × 1000 with a device scale factor of 2, in
   light and dark mode, saved as WebP (quality 80).

To change what the screenshots show, edit ``photos.json`` (add only CC0
photos) or the decisions and screens in ``capture.mjs``. The landing page
credits the authors of the photos on screen; ``photos.json`` lists all of
them.
