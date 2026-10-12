"""Download the decoding-spike test corpus into spikes/decoding/corpus/.

RAW samples come from raw.pixls.us (CC0). One file per listed camera.
Orientation samples: github.com/recurser/exif-orientation-examples (MIT).
HEIC samples: libheif's example images (github.com/strukturag/libheif).
"""
import re
import sys
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = "https://raw.pixls.us/data/"
OUT = Path(__file__).resolve().parent.parent / "corpus"

# (make directory, regex matched against model directory names)
CAMERAS = [
    ("Canon", r"^EOS 5D Mark IV$"),
    ("Canon", r"^EOS R5$"),
    ("Canon", r"^Canon EOS R6 Mark III$"),
    ("Nikon", r"^D850$"),
    ("Nikon", r"^Z 8$"),
    ("Sony", r"^ILCE-7M3$"),
    ("Sony", r"^ILCE-7RM5$"),
    ("Fujifilm", r"^X-T3$"),
    ("Fujifilm", r"^X-T5$"),
    ("Olympus", r"^E-M1MarkII$"),
    ("OM Digital Solutions", r"^OM-3$"),
    ("Panasonic", r"^DC-GH5$"),
    ("Pentax", r"^K-1$"),
    ("Leica", r"^Q2$"),
    ("Apple", r"^iPhone 1[2-6] Pro"),
    ("Google", r"^Pixel"),
]

OTHER = {
    "orientation": [
        f"https://raw.githubusercontent.com/recurser/exif-orientation-examples/master/Landscape_{i}.jpg"
        for i in range(1, 9)
    ],
    "heic": [
        "https://raw.githubusercontent.com/strukturag/libheif/master/examples/example.heic",
    ],
}


def listing(url):
    html = urllib.request.urlopen(url, timeout=60).read().decode()
    return [urllib.parse.unquote(h) for h in re.findall(r'href="([^"?/][^"]*)"', html)]


def download(url, dest):
    if dest.exists():
        return
    dest.parent.mkdir(parents=True, exist_ok=True)
    print(f"  {url}", flush=True)
    tmp = dest.with_suffix(dest.suffix + ".part")
    with urllib.request.urlopen(url, timeout=600) as r, open(tmp, "wb") as f:
        while chunk := r.read(1 << 20):
            f.write(chunk)
    tmp.rename(dest)


def main():
    for make, pattern in CAMERAS:
        make_url = ROOT + urllib.parse.quote(make) + "/"
        try:
            models = [m.rstrip("/") for m in listing(make_url) if m.endswith("/")]
        except Exception as e:  # noqa: BLE001
            print(f"skip {make}: {e}", file=sys.stderr)
            continue
        matches = [m for m in models if re.search(pattern, m)]
        if not matches:
            print(f"skip {make}/{pattern}: no model (have {models[:8]}...)", file=sys.stderr)
            continue
        model = matches[0]
        model_url = make_url + urllib.parse.quote(model) + "/"
        files = [f for f in listing(model_url) if not f.endswith("/") and "." in f]
        if not files:
            print(f"skip {make}/{model}: no files", file=sys.stderr)
            continue
        name = files[0]
        print(f"{make} / {model}: {name}")
        download(model_url + urllib.parse.quote(name), OUT / "raw" / f"{make}_{model}_{name}".replace(" ", "_"))

    for kind, urls in OTHER.items():
        for url in urls:
            download(url, OUT / kind / url.rsplit("/", 1)[1])


if __name__ == "__main__":
    main()
