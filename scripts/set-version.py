"""Sets the app version everywhere it is recorded (docs/development/releasing.rst).

    python3 scripts/set-version.py 0.2.0          # change
    python3 scripts/set-version.py --check 0.2.0  # fail unless all match

Updates the workspace version in Cargo.toml (and the workspace crates in
Cargo.lock), src-tauri/tauri.conf.json and ui/package.json (and the root
package-lock.json entry for the ui workspace).
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CRATES = {"picturesque-core", "picturesque-app", "picturesque-infra",
          "picturesque-devserver", "picturesque-desktop"}


def versions():
    cargo = re.search(r'^\[workspace\.package\][^\[]*?^version = "([^"]+)"',
                      (ROOT / "Cargo.toml").read_text(), re.M | re.S).group(1)
    tauri = json.loads((ROOT / "src-tauri/tauri.conf.json").read_text())["version"]
    ui = json.loads((ROOT / "ui/package.json").read_text())["version"]
    return {"Cargo.toml": cargo, "tauri.conf.json": tauri, "ui/package.json": ui}


def set_version(new):
    path = ROOT / "Cargo.toml"
    text = path.read_text()
    text = re.sub(r'(^\[workspace\.package\][^\[]*?^version = ")[^"]+(")',
                  rf"\g<1>{new}\g<2>", text, count=1, flags=re.M | re.S)
    path.write_text(text)

    path = ROOT / "Cargo.lock"
    blocks = path.read_text().split("\n[[package]]\n")
    for i, block in enumerate(blocks):
        name = re.search(r'^name = "([^"]+)"', block, re.M)
        if name and name.group(1) in CRATES:
            blocks[i] = re.sub(r'^version = "[^"]+"', f'version = "{new}"', block,
                               count=1, flags=re.M)
    path.write_text("\n[[package]]\n".join(blocks))

    for rel in ["src-tauri/tauri.conf.json", "ui/package.json"]:
        path = ROOT / rel
        data = json.loads(path.read_text())
        data["version"] = new
        path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n")

    path = ROOT / "package-lock.json"
    if path.exists():
        lock = json.loads(path.read_text())
        if "ui" in lock.get("packages", {}):
            lock["packages"]["ui"]["version"] = new
        path.write_text(json.dumps(lock, indent=2, ensure_ascii=False) + "\n")


def main():
    args = sys.argv[1:]
    check = args[:1] == ["--check"]
    version = (args[1:] if check else args)[:1]
    if not version or not re.fullmatch(r"\d+\.\d+\.\d+(-[0-9A-Za-z.]+)?", version[0]):
        sys.exit("usage: set-version.py [--check] X.Y.Z")
    if not check:
        set_version(version[0])
    found = versions()
    wrong = {k: v for k, v in found.items() if v != version[0]}
    if wrong:
        sys.exit(f"version mismatch, expected {version[0]}: {wrong}")
    print(f"version {version[0]} everywhere")


main()
