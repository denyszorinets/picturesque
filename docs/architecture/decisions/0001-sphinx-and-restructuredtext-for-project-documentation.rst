ADR-0001: Sphinx and reStructuredText for Project Documentation
================================================================

:Status: Accepted
:Date: 2026-10-12

Context
-------

Picturesque needs documentation for three audiences: photographers using the
app, contributors working on the code, and maintainers recording
architectural decisions. Documentation is written alongside the code, in the
same pull requests, so it must live in the repository and be reviewable as
plain text.

Requirements:

- Plain-text sources reviewed in pull requests.
- Cross-references that the build verifies, so renamed or deleted pages are
  caught before merge.
- A strict check that can run in CI.
- Diagrams (component and data-flow diagrams) without binary image files.
- A static site that can be published to GitHub Pages once the repository
  is public.
- Tooling that installs reproducibly in the devcontainer and in CI.

The project is a Rust and TypeScript codebase. API documentation for Rust
crates is produced separately by ``cargo doc``; this site covers user,
contributor and architecture documentation.

Decision
--------

Use **Sphinx** with **reStructuredText** sources in ``docs/``, themed with
**Furo**.

- ``docs/`` is a self-contained Python project managed with ``uv``
  (``pyproject.toml`` and ``uv.lock`` committed). It is not part of the Cargo
  workspace.
- Extensions: ``sphinx-design`` (tabs, cards), ``sphinxcontrib-mermaid``
  (diagrams), ``sphinx.ext.extlinks`` (``:issue:`` and ``:repo:`` roles).
- ``make docs-check`` builds with warnings treated as errors, nitpicky
  cross-reference checking, and runs ``linkcheck``. It must pass before
  documentation changes merge.
- Architecture decisions are ADRs under ``docs/architecture/decisions/``.

Alternatives Considered
-----------------------

mdBook
~~~~~~

The Rust project's Markdown book generator.

Advantages:

- Rust-native; a single binary.
- Familiar to Rust contributors.

Disadvantages:

- Cross-references are plain relative links; broken targets are not
  reliably reported at build time.
- No equivalent of a strict, warnings-as-errors build with reference
  checking.

MkDocs (Material)
~~~~~~~~~~~~~~~~~

Markdown-based static site generator with a popular theme.

Advantages:

- Markdown is widely known.
- Polished default theme.

Disadvantages:

- Weaker cross-reference model than Sphinx roles; link validation relies on
  plugins.
- Markdown lacks standard directives for admonitions, includes and labelled
  references, so content depends on extensions.

Markdown files in the repository, no site
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Rendered directly by GitHub.

Advantages:

- No tooling at all.

Disadvantages:

- No build-time link validation, no search, no publishable site.

Decision Criteria
-----------------

- **Link integrity:** documentation is updated in the same pull request as
  code; the build must catch broken references.
- **CI strictness:** a single command that fails on any warning.
- **Diagrams as text.**
- **Reproducible tooling** in the devcontainer and CI.
- **Contributor familiarity** (weighed lower than link integrity).

Rationale
---------

Sphinx's roles (``:doc:``, ``:ref:``) are resolved at build time and, with
``-W`` and ``nitpicky``, any broken reference fails the build. That directly
supports documenting features in the same change that implements them.
reStructuredText is less familiar than Markdown, but its directives
(includes, labelled sections, admonitions) are standard rather than
plugin-dependent. ``uv`` makes the Python toolchain reproducible with one
lockfile, and Python is already present in the devcontainer.

Consequences
------------

Positive
~~~~~~~~

- Broken cross-references fail CI.
- One command builds and checks the site.
- Diagrams are reviewable text.

Negative
~~~~~~~~

- Contributors must write reStructuredText, not Markdown.
- A Python toolchain is required alongside Rust and Node.

Risks
~~~~~

- Link checks against the repository fail while it is private; those URLs
  are excluded from ``linkcheck`` until the repository is public.

Follow-up
~~~~~~~~~

- Run ``make docs-check`` in CI (:issue:`9`).
- Publish to GitHub Pages once the repository is public.

Validation
----------

``make docs-check`` passes on the initial site.

References
----------

- `Sphinx <https://www.sphinx-doc.org/>`_
- `Furo <https://pradyunsg.me/furo/>`_
- :doc:`/development/documentation`
