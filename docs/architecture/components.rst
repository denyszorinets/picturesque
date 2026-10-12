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
   combination and best-in-series ranking. Depends only on small utility
   crates.

``picturesque-app``
   Use cases (import, analyze, cull, export, settings) and the **ports** they
   need, defined as traits where they are consumed: catalog repositories,
   file system, metadata reader, image decoder, inference engine, geocoder,
   sidecar writer, clock. Owns the command and event types exposed to the
   UI, so both transports share one definition. Depends on ``core``.

``picturesque-infra``
   Implementations of the ports: SQLite catalog (``rusqlite``), image
   decoding (:doc:`ADR-0005 </architecture/decisions/0005-image-decoding-stack>`),
   ONNX inference (``ort``), offline geocoder, XMP reader and writer, file
   system and hashing. Native libraries and their types stay inside this
   crate.

``picturesque-desktop``
   The Tauri 2 application. Builds the infrastructure, wires it into the use
   cases, and exposes app commands as Tauri commands and events. Serves
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
   inside the desktop app and HTTP otherwise. Views: import, timeline grid
   with series frames, loupe, compare, filters, settings.

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
