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

``open_folder``
   ``{ "path": "/home/me/Pictures/2026-10-18 Hike" }``. Closes the library
   or the folder open now (writing pending sidecars first) and culls this
   folder in place (:doc:`/features/folders`). Returns the settings as the
   folder sees them: ``open_folder`` and ``library_root`` are the folder.
   A relative path, a folder that cannot be read, a whole disk, and a
   folder inside or holding the library are ``invalid_arguments``.

``open_library``
   No arguments. Goes back to the library from a folder; returns the
   settings.

``clear_plan``
   No arguments. What **Clear folder…** would delete: ``{ "mode":
   "verified", "photos": 40, "files": 80, "bytes": 2000000000,
   "rejects": 0, "partly_copied": 1 }``. Only in a folder session
   (``invalid_arguments`` in the library).

``clear_folder``
   No arguments. Deletes what the plan says, re-verifying every copy
   first (:ref:`features-folders-clear`). Returns ``{ "deleted": 39,
   "files": 78, "bytes": 1950000000, "kept": [{ "photo_id": 7, "reason":
   "its copy … changed since it was copied" }] }``. Permanent.

``finish_status``
   No arguments. ``{ "auto_picked": 12, "labels_to_write": 12, "picks": 30,
   "rejects": 41 }``: auto-picked photos, how many of them still need the
   auto-pick label in their sidecars (or have it but are no longer
   auto-picked), and the picks and rejects (:ref:`features-folders-finish`).

``finish_folder``
   No arguments. Marks exactly the auto-picked photos for the auto-pick
   label and writes every pending sidecar now. Returns ``{ "auto_labeled":
   12, "written": 15, "failed": 0, "error": null }``.

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

   ``camera`` matches every spelling of the camera with the same display
   name (``Canon``/``EOS R5`` and ``Canon``/``Canon EOS R5``).

   ``min_sharpness``, ``min_exposure`` and ``min_noise`` (0–1) keep photos
   whose technical score is at least that (:doc:`quality-scores`); photos
   not analyzed yet never match. ``min_aesthetic`` (0–1) does the same for
   the aesthetic score, ``hide_closed_eyes: true`` hides photos where
   someone important has closed eyes, and ``people`` keeps only photos with
   (``true``) or without (``false``) faces. ``auto_picked: true`` keeps
   auto-picked photos (:ref:`features-library-auto-pick`); with ``flags``,
   photos matching either. ``best_only: true`` hides photos that are
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
   ``has_raw``, ``has_image`` (both are ``true`` for a RAW+JPEG pair),
   ``file_name``, ``series_id`` (the first photo of its
   series, or ``null``), ``best_in_series``, ``series_size`` (0 when
   in no series) and ``quality`` (the combined score,
   :ref:`features-series-score`; ``null`` until analyzed), ``aesthetic``
   (0–1 or ``null``), ``eyes_closed`` (``true``/``false``, ``null`` when
   undecided), ``faces`` (count) and ``keeper`` (suggested by auto-pick;
   the photo shows as auto-picked while its ``flag`` is ``unflagged``).
   Facets include ``auto_picked``, the number of auto-picked photos.

``get_photo``
   ``{ "photo_id": 7 }``. A ``PhotoDetail``: capture time and UTC offset,
   camera, lens, ``iso``, ``aperture`` (``f/2.8``), ``shutter``
   (``1/250 s``), ``focal_length`` (``35 mm``), GPS, ``place``, displayed
   size, ``flag``, ``rating``, ``files`` (path, format, size) and
   ``quality`` (``sharpness``, ``exposure``, ``noise``, ``overall``,
   ``highlights_clipped``, ``shadows_clipped``; ``null`` until analyzed,
   see :doc:`quality-scores`) and ``aesthetic`` (``score`` 0–1 and the
   predictor's ``rating``, about 1–10; ``null`` until the CLIP model
   analyzed the photo, see :ref:`features-ai-aesthetic`), ``taste`` (how
   well the photo matches the user's picks, 0–1; ``null`` until enough
   picks and rejects taught it, see :ref:`features-ai-taste`) and ``faces``
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
   ``days`` (newest first), ``cameras`` (with display labels; spellings
   of one camera are one entry, keyed by the most used), ``lenses``,
   ``places``, ``flags``, ``ratings`` (counts for 0–5 stars), plus
   ``total``, ``first_day`` and ``last_day``.

