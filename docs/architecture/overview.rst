Overview
========

Picturesque is a desktop application that imports photos into an organized
library and helps the user cull them: it groups shots into series, scores
each photo, and highlights the best ones. Everything runs locally; no photo
or metadata leaves the machine.

System context
--------------

.. mermaid::

   flowchart LR
     user([Photographer])
     subgraph machine[User's computer]
       app[Picturesque]
       src[(Source folders)]
       clean[(Clean library<br/>photos + XMP sidecars)]
       catalog[(Catalog<br/>SQLite + caches)]
       models[(AI models)]
     end
     other[Lightroom / darktable / digiKam]

     user --> app
     app -- reads, never modifies --> src
     app -- copies, organizes --> clean
     app <--> catalog
     app -- loads --> models
     other -- reads ratings/flags --> clean

- **Source folders** are read-only inputs. Picturesque never modifies or
  deletes them
  (:doc:`ADR-0008 </architecture/decisions/0008-non-destructive-verified-import>`).
- **Clean library** is the organized output: photos copied into a folder
  structure rendered from a user template, plus XMP sidecars that carry
  ratings and pick/reject flags to other tools
  (:doc:`ADR-0009 </architecture/decisions/0009-xmp-sidecars-for-culling-decisions>`).
- **Catalog** is the application's own state: photo records, metadata,
  scores, series, decisions, thumbnails and embeddings
  (:doc:`ADR-0004 </architecture/decisions/0004-sqlite-catalog>`).
- **AI models** are ONNX files downloaded once and run on-device
  (:doc:`ADR-0006 </architecture/decisions/0006-local-ai-with-onnx-runtime>`).

Main workflows
--------------

Import
   Scan source folders, read metadata, resolve GPS to place names offline,
   render the target path from the template, copy and verify each file,
   record it in the catalog.

Analyze
   In the background: generate thumbnails and previews, compute technical
   scores (sharpness, exposure), AI scores (aesthetics, faces and eyes) and
   embeddings, group photos into series, and rank the best photo in each.

Cull
   Browse the timeline, where each series is drawn inside a frame and the
   best photo has a green border; pick, reject and rate with the keyboard;
   filter by metadata and scores. Decisions are stored in the catalog and
   written to XMP sidecars.

Output
   Move rejects out of the way and export picks.

Technology
----------

.. list-table::
   :header-rows: 1

   * - Concern
     - Choice
     - Decision
   * - Desktop shell and UI
     - Tauri 2, Svelte 5, TypeScript
     - :doc:`ADR-0002 </architecture/decisions/0002-tauri-2-and-svelte-5>`
   * - Code structure
     - Cargo workspace with layered crates
     - :doc:`ADR-0003 </architecture/decisions/0003-layered-cargo-workspace>`
   * - Catalog
     - SQLite via ``rusqlite``
     - :doc:`ADR-0004 </architecture/decisions/0004-sqlite-catalog>`
   * - Image decoding
     - libvips + LibRaw (proposed)
     - :doc:`ADR-0005 </architecture/decisions/0005-image-decoding-stack>`
   * - AI inference
     - ONNX Runtime via ``ort``
     - :doc:`ADR-0006 </architecture/decisions/0006-local-ai-with-onnx-runtime>`
   * - Reverse geocoding
     - Offline GeoNames dataset
     - :doc:`ADR-0007 </architecture/decisions/0007-offline-reverse-geocoding>`
   * - Import safety
     - Copy, hash, verify; sources read-only
     - :doc:`ADR-0008 </architecture/decisions/0008-non-destructive-verified-import>`
   * - Interoperability
     - XMP sidecars
     - :doc:`ADR-0009 </architecture/decisions/0009-xmp-sidecars-for-culling-decisions>`
   * - UI transport
     - Tauri IPC, plus an HTTP transport for development
     - :doc:`ADR-0010 </architecture/decisions/0010-dual-transport-tauri-ipc-and-dev-http>`

Quality priorities
------------------

1. **Never lose a photo.** Imports are copy-only and verified; destructive
   actions (moving rejects) require confirmation and can be undone.
2. **Fast culling.** The grid and loupe stay responsive while analysis runs
   in the background.
3. **Private and offline.** No network access is needed after models are
   downloaded.
4. **Interoperable.** Decisions are readable by other photo tools.

See :doc:`components` for the code structure and :doc:`data` for the data
model.
