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
     PHOTO ||--o| SCORES : has
     PHOTO ||--o| EMBEDDING : has
     PHOTO }o--o| SERIES : "member of"
     PHOTO ||--o| DECISION : has
     IMPORT ||--|{ IMPORT_ITEM : contains
     IMPORT_ITEM }o--o| ASSET : produced

``photo``
   One captured image. Capture time (with UTC offset when known), camera,
   lens, exposure, GPS, resolved place, orientation.

``asset``
   One file belonging to a photo: the RAW, the JPEG, an XMP sidecar. Stores
   the path inside the Clean library, size and BLAKE3 content hash. RAW+JPEG
   pairs are two assets of one photo.

``scores`` (planned)
   Technical scores (sharpness, exposure clipping, noise), AI scores
   (aesthetics, faces, eyes closed), the combined score, and the model and
   algorithm versions that produced them, so outdated scores can be
   recomputed.

``embedding`` (planned)
   CLIP image embedding and model version. Used for semantic search and
   visual similarity.

``series`` (planned)
   A group of photos taken close together that look alike. Records the best
   photo and whether membership was edited by the user, so manual splits and
   merges survive re-analysis.

``decision`` (planned)
   Pick, reject or unflagged; star rating; colour label; timestamp of the
   last change and of the last sidecar write.

``import`` / ``import_item`` (planned, :issue:`19`)
   An import run and each planned source file: source path, source hash,
   target path, status (planned, copied, verified, skipped as duplicate,
   failed). Lets an interrupted import resume.

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

Conventions:

- Capture time is stored as local ISO 8601 text (``captured_local``),
  which sorts chronologically, plus the UTC offset in seconds when known.
- Asset paths are relative to the Clean library root and always use
  ``/``, so a library on an external drive reads the same on every
  operating system.
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