``library_stats``
   No arguments. Keepers per camera and lens and focal lengths per camera
   (:doc:`/features/statistics`):

   .. code-block:: json

      { "all": { "photos": 812, "picks": 120, "auto_picked": 64, "rejects": 230 },
        "keeper_rate": 0.23,
        "cameras": [{ "label": "Canon EOS R6", "camera": { "make": "Canon", "model": "Canon EOS R6" },
                      "lens": null, "counts": { "photos": 500, "picks": 90, "auto_picked": 40, "rejects": 150 },
                      "keeper_rate": 0.26, "few_photos": false,
                      "focal_lengths": [{ "value": 35, "count": 210 }, { "value": 85, "count": 120 }] }],
        "lenses": [{ "label": "RF35mm F1.8", "camera": null, "lens": "RF35mm F1.8", "...": "..." }],
        "min_photos": 20 }

   A keeper is a picked or auto-picked photo; ``keeper_rate`` is keepers
   per photo, 0–1. ``cameras`` and ``lenses`` come most successful first:
   by keeper rate, then by photos, with gear of fewer than ``min_photos``
   photos (``few_photos``) last. ``focal_lengths`` (cameras only) counts
   photos per focal length in whole millimetres, most used first.

``reanalyze_photos``
   Optional ``{ "photo_ids": [7, 8] }``; without it, every photo. Marks
   the photos' analysis results stale so background analysis runs again
   (:ref:`features-library-reanalyze`), the given photos first. Stale
   results stay in use until replaced. Returns ``{ "photos": 2,
   "waiting_for_models": ["faces and closed eyes"] }``: how many of the
   photos exist (unknown ids are ignored), and what is not analyzed until
   its model is downloaded (as in ``analysis_status``).

``taste_status``
   No arguments. What the personal taste model learned
   (:ref:`features-ai-taste`):

   .. code-block:: json

      { "picks": 42, "rejects": 118, "needed": 10, "learned": true,
        "agreement": 0.87 }

   ``picks`` and ``rejects`` count decided photos with a CLIP embedding;
   ``needed`` of each are required before anything is learned.
   ``agreement`` is the cross-validated chance that the model scores one
   of the user's picks above one of their rejects (``null`` until
   learned).

``critic_status``
   No arguments. Whether photo critiques can run
   (:ref:`features-ai-critique`). While critiques are on, asks the server
   for its models (5 s timeout):

   .. code-block:: json

      { "enabled": true, "backend": "server",
        "server": "http://127.0.0.1:11434/v1", "local": true,
        "reachable": true, "models": ["qwen3-vl:4b"], "model": "qwen3-vl:4b",
        "model_installed": true, "problem": null,
        "suggested": [{ "name": "qwen3-vl:2b", "size": 1900000000,
                        "license": "Apache-2.0", "license_url": "…", "note": "…" }],
        "default_rubric": "- Composition: …" }

   With the ``builtin`` backend, ``reachable`` says whether the helper is
   installed, ``model_installed`` whether the model is downloaded, and
   ``model`` is ``"Qwen3-VL 4B (built-in)"``; the server is not asked.
   ``local`` is ``false`` when the server is on another computer.
   ``problem`` says what to do when the server is not running or lacks the
   model.

``get_critique``
   ``{ "photo_id": 7 }``. The stored critique by the current model and
   rubric, or ``null``. Never contacts the server.

``critique_photo``
   ``{ "photo_id": 7, "refresh": false }``. Asks the model for a critique,
   or returns the stored one unless ``refresh``. Can take minutes on a
   CPU. Returns ``{ "model": "qwen3-vl:4b", "rating": 7.5, "score": 0.72,
   "summary": "…", "strengths": ["…"], "weaknesses": ["…"] }`` (``rating``
   1–10, ``score`` the same as 0–1). ``invalid_arguments`` while critiques
   are off; ``internal`` with the reason when the server fails or the reply
   is not a critique.

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

``sidecar_status``
   No arguments. ``{ "pending": 0, "failed": 0, "error": null, "folder":
   "/home/me/Pictures/Clean" }``: photos whose XMP sidecar does not yet
   hold their latest decision, photos whose sidecar could not be written
   on the latest attempt (with the first reason), and the folder sidecars
   are written in (``null`` before a library is chosen). See
   :ref:`features-library-sidecars`.

``analysis_status``
   No arguments. ``{ "analyzed": 120, "total": 300, "running": true,
   "paused": null, "waiting_for_models": ["AI scores and similar photos"],
   "ai_unavailable": null }``: photos with every analysis done, all
   photos, whether workers are busy, why analysis waits (a full disk), if
   it does (:doc:`/architecture/decisions/0011-resumable-background-analysis`),
   what is not analyzed until its AI model is downloaded (empty with every
   model downloaded), and why AI analysis cannot run on this computer at
   all, if so (a processor without AVX2,
   :doc:`/architecture/decisions/0015-onnx-runtime-loaded-at-first-use`).

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

