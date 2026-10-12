# Picturesque

Open-source photo culling for Linux, macOS and Windows.

Picturesque imports photos from scattered folders into a clean, organized
library, groups shots into series, and highlights the best photos using
technical analysis and on-device AI. Nothing leaves your machine.

> **Status:** early development. See the
> [milestones](https://github.com/denyszorinets/picturesque/milestones).

## Planned features

- **Import:** copy photos into a "Clean" folder structure built from a template
  (date, place from GPS, camera, …), skipping duplicates and verifying every copy.
  Source folders are never modified.
- **Series:** bursts and retakes grouped together, with the best shot highlighted.
- **Scores:** sharpness, exposure, closed eyes and an AI aesthetic score.
- **Culling:** keyboard-driven pick/reject/rating, filters, compare view.
- **Interoperability:** decisions written to XMP sidecars for Lightroom,
  darktable and digiKam.

## Development

Built with Rust, Tauri 2 and Svelte 5. The devcontainer in `.devcontainer/`
has every dependency installed.

```bash
make check       # format check, clippy, tests, build
make docs-serve  # documentation preview
```

See `docs/development/building.rst` for details and
`docs/architecture/` for the architecture and decision records.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
