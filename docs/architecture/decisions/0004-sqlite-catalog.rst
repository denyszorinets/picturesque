ADR-0004: SQLite Catalog
========================

:Status: Accepted
:Date: 2026-10-12

Context
-------

Picturesque needs durable local state: photo metadata, scores, embeddings,
series, decisions, import progress and settings. A typical library holds
fewer than 10,000 photos. The app is single-user and runs on the user's
machine; there is no server. Filtering and sorting the timeline by date,
place, camera and scores must be fast, and an interrupted import must resume
from durable state.

Decision
--------

Store the catalog in a single **SQLite** database in the platform app-data
directory, accessed through **rusqlite** with the bundled SQLite build.

- Schema changes are versioned migrations embedded in the binary and applied
  at startup.
- WAL journal mode, foreign keys on.
- Database access lives only in ``picturesque-infra`` behind repository
  ports.
- Embeddings are stored as BLOBs; similarity search is done in memory, which
  is adequate below roughly 100,000 photos.
- Thumbnails and previews are files in the cache directory, not BLOBs.

Alternatives Considered
-----------------------

sqlx (async, SQLite)
~~~~~~~~~~~~~~~~~~~~

Advantages:

- Compile-time checked queries.

Disadvantages:

- Async adds complexity for a local, single-user database; compile-time
  checks need a database at build time or an offline cache.

Embedded key-value store (redb, sled)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Advantages:

- Pure Rust, simple.

Disadvantages:

- No ad-hoc queries; filters and sorts must be hand-built as indexes.

JSON files
~~~~~~~~~~

Advantages:

- Human-readable.

Disadvantages:

- No transactions; poor at incremental updates and queries.

Decision Criteria
-----------------

Query flexibility for filters, transactional durability for imports,
operational simplicity, maturity.

Rationale
---------

SQLite gives transactional, crash-safe storage and flexible queries in one
file with no server. ``rusqlite`` is synchronous, which matches a local
database accessed from worker threads, and the bundled build avoids
depending on the system SQLite version.

Consequences
------------

Positive
~~~~~~~~

- One file to back up; standard tools can inspect it.
- Filters map directly to SQL.

Negative
~~~~~~~~

- Queries are checked by tests, not at compile time.

Risks
~~~~~

- A future library much larger than 100,000 photos would need an
  approximate nearest-neighbour index for embeddings.

Follow-up
~~~~~~~~~

- Create the catalog and migrations (:issue:`13`).

Validation
----------

Query latency target in :issue:`24`: filtered queries over 10,000 photos
under 50 ms.

References
----------

- :doc:`/architecture/data`
