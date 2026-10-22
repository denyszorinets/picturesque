#!/usr/bin/env bash
# Makes orient_1.jpg … orient_8.jpg: the same 144×96 photo stored so that
# EXIF orientation N displays it upright (tests/thumbnails.rs and the smoke
# test). Needs vips and exiv2.
#
#   crates/infra/tests/fixtures/make_orient.sh <photo>
#
# The fixtures are made from "Kayaks on a mountain lake (Unsplash).jpg" by
# Matheus Bandoch, CC0, from Wikimedia Commons (README.md).
set -euo pipefail
src=$1
dir=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
vips thumbnail "$src" "$tmp/upright.v" 144 --height 96 --crop centre
# The stored pixels: the inverse of what orientation N does when displayed.
store() {
  case $1 in
    1) vips copy "$tmp/upright.v" "$2" ;;
    2) vips flip "$tmp/upright.v" "$2" horizontal ;;
    3) vips rot "$tmp/upright.v" "$2" d180 ;;
    4) vips flip "$tmp/upright.v" "$2" vertical ;;
    5) vips rot "$tmp/upright.v" "$tmp/r.v" d90 && vips flip "$tmp/r.v" "$2" horizontal ;;
    6) vips rot "$tmp/upright.v" "$2" d270 ;;
    7) vips rot "$tmp/upright.v" "$tmp/r.v" d270 && vips flip "$tmp/r.v" "$2" horizontal ;;
    8) vips rot "$tmp/upright.v" "$2" d90 ;;
  esac
}
for n in 1 2 3 4 5 6 7 8; do
  out=$dir/orient_$n.jpg
  store "$n" "$tmp/stored.v"
  vips copy "$tmp/stored.v" "$out[Q=90,strip]"
  exiv2 -M "set Exif.Image.Orientation Short $n" mo "$out"
done
echo "made $dir/orient_{1..8}.jpg"
