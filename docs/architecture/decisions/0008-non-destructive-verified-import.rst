ADR-0008: Non-Destructive, Verified Import
==========================================

:Status: Accepted; amended by
   :doc:`ADR-0012 <0012-continuous-import-pipeline>` (import runs with a
   review step replaced by a per-photo pipeline; the copy guarantees stand),
   :doc:`ADR-0013 <0013-folder-sessions>` (a folder culled in place
   gets XMP sidecars written into it) and
   :doc:`ADR-0014 <0014-optional-source-deletion>` (in a folder session,
   originals with a verified copy may be deleted when the user asks)
:Date: 2026-10-12

Context
-------

Users import photos from existing folders, often their only copy, into the
Clean library. Losing or corrupting a photo is the worst possible failure.
Imports can be interrupted (app closed, disk full, drive unplugged). Source
folders may contain the same file several times.

Decision
--------

- **Sources are read-only.** Import never moves, renames, modifies or
  deletes source files.
- **Copy, then verify.** Each file is copied to a temporary file in the
  target directory, flushed to disk, re-read and hashed with **BLAKE3**,
  and renamed into place only if the hash matches the source hash.
- **Exact duplicates are skipped.** A file whose hash already exists in the
  catalog, or earlier in the same import, is recorded as a duplicate and
  not copied.
- **Resumable.** The import plan and per-file status are stored in the
  catalog; restarting continues from the last verified file.
- **Dry run first.** The user sees the planned target paths, duplicates and
  problems before anything is copied.
- **No overwrites.** A target path collision with a different file gets a
  numeric suffix; existing files are never replaced.

Alternatives Considered
-----------------------

Move instead of copy
~~~~~~~~~~~~~~~~~~~~

Advantages:

- Saves disk space; no leftover source folders.

Disadvantages:

- A bug or crash can lose the only copy. Users can delete sources
  themselves once satisfied.

Copy without verification
~~~~~~~~~~~~~~~~~~~~~~~~~

Advantages:

- Faster.

Disadvantages:

- Silent corruption on failing drives or cables goes unnoticed.

SHA-256 instead of BLAKE3
~~~~~~~~~~~~~~~~~~~~~~~~~

Advantages:

- Ubiquitous.

Disadvantages:

- Several times slower; hashing would dominate import time on fast disks.

Decision Criteria
-----------------

Data safety above all, then import speed and resumability.

Rationale
---------

Copy-only with verification makes data loss through Picturesque practically
impossible, at the cost of disk space and one extra read per file. BLAKE3 is
fast enough that verification costs little next to the copy itself.

Consequences
------------

Positive
~~~~~~~~

- Source data is never at risk.
- Corruption during copy is detected.
- Interrupted imports resume.

Negative
~~~~~~~~

- Disk usage doubles until the user removes the sources.

Risks
~~~~~

- Files changed in the source during import: the hash recorded at planning
  time no longer matches, and the file is reported as failed rather than
  copied inconsistently.

Follow-up
~~~~~~~~~

- Implement the import use case (:issue:`19`).

Validation
----------

:issue:`19` tests interruption and resume, corrupted copies and duplicate
handling.

Findings (2026-10-12), from implementing :issue:`19`:

- Hashing every source while planning would read every file three times
  (plan, copy, verify). Instead, planning hashes only files whose size
  matches another file (the only possible duplicates); copying hashes the
  bytes as it streams them and then re-reads the temporary copy. A source
  that changes after planning is detected by its size and modification
  time, and by its hash when one was taken.
- A photo is imported all-or-nothing: if any of its files fails, copies
  already made for it are removed, so a RAW is never left without its
  JPEG.
- The no-overwrite rename uses a hard link (fails atomically if the target
  exists), falling back to check-then-rename on file systems without hard
  links, such as exFAT on external drives.
- Tests inject corrupted copies, failing reads, sources changing before and
  during a copy, files left by an interrupted attempt, and occupied
  targets; deliberately removing the rollback or the verification makes
  them fail.

References
----------

- `BLAKE3 <https://github.com/BLAKE3-team/BLAKE3>`_
- :doc:`/architecture/data`
