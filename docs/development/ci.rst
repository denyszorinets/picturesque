Continuous Integration
======================

GitHub Actions runs :repo:`.github/workflows/ci.yml` on every pull request
into ``develop`` or ``main``, on pushes to those branches, and on demand.

.. list-table::
   :header-rows: 1

   * - Job
     - Runner
     - What it runs
   * - Linux (check, bindings, e2e)
     - Ubuntu 24.04
     - ``make check`` (formatting, clippy, Rust tests, build, UI checks),
       ``make bindings-check``, then ``make e2e`` in headless Chromium.
       The Playwright report is uploaded when the job fails.
   * - Desktop app smoke test (Linux)
     - Ubuntu 24.04
     - ``make smoke``: builds the real desktop app and drives it on Xvfb
       through ``tauri-driver`` and WebKitWebDriver
       (:repo:`ui/smoke/app-smoke.mjs`): the window opens, a library folder
       is set, three fixture photos are imported and one is picked with
       **P**. Screenshots are uploaded.
   * - macOS / Windows (build, test)
     - macOS 15, Windows Server 2025
     - ``cargo build`` and ``cargo test`` for the whole workspace,
       including the desktop shell, so platform-specific breakage is caught.
   * - Documentation
     - Ubuntu 24.04
     - ``make docs-check``.

Every pull request must be green before it is merged.

Smoke test of the real app
--------------------------

The end-to-end tests drive the UI in a browser against the development
server; the smoke test checks the real Tauri window. Run it locally with
``make smoke`` (the devcontainer has Xvfb, ``tauri-driver`` and
WebKitWebDriver). It starts its own Xvfb on ``:99`` and clears
``WAYLAND_DISPLAY``, so no window appears on a real screen, and gives the
app fresh data folders. Screenshots land in ``target/smoke-results``.

Conventions
-----------

- **The Makefile is the interface.** Jobs call the same ``make`` targets
  developers run locally. To change what CI checks, change the Makefile
  target, not the workflow.
- **Locked dependencies.** CI sets ``CARGO_FLAGS=--locked``, so a build
  fails if ``Cargo.lock`` is out of date; ``npm ci`` does the same for
  ``package-lock.json``.
- **Pinned actions.** Third-party actions are pinned to a commit SHA, with
  the release tag in a comment. Update both together.
- **Caching.** Cargo builds are cached per job with ``Swatinem/rust-cache``;
  npm downloads with ``actions/setup-node``.

Validate workflow changes locally with
`actionlint <https://github.com/rhysd/actionlint>`_:

.. code-block:: bash

   actionlint .github/workflows/*.yml

Cost
----

While the repository is private, Actions minutes count against the
account's quota, and macOS minutes count ten times. The macOS job is the
first candidate to restrict (for example to pushes on ``develop``) if the
quota runs low. Public repositories have no limit.
