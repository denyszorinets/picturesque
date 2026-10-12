Building and Testing
====================

Prerequisites
-------------

The devcontainer in :repo:`.devcontainer/Containerfile` installs everything
needed. Outside it you need:

- Rust 1.99.0 with ``rustfmt`` and ``clippy``. :repo:`rust-toolchain.toml`
  pins the version, so ``rustup`` installs it automatically.
- `uv <https://docs.astral.sh/uv/>`_ for the documentation site.

Later milestones add Node.js, the Tauri system libraries, libvips and
LibRaw; this page lists them as they are introduced.

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
     - Format Rust code with ``rustfmt``.
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
   * - ``make check``
     - ``format-check``, ``lint``, ``test`` and ``build``. Run before every
       commit; CI runs the same target.
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
