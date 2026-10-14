#!/usr/bin/env bash
# Makes the macOS dmg from the app bundle built by `npx tauri build
# --bundles app`: copies Homebrew's libvips, LibRaw and their dependencies
# into the bundle, signs it ad hoc and packs it into
# target/release/bundle/dmg/Picturesque_<version>_<arch>.dmg.
#
# Needs: brew install vips libraw libheif dylibbundler
set -euo pipefail
cd "$(dirname "$0")/.."

version=$(node -p "require('./src-tauri/tauri.conf.json').version")
arch=$(uname -m | sed 's/arm64/aarch64/')
app=target/release/bundle/macos/Picturesque.app
out=target/release/bundle/dmg

[ -d "$app" ] || { echo "missing $app; run npx tauri build --bundles app first" >&2; exit 1; }

dylibbundler -od -b \
  -x "$app/Contents/MacOS/picturesque-desktop" \
  -d "$app/Contents/Frameworks" \
  -p @executable_path/../Frameworks/
codesign --force --deep --sign - "$app"
mkdir -p "$out"
hdiutil create -volname Picturesque -srcfolder "$app" -ov -format UDZO \
  "$out/Picturesque_${version}_${arch}.dmg"
echo "built $out/Picturesque_${version}_${arch}.dmg"
