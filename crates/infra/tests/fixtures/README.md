# Test fixtures

| File | What | How it was made |
|---|---|---|
| `exif.heic` | 64×48 HEIC with EXIF: Make `Apple`, Model `iPhone 15 Pro`, DateTimeOriginal `2026:10:12 09:15:00`, OffsetTimeOriginal `-04:00` | JPEG written by `tests/support::write_jpeg` with those fields, then `heif-enc -q 50 -o exif.heic in.jpg` (libheif 1.23, x265) |

JPEG fixtures are generated at test time by `tests/support/mod.rs`.
