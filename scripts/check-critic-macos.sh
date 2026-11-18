#!/usr/bin/env bash
# Verify the shipped helper and every dynamically loaded backend, including
# dependencies that an EOF startup probe alone would not load.
set -euo pipefail
dir=${1:?usage: check-critic-macos.sh path/to/critic}
dir=$(cd "$dir" && pwd)
[ -x "$dir/picturesque-critic" ] || { echo "missing critic helper" >&2; exit 1; }
for file in "$dir/picturesque-critic" "$dir"/*.dylib "$dir"/*.so; do
  [ -f "$file" ] || continue
  dependencies=$(otool -L "$file")
  while IFS= read -r line; do
    dependency=$(sed -E 's/^[[:space:]]+//; s/ \(compatibility version.*$//' <<<"$line")
    case "$dependency" in
      /usr/lib/*|/System/Library/*) continue ;;
      @rpath/*) local_path="$dir/${dependency#@rpath/}" ;;
      @loader_path/*) local_path="$dir/${dependency#@loader_path/}" ;;
      @executable_path/*) local_path="$dir/${dependency#@executable_path/}" ;;
      *) echo "external critic dependency: $file -> $dependency" >&2; exit 1 ;;
    esac
    [ -f "$local_path" ] || { echo "missing critic dependency: $file -> $dependency" >&2; exit 1; }
  done < <(tail -n +2 <<<"$dependencies")
done
# EOF must exit successfully even without models. Do not let a developer's
# library search paths conceal a broken package.
env -u DYLD_LIBRARY_PATH -u DYLD_FALLBACK_LIBRARY_PATH -u DYLD_FRAMEWORK_PATH \
  "$dir/picturesque-critic" </dev/null
echo "critic-macos: dependencies and startup verified"
