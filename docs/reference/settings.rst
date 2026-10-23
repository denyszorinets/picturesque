Settings
========

Settings are stored in the catalog database and apply immediately; no
restart is needed. Invalid values are rejected with a message explaining
what to change, and nothing is saved when any value in a change is
invalid.

.. _reference-settings-library-root:

``library_root``
----------------

The root folder of the Clean library, where imported photos are copied.

:Type: absolute path, or unset
:Default: unset (chosen on first import)
:Validation: must be an absolute path to an existing folder that
   Picturesque can create files in. Once the library has photos, the
   folder cannot be changed (photo locations are stored relative to it);
   moving a library is not supported yet. Writability is checked by creating and
   removing a temporary file, so read-only mounts and folders without
   permission are caught.

.. _reference-settings-path-template:

``path_template``
-----------------

How photos are arranged inside the library. Tokens in braces are replaced
with values from each photo.

:Type: text
:Default: ``{year|Undated}/{date}/{name}.{ext}``, for example
   ``2026/2026-10-12/IMG_0042.CR3``
:Validation: must be a valid template; see :doc:`path-templates` for the
   tokens and rules. An invalid template is rejected with a message naming
   the problem.

.. _reference-settings-pair-view:

``pair_view``
-------------

Which file of a RAW+JPEG pair the grid, loupe and compare show
(:ref:`features-library-raw-jpeg`). In the Settings tab: **RAW+JPEG pairs**.

:Type: ``image`` or ``raw``
:Default: ``image``
:Values: ``image`` shows the camera's JPEG (or other rendered image, such as
   HEIC); ``raw`` shows the RAW through its embedded preview. Photos with
   only one of the two show that one either way. The loupe can switch to the
   other file at any time (**R**). Analysis always uses the JPEG of a pair,
   so scores do not change with this setting. Applies right away; grid
   images of the newly chosen files are made as they come into view.

.. _reference-settings-sidecar-naming:

``sidecar_naming``
------------------

How new XMP sidecars are named (:ref:`features-library-sidecars`).
Existing sidecars are always updated where they are.

:Type: ``file_extension`` or ``base_name``
:Default: ``file_extension``
:Values: ``file_extension`` writes ``IMG_0042.CR3.xmp`` (darktable,
   digiKam); ``base_name`` writes ``IMG_0042.xmp`` (Lightroom, Bridge).

.. _reference-settings-series:

``series``
----------

When photos form a series (:doc:`/features/series`).

:Type: object with ``max_gap_seconds`` and ``max_distance``
:Default: ``{ "max_gap_seconds": 30, "max_distance": 16 }``
:Values: ``max_gap_seconds``: the longest pause between consecutive shots
   of a series, 1–3600. ``max_distance``: the most perceptual-hash bits a
   photo may differ from the series' latest photos, 0–32; lower is
   stricter (:doc:`similarity`).

Changing it regroups series immediately.

.. _reference-settings-ranking:

``ranking``
-----------

How much each score counts when choosing the best photo of a series
(:doc:`/features/series`).

:Type: object with ``sharpness``, ``exposure``, ``noise`` and ``aesthetic``
:Default: ``{ "sharpness": 0.6, "exposure": 0.3, "noise": 0.1, "aesthetic": 0.4 }``
:Values: numbers, zero or more, at least one of the technical weights above
   zero. Only the ratios matter: ``{1, 0, 0, 0}`` ranks by sharpness alone.
   ``aesthetic`` counts only once the CLIP model has analyzed the photos.

Changing it re-ranks series immediately.

.. _reference-settings-ai:

``ai``
------

Where local AI models run (:doc:`/features/ai`).

:Type: object with ``provider``
:Default: ``{ "provider": "auto" }``
:Values: ``auto``, ``cpu``, ``directml``, ``coreml`` or ``webgpu``. A
   provider the build lacks, or that fails to start, falls back to the CPU.

.. _reference-settings-auto-pick:

