# Aesthetic score: results

2026-10-13. Sample: 150 Wikimedia Commons *Featured pictures* (community
vote) and 150 random Commons camera photos, 1024 px, processed like the
app (preview → shortest side 224, centre crop → CLIP → linear head).
AUC: the chance that a random featured photo scores higher than a random
ordinary one (0.5 = chance; with 150 + 150 photos, ±0.03).

| Embeddings | Head | AUC | Featured median | Ordinary median | CPU per photo |
|---|---|---|---|---|---|
| OpenCLIP ViT-B/32 (LAION-2B) | LAION V1 B/32 (MIT) | 0.55 (60 + 60) | 2.19 | 2.10 | 37 ms |
| OpenAI CLIP ViT-B/32 | LAION V1 B/32 (MIT) | 0.62 | 6.09 | 5.74 | 37 ms |
| OpenAI CLIP ViT-L/14 | improved LAION predictor (Apache-2.0) | **0.80** | 5.64 | 5.13 | 325 ms |
| — | technical score (#32) | 0.66 | 0.98 | 0.93 | — |

CPU: Ryzen 7 PRO 7840HS, 16 threads, including JPEG decoding.

- The V1 head was trained on OpenAI embeddings; on OpenCLIP embeddings it
  is uncalibrated and near chance.
- Even on matching embeddings, V1 is weaker than the plain technical
  score. The improved predictor on ViT-L/14 is clearly better.
- The improved predictor's layers are linear without activations, so they
  collapse into a single 768-weight linear head.

**Decision:** the app uses OpenAI CLIP ViT-L/14 (1.2 GB download) with the
improved predictor. A 10,000-photo library takes about an hour on this
CPU in the background (photos on screen first). Ratings are mapped to a
0–1 score linearly from 4.5 to 6.5, about the 2nd and 98th percentile of
this sample (ordinary median → 0.31, featured median → 0.57).
