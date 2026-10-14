Releasing
=========

Work happens on ``develop`` (feature branches ``feature/<issue>_<name>``
merged by pull request); ``main`` only receives releases. A release goes
through a ``release/X.Y.Z`` branch and a tag, and the release workflow
(:repo:`.github/workflows/release.yml`) builds the installers
(:doc:`packaging`).

Steps
-----

1. **Branch** from ``develop``::

      git switch -c release/0.2.0 develop

2. **Set the version** everywhere it is recorded (Cargo workspace,
   ``Cargo.lock``, ``tauri.conf.json``, ``ui/package.json``,
   ``package-lock.json``)::

      python3 scripts/set-version.py 0.2.0

   Commit, push, and open a pull request into ``main``. Only fixes go on
   the release branch from now on.

3. **Dry run**: pushing the release branch runs the *Release* workflow,
   which builds every installer and keeps them as workflow artifacts,
   without creating a release. Install and start them
   (:ref:`checking an installer <development-packaging-check>`).

4. **Merge** the pull request into ``main`` (a maintainer), then tag the
   merge commit and push the tag::

      git switch main && git pull
      git tag -a v0.2.0 -m "Picturesque 0.2.0"
      git push origin v0.2.0

   The workflow checks that the tag matches the version in the sources,
   builds the installers and creates a **draft** GitHub release with them,
   ``SHA256SUMS.txt`` and generated notes.

5. **Publish**: review the draft, edit the notes, and publish it.

6. **Merge back**: merge ``main`` into ``develop`` so the version and any
   release fixes are there too.

What the workflow builds
------------------------

.. list-table::
   :header-rows: 1

   * - Job
     - Builds
     - Holds back the release if it fails
   * - Ubuntu 24.04
     - ``.deb``, ``.AppImage``
     - yes
   * - Fedora 44 (container)
     - ``.rpm``
     - yes
   * - macOS 15
     - ``.dmg`` with Homebrew's libraries bundled (``dylibbundler``),
       ad-hoc signed
     - no (experimental until verified, :issue:`52`)
   * - Windows Server 2025
     - ``.msi`` and the NSIS setup ``.exe``; the job installs the setup
       silently and checks that the app starts and opens its catalog
     - yes

Installers are not signed; see :doc:`packaging` for what signing needs.

The workflow also runs, without creating a release, on every push to a
``release/*`` branch and on pull requests that change packaging (the
workflow, the Tauri configuration or the packaging scripts).
