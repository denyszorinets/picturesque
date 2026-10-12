"""Build the offline reverse-geocoding dataset (ADR-0007).

Downloads GeoNames cities500, admin1 names and country names, keeps
populated places that name a town or city (dropping neighbourhoods,
historical, abandoned and destroyed places), and writes
crates/infra/data/geonames-cities500.tsv.gz:

    C <tab> country code <tab> country name
    P <tab> place name <tab> region name <tab> country code <tab> lat <tab> lon <tab> population

GeoNames data is licensed CC BY 4.0 (https://www.geonames.org/).

Usage: python3 -I scripts/build_geonames.py [download-dir]
"""

import csv
import datetime
import gzip
import io
import sys
import urllib.request
import zipfile
from pathlib import Path

BASE = "https://download.geonames.org/export/dump/"
OUT = Path(__file__).resolve().parent.parent / "crates/infra/data/geonames-cities500.tsv.gz"
# Sections of towns, historical, abandoned, destroyed and religious places.
EXCLUDED_FEATURES = {"PPLX", "PPLH", "PPLQ", "PPLW", "PPLCH", "PPLR"}


def fetch(name, cache):
    path = cache / name
    if not path.exists():
        print(f"downloading {name}", file=sys.stderr)
        urllib.request.urlretrieve(BASE + name, path)
    return path


def main():
    cache = Path(sys.argv[1]) if len(sys.argv) > 1 else Path.cwd()
    cache.mkdir(parents=True, exist_ok=True)

    countries = {}
    for line in fetch("countryInfo.txt", cache).read_text(encoding="utf-8").splitlines():
        if line.startswith("#") or not line.strip():
            continue
        cols = line.split("\t")
        countries[cols[0]] = cols[4]

    regions = {}
    for line in fetch("admin1CodesASCII.txt", cache).read_text(encoding="utf-8").splitlines():
        code, name, _ascii, _id = line.split("\t")
        regions[code] = name

    places = []
    with zipfile.ZipFile(fetch("cities500.zip", cache)) as z:
        with z.open("cities500.txt") as f:
            for row in csv.reader(io.TextIOWrapper(f, encoding="utf-8"), delimiter="\t", quoting=csv.QUOTE_NONE):
                feature, cc, admin1 = row[7], row[8], row[10]
                if feature in EXCLUDED_FEATURES or cc not in countries:
                    continue
                name = row[1].strip()
                if not name:
                    continue
                region = regions.get(f"{cc}.{admin1}", "")
                population = row[14] or "0"
                places.append((name, region, cc, f"{float(row[4]):.4f}", f"{float(row[5]):.4f}", population))

    places.sort(key=lambda p: (p[2], p[0]))
    used = sorted({p[2] for p in places})
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with gzip.open(OUT, "wt", encoding="utf-8", compresslevel=9) as out:
        out.write(f"# GeoNames cities500, CC BY 4.0, https://www.geonames.org/ - built {datetime.date.today()}\n")
        for cc in used:
            out.write(f"C\t{cc}\t{countries[cc]}\n")
        for p in places:
            out.write("P\t" + "\t".join(p) + "\n")
    print(f"{len(places)} places, {len(used)} countries -> {OUT} ({OUT.stat().st_size / 1e6:.1f} MB)", file=sys.stderr)


if __name__ == "__main__":
    main()
