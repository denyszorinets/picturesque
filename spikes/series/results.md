# Series signals: results

2026-10-13, #47. How well two signals recognize the same scene: for each
of 150 random Wikimedia Commons photos (`spikes/aesthetic/sample/ordinary`),
three variants of the kind seen in bursts and retakes (a 75% centre crop =
zooming in, a crop shifted sideways = reframing, a copy at 60% brightness =
exposure change), compared with the original; and all 11,175 pairs of
different photos. `crates/infra/examples/series_similarity.rs`, OpenAI CLIP
ViT-L/14.

| Pair | CLIP similarity p5 / median / max | Perceptual hash bits p5 / median / max |
|---|---|---|
| same scene, zoomed in | 0.875 / 0.952 / 0.985 | 14 / 24 / 36 |
| same scene, reframed | 0.930 / 0.977 / 0.995 | 16 / 22 / 36 |
| same scene, darker | 0.933 / 0.975 / 0.993 | 0 / 0 / 2 |
| different photos | 0.366 / 0.529 / **0.901** (p95 0.686) | 24 / 32 / 48 |

- The perceptual hash (#33) recognizes exposure changes but misses most
  zoomed or reframed retakes at its 16-bit limit.
- CLIP embeddings recognize all three, and different photos rarely come
  close: none above 0.90, 95% below 0.69.
- **Decision:** two photos look alike when their hashes are within the
  limit *or* their embeddings' similarity is at least 0.87, which keeps
  about 95% of zoomed-in and nearly all reframed variants. Photos must
  still be taken within the series' time gap.