.. _reference-api-import:

``list_import_sources``
   No arguments. Every ``ImportSource``: ``id``, ``path``, ``watch``,
   ``scanned_at`` and ``scan_error``.

``add_import_source``
   ``{ "path": "/absolute/folder", "watch": true }`` (``watch`` defaults
   to ``true``). Stores the folder and scans it right away; returns the
   ``ImportSource``. Adding a stored folder again returns it unchanged.
   Fails if the path is relative, cannot be read, or is inside or contains
   the library folder.

``remove_import_source``
   ``{ "id": 1 }``. Forgets the folder and drops its waiting photos;
   imported photos stay. Returns the remaining sources.

``set_import_source_watch``
   ``{ "id": 1, "watch": false }``. Returns the ``ImportSource``.

``scan_import_sources``
   ``{ "id": 1 }`` scans one source; without ``id``, all of them.
   Returns the ``PipelineStatus`` at once; the scan runs in the background.

``pipeline_status``
   No arguments. The ``PipelineStatus``: number of photos ``queued``,
   ``imported``, ``duplicates`` and ``failed``; the ``active`` photo
   (``job_id``, ``file_name``, ``stage``) or ``null``; whether a scan is
   running; and ``paused`` (why importing waits, for example no library
   folder) or ``null``.

``pipeline_jobs``
   ``{ "filter": "all", "limit": 200, "before": 1234 }`` (all optional).
   Photos in the pipeline, newest first, as ``JobView``: ``id``,
   ``file_name``, ``source_path``, ``files``, ``stage`` (``queued``,
   ``reading``, ``copying``, ``imported``, ``duplicate``, ``failed``),
   ``error``, ``failed_stage`` (``reading`` or ``copying``, for failed
   photos), ``photo_id``, ``updated_at``, and for imported photos
   whether the ``thumbnail`` is ready and ``analysis`` progress as
   ``[done, total]``. ``filter`` is ``all``, ``active``, ``imported``,
   ``duplicate`` or ``failed``; page with ``before`` (the last ``id``).

``pipeline_log``
   ``{ "min_level": "info", "limit": 200, "before": 5678 }`` (all
   optional). Log entries newest first: ``id``, ``at``, ``level``
   (``info``, ``warn``, ``error``), ``job_id``, ``photo_id`` and
   ``message``.

``retry_failed_imports``
   ``{ "ids": [3, 4] }`` queues those failed photos again; without
   ``ids``, all failed photos. Returns the ``PipelineStatus``.

Images
------

Photo images are served as JPEG files, not JSON, so they can be used
directly as ``<img src>``:

.. list-table::
   :header-rows: 1

   * - Where
     - URL
   * - Desktop app
     - ``picturesque://localhost/{size}%2F{photo_id}[%2F{view}]`` (``http://picturesque.localhost/...``
       on Windows); built with ``convertFileSrc`` by ``imageUrl()`` in
       ``ui/src/lib/api``
   * - Development server
     - ``GET /api/images/{size}/{photo_id}[/{view}]``

``size`` is ``grid`` (longest edge 512 px), ``preview`` (2048 px) or
``full`` (native size, for 100% zoom). The optional ``view`` is ``image``
or ``raw``: the file of a RAW+JPEG pair to show. Without it, the
:ref:`pair_view <reference-settings-pair-view>` setting decides; photos
with one file ignore it.
Images are generated on first request and cached
(:doc:`data-locations`). Responses carry an ``ETag``; a request with a
matching ``If-None-Match`` gets ``304 Not Modified``. An unknown photo is
``404``, a malformed path ``400``.

Events
------

Events are JSON objects with a ``type`` field. In the desktop app they
arrive as the Tauri event ``app-event``; in development, as server-sent
events from ``GET /api/events``.

``pipeline_progress``
   A ``PipelineStatus`` (see ``pipeline_status``), sent at most four
   times a second while the pipeline works, and whenever a photo finishes
   or a scan starts or ends.

``analysis_progress``
   The ``analysis_status``, sent after each analyzed photo.

``sidecar_progress``
   The ``sidecar_status``, sent when decisions change and after each
   batch of sidecars is written.

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

Example:

.. code-block:: json

   { "type": "pipeline_progress", "queued": 280, "imported": 120,
     "duplicates": 3, "failed": 1, "scanning": false, "paused": null,
     "active": { "job_id": 124, "file_name": "IMG_0124.CR3", "stage": "copying" } }
