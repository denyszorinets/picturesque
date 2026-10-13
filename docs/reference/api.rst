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

   ``sort`` is ``captured_desc`` (default), ``captured_asc``,
   ``imported_desc`` or ``quality_desc`` (combined score, best first;
   photos not analyzed last). With the capture-date sorts, photos
   without a capture date come last.
   Returns ``{ "photos": [PhotoSummary], "next_cursor": "...", "total": 1234 }``.
   Pass ``next_cursor`` back as ``cursor`` for the next page; it is
   ``null`` on the last page. Paging is keyset-based, so later pages are as
   fast as the first. ``limit`` is 1–1000.

   ``min_sharpness``, ``min_exposure`` and ``min_noise`` (0–1) keep photos
   whose technical score is at least that (:doc:`quality-scores`); photos
   not analyzed yet never match. ``min_aesthetic`` (0–1) does the same for
   the aesthetic score, ``hide_closed_eyes: true`` hides photos where
   someone important has closed eyes, and ``people`` keeps only photos with
   (``true``) or without (``false``) faces. ``best_only: true`` hides photos that are
   in a series but not its best photo. ``photo_ids`` (at most 1000) limits
   the result to those photos.
   ``collapse_series: true`` lists one photo per series: the user's chosen
   best photo if it matches the other filters, else the best-ranked
   matching one. Series listed in ``expanded_series`` (by ``series_id``)
   are shown whole. Facets (``photo_facets``) take the same fields, so day
   counts match the list.

   A ``PhotoSummary`` has ``id``, ``captured_at``, ``day``
   (``YYYY-MM-DD``, for grouping), ``camera``, ``lens``, ``city``,
   ``country``, ``flag``, ``rating``, displayed ``width``/``height``,
   ``has_raw``, ``file_name``, ``series_id`` (the first photo of its
   series, or ``null``), ``best_in_series``, ``series_size`` (0 when
   in no series) and ``quality`` (the combined score,
   :ref:`features-series-score`; ``null`` until analyzed), ``aesthetic``
   (0–1 or ``null``), ``eyes_closed`` (``true``/``false``, ``null`` when
   undecided) and ``faces`` (count).

``get_photo``
   ``{ "photo_id": 7 }``. A ``PhotoDetail``: capture time and UTC offset,
   camera, lens, ``iso``, ``aperture`` (``f/2.8``), ``shutter``
   (``1/250 s``), ``focal_length`` (``35 mm``), GPS, ``place``, displayed
   size, ``flag``, ``rating``, ``files`` (path, format, size) and
   ``quality`` (``sharpness``, ``exposure``, ``noise``, ``overall``,
   ``highlights_clipped``, ``shadows_clipped``; ``null`` until analyzed,
   see :doc:`quality-scores`) and ``aesthetic`` (``score`` 0–1 and the
   predictor's ``rating``, about 1–10; ``null`` until the CLIP model
   analyzed the photo, see :ref:`features-ai-aesthetic`) and ``faces``
   (largest first; each with ``x``, ``y``, ``width``, ``height`` as
   fractions of the photo, detector ``score``, ``eyes_closed`` —
   ``true``, ``false`` or ``null`` when undecided — ``eye_openness`` and
   ``sharpness`` 0–1; ``null`` until the face models analyzed the photo, see
   :ref:`features-ai-faces`), ``score``, the combined quality 0–1
   (:ref:`features-series-score`), and ``reasons``: why it scored so,
   most important first, each ``{ "tone": "good" | "bad" | "info",
   "text": "Eyes closed: the score is halved" }``. Unknown photos are ``not_found``.

``set_decisions``
   ``{ "changes": [{ "photo_id": 7, "flag": "pick" }, { "photo_id": 8, "rating": 4 }] }``.
   Each change sets a photo's ``flag`` (``unflagged``, ``pick``,
   ``reject``), ``rating`` (0–5), or both; omitted fields stay as they
   are. Returns each photo's resulting ``{ "photo_id", "flag", "rating" }``.
   All changes are validated first: an unknown photo (``not_found``) or a
   rating above 5 (``invalid_arguments``) changes nothing.

``photo_facets``
   Optional ``{ "filter": PhotoQuery }``. With a filter, ``total``,
   ``first_day``, ``last_day`` and ``days`` count only matching photos
   (they drive the timeline layout); the option lists below always cover
   the whole library. The values present in the library with photo counts:
   ``days`` (newest first), ``cameras`` (with display labels), ``lenses``,
   ``places``, ``flags``, ``ratings`` (counts for 0–5 stars), plus
   ``total``, ``first_day`` and ``last_day``.

``ai_status``
   No arguments. Where local AI models run (:doc:`/features/ai`):

   .. code-block:: json

      { "available": ["webgpu", "cpu"], "choice": "auto", "provider": "webgpu",
        "problem": null }

   ``available`` lists the providers of this build, preferred first.
   ``provider`` is the one a tiny test model actually ran on (``null`` if
   even the CPU failed); ``problem`` says why the chosen provider was not
   used. The check runs once per choice.

