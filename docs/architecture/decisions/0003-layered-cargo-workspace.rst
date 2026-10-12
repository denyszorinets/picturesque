ADR-0003: Layered Cargo Workspace
=================================

:Status: Accepted
:Date: 2026-10-12

Context
-------

The backend combines pure logic that benefits from fast, exhaustive unit
tests (path templates, series grouping, ranking) with heavy infrastructure
(SQLite, native image libraries, ONNX Runtime, the Tauri framework). Native
libraries are slow to compile and hard to install, and the Tauri shell
cannot run in the development container. The project must keep logic
testable without any of them, and must be able to swap infrastructure, for
example the decoding stack or the inference runtime, as spikes conclude.

Decision
--------

Organize the backend as a Cargo workspace with one crate per layer:

``picturesque-core``
   Domain types and pure logic. No I/O, no framework.

``picturesque-app``
   Use cases and the port traits they consume. Depends on ``core``.

``picturesque-infra``
   Port implementations (SQLite, decoding, ``ort``, geocoder, XMP, file
   system). Depends on ``app`` and ``core``.

``picturesque-desktop`` and ``picturesque-devserver``
   Entry points that wire ``infra`` into ``app`` and expose commands. Thin.

Shared settings (edition, Rust version, lints, dependency versions) live in
the workspace root ``Cargo.toml``.

Alternatives Considered
-----------------------

Single crate with modules
~~~~~~~~~~~~~~~~~~~~~~~~~

Advantages:

- Simplest layout.

Disadvantages:

- Layer rules are conventions only; nothing stops domain code from using
  SQLite.
- Every test build compiles native dependencies.

Fine-grained crates per feature
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Advantages:

- Maximum isolation and parallel compilation.

Disadvantages:

- Many crates and manifests for a small team; ceremony without benefit at
  this size.

Decision Criteria
-----------------

Enforced boundaries, test speed, isolation of native dependencies,
simplicity.

Rationale
---------

Three layers plus thin entry points is the smallest structure where the
compiler enforces the dependency rule: ``core`` cannot name a database
because it does not depend on one. ``cargo test -p picturesque-core`` runs
without compiling any native library. Infrastructure can be replaced behind
port traits without touching use cases.

Consequences
------------

Positive
~~~~~~~~

- Dependency direction is checked at compile time.
- Domain and use-case tests are fast and need no native libraries.
- Two entry points share all logic.

Negative
~~~~~~~~

- Port traits add some indirection.
- Mapping between infrastructure and domain types is extra code.

Risks
~~~~~

- Over-abstraction. Ports are introduced only where a use case needs an
  external capability, not for every type.

Follow-up
~~~~~~~~~

- Scaffold the workspace (:issue:`6`).

Validation
----------

Confirmed when ``cargo test -p picturesque-core`` builds without native
libraries installed.

References
----------

- :doc:`/architecture/components`
