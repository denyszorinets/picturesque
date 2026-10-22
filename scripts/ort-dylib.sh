#!/usr/bin/env bash
# Makes target/<profile>/libonnxruntime.so, the ONNX Runtime the app loads
# at first use on Linux x86-64 (ADR-0015, #154), and copies Dawn
# (libwebgpu_dawn.so, its WebGPU provider) beside it:
#
#   scripts/ort-dylib.sh [debug|release]
#
# pyke only ships ONNX Runtime as a static library, so this links it into a
# shared one that exports OrtGetApiBase, the entry point `ort` loads, and
# nothing else. Does nothing on other systems, where the runtime is linked
# into the executable.
set -euo pipefail
cd "$(dirname "$0")/.."
profile=${1:-debug}
[ "$(uname -s)-$(uname -m)" = Linux-x86_64 ] || exit 0

out=target/$profile
mkdir -p "$out"
# The folder of pyke's build, from ort-sys's build script.
dir=$(cargo build -q -p picturesque-ort-dylib --message-format=json |
  python3 -c '
import json, sys
for line in sys.stdin:
    m = json.loads(line)
    if m.get("reason") == "build-script-executed" and "/ort-sys" in m.get("package_id", "") + m.get("out_dir", ""):
        for p in m.get("linked_paths", []):
            print(p.split("=", 1)[-1])
' | head -1)
archive=$dir/libonnxruntime.a
[ -f "$archive" ] || { echo "ort-dylib: no ONNX Runtime archive in '$dir'" >&2; exit 1; }

# A real copy, also where an older build left a link into pyke's cache.
rm -f "$out/libwebgpu_dawn.so"
cp -p "$dir/libwebgpu_dawn.so" "$out/libwebgpu_dawn.so"
if [ "$out/libonnxruntime.so" -nt "$archive" ]; then
  exit 0
fi
map=$(mktemp)
trap 'rm -f "$map"' EXIT
echo '{ global: OrtGetApiBase; local: *; };' > "$map"
${CXX:-c++} -shared -o "$out/libonnxruntime.so" \
  -Wl,-soname,libonnxruntime.so \
  -Wl,--undefined=OrtGetApiBase -Wl,--version-script="$map" -Wl,--gc-sections \
  -Wl,-rpath,'$ORIGIN' \
  "$archive" -L"$dir" -lwebgpu_dawn -lpthread -ldl -lm
echo "ort-dylib: $out/libonnxruntime.so"
