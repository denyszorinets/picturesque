ADR-0009: XMP Sidecars for Culling Decisions
============================================

:Status: Accepted; amended by
   :doc:`ADR-0013 <0013-folder-sessions>` (in a folder session, sidecars
   are written next to the originals in that folder)
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

Findings (2026-10-12), from implementing :issue:`29`:

- Field mapping: ``xmp:Rating`` (stars, −1 for rejects),
  ``digiKam:PickLabel`` (3 pick, 1 reject) and Picturesque's own
  ``picturesque:Flag`` / ``picturesque:Stars`` namespace, needed because
  ``xmp:Rating = -1`` loses a rejected photo's stars.
- Merging rewrites the XML as an event stream with namespace resolution,
  so properties are matched by namespace URI whatever their prefix
  (Lightroom files still use ``xap:``) and in attribute or element form;
  everything else, including comments, is copied. ``xmltree`` was rejected
  because it drops attribute prefixes. Merging is idempotent (tested).
- New sidecars are named ``IMG_0042.CR3.xmp`` by default; a setting
  switches to ``IMG_0042.xmp`` for Lightroom.
- Writes are debounced in a background worker. A per-decision revision
  counter, rather than a timestamp, decides whether a sidecar is current,
  so a change made while a sidecar is being written is never marked as
  written.
- Reading existing sidecars on import (the last part of this decision) is
  implemented.
- Verifying in darktable and digiKam needs a desktop session; the steps
  are documented in :ref:`features-library-sidecars` and are still to be
  carried out.

References
----------

- `XMP specification <https://developer.adobe.com/xmp/docs/>`_
