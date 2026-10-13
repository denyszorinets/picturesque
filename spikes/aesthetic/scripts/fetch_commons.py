"""Download an aesthetic-evaluation sample from Wikimedia Commons into
spikes/aesthetic/sample/{featured,ordinary}/ (not committed).

featured: random camera photos from "Featured pictures on Wikimedia
Commons", chosen by community vote. ordinary: random camera photos (files
with EXIF camera model) from all of Commons. Both at 1024 px wide.
Images keep their own licenses (mostly CC BY-SA); they are only used
locally to measure the aesthetic score.

    python3 spikes/aesthetic/scripts/fetch_commons.py [count]
"""

import json
import random
import sys
import time
import urllib.parse
import urllib.request
from pathlib import Path

API = "https://commons.wikimedia.org/w/api.php"
OUT = Path(__file__).resolve().parent.parent / "sample"
HEADERS = {"User-Agent": "picturesque-aesthetic-eval/0.1 (https://github.com/denyszorinets/picturesque)"}
COUNT = int(sys.argv[1]) if len(sys.argv) > 1 else 60


def api(**params):
    params |= {"format": "json", "formatversion": "2"}
    req = urllib.request.Request(f"{API}?{urllib.parse.urlencode(params)}", headers=HEADERS)
    with urllib.request.urlopen(req, timeout=60) as r:
        return json.load(r)


def info(titles):
    """Thumbnail URL for camera JPEGs among `titles`."""
    data = api(
        action="query",
        titles="|".join(titles),
        prop="imageinfo",
        iiprop="url|mime|commonmetadata",
        iiurlwidth="1024",
    )
    for page in data["query"]["pages"]:
        ii = (page.get("imageinfo") or [None])[0]
        if not ii or ii.get("mime") != "image/jpeg":
            continue
        meta = {m["name"]: m["value"] for m in ii.get("commonmetadata", [])}
        if "Model" in meta and ii.get("thumburl"):
            yield page["title"], ii["thumburl"]


def featured(n):
    titles, cont = [], {}
    while True:
        data = api(
            action="query",
            list="categorymembers",
            cmtitle="Category:Featured pictures on Wikimedia Commons",
            cmtype="file",
            cmlimit="500",
            **cont,
        )
        titles += [m["title"] for m in data["query"]["categorymembers"]]
        if "continue" not in data or len(titles) >= 5000:
            break
        cont = data["continue"]
    random.seed(7)
    random.shuffle(titles)
    found = []
    for i in range(0, len(titles), 50):
        found += list(info(titles[i : i + 50]))
        if len(found) >= n:
            break
    return found[:n]


def ordinary(n):
    found = []
    while len(found) < n:
        data = api(action="query", list="random", rnnamespace="6", rnlimit="50")
        found += list(info([p["title"] for p in data["query"]["random"]]))
    return found[:n]


def download(items, folder):
    folder.mkdir(parents=True, exist_ok=True)
    for i, (title, url) in enumerate(items):
        dest = folder / f"{i:03d}.jpg"
        if dest.exists():
            continue
        req = urllib.request.Request(url, headers=HEADERS)
        with urllib.request.urlopen(req, timeout=60) as r:
            dest.write_bytes(r.read())
        (folder / "sources.txt").open("a").write(f"{dest.name}\t{title}\n")
        time.sleep(0.2)


download(featured(COUNT), OUT / "featured")
download(ordinary(COUNT), OUT / "ordinary")
print(f"wrote {OUT}")
