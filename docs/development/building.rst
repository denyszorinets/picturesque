Building and Testing
====================

Prerequisites
-------------

The devcontainer in :repo:`.devcontainer/Containerfile` installs everything
needed. Outside it you need:

- Rust 1.99.0 with ``rustfmt`` and ``clippy``. :repo:`rust-toolchain.toml`
  pins the version, so ``rustup`` installs it automatically.
- Node.js 22 or newer with npm, for the front end and the Tauri CLI.
- The `Tauri 2 system dependencies <https://v2.tauri.app/start/prerequisites/>`_
  for your platform (WebKitGTK 4.1 and friends on Linux, WebView2 on
  Windows, Xcode command-line tools on macOS).
- `uv <https://docs.astral.sh/uv/>`_ for the documentation site.

- libvips (8.18 or newer, with HEIF support), LibRaw (0.22 or newer) and
  libheif development packages
  (:doc:`ADR-0005 </architecture/decisions/0005-image-decoding-stack>`).
- An HEVC decoder plugin for libheif (libde265), needed to read HEIC photos.
  Many distributions omit it for patent reasons; on Fedora, install
  ``libheif-freeworld`` from RPM Fusion. Check with
  ``heif-dec --list-decoders``: libde265 must appear under *HEIC decoders*.
  On macOS: ``brew install vips libraw libheif`` and
  ``export LIBRARY_PATH=$(brew --prefix)/lib``.

.. warning::

   Building the full workspace on Windows is not set up yet: it needs
   libvips and LibRaw import libraries. ``picturesque-core`` and
   ``picturesque-app`` build and test on Windows.

After cloning, install the front-end dependencies once:

.. code-block:: bash

   make ui-deps

.. note::

   The devcontainer has no display. The desktop window cannot be opened
   there; the UI is developed in a browser against the development server
   (:doc:`ADR-0010 </architecture/decisions/0010-dual-transport-tauri-ipc-and-dev-http>`).

Workspace layout
----------------

.. code-block:: text

   Cargo.toml            workspace manifest: shared versions, lints
   crates/core/          picturesque-core   domain, no I/O
   crates/app/           picturesque-app    use cases and ports
   crates/infra/         picturesque-infra  adapters (SQLite, decoding, ...)
   crates/devserver/     picturesque-devserver  development HTTP transport
   src-tauri/            picturesque-desktop  Tauri 2 shell
   ui/                   Svelte 5 + TypeScript front end (Vite)
   package.json          npm workspace root; holds the Tauri CLI
   docs/                 this site (Sphinx, uv project)

See :doc:`/architecture/components` for the layer rules.

Make targets
------------

The :repo:`Makefile` is the entry point for everyday tasks.

.. list-table::
   :header-rows: 1

   * - Target
     - What it does
   * - ``make format``
     - Format Rust code with ``rustfmt`` and front-end code with Prettier.
   * - ``make format-check``
     - Fail if any Rust file is not formatted.
   * - ``make lint``
     - Run ``clippy`` on all targets; warnings are errors.
   * - ``make test``
     - Run unit, integration and doc tests for the whole workspace.
   * - ``make build``
     - Build every workspace target.
   * - ``make doc``
     - Build Rust API docs; warnings are errors.
   * - ``make ui-deps``
     - Install front-end dependencies from ``package-lock.json`` and the
       Chromium build Playwright uses.
   * - ``make ui-check``
     - Front end: ``svelte-check`` (warnings fail), ESLint, Prettier check
       and Vitest.
   * - ``make check``
     - ``format-check``, ``lint``, ``test``, ``build`` and ``ui-check``.
       Run before every commit; CI runs the same target.
   * - ``make dev-web``
     - Run the UI in a browser against the development server
       (:doc:`ui-in-browser`). Works in the devcontainer.
   * - ``make e2e``
     - Playwright end-to-end tests in headless Chromium.
   * - ``make bindings-check``
     - Run the tests (which regenerate the TypeScript API types) and fail
       if ``ui/src/lib/api/generated/`` changed.
   * - ``make app-dev``
     - Run the desktop app with hot reload (``tauri dev``). Needs a
       display, so it does not work in the devcontainer.
   * - ``make clean``
     - Remove Rust and documentation build output.
   * - ``make docs`` / ``docs-serve`` / ``docs-check``
     - Build, preview or strictly check the documentation
       (:doc:`documentation`).

Lints
-----

Lint levels are set once in ``[workspace.lints]`` in :repo:`Cargo.toml`.
``unsafe_code`` is denied workspace-wide, and ``picturesque-core`` and
``picturesque-app`` forbid it outright. Suppress a lint only on the
narrowest item, with ``#[expect(..., reason = "...")]``.

Front end
---------

The UI lives in ``ui/`` and is a plain Vite single-page app (no SvelteKit).
Code reaches the backend only through ``ui/src/lib/api``; components never
call Tauri directly, so the same UI runs in a browser against the
development server (:doc:`ui-in-browser`). API types come from
``ui/src/lib/api/generated/``, generated from Rust
(:ref:`see Command API <architecture-components-command-api>`).

Front-end commands run from the repository root through the npm workspace:

.. code-block:: bash

   npm run dev -w ui      # Vite dev server on http://127.0.0.1:5173
   npm run test -w ui     # Vitest
   npm run check -w ui    # svelte-check

Dependency versions
~~~~~~~~~~~~~~~~~~~

- TypeScript stays on 6.0 until ``typescript-eslint`` and ``svelte-check``
  support TypeScript 7.
- New dependencies should be releases that are at least two weeks old,
  unless a newer release fixes a security advisory. With npm:
  ``npm install --before=<date two weeks ago> ...``.
- The ``@tauri-apps/api`` and ``@tauri-apps/cli`` npm packages must have
  the same major and minor version as the ``tauri`` crate.

RAW corpus tests
----------------

Some infrastructure tests run against real camera files from
raw.pixls.us (16 cameras, about 630 MB). They are ignored by default.
Fetch the corpus once, then run them explicitly:

.. code-block:: bash

   python3 -I spikes/decoding/scripts/fetch_corpus.py
   cargo test -p picturesque-infra -- --ignored

Small fixtures that are committed (for example ``exif.heic``) are
described in ``crates/infra/tests/fixtures/README.md``.
