Packaging
=========

Installers are built with ``make dist`` on each platform (:issue:`52`);
the release workflow builds them for every platform (:doc:`releasing`).
They land in ``target/release/bundle/<format>/``.

.. list-table::
   :header-rows: 1

   * - Target
     - Builds
     - Runs on
   * - ``make dist``
     - Everything for the current OS
     - Linux, macOS, Windows
   * - ``make dist-deb``
     - ``.deb``
     - Linux (Ubuntu or Debian for the right dependencies)
   * - ``make dist-rpm``
     - ``.rpm``
     - Linux (Fedora for the right dependencies)
   * - ``make dist-appimage``
     - ``.AppImage``
     - Linux
   * - ``make dist-linux``
     - ``.deb``, ``.rpm`` and ``.AppImage`` in one build
     - Linux
   * - ``make dist-macos``
     - ``.dmg`` (:repo:`scripts/bundle-macos.sh`)
     - macOS, with ``brew install vips libraw libheif dylibbundler``
   * - ``make dist-windows``
     - ``.msi`` and the NSIS setup ``.exe``
     - Windows (``make`` from Git Bash), after
       ``scripts/setup-windows-libvips.sh`` (:doc:`building`)

A target run on the wrong OS stops with a message. Any Linux can produce
every Linux format, but a package's declared dependencies only match the
distribution it was built on.

.. list-table::
   :header-rows: 1
   :widths: 18 22 60

   * - Format
     - Build on
     - Notes
   * - ``.rpm``
     - Fedora 44
     - Declares ``vips``, ``LibRaw`` and ``libheif`` as dependencies;
       ``libonnxruntime.so`` and ``libwebgpu_dawn.so`` go to
       ``/usr/lib/picturesque``.
   * - ``.deb``
     - Ubuntu 24.04
     - Declares ``libvips42t64``, ``libraw23t64`` and ``libheif1`` (with
       the pre-``t64`` names as alternatives).
   * - ``.AppImage``
     - Ubuntu 24.04
     - Self-contained: linuxdeploy copies libvips, LibRaw and their
       dependencies; ``libonnxruntime.so`` and Dawn go to
       ``usr/lib/picturesque``. HEIC support depends on how the build host's libvips
       reads HEIF (Fedora loads it from a separate module that is not
       bundled), so the deb or rpm is the better choice where available.
   * - ``.dmg``
     - macOS 15 (Apple Silicon)
     - Homebrew's libvips and LibRaw are copied into the app bundle with
       ``dylibbundler``; :repo:`scripts/dedupe-rpaths.sh` then removes
       rpaths it added twice, which macOS 15.4 and later refuse to load
       (the 0.1.2 dmg stopped at launch with *duplicate LC_RPATH*). The
       release workflow starts the app from the dmg. Verified on a
       MacBook Pro (M1) with macOS 15.6 (:issue:`52`).
   * - ``.msi``, setup ``.exe``
     - Windows 10 or later, 64-bit
     - Self-contained: the libvips, LibRaw and libheif DLLs from
       ``target/windows-runtime`` are installed next to the executable
       (:repo:`src-tauri/tauri.windows.conf.json`). The setup ``.exe``
       installs for the current user without administrator rights; the
       ``.msi`` installs for all users. The WebView2 runtime, part of
       Windows 11 and current Windows 10, is installed if missing. The
       libvips build has no HEVC decoder, so HEIC photos are not decoded
       (AVIF is; :issue:`135`).

Each package must be built on the system it targets, because the binary
links the libvips and LibRaw versions of the build host.

Every installer also carries the built-in critic: the ``critic`` folder
made by ``scripts/critic-libs.sh release`` (the ``picturesque-critic``
helper, llama.cpp's libraries and its backends, about 80 MB on Linux),
installed as ``/usr/lib/picturesque/critic`` (deb, rpm, AppImage),
``Contents/MacOS/critic`` (dmg) or ``critic`` next to the executable
(Windows). The helper finds its libraries beside it, so the folder is
self-contained. On Windows it runs on the CPU only for now. The release
workflow checks that the deb contains it.

Linux
-----

.. code-block:: bash

   make dist-rpm        # on Fedora
   make dist-deb        # on Ubuntu
   make dist-appimage

Each target first makes ``libonnxruntime.so`` (``scripts/ort-dylib.sh
release``). The AppImage build runs linuxdeploy extracted
(``APPIMAGE_EXTRACT_AND_RUN=1``) so no FUSE is needed.

The executable does not link ONNX Runtime or Dawn: it loads
``libonnxruntime.so`` the first time a model is needed, after checking that
the processor has the instructions pyke's build uses
(:doc:`/architecture/decisions/0015-onnx-runtime-loaded-at-first-use`). It
looks next to itself (development builds) and in ``../lib/picturesque``
(deb, rpm and AppImage); ``libonnxruntime.so`` finds Dawn beside it. The
release workflow fails if the executable links either at load time.

.. _development-packaging-check:

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
