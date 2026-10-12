API
===

The UI talks to the backend through named commands with JSON arguments
(:ref:`architecture-components-command-api`). In the desktop app they go
through the ``api`` Tauri command; in development, through
``POST /api/commands/{name}`` (:doc:`/development/ui-in-browser`). Types
are generated into ``ui/src/lib/api/generated/``.

Errors are ``{ "code": ..., "message": ... }`` with ``code`` one of
``invalid_arguments``, ``unknown_command``, ``not_found`` and
``internal``. Messages are
written to be shown to users.

Commands
--------

``app_info``
   No arguments. ``{ "name": "Picturesque", "version": "0.1.0" }``.

``get_settings``, ``update_settings``
   See :doc:`settings`.

``template_presets``
   No arguments. The built-in templates: ``[{ "id", "label", "template" }]``.

``preview_template``
   ``{ "template": "..." }``. ``{ "error": null, "examples": [...] }``:
   either the reason the template is invalid, or the paths it produces for
   three sample photos (RAW+JPEG with a place, a phone photo without
   location, an undated scan).

``query_photos``
   A ``PhotoQuery``; every field is optional and filters combine:

   .. code-block:: json

      { "from": "2026-10-01", "to": "2026-10-31", "country_code": "PT",
        "city": "Lisbon", "camera": { "make": "Canon", "model": "Canon EOS R5" },
        "lens": "RF50mm F1.2 L USM", "flags": ["pick", "unflagged"],
        "min_rating": 3, "sort": "captured_desc", "limit": 200, "cursor": null }

   ``sort`` is ``captured_desc`` (default), ``captured_asc`` or
   ``imported_desc``; photos without a capture date always come last.
   Returns ``{ "photos": [PhotoSummary], "next_cursor": "...", "total": 1234 }``.
   Pass ``next_cursor`` back as ``cursor`` for the next page; it is
   ``null`` on the last page. Paging is keyset-based, so later pages are as
   fast as the first. ``limit`` is 1–1000.

   A ``PhotoSummary`` has ``id``, ``captured_at``, ``day``
   (``YYYY-MM-DD``, for grouping), ``camera``, ``lens``, ``city``,
   ``country``, ``flag``, ``rating``, displayed ``width``/``height``,
   ``has_raw`` and ``file_name``.

``get_photo``
   ``{ "photo_id": 7 }``. A ``PhotoDetail``: capture time and UTC offset,
   camera, lens, ``iso``, ``aperture`` (``f/2.8``), ``shutter``
   (``1/250 s``), ``focal_length`` (``35 mm``), GPS, ``place``, displayed
   size, ``flag``, ``rating`` and ``files`` (path, format, size). Unknown
   photos are ``not_found``.

``photo_facets``
   No arguments. The values present in the library with photo counts:
   ``days`` (newest first), ``cameras`` (with display labels), ``lenses``,
   ``places``, ``flags``, ``ratings`` (counts for 0–5 stars), plus
   ``total``, ``first_day`` and ``last_day``.

``plan_import``
   ``{ "sources": ["/absolute/folder", ...] }``. Starts planning in the
   background and returns the run's ``ImportStatus`` (state
   ``planning``). Fails if no library folder is set, a source is relative
   or inside the library, or another import is running.

``get_import``
   ``{ "run_id": 1 }``. The run's ``ImportPlanView``: state, library
   folder, template, summary, every photo with its files (source and target)
   and status, and problems.

``import_status``
   ``{ "run_id": 1 }``. The run's ``ImportStatus``: state, current phase
   and progress, and the summary once a plan exists.

``start_import``
   ``{ "run_id": 1 }``. Starts or resumes copying a run that is
   ``planned``, ``cancelled``, ``interrupted`` or ``failed``.

``cancel_import``
   ``{ "run_id": 1 }``. Asks the running planning or copying to stop.
   Copying stops between photos.

Import states
-------------

.. list-table::
   :header-rows: 1

   * - State
     - Meaning
   * - ``planning``
     - Scanning sources and reading metadata.
   * - ``planned``
     - Plan ready for review; nothing copied.
   * - ``copying``
     - Copying in progress.
   * - ``completed``
     - Every planned photo was imported or failed.
   * - ``cancelled``
     - Stopped by the user; can be started again.
   * - ``interrupted``
     - The app stopped while planning or copying; can be started again.
   * - ``failed``
     - The run could not continue (for example the library folder
       disappeared); the error says why. Can be started again.

Images
------

Photo images are served as JPEG files, not JSON, so they can be used
directly as ``<img src>``:

.. list-table::
   :header-rows: 1

   * - Where
     - URL
   * - Desktop app
     - ``picturesque://localhost/{size}%2F{photo_id}`` (``http://picturesque.localhost/...``
       on Windows); built with ``convertFileSrc`` by ``imageUrl()`` in
       ``ui/src/lib/api``
   * - Development server
     - ``GET /api/images/{size}/{photo_id}``

``size`` is ``grid`` (longest edge 512 px), ``preview`` (2048 px) or
``full`` (native size, for 100% zoom).
Images are generated on first request and cached
(:doc:`data-locations`). Responses carry an ``ETag``; a request with a
matching ``If-None-Match`` gets ``304 Not Modified``. An unknown photo is
``404``, a malformed path ``400``.

Events
------

Events are JSON objects with a ``type`` field. In the desktop app they
arrive as the Tauri event ``app-event``; in development, as server-sent
events from ``GET /api/events``.

``import_progress``
   An ``ImportStatus``. Sent when a phase starts, about every percent of
   progress, and when a run ends (with its summary).

   .. code-block:: json

      { "type": "import_progress", "run_id": 3, "state": "copying",
        "phase": "copying", "done": 120, "total": 400,
        "summary": null, "error": null }
