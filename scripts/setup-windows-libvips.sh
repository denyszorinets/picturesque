#!/usr/bin/env bash
# Prepares libvips for Windows builds (docs/development/building.rst):
# downloads the official 64-bit "all" build (libvips with LibRaw, libheif
# and their dependencies), checks its checksum and
#
# - makes MSVC import libraries available under the names our FFI
#   bindings link (vips, glib-2.0, gobject-2.0, raw_r),
# - copies the DLLs to target/windows-runtime, which the Windows installers
#   bundle next to the executable (src-tauri/tauri.windows.conf.json).
#
# On GitHub Actions it also sets the linker path and PATH for later steps;
# elsewhere it prints them.
set -euo pipefail

VERSION=8.18.7
SHA256=78e56ba30c267ba32ff965200534bd5721364cadbead8c4d595e44b58dfeff15
ZIP="vips-dev-x64-all-$VERSION.zip"

repo=$(cd "$(dirname "$0")/.." && pwd)
work="$repo/target/libvips-windows"
root="$work/vips-dev-${VERSION%.*}"
mkdir -p "$work"

# Build caches may restore part of target/ (without the .lib files):
# unpack again unless the libraries are really there.
if [ ! -f "$root/lib/libvips.lib" ] || [ ! -f "$root/bin/libvips-42.dll" ]; then
  rm -rf "$root"
  curl -fsSL -o "$work/$ZIP" \
    "https://github.com/libvips/build-win64-mxe/releases/download/v$VERSION/$ZIP"
  echo "$SHA256  $work/$ZIP" | sha256sum -c -
  unzip -q -o "$work/$ZIP" -d "$work"
fi

link="$work/link"
mkdir -p "$link"
cp "$root/lib/libvips.lib" "$link/vips.lib"
cp "$root/lib/libglib-2.0.lib" "$link/glib-2.0.lib"
cp "$root/lib/libgobject-2.0.lib" "$link/gobject-2.0.lib"
cp "$root/lib/libraw_r.lib" "$link/raw_r.lib"

runtime="$repo/target/windows-runtime"
rm -rf "$runtime"
mkdir -p "$runtime"
cp -r "$root/bin/." "$runtime/"
# Command-line tools are not needed by the app.
rm -f "$runtime"/*.exe

to_windows() { cygpath -w "$1" 2>/dev/null || echo "$1"; }
flags="-L native=$(to_windows "$link")"
if [ -n "${GITHUB_ENV:-}" ]; then
  echo "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS=$flags" >> "$GITHUB_ENV"
  to_windows "$runtime" >> "$GITHUB_PATH"
else
  echo "export CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS='$flags'"
  echo "export PATH=\"$runtime:\$PATH\""
fi
