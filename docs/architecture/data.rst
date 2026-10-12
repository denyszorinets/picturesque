Data
====

Picturesque keeps three kinds of data:

.. list-table::
   :header-rows: 1

   * - Data
     - Location
     - Source of truth for
   * - Photos and XMP sidecars
     - Clean library (user-chosen folder)
     - The photo files; decisions shared with other tools
   * - Catalog
     - ``catalog.sqlite`` in the app data directory
     - Metadata, scores, series, decisions, import history, settings
   * - Caches
     - App cache directory
     - Thumbnails and previews (always rebuildable)

The catalog is the source of truth for decisions inside Picturesque. XMP
sidecars are written from it
(:doc:`ADR-0009 </architecture/decisions/0009-xmp-sidecars-for-culling-decisions>`);
on import, existing sidecar values are read into the catalog.

Catalog entities
----------------

Entities marked *planned* do not have tables yet; they are added by the
task that introduces them. The rest is in the schema today
(:ref:`architecture-data-schema`).

.. mermaid::

   erDiagram
     PHOTO ||--|{ ASSET : "has files"
     PHOTO ||--o{ ANALYSIS : has
     PHOTO ||--o| SCORES : has
     PHOTO ||--o| EMBEDDING : has
     PHOTO ||--o| SERIES_MEMBER : "member of"
     PHOTO ||--o| SERIES_EDIT : "corrected by"
     PHOTO ||--o| DECISION : has
     IMPORT_RUN ||--|{ IMPORT_PHOTO : contains
     IMPORT_PHOTO ||--|{ IMPORT_FILE : has
     IMPORT_PHOTO }o--o| PHOTO : "becomes"

``photo``
   One captured image. Capture time (with UTC offset when known), camera,
   lens, exposure, GPS, resolved place, orientation.

``asset``
   One file belonging to a photo: the RAW, the JPEG, an XMP sidecar. Stores
   the path inside the Clean library, size and BLAKE3 content hash. RAW+JPEG
   pairs are two assets of one photo.

``analysis``
   One analyzer's result for a photo: analyzer name, version and a JSON
   result (or ``{"error": ...}``). A photo is analyzed again when an
   analyzer's version changes
   (:doc:`ADR-0011 </architecture/decisions/0011-resumable-background-analysis>`).

``scores`` (planned)
   Technical scores are stored as the ``technical`` analysis result
   (:doc:`/reference/quality-scores`). AI scores (aesthetics, faces, eyes
   closed) and the combined score will join them; values needed for
   filtering and sorting may be copied into columns (:issue:`38`).

``embedding`` (planned)
   CLIP image embedding and model version. Used for semantic search and
   visual similarity. Until then, similarity uses a perceptual hash stored
   as the ``similarity`` analysis result (:doc:`/reference/similarity`).

``series_member`` / ``series_edit``
   Series are derived: ``series_member`` maps each photo in a series to the
   series (identified by its first photo) and its position, and is
   rewritten whenever series are recomputed. ``series_edit`` holds the
   user's corrections (split or join before a photo), which are kept and
   applied on every run (:doc:`/features/series`). The best photo of a
   series is planned (:issue:`35`).

``decision``
   Pick, reject or unflagged; star rating; timestamp of the
   last change and of the last sidecar write.

``import_run`` / ``import_photo`` / ``import_file``
   An import run (state, library folder, template, sources, problems),
   each photo in it (capture metadata as JSON, status: planned, duplicate,
   imported or failed, and the catalog photo once imported) and each file
   (source path, size and modification time at planning, hash when known,
   target path, status: planned, verified or failed). Lets plans be
   reviewed and interrupted imports resume.

.. _architecture-data-schema:

Schema and migrations
---------------------

The schema lives in numbered SQL migrations in
:repo:`crates/infra/src/catalog/migrations/`, embedded in the binary and
applied in order when the catalog opens. ``PRAGMA user_version`` records
how many have run. Each migration runs in its own transaction.

- Migrations are **append-only**: once merged, a migration is never edited;
  changes go in a new migration.
- A catalog whose ``user_version`` is newer than the running build is
  refused rather than modified, so downgrading the app cannot damage it.
- The catalog uses WAL journaling and enforces foreign keys.

.. list-table::
   :header-rows: 1

   * - Version
     - Migration
     - Adds
   * - 1
     - ``0001_photos.sql``
     - ``photo`` (capture metadata, place, orientation, dimensions) and
       ``asset`` (role, format, library path, size, BLAKE3 hash)
   * - 2
     - ``0002_settings.sql``
     - ``setting``: JSON documents by key; the settings document lives
       under ``settings`` (:doc:`/reference/settings`)
   * - 3
     - ``0003_imports.sql``
     - ``import_run``, ``import_photo``, ``import_file``
       (:doc:`/features/import`)
   * - 4
     - ``0004_decisions.sql``
     - ``decision`` (flag, rating, when the sidecar was last written);
       indexes for browsing by date, camera, lens and place
   * - 5
     - ``0005_sidecars.sql``
     - ``decision.revision`` / ``sidecar_revision`` (a sidecar is up to
       date when it reflects the latest revision); ``import_photo.decision``
       (decisions read from existing sidecars)
   * - 6
     - ``0006_analysis.sql``
     - ``analysis`` (result per photo and analyzer, with version)
   * - 7
     - ``0007_series.sql``
     - ``series_member`` (derived series membership) and ``series_edit``
       (user corrections)

Conventions:

- Capture time is stored as local ISO 8601 text (``captured_local``),
  which sorts chronologically, plus the UTC offset in seconds when known.
- Asset paths are relative to the Clean library root and always use
  ``/``, so a library on an external drive reads the same on every
  operating system.
- A photo without a ``decision`` row is unflagged and unrated; queries
  use ``LEFT JOIN`` with defaults.
- Browsing queries page with keyset cursors on
  ``(captured_local, id)``; over 10,000 photos a filtered page takes well
  under 50 ms (tested).
- A unique partial index on ``asset.hash`` (excluding sidecars) enforces
  that each RAW or image content is catalogued once.

Identity and duplicates
-----------------------

A file's identity is its BLAKE3 hash. Importing a file whose hash already
exists in the catalog is skipped as an exact duplicate
(:doc:`ADR-0008 </architecture/decisions/0008-non-destructive-verified-import>`).
Near-duplicates (the same scene shot twice) are not deduplicated; they are
grouped into series instead.

Data flow
---------

.. mermaid::

   flowchart LR
     scan[Scan sources] --> meta[Read metadata] --> geo[Reverse geocode]
     geo --> plan[Render target paths] --> copy[Copy + verify] --> rec[Record in catalog]
     rec --> thumbs[Thumbnails] --> analyze[Scores + embeddings]
     analyze --> series[Group series] --> rank[Rank best]
     rank --> ui[Timeline UI]
     ui --> decide[Decisions] --> xmp[XMP sidecars]
