Documentation
=============

This site is built with `Sphinx <https://www.sphinx-doc.org/>`_ from
reStructuredText sources in ``docs/``
(:doc:`ADR-0001 </architecture/decisions/0001-sphinx-and-restructuredtext-for-project-documentation>`).

``docs/`` is a self-contained Python project managed with
`uv <https://docs.astral.sh/uv/>`_; it is not part of the Cargo workspace.

Build and check
---------------

.. code-block:: bash

   make docs          # build HTML into docs/_build/html
   make docs-serve    # live-reloading preview on http://127.0.0.1:8000
   make docs-check    # strict build (every warning is an error) + link check

``make docs-check`` must pass before documentation changes are committed.

Writing pages
-------------

- Put each page in the section that matches what the reader wants to do:
  learn (getting started), understand (concepts, architecture), accomplish
  a task (how-to), or look something up (reference).
- Section index pages include every page in their directory automatically.
  Pages elsewhere must be added to a ``toctree``.
- Link with Sphinx roles so the build verifies links: ``:doc:``, ``:ref:``,
  ``:issue:`` and ``:repo:``.

Dependencies
------------

Add or upgrade documentation dependencies with ``uv`` from ``docs/``:

.. code-block:: bash

   cd docs
   uv add <package>
   uv lock --upgrade-package <package>

Commit both ``pyproject.toml`` and ``uv.lock``.
