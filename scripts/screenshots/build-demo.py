#!/usr/bin/env python3
"""Builds the demo "memory card" for the screenshots (docs/development/website.rst).

    python3 scripts/screenshots/build-demo.py <card folder>

Downloads the photos of photos.json from Wikimedia Commons (cached in
target/screenshots/originals), refuses any that is not CC0, and turns each
into a burst: shifted crops a second apart, all but one blurred, with the
trip's camera, lens, focal length, time and GPS position in their EXIF.
Needs the `vips` and `exiv2` command-line tools.
"""

import json
import re
import shutil
import subprocess
import sys
import urllib.parse
import urllib.request
from datetime import datetime, timedelta
from pathlib import Path

HERE = Path(__file__).resolve().parent
CACHE = HERE.parents[1] / "target" / "screenshots" / "originals"
API = "https://commons.wikimedia.org/w/api.php"
HEADERS = {"User-Agent": "picturesque-screenshots/1.0 (https://github.com/denyszorinets/picturesque)"}
# Long side of the downloaded originals and of the frames made from them.
DOWNLOAD_WIDTH = 3200
FRAME_SIZE = 2400


def commons_info(title: str) -> dict:
    query = urllib.parse.urlencode(
        {
            "action": "query",
            "titles": f"File:{title}",
            "prop": "imageinfo",
            "iiprop": "url|extmetadata",
            "iiurlwidth": DOWNLOAD_WIDTH,
            "iiextmetadatafilter": "LicenseShortName|Artist",
            "format": "json",
        }
    )
    request = urllib.request.Request(f"{API}?{query}", headers=HEADERS)
    with urllib.request.urlopen(request, timeout=60) as response:
        pages = json.load(response)["query"]["pages"]
    page = next(iter(pages.values()))
    if "imageinfo" not in page:
        sys.exit(f"not on Wikimedia Commons: {title}")
    return page["imageinfo"][0]


def download(title: str, author: str) -> Path:
    path = CACHE / title
    if path.exists():
        return path
    info = commons_info(title)
    meta = info.get("extmetadata", {})
    licence = meta.get("LicenseShortName", {}).get("value", "")
    if "CC0" not in licence:
        sys.exit(f"{title} is {licence or 'not licensed'}, not CC0")
    artist = re.sub(r"<[^>]+>", "", meta.get("Artist", {}).get("value", ""))
    if author.split()[0].lower() not in artist.lower():
        sys.exit(f"{title}: Commons credits {artist.strip()!r}, not {author!r}")
    CACHE.mkdir(parents=True, exist_ok=True)
    request = urllib.request.Request(info["thumburl"], headers=HEADERS)
    with urllib.request.urlopen(request, timeout=120) as response:
        path.with_suffix(".part").write_bytes(response.read())
    path.with_suffix(".part").rename(path)
    return path


def run(*args: str) -> None:
    subprocess.run(args, check=True, stdout=subprocess.DEVNULL)


def dms(value: float) -> str:
    """Degrees, minutes and hundredths of seconds as EXIF rationals."""
    value = abs(value)
    degrees = int(value)
    minutes = int((value - degrees) * 60)
    seconds = round(((value - degrees) * 60 - minutes) * 60 * 100)
    return f"{degrees}/1 {minutes}/1 {seconds}/100"


def header(path: Path, field: str) -> int:
    return int(subprocess.check_output(["vipsheader", "-f", field, str(path)], text=True))


def make_frame(source: Path, out: Path, k: int, frames: int, best: int) -> None:
    """Frame `k` of a burst: a 90% crop moving sideways, blurred unless best."""
    width, height = header(source, "width"), header(source, "height")
    crop_w, crop_h = int(width * 0.9), int(height * 0.9)
    left = int((width - crop_w) * k / max(frames - 1, 1))
    top = (height - crop_h) // 2
    tmp = out.with_suffix(".crop.v")
    run("vips", "crop", str(source), str(tmp), str(left), str(top), str(crop_w), str(crop_h))
    if k != best:
        blurred = out.with_suffix(".blur.v")
        run("vips", "gaussblur", str(tmp), str(blurred), str(1.2 + abs(k - best) * 0.9))
        tmp.unlink()
        tmp = blurred
    run("vips", "thumbnail", str(tmp), f"{out}[Q=88,strip]", str(FRAME_SIZE))
    tmp.unlink()


def write_exif(path: Path, trip: dict, taken: datetime) -> None:
    lat, lon = trip["gps"]
    make, model = trip["camera"]
    commands = [
        f"set Exif.Image.Make {make}",
        f"set Exif.Image.Model {model}",
        f"set Exif.Photo.LensModel {trip['lens']}",
        f"set Exif.Photo.FocalLength {trip['focal']}/1",
        f"set Exif.Photo.DateTimeOriginal {taken:%Y:%m:%d %H:%M:%S}",
        f"set Exif.Image.DateTime {taken:%Y:%m:%d %H:%M:%S}",
        "set Exif.Photo.FNumber 28/10",
        "set Exif.Photo.ExposureTime 1/250",
        "set Exif.Photo.ISOSpeedRatings 400",
        f"set Exif.GPSInfo.GPSLatitudeRef {'N' if lat >= 0 else 'S'}",
        f"set Exif.GPSInfo.GPSLatitude {dms(lat)}",
        f"set Exif.GPSInfo.GPSLongitudeRef {'E' if lon >= 0 else 'W'}",
        f"set Exif.GPSInfo.GPSLongitude {dms(lon)}",
    ]
    args = ["exiv2"]
    for command in commands:
        args += ["-M", command]
    run(*args, "mo", str(path))


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    card = Path(sys.argv[1])
    if card.exists():
        shutil.rmtree(card)
    spec = json.loads((HERE / "photos.json").read_text())
    number = 1000
    for trip in spec["trips"]:
        folder = card / "DCIM" / trip["name"]
        folder.mkdir(parents=True)
        taken = datetime.fromisoformat(trip["start"])
        for photo in trip["photos"]:
            source = download(photo["file"], photo["author"])
            frames = photo["frames"]
            best = frames // 2
            for k in range(frames):
                number += 1
                out = folder / f"IMG_{number}.JPG"
                make_frame(source, out, k, frames, best)
                write_exif(out, trip, taken)
                taken += timedelta(seconds=1)
            # The next scene a few minutes later: a new series.
            taken += timedelta(minutes=7)
        print(f"{trip['name']}: {sum(p['frames'] for p in trip['photos'])} photos")
    print(f"demo card: {card}")


if __name__ == "__main__":
    main()
