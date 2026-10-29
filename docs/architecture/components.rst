Components
==========

Picturesque is a Cargo workspace plus a Svelte front end. Dependencies point
inward toward the domain
(:doc:`ADR-0003 </architecture/decisions/0003-layered-cargo-workspace>`).

.. mermaid::

   flowchart TB
     ui[ui/<br/>Svelte 5 + TypeScript]
     desktop[picturesque-desktop<br/>Tauri 2 shell]
     devserver[picturesque-devserver<br/>HTTP, development only]
     infra[picturesque-infra<br/>SQLite, decoding, ort, geocoder, XMP, filesystem]
     app[picturesque-app<br/>use cases + ports]
     core[picturesque-core<br/>domain]

     ui -- Tauri IPC --> desktop
     ui -. HTTP .-> devserver
     desktop --> app
     devserver --> app
     desktop --> infra
     devserver --> infra
     infra --> app
     app --> core
     infra --> core

Crates
------

``picturesque-core``
   The domain, with no I/O. Photo and metadata types, content-hash identity,
   path-template rendering and filename sanitization, series grouping, score
   combination and best-in-series ranking, and ``insights``, the weighted
   aggregation of stored critiques. Depends only on small utility
   crates.

``picturesque-app``
   Use cases (import, analyze, cull, export, settings) and the **ports** they
   need, defined as traits where they are consumed: catalog repositories,
   file system, metadata reader, image decoder, inference engine, geocoder,
   sidecar writer, clock. Owns the command and event types exposed to the
   UI, so both transports share one definition. Depends on ``core``.
   ``sessions::Host`` runs the library or one folder culled in place at a
   time, each as its own ``Api`` over its own catalog, and swaps them
   (:doc:`ADR-0013 </architecture/decisions/0013-folder-sessions>`).
   ``insights`` maps stored critiques to ``core::insights`` for the
   ``library_insights`` command (:doc:`ADR-0016
   </architecture/decisions/0016-local-vision-language-critic>`).

``picturesque-infra``
   Implementations of the ports: SQLite catalog (``rusqlite``), image
   decoding (:doc:`ADR-0005 </architecture/decisions/0005-image-decoding-stack>`),
   ONNX inference (``ort``), offline geocoder, XMP reader and writer, file
   system and hashing. Native libraries and their types stay inside this
   crate. ``sessions::StandardSessions`` builds each session's context, shared by
   both entry points.

``picturesque-desktop``
   The Tauri 2 application. Builds the infrastructure, starts a ``Host``
   over it, and exposes app commands as Tauri commands and events. Serves
   images through a custom protocol. Contains no business logic.

``picturesque-devserver``
   A development-only HTTP server exposing the same commands, events and
   image routes, so the real UI can run in a browser where no display is
   available (:doc:`ADR-0010 </architecture/decisions/0010-dual-transport-tauri-ipc-and-dev-http>`).
   Not shipped to users.

Front end
---------

``ui/``
   Svelte 5 and TypeScript, built with Vite. An ``api`` module is the only
   code that knows how commands reach the backend; it selects Tauri IPC
   inside the desktop app and HTTP otherwise. Views: timeline grid with
   series frames, loupe, compare, filters; activity (import folders,
   per-photo progress, log); settings.

.. _architecture-components-command-api:

Command API
-----------

The UI calls the backend by command name with JSON arguments.
``picturesque_app::api::Api::call`` dispatches every command, so both
transports are thin adapters around one function:

- The desktop shell registers a single Tauri command, ``api``, taking
  ``command`` and ``args``.
- The development server exposes ``POST /api/commands/{name}``
  (:doc:`/development/ui-in-browser`).

``Api::call`` is synchronous; adapters run it on a blocking thread, so use
cases can do file and database I/O without depending on an async runtime.
Failures are an ``ApiError`` with a ``code`` (``unknown_command``,
``invalid_arguments``, ``internal``) and a ``message``.

Types that cross the boundary derive ``ts_rs::TS``. Running the Rust tests
writes TypeScript definitions to ``ui/src/lib/api/generated/``, which are
committed; ``make bindings-check`` fails when they are out of date. The UI
never hand-writes these types.

Boundaries
----------

- ``core`` never depends on ``app``, ``infra`` or any framework.
- ``app`` depends on ``core`` and on its own port traits; it never names a
  concrete database, decoder or model runtime.
- Types from ``rusqlite``, ``ort``, libvips, LibRaw and Tauri do not appear
  in ``app`` or ``core`` signatures.
- The desktop shell and the dev server are thin: a command handler converts
  arguments, calls a use case, and converts the result.

Crate dependencies enforce the first two rules at compile time; the rest are
enforced in review.

Background work
---------------

Long-running work (import, thumbnail generation, analysis) runs as jobs
owned by the application layer. Jobs report progress through events, can be
cancelled, and record enough state in the catalog to resume after a restart.
CPU-heavy steps run on a bounded worker pool so the UI stays responsive.

Importing is a persistent queue of photos found in watched folders,
processed one at a time; each photo is catalogued as soon as it is
copied
(:doc:`ADR-0012 </architecture/decisions/0012-continuous-import-pipeline>`).

Photo analysis (scores, hashes, AI models) is a queue of analyzers run over
every photo's preview, with results stored per analyzer version; photos on
screen go first
(:doc:`ADR-0011 </architecture/decisions/0011-resumable-background-analysis>`).
