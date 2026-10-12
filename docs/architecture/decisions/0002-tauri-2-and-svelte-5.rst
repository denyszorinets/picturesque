ADR-0002: Tauri 2 and Svelte 5 for the Desktop Application
==========================================================

:Status: Accepted
:Date: 2026-10-12

Context
-------

Picturesque is an open-source desktop application for Linux, macOS and
Windows. Its core work (decoding, hashing, image analysis, AI inference) is
CPU- and memory-intensive and is written in Rust. The interface is
image-heavy: a timeline of thousands of thumbnails grouped into series
frames, a loupe, a compare view and filter panels, all driven by the
keyboard.

Requirements:

- Run on Linux, macOS and Windows from one codebase.
- Call Rust code directly, without a separate server process.
- A flexible UI for a custom, non-standard grid layout.
- Reasonable installer size and memory use.
- Approachable for open-source contributors.

Decision
--------

Build the desktop shell with **Tauri 2** and the interface with **Svelte 5
and TypeScript**, bundled by **Vite**.

- The Rust backend runs in the Tauri process; the UI calls it through Tauri
  commands and receives progress through events.
- Images are served to the web view through a custom URI scheme rather than
  base64 payloads.
- The UI reaches the backend only through its ``api`` module
  (:doc:`ADR-0010 <0010-dual-transport-tauri-ipc-and-dev-http>`).

Alternatives Considered
-----------------------

Electron
~~~~~~~~

Advantages:

- Identical Chromium rendering on every platform.
- Very large ecosystem.

Disadvantages:

- Ships a full Chromium and Node runtime: much larger installers and memory
  use.
- Rust code must run as a native Node module or a sidecar process.

Native Rust GUI (egui, Iced, Slint)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Advantages:

- Single language; no web view.
- Direct GPU rendering of images.

Disadvantages:

- Less mature layout, text and accessibility support for a rich interface.
- Fewer contributors know these toolkits.

Tauri with React, or with Leptos
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Advantages:

- React: largest component ecosystem.
- Leptos: Rust on both sides, shared types.

Disadvantages:

- React: more boilerplate and a heavier runtime for a custom grid.
- Leptos: smaller ecosystem, slower iteration, fewer contributors.

Decision Criteria
-----------------

Functional fit for an image-heavy custom UI, direct Rust integration,
installer size and memory, contributor familiarity, cross-platform support.

Rationale
---------

Tauri runs the Rust backend in-process and uses the operating system's web
view (WebView2, WKWebView, WebKitGTK), which keeps installers small. Web
technology gives the most freedom for the series-frame timeline. Svelte 5
compiles to small, fast code with little boilerplate, and its fine-grained
reactivity suits a large grid where individual tiles change state.

Consequences
------------

Positive
~~~~~~~~

- One Rust process; no IPC to a separate server.
- Small installers.
- Flexible, familiar UI technology.

Negative
~~~~~~~~

- Rendering differs between the three platform web views and must be
  tested on each.
- Two languages (Rust and TypeScript) in one project.

Risks
~~~~~

- WebKitGTK on Linux is slower than Chromium for very large DOMs; the grid
  must be virtualized.
- The development container has no display, so the Tauri window cannot run
  there; this is addressed by
  :doc:`ADR-0010 <0010-dual-transport-tauri-ipc-and-dev-http>`.

Follow-up
~~~~~~~~~

- Scaffold the application (:issue:`7`).
- Smoke-test the real window in CI (:issue:`54`).

Validation
----------

To be confirmed by the scaffold (:issue:`7`) and the timeline grid
performance target in :issue:`25`.

References
----------

- `Tauri 2 <https://v2.tauri.app/>`_
- `Svelte <https://svelte.dev/>`_
- :doc:`/architecture/components`
