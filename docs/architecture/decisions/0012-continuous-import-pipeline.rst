ADR-0012: Continuous Import Pipeline
====================================

:Status: Accepted
:Date: 2026-10-14

Context
-------

The first import design
(:doc:`ADR-0008 <0008-non-destructive-verified-import>`) was a run: plan
every photo in the chosen folders, review the plan, then copy. With a large
RAW folder this has three problems users ran into:

- Nothing reaches the library until planning has read every file, so
  photos cannot be culled while an import is running.
- If the app stops during planning, the run is interrupted and nothing has
  been imported; the folders have to be chosen again.
- Sources are not remembered, so new photos in the same folders need a new
  import each time.

Users also want to set a folder once and have new photos imported
automatically (:issue:`106`), and to see what happens to each photo
(:issue:`107`).

Decision
--------

Replace runs with a **persistent, per-photo import pipeline** in the
application layer.

- **Sources** are stored folders. Adding one starts importing it; a
  *watched* source is scanned again when it changes and periodically.
- A **scan** groups a source's files into photos and offers each to the
  queue, keyed by its primary file. A photo is queued when it is new, or
  when it finished before and its files' sizes or modification times have
  changed. Unchanged photos are not read again.
- One **job** per photo moves through stages: ``queued`` →
  ``reading`` (metadata, duplicate check) → ``copying`` (copy, verify,
  catalogue) → ``imported``, ``duplicate`` or ``failed``. Jobs live in the
  catalog (``import_job``), so the queue survives restarts: jobs left in
  ``reading`` or ``copying`` are queued again on start.
- A single worker processes jobs oldest first and **commits each photo to
  the catalog as soon as it is copied**, then emits progress, so the
  library grows while the import runs. The imported photo is handed to
  thumbnails, analysis and series right away.
- Copying keeps the guarantees of ADR-0008: temporary file, read-back
  verification by hash, rename, all-or-nothing per photo, never overwrite,
  never modify sources.
- Duplicates are found per photo against the catalog by content hash, so
  the same photo in two sources is imported once.
- Without a library folder the worker pauses, and continues when one is
  set.
- Each step that matters to a user is written to a **pipeline log**
  (``pipeline_log``), kept to the newest 20,000 entries.

Alternatives Considered
-----------------------

Keep runs, commit photos as they are copied
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Advantages:

- Smaller change; the review step stays.

Disadvantages:

- Planning a large folder still has to finish before anything is
  imported, and watched folders would create a run per change.

Derive pending work from the file system (no job table)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Advantages:

- Like analysis (:doc:`ADR-0011 <0011-resumable-background-analysis>`),
  nothing to keep in sync.

Disadvantages:

- Every check would rescan and rehash sources; failures and per-photo
  history could not be shown.

Decision Criteria
-----------------

Photos visible as soon as they are imported, survives restarts at any
point, imports new photos without user action, keeps the safety
guarantees of ADR-0008, explainable per photo.

Rationale
---------

A per-photo job is the smallest unit that can be committed, resumed,
retried and shown. Storing the files' size and modification time with the
job makes rescans cheap and detects changed photos without hashing.

Consequences
------------

Positive
~~~~~~~~

- The library fills while importing; stopping the app loses at most the
  photo being copied, which is queued again.
- Sources and history persist.

Negative
~~~~~~~~

- There is no review before copying; duplicates and problems are reported
  per photo in the activity view instead.
- Photos with the same capture time and name in two sources import to the
  first free target name, as before.

Risks
~~~~~

- One worker copies one photo at a time; enough for a USB card or disk,
  which is the bottleneck, but it may be parallelized later.

Follow-up
~~~~~~~~~

- Watching sources with file-system notifications and periodic rescans
  (:issue:`106`).
- The activity view with per-photo stages and the log (:issue:`107`);
  the run-based import screen and API are removed there.

Validation
----------

Unit tests cover photo-by-photo import, rescans, duplicates, pausing
without a library folder, resuming after a stop, failures and retries,
and source validation; an integration test runs the pipeline with the real
adapters across a catalog reopen.
