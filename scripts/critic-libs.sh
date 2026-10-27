#!/usr/bin/env bash
# Builds the critic helper (crates/critic, ADR-0016) and assembles it with
# llama.cpp's shared libraries and run-time backends (GPU and CPU variants)
# in target/<profile>/critic/, a self-contained folder: the helper finds
# the libraries beside it ($ORIGIN rpath) and loads the backends from its
# own folder. Installers ship this folder next to the app.
#
#   scripts/critic-libs.sh [debug|release]
#
# Needs patchelf on Linux.
#
# llama-cpp-sys-2 copies only some libraries (none where CMake installs to
# lib64) and never the backends, so they are copied from its build folder.
set -euo pipefail
cd "$(dirname "$0")/.."
profile=${1:-debug}
flags=()
[ "$profile" = release ] && flags=(--release)

out=target/$profile/critic
# llama-cpp-sys-2 leaves dangling lib*.so links here, and its next build
# fails on them with "File exists" (see crates/critic/build.rs, which
# removes them too, but only when its build script runs).
for d in "target/$profile" "target/$profile/deps" "target/$profile/examples"; do
  [ -d "$d" ] || continue
  for f in "$d"/lib*.so*; do
    if [ -L "$f" ] && [ ! -e "$f" ]; then rm -f "$f"; fi
  done
done
# The build folder of llama-cpp-sys-2, from its build script's output
# (Windows paths come JSON-escaped: C:\\a\\b).
dir=$(cargo build -q -p picturesque-critic ${flags[@]+"${flags[@]}"} --message-format=json |
  grep '"reason":"build-script-executed"' | grep 'llama-cpp-sys-2' |
  sed -E 's/.*"out_dir":"([^"]*)".*/\1/; s#\\\\#/#g' | head -1)
[ -d "$dir/backends" ] || { echo "critic-libs: no llama.cpp backends in '$dir'" >&2; exit 1; }

exe=
case "$(uname -s)" in
  Linux) libs=("$dir"/lib*/lib*.so.*[0-9]) ;;
  Darwin) libs=("$dir"/lib/lib*.dylib) ;;
  *) libs=("$dir"/bin/*.dll) exe=.exe ;;
esac
rm -rf "$out"
mkdir -p "$out"
cp "target/$profile/picturesque-critic$exe" "$out/"
for f in "${libs[@]}" "$dir"/backends/*; do
  [ -f "$f" ] || continue
  # One copy per library, under the name it is linked by (libllama.so.0).
  [[ $(basename "$f") =~ \.so\.[0-9]+\.[0-9] ]] && continue
  # Links keep their names (libllama.so.0); copy the files they point to.
  cp -fL "$f" "$out/$(basename "$f")"
done
# llama.cpp's libraries link each other (libggml.so.0 needs
# libggml-base.so.0) without an rpath; $ORIGIN makes them find their
# siblings wherever the folder is, which linuxdeploy (AppImage) also needs.
if [ "$(uname -s)" = Linux ]; then
  for f in "$out"/lib*.so*; do
    patchelf --set-rpath '$ORIGIN' "$f"
  done
fi
echo "critic-libs: helper, llama.cpp libraries and $(ls "$dir"/backends | wc -l) backends in $out"