``auto_pick``
-------------

Which photos the app marks as keepers
(:ref:`features-library-auto-pick`).

:Type: object with ``enabled`` and ``min_quality``
:Default: ``{ "enabled": true, "min_quality": 0.5 }``
:Values: ``min_quality``: the combined quality score a photo needs,
   0–1 (shown as a percentage in Settings).

Changing it updates the suggestions immediately.

.. _reference-settings-import:

``import``
----------

Watched import folders (:ref:`features-import-watching`).

:Type: object with ``rescan_minutes``
:Default: ``{ "rescan_minutes": 5 }``
:Values: ``rescan_minutes``: how often watched folders are scanned again,
   0–1440 minutes. ``0`` scans only when a folder reports a change and when
   the app starts.

.. _reference-settings-labels:

``labels``
----------

The colour labels written to XMP sidecars (:ref:`features-library-sidecars`),
so tools without a pick flag, like darktable, show picks.

:Type: object with ``pick`` and ``auto_pick``, each ``red``, ``yellow``,
   ``green``, ``blue``, ``purple`` or ``null`` (no label)
:Default: ``{ "pick": "green", "auto_pick": "yellow" }``
:Behaviour: ``pick`` is written for picks; ``auto_pick`` for auto-picked
   photos when a folder is finished. A change applies to sidecars written
   from then on.

.. _reference-settings-delete-after-copy:

``delete_after_copy``
---------------------

What **Clear folder…** may delete after keepers were copied out of a
folder culled in place (:ref:`features-folders-clear`).

:Type: ``never``, ``verified`` or ``verified_and_rejects``
:Default: ``verified``
:Behaviour: ``verified`` deletes photos whose copies are verified again
   first; ``verified_and_rejects`` also deletes rejected photos, which
   have no copy; ``never`` turns the action off. Deleting is permanent and
   only happens when asked.

.. _reference-settings-folders:

``open_folder``, ``recent_folders``
-----------------------------------

The folder culled in place, and the folders opened recently
(:doc:`/features/folders`). They are changed with the ``open_folder`` and
``open_library`` commands (:doc:`api`), not with ``update_settings``.

:Type: ``open_folder``: absolute path, or ``null`` in the library.
   ``recent_folders``: list of ``{ "path", "opened_at" }``, most recent
   first, ``opened_at`` in seconds since 1970.
:Default: ``null`` and ``[]``
:Behaviour: while a folder is open, ``library_root`` reads as that folder;
   the library folder and ``path_template`` are kept for the library.
   Folders not opened for 30 days are removed from the list at startup.

Where settings are stored
-------------------------

In the catalog database; see :doc:`data-locations`.

API
---

``get_settings``
   No arguments. Returns all settings:

   .. code-block:: json

      { "library_root": null, "path_template": "{year|Undated}/{date}/{name}.{ext}",
        "sidecar_naming": "file_extension",
        "series": { "max_gap_seconds": 30, "max_distance": 16 },
        "ranking": { "sharpness": 0.6, "exposure": 0.3, "noise": 0.1,
                     "aesthetic": 0.4 },
        "ai": { "provider": "auto" },
        "import": { "rescan_minutes": 5 },
        "auto_pick": { "enabled": true, "min_quality": 0.5 },
        "onboarding": { "tour_done": false },
        "open_folder": null, "recent_folders": [],
        "labels": { "pick": "green", "auto_pick": "yellow" },
        "delete_after_copy": "verified", "pair_view": "image" }

``update_settings``
   Arguments: any subset of the settings. Unknown fields are rejected.
   Returns all settings after the change.

   .. code-block:: console

      $ curl -s -X POST http://127.0.0.1:5174/api/commands/update_settings \
          -d '{"library_root": "/does/not/exist"}'
      {"code":"invalid_arguments","message":"Cannot use `/does/not/exist` as the library folder: it does not exist. Choose an existing folder you can write to."}
