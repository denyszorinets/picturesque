ADR-0013: Folder Sessions with One Catalog per Folder
=====================================================

:Status: Accepted
:Date: 2026-10-19

Context
-------

Picturesque was built around one Clean library: photos are copied into it
(:doc:`ADR-0008 <0008-non-destructive-verified-import>`), and one catalog
belongs to it, with photo paths stored relative to its folder
(:doc:`/concepts/library-and-catalog`).

Early users asked to cull photos **where they already are**: a darktable
collection, a dump folder, a memory card. They do not want a second
library, and they want to work one folder at a time: open a folder, cull,
move on to the next, without Picturesque keeping several folders active
(:issue:`157`). The library workflow must keep working as it does today,
and stays the default.

Constraints:

- Culling in place must not copy photos, and must write decisions to XMP
  sidecars next to the originals so darktable, Lightroom and digiKam see
  them.
- Analysis is expensive (minutes per thousand photos with AI models);
  reopening a folder should not start over.
- Settings (AI models, ranking weights, colour labels, sidecar naming) are
  the user's, not a folder's.
- Every use case reads ``library_root`` as the folder its photo paths are
  relative to, and background workers (import, thumbnails, analysis,
  series, sidecars) run against one catalog for the life of the
  application.

Decision
--------

A **folder session** is culling one folder in place. It has its own
catalog, and the application layer runs one session (or the library) at a
time.

- **One catalog per folder.** A session's catalog is
  ``sessions/<hash>.sqlite`` in the app data directory, where ``<hash>`` is
  derived from the folder's absolute path; its thumbnail cache is
  ``sessions/<hash>/`` in the cache directory. The library keeps
  ``catalog.sqlite`` and the existing cache. The schema is the same, so
  every use case works unchanged on either.
- **One ``Api`` per session.** Opening a folder builds a new ``Api`` over
  that catalog and starts its background work; the previous ``Api`` is
  shut down first (pending sidecars are written, then every worker stops).
  The transports (Tauri, development server) hold the current ``Api``
  behind a lock and swap it. Nothing from one session can run against
  another session's catalog.
- **Settings stay global.** They are stored in ``catalog.sqlite``. A
  session reads them through a settings adapter that reports the session
  folder as ``library_root`` and marks the mode as *in place*; saving from
  a session never changes the library folder. The list of recent folders
  is stored with the settings.
- **In place, not copied.** In a session, the import pipeline catalogs
  photos where they are: the folder is both the source and the root,
  nothing is copied, and the *Copied/Verified* steps are skipped. The
  folder is watched like an import folder.
- **Sidecars next to the originals.** In a session, sidecars are written in
  the folder, next to the photo's files. This is the only change a session
  makes to the folder (it amends the placement in
  :doc:`ADR-0009 <0009-xmp-sidecars-for-culling-decisions>` and the
  "sources are only read" rule of
  :doc:`ADR-0008 <0008-non-destructive-verified-import>` for sessions).
- **Pruning.** At startup, session catalogs and caches not opened for 30
  days are deleted. The folders and their sidecars are never touched by
  pruning.

Alternatives Considered
-----------------------

One catalog with several roots
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Store a root per photo and filter every query by the active root.

Advantages:

- One database file; settings stay where they are.

Disadvantages:

- Every query, index and use case changes; a missed filter shows or
  modifies another folder's photos.
- Series, auto-picks, statistics and the embedding index would all need
  per-root scoping.
- Removing a folder means deleting rows across most tables.

Swap the database connection in place
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Keep one ``Api`` and let the catalog switch its connection to another
file.

Advantages:

- No change to the transports.

Disadvantages:

- Background workers hold photo IDs across several catalog calls; a switch
  in the middle writes one folder's results into another folder's catalog
  under the same ID. Preventing that needs the same quiescing as a
  shutdown, plus resetting every in-memory cache (thumbnails, embeddings,
  queues) by hand.

Import into a hidden library and delete afterwards
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Copy the folder into a temporary library, cull there, then write sidecars
back.

Advantages:

- No new concept in the application layer.

Disadvantages:

- Doubles disk use and import time for the case users asked to avoid;
  sidecars would have to be copied back and kept in step.

Decision Criteria
-----------------

- Correctness: a session can never read or write another session's data.
- Simplicity: use cases keep working on "a catalog and a root".
- Reversibility: a session catalog is a disposable file.
- Responsiveness: reopening a folder reuses its analysis.

Rationale
---------

Separate catalogs make isolation structural instead of a filter every
query must remember, and keep the library's catalog exactly as it is.
Rebuilding the ``Api`` per session reuses the existing wiring: every
service already takes one catalog and one settings store, so a session is
just a different ``AppContext``. Workers need an explicit shutdown either
way, and with a fresh ``Api`` their in-memory state (queues, caches,
embedding index) starts clean for free.

Consequences
------------

Positive
~~~~~~~~

- Library mode is unchanged.
- Folder sessions get import, analysis, series, auto-pick, culling,
  sidecars, export and statistics without per-feature work.
- Deleting a session is deleting two paths.

Negative
~~~~~~~~

- Background workers need a shutdown signal they did not have.
- Switching takes a moment: pending sidecars are written and workers
  finish their current photo before the new session starts.
- Settings reads in a session go through an adapter, so "the library
  folder" in settings code means "the current root".

Risks
~~~~~

- A worker that ignores the shutdown signal keeps a catalog open; tests
  check that every worker stops.
- Two app instances opening the same folder would share a session
  catalog; SQLite's locking keeps it consistent, as with the library.

Follow-up
~~~~~~~~~

- Folder sessions, switching and recent folders (:issue:`164`).
- In-place sidecars, including every file of a RAW+JPEG pair
  (:issue:`165`).

Validation
----------

- Tests switch between the library and folders and check that photos,
  decisions and analysis stay with their folder.
- Tests check that every background worker stops on shutdown.

References
----------

- :doc:`/concepts/library-and-catalog`
- :doc:`/architecture/data`
- :doc:`ADR-0004 <0004-sqlite-catalog>`
- :doc:`ADR-0008 <0008-non-destructive-verified-import>`
- :doc:`ADR-0009 <0009-xmp-sidecars-for-culling-decisions>`
- :doc:`ADR-0010 <0010-dual-transport-tauri-ipc-and-dev-http>`
- :issue:`163`
