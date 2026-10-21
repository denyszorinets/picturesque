#!/usr/bin/env bash
# Removes duplicate LC_RPATH entries from Mach-O files, keeping one of each:
#
#   scripts/dedupe-rpaths.sh <file>...
#
# Since macOS 15.4, dyld refuses to load a library that lists the same rpath
# twice ("duplicate LC_RPATH"), and dylibbundler adds one for each bundled
# dependency it fixes (#52). Exits with an error if a duplicate remains.
# OTOOL and INSTALL_NAME_TOOL override the tools (llvm-otool and
# llvm-install-name-tool work too).
set -euo pipefail
otool=${OTOOL:-otool}
install_name_tool=${INSTALL_NAME_TOOL:-install_name_tool}

rpaths() {
  "$otool" -l "$1" | awk '/cmd LC_RPATH/ { getline; getline; print $2 }'
}

# How often `file` lists `rpath`. Reads the whole list: `grep -q` would stop
# at the first match and, under pipefail, fail the pipeline when awk loses
# its reader.
count() {
  local list
  list=$(rpaths "$1")
  grep -cxF -- "$2" <<<"$list" || true
}

status=0
for file in "$@"; do
  while read -r rpath; do
    [ -n "$rpath" ] || continue
    # Delete every copy, then add it back once.
    while [ "$(count "$file" "$rpath")" -gt 0 ]; do
      "$install_name_tool" -delete_rpath "$rpath" "$file"
    done
    "$install_name_tool" -add_rpath "$rpath" "$file"
    echo "dedupe-rpaths: $(basename "$file"): one $rpath left"
  done < <(rpaths "$file" | sort | uniq -d)
  if [ -n "$(rpaths "$file" | sort | uniq -d)" ]; then
    echo "dedupe-rpaths: $file still lists an rpath twice" >&2
    status=1
  fi
done
exit $status
