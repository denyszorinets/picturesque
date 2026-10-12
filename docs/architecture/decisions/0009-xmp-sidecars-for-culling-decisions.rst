ADR-0009: XMP Sidecars for Culling Decisions
============================================

:Status: Accepted
:Date: 2026-10-12

Context
-------

After culling, photographers edit their picks in other tools such as
Lightroom, darktable or digiKam. Ratings and pick/reject decisions made in
Picturesque should carry over without manual re-entry. Original photo files
must not be modified
(:doc:`ADR-0008 <0008-non-destructive-verified-import>`).

Decision
--------

Write decisions to **XMP sidecar files** next to each photo in the Clean
library, and keep the catalog as Picturesque's source of truth.

- Written fields: ``xmp:Rating`` (−1 for rejected, as darktable does; 0–5
  otherwise) and ``xmp:Label`` for colour labels. The exact field mapping
  per tool is settled and documented in :issue:`29`.
- Existing sidecars are merged, not overwritten: unknown namespaces and
  fields are preserved.
- Writes are debounced and happen in the background after a decision
  changes.
- On import, values from existing sidecars are read into the catalog.

Alternatives Considered
-----------------------

Embed metadata in the image files
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Advantages:

- No extra files.

Disadvantages:

- Modifies originals; impossible for most RAW formats; risk of corruption.

Catalog only
~~~~~~~~~~~~

Advantages:

- Simplest.

Disadvantages:

- Decisions are locked inside Picturesque.

Decision Criteria
-----------------

Interoperability, safety of originals, compatibility with existing sidecars.

Rationale
---------

XMP sidecars are the common interchange format for RAW workflows, and
writing them never touches the photo files. Keeping the catalog
authoritative avoids parsing sidecars on every read; sidecars are an export
of state.

Consequences
------------

Positive
~~~~~~~~

- Ratings and rejects appear in other tools.
- Originals untouched.

Negative
~~~~~~~~

- Edits made in other tools are not synced back automatically after import.

Risks
~~~~~

- Tools disagree on how to represent pick/reject; some fields may not be
  understood by every tool.

Follow-up
~~~~~~~~~

- Implement the sidecar writer (:issue:`29`).

Validation
----------

:issue:`29` checks sidecars are read correctly by darktable and digiKam.

References
----------

- `XMP specification <https://developer.adobe.com/xmp/docs/>`_
