Path Templates
==============

A path template decides where each imported photo goes inside the Clean
library. It is a path relative to the library folder, with ``/`` between
folders and tokens in braces:

.. code-block:: text

   {year|Undated}/{date} {city}/{name}.{ext}

renders, for a photo taken in Lisbon on 12 October 2026, as::

   2026/2026-10-12 Lisbon/IMG_0042.CR3

The template is set in :ref:`reference-settings-path-template`.

.. _reference-path-templates-tokens:

Tokens
------

.. list-table::
   :header-rows: 1
   :widths: 20 45 35

   * - Token
     - Value
     - Example
   * - ``{year}``
     - Capture year
     - ``2026``
   * - ``{month}``
     - Capture month, two digits
     - ``10``
   * - ``{day}``
     - Capture day, two digits
     - ``12``
   * - ``{date}``
     - Capture date
     - ``2026-10-12``
   * - ``{time}``
     - Capture time, ``HHMMSS``
     - ``143005``
   * - ``{city}``
     - Place name (:ref:`features-import-places`)
     - ``Lisbon``
   * - ``{region}``
     - Region or state
     - ``Lisbon``
   * - ``{country}``
     - Country name
     - ``Portugal``
   * - ``{country_code}``
     - ISO country code
     - ``PT``
   * - ``{camera}``
     - Camera name without a repeated brand
     - ``Canon EOS R5``
   * - ``{make}``
     - Camera brand
     - ``Canon``
   * - ``{lens}``
     - Lens model
     - ``RF24-70mm F2.8 L IS USM``
   * - ``{name}``
     - Original file name without extension
     - ``IMG_0042``
   * - ``{seq}``
     - Position in the import, four digits, in capture order
     - ``0007``
   * - ``{ext}``
     - Original extension of each file
     - ``CR3``

Dates and times are the local time the photo was taken
(:ref:`features-import-metadata`).

Rules
-----

- The template must end with ``.{ext}``, and ``{ext}`` may appear nowhere
  else. All files of a photo (RAW, JPEG and sidecars) get the same name and
  differ only in extension, so a RAW+JPEG pair stays a pair.
- The template is relative: it must not start with ``/`` or contain
  ``..``.
- Unknown tokens and unbalanced braces are rejected with a message listing
  the valid tokens.

Missing values
--------------

When a photo has no value for a token (no GPS, so no ``{city}``), the
token renders as nothing, and one run of separators (spaces, ``-``,
``_``, ``,``, ``.``) next to it in the template is removed. Folders that
end up empty are skipped:

.. list-table::
   :header-rows: 1

   * - Template
     - Without a place
   * - ``{year}/{date} {city}/{name}.{ext}``
     - ``2026/2026-10-12/IMG_0042.CR3``
   * - ``{year}/{city}/{name}.{ext}``
     - ``2026/IMG_0042.CR3``

Give a token a fallback with ``|``: ``{city|Unknown place}`` renders
*Unknown place* when the photo has no place. If the file name would be
empty, the original name is used.

Original names are never trimmed: ``_DSC0001`` stays ``_DSC0001``.

File names that work everywhere
-------------------------------

Names are adjusted so the library works when the drive is moved between
Linux, macOS and Windows:

- ``< > : " / \ | ? *`` and control characters become ``_``. A ``/`` inside
  a value (for example in a place name) becomes ``-``, so values never
  create folders.
- Repeated whitespace is collapsed; trailing dots and spaces are removed.
- Windows device names (``CON``, ``NUL``, ``COM1`` …) get a ``_`` suffix.
- Names longer than 200 bytes are shortened.

Name collisions
---------------

If the target name is already taken (in the library, or by another photo
in the same import), a counter is added before the extension:
``IMG_0042 (2).CR3``. Existing files are never overwritten
(:doc:`ADR-0008 </architecture/decisions/0008-non-destructive-verified-import>`).

Presets
-------

.. list-table::
   :header-rows: 1

   * - Preset
     - Template
     - Example
   * - By date (default)
     - ``{year|Undated}/{date}/{name}.{ext}``
     - ``2026/2026-10-12/IMG_0042.CR3``
   * - By date and place
     - ``{year|Undated}/{date} {city}/{name}.{ext}``
     - ``2026/2026-10-12 Lisbon/IMG_0042.CR3``
   * - By month
     - ``{year|Undated}/{year}-{month}/{name}.{ext}``
     - ``2026/2026-10/IMG_0042.CR3``
   * - By country, then date
     - ``{country|Unknown place}/{year|Undated}/{date} {city}/{name}.{ext}``
     - ``Portugal/2026/2026-10-12 Lisbon/IMG_0042.CR3``
   * - By date and camera
     - ``{year|Undated}/{date}/{camera}/{name}.{ext}``
     - ``2026/2026-10-12/Canon EOS R5/IMG_0042.CR3``
   * - Renamed by capture time
     - ``{year|Undated}/{date}/{date}_{time}_{seq}.{ext}``
     - ``2026/2026-10-12/2026-10-12_143005_0007.CR3``

Photos without a capture date go to ``Undated/`` with the date presets.
