# Test fixtures

| File | What | How it was made |
|---|---|---|
| `exif.heic` | 64×48 HEIC with EXIF: Make `Apple`, Model `iPhone 15 Pro`, DateTimeOriginal `2026:10:12 09:15:00`, OffsetTimeOriginal `-04:00` | JPEG written by `tests/support::write_jpeg` with those fields, then `heif-enc -q 50 -o exif.heic in.jpg` (libheif 1.23, x265) |

| `orient_1.jpg` … `orient_8.jpg` | 144×96 photo stored so that EXIF orientation N shows it upright | `make_orient.sh` from [Kayaks on a mountain lake (Unsplash).jpg](https://commons.wikimedia.org/wiki/File:Kayaks_on_a_mountain_lake_(Unsplash).jpg) by Matheus Bandoch, CC0 |
| `faces/*.jpg` | Open and closed eyes | Public domain and CC0 photos from Wikimedia Commons; see [faces/SOURCES.md](faces/SOURCES.md) |

Other JPEG fixtures are generated at test time by `tests/support/mod.rs`.
Every image here is CC0, public domain or made by the project.
