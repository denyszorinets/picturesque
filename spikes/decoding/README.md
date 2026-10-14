# Decoding spike (issue #10)

Throwaway benchmark behind ADR-0005
(`docs/architecture/decisions/0005-image-decoding-stack.rst`). Not part of
the Cargo workspace and not maintained.

Needs libvips, LibRaw and libheif development packages (the devcontainer has
them). HEIC needs an HEVC decoder plugin, e.g. RPM Fusion's
`libheif-freeworld` on Fedora.

```bash
python3 -I scripts/fetch_corpus.py        # ~630 MB into corpus/ (git-ignored)
cargo run --release -- corpus > results.md
```

`results.md` holds the run recorded in the ADR.
