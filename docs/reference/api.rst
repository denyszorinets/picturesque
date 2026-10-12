API
===

The UI talks to the backend through named commands with JSON arguments
(:ref:`architecture-components-command-api`). In the desktop app they go
through the ``api`` Tauri command; in development, through
``POST /api/commands/{name}`` (:doc:`/development/ui-in-browser`). Types
are generated into ``ui/src/lib/api/generated/``.

Errors are ``{ "code": ..., "message": ... }`` with ``code`` one of
``invalid_arguments``, ``unknown_command`` and ``internal``. Messages are
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
