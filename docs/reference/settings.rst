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

.. _reference-settings-sidecar-naming:

``sidecar_naming``
------------------

How new XMP sidecars are named (:ref:`features-library-sidecars`).
Existing sidecars are always updated where they are.

:Type: ``file_extension`` or ``base_name``
:Default: ``file_extension``
:Values: ``file_extension`` writes ``IMG_0042.CR3.xmp`` (darktable,
   digiKam); ``base_name`` writes ``IMG_0042.xmp`` (Lightroom, Bridge).

Where settings are stored
-------------------------

In the catalog database; see :doc:`data-locations`.

API
---

``get_settings``
   No arguments. Returns all settings:

   .. code-block:: json

      { "library_root": null, "path_template": "{year|Undated}/{date}/{name}.{ext}",
        "sidecar_naming": "file_extension" }

``update_settings``
   Arguments: any subset of the settings. Unknown fields are rejected.
   Returns all settings after the change.

   .. code-block:: console

      $ curl -s -X POST http://127.0.0.1:5174/api/commands/update_settings \
          -d '{"library_root": "/does/not/exist"}'
      {"code":"invalid_arguments","message":"Cannot use `/does/not/exist` as the library folder: it does not exist. Choose an existing folder you can write to."}