``start_export``
   ``{ "destination": "/home/me/Export", "query": { "flags": ["pick"] },
   "format": "jpeg", "naming": "{date}_{seq}.{ext}", "include_raw": true,
   "include_sidecars": true, "max_edge": 2048, "quality": 90 }``; only
   ``destination`` is required (default: the picks, originals, original
   names). Starts an export in the background (:doc:`/features/export`)
   and returns an ``ExportStatus``: ``{ "state": "running", "destination":
   "...", "done": 0, "total": 12, "files": 0, "failed": [] }`` (``state``
   is ``idle``, ``running``, ``completed`` or ``cancelled``; ``failed``
   lists ``{ "photo_id", "error" }``). Invalid options, or an export
   already running, are ``invalid_arguments``.

``export_status`` / ``cancel_export``
   No arguments. The current ``ExportStatus``; cancelling stops after the
   photo being exported.

``plan_reject_move``
   No arguments. ``{ "photos": 12, "files": 30, "bytes": 912345678,
   "undo_batch": 3, "undo_photos": 8 }``: rejected photos not moved yet,
   and the latest move that can be undone (:doc:`/features/rejects`).

``move_rejects``
   No arguments. Moves every rejected photo's files into ``_Rejected``.
   Returns ``{ "batch": 4, "moved": 11, "failed": [{ "photo_id": 7,
   "error": "..." }] }``; failed photos stayed where they were.

``undo_reject_move``
   ``{ "batch": 4 }``. Moves the latest batch back; another batch is
   ``invalid_arguments``. Returns the same shape as ``move_rejects``.

``similar_photos``
   ``{ "photo_id": 7, "limit": 50 }`` (``limit`` 1–500, default 50). The
   photos that look most like this one, most similar first, by CLIP
   embedding (:ref:`features-ai-embeddings`):
   ``[{ "photo_id": 12, "similarity": 0.93 }]``. Empty until the photo has
   an embedding; unknown photos are ``not_found``.

``search_photos``
   ``{ "text": "dog on a beach", "limit": 200 }`` (``limit`` 1–1000,
   default 200). Photos matching the description, best first, by CLIP
   (:ref:`features-ai-search`): ``[{ "photo_id": 12, "similarity": 0.23 }]``.
   Photos below a similarity of 0.17 are left out. Empty text, or a missing
   CLIP model (the message says which), is ``invalid_arguments``.

``list_models``
   No arguments. The downloadable AI models (:doc:`/features/ai`), each a
   ``ModelView``:

   .. code-block:: json

      { "id": "clip-image", "name": "CLIP ViT-L/14 image encoder (OpenAI)",
        "purpose": "...", "version": 1, "license": "MIT",
        "license_url": "https://...", "homepage": "https://...",
        "size": 1216297719,
        "state": "downloading", "downloaded": 12000000, "error": null }

   ``state`` is ``not_installed``, ``downloading``, ``installed`` or
   ``failed`` (with ``error``). Never uses the network.

``install_model`` / ``cancel_model`` / ``remove_model``
   ``{ "id": "clip-image" }``. Start a download in the background, stop
   it, or delete the model's files; each returns the ``ModelView``.
   Removing a model while it downloads is ``invalid_arguments``; unknown
   models are ``not_found``.

``analysis_status``
   No arguments. ``{ "analyzed": 120, "total": 300, "running": true }``:
   photos with every analysis done, all photos, and whether workers are
   busy (:doc:`/architecture/decisions/0011-resumable-background-analysis`).

``prioritize_analysis``
   ``{ "photo_ids": [7, 8, 9] }``. Analyzes these photos next; the library
   sends the photos on screen. Unknown or finished photos are ignored.
   Returns the ``analysis_status``.

``photo_series``
   ``{ "photo_id": 7 }``. The photo's series:

   .. code-block:: json

      { "photo_id": 7, "series_id": 5, "photo_ids": [5, 6, 7], "edit": null,
        "best_photo_id": 6, "best_chosen": false,
        "ranking": [{ "photo_id": 6, "score": 0.97 }, { "photo_id": 5, "score": 0.81 },
                    { "photo_id": 7, "score": null }] }

   ``series_id`` is the first photo's id (``null``, with empty lists,
   when the photo is in no series); ``edit`` is the user's correction before
   this photo; ``ranking`` lists members best first with scores
   (``null`` until analyzed); ``best_chosen`` says whether the user chose
   the best photo. Unknown photos are ``not_found``.

``edit_series``
   ``{ "photo_id": 7, "edit": "split" }``. Sets the correction before a
   photo: ``split`` (it starts a new series), ``join`` (it joins the series
   of the photo before it) or ``null`` (automatic). Series are recomputed
   immediately; returns the photo's ``photo_series``
   (:doc:`/features/series`).

``choose_best``
   ``{ "photo_id": 6, "best": true }``. Makes the photo the best of its
   series (the latest choice wins); ``"best": false`` withdraws the choice.
   Returns the photo's ``photo_series``.

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

``analysis_progress``
   The ``analysis_status``, sent after each analyzed photo.

``series_changed``
   ``{ "series": 12, "photos": 57 }``: series membership changed (after
   analysis, an import, an edit or a settings change).

``export_progress``
   An ``ExportStatus`` (see ``start_export``), sent about five times a
   second during an export and when it ends.

``model_progress``
   A ``ModelView`` (see ``list_models``), sent about four times a second
   while a model downloads, and when a download ends or a model is
   removed.

   .. code-block:: json

      { "type": "import_progress", "run_id": 3, "state": "copying",
        "phase": "copying", "done": 120, "total": 400,
        "summary": null, "error": null }
