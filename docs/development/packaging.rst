Packaging
=========

Installers are built with ``npx tauri build`` (:issue:`52`); the release
workflow builds them for every platform (:issue:`53`).

.. list-table::
   :header-rows: 1
   :widths: 18 22 60

   * - Format
     - Build on
     - Notes
   * - ``.rpm``
     - Fedora 44
     - Declares ``vips``, ``LibRaw`` and ``libheif`` as dependencies;
       ``libwebgpu_dawn.so`` goes to ``/usr/lib/picturesque``.
   * - ``.deb``
     - Ubuntu 24.04
     - Declares ``libvips42t64``, ``libraw23t64`` and ``libheif1`` (with
       the pre-``t64`` names as alternatives).
   * - ``.AppImage``
     - Ubuntu 24.04
     - Self-contained: linuxdeploy copies libvips, LibRaw, Dawn and their
       dependencies. HEIC support depends on how the build host's libvips
       reads HEIF (Fedora loads it from a separate module that is not
       bundled), so the deb or rpm is the better choice where available.
   * - ``.dmg``
     - macOS 15 (Apple Silicon)
     - Homebrew's libvips and LibRaw are copied into the app bundle with
       ``dylibbundler``. Not verified yet.
   * - ``.msi``
     - Windows
     - Not available yet: the workspace does not build on Windows until
       libvips and LibRaw import libraries are set up
       (:doc:`building`).

Each package must be built on the system it targets, because the binary
links the libvips and LibRaw versions of the build host.

Linux
-----

.. code-block:: bash

   # rpm (on Fedora), deb (on Ubuntu)
   npx tauri build --bundles rpm
   npx tauri build --bundles deb
   # AppImage: linuxdeploy must find libwebgpu_dawn.so
   LD_LIBRARY_PATH=$PWD/target/release npx tauri build --bundles appimage

The executable finds ``libwebgpu_dawn.so`` (ONNX Runtime's WebGPU provider)
through its run path: next to itself in development builds and AppImages,
in ``../lib/picturesque`` in deb and rpm packages
(:repo:`src-tauri/build.rs`).

Checking an installer
---------------------

Verified for the rpm (Fedora 44): installing it into an empty Fedora root
(``dnf --installroot … install Picturesque-*.rpm``) pulls in every
dependency, and ``ldd`` there resolves every library, including Dawn from
``/usr/lib/picturesque``. Installed in the devcontainer, the app starts and
shows its window under Xvfb; the AppImage does too.

To start the app headless, run it on Xvfb and make sure it does not reach
another display::

   Xvfb :99 &
   unset WAYLAND_DISPLAY
   DISPLAY=:99 GDK_BACKEND=x11 picturesque-desktop

Signing
-------

None of the installers are signed yet.

- **macOS**: Gatekeeper blocks unsigned apps downloaded from the internet
  (right-click → Open once to allow). Distribution needs an Apple
  Developer ID certificate and notarization; Tauri signs and notarizes
  when ``APPLE_CERTIFICATE``, ``APPLE_SIGNING_IDENTITY``, ``APPLE_ID``,
  ``APPLE_PASSWORD`` and ``APPLE_TEAM_ID`` are set in the build
  environment.
- **Windows**: SmartScreen warns about unsigned installers; an
  Authenticode certificate (``bundle.windows.certificateThumbprint``) or a
  signing service removes the warning.
- **Linux**: deb and rpm packages are not signed; checksums are published
  with each release. A signed repository can come later.
