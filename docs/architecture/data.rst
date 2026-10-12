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

This is the intended model. The schema is created with migrations in
:issue:`13`, and this page is updated as it evolves.

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

``scores``
   Technical scores (sharpness, exposure clipping, noise), AI scores
   (aesthetics, faces, eyes closed), the combined score, and the model and
   algorithm versions that produced them, so outdated scores can be
   recomputed.

``embedding``
   CLIP image embedding and model version. Used for semantic search and
   visual similarity.

``series``
   A group of photos taken close together that look alike. Records the best
   photo and whether membership was edited by the user, so manual splits and
   merges survive re-analysis.

``decision``
   Pick, reject or unflagged; star rating; colour label; timestamp of the
   last change and of the last sidecar write.

``import`` / ``import_item``
   An import run and each planned source file: source path, source hash,
   target path, status (planned, copied, verified, skipped as duplicate,
   failed). Lets an interrupted import resume.

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
