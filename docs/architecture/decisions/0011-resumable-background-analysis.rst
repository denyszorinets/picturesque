ADR-0011: Resumable Background Analysis
=======================================

:Status: Accepted
:Date: 2026-10-12

Context
-------

Choosing good photos and series needs per-photo analysis: technical
scores (:issue:`32`), perceptual hashes (:issue:`33`), and later AI models
(:doc:`ADR-0006 <0006-local-ai-with-onnx-runtime>`). Analysis takes from
milliseconds to seconds per photo, a library holds up to about 10,000
photos, and the app may be closed at any time. Analyzers will gain new
versions as algorithms and models improve.

Decision
--------

Run analysis as a **resumable background queue** in the application layer.

- An *analyzer* is a function from a photo and its pixels to a JSON
  result, identified by a name and a version.
- Results are stored in one ``analysis`` table, one row per photo and
  analyzer, with the version. A photo is *pending* while any analyzer lacks
  a row at its current version. No separate queue is persisted.
- Analyzers get the cached 2048 px preview
  (:doc:`ADR-0005 <0005-image-decoding-stack>`) shrunk to 1024 px RGB, so a
  RAW file is decoded once for display and analysis.
- Workers (half the CPU cores) take photos the UI asks for first (the ones
  on screen), then the rest oldest first. They are woken when an import
  completes.
- A failed analysis is stored as ``{"error": ...}`` so a broken file is not
  retried forever; a new analyzer version retries it.
- Pure algorithms live in ``picturesque-core`` and take pixel buffers;
  decoding is behind the ``PixelLoader`` port.

Alternatives Considered
-----------------------

A persisted job queue
~~~~~~~~~~~~~~~~~~~~~

Advantages:

- Explicit job states, retries and history.

Disadvantages:

- Duplicates what the results already say; jobs and results can drift
  apart; needs cleanup when photos are deleted.

Analysis during import
~~~~~~~~~~~~~~~~~~~~~~

Advantages:

- Results ready when the import finishes.

Disadvantages:

- Slows imports; nothing reruns when an analyzer improves.

Typed columns per score
~~~~~~~~~~~~~~~~~~~~~~~

Advantages:

- Scores can be filtered and sorted in SQL directly.

Disadvantages:

- A migration for every new analyzer. Values needed for filtering can
  still be indexed with SQLite JSON expressions or copied into columns
  later (:issue:`38`).

Decision Criteria
-----------------

Survives restarts, handles analyzer upgrades, keeps imports and the UI
responsive, keeps algorithms testable without I/O.

Rationale
---------

Deriving pending work from the results makes resuming and upgrades the
same mechanism and leaves nothing to clean up. Prioritizing visible photos
gives useful scores where the user is looking within seconds.

Consequences
------------

Positive
~~~~~~~~

- Restarting the app or upgrading an analyzer needs no special handling.
- New analyzers only implement one trait.

Negative
~~~~~~~~

- Finding pending photos is a query over photos and results; fine at
  10,000 photos.
- Results are JSON, so code reading them must handle missing or error
  values.

Risks
~~~~~

- Analysis competes with the UI for CPU; the worker count may need a
  setting.

Follow-up
~~~~~~~~~

- Technical scores (:issue:`32`) and perceptual hashes (:issue:`33`) are
  the first analyzers.
- AI analyzers need a downloaded model (:issue:`43`): an analyzer reports
  whether it is *ready*, and photos are pending only for ready analyzers,
  so analysis completes without the model and resumes when it is
  downloaded.

Validation
----------

Unit tests cover resuming, version upgrades, failures, priorities and
background workers.
