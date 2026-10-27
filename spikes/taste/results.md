# Personal taste: results

2026-10-27. Sample: the aesthetic spike's 150 Wikimedia Commons *Featured
pictures* and 150 random Commons photos (`spikes/aesthetic/results.md`),
embedded like the app (CLIP ViT-L/14). The first half of each group trains
the model; the second half (150 photos) is held out. AUC: the chance that
a held-out "pick" outscores a held-out "reject" (0.5 = chance; ±0.04 at
this size). *Cross-val* is the model's own 5-fold estimate, which the app
reports as agreement.

Picks = featured, rejects = ordinary (a taste close to the general one):

| Picks and rejects each | Cross-val | Held-out | Blend 50 % with LAION |
|---|---|---|---|
| general score (LAION) only | — | 0.787 | — |
| 10 | 0.720 | 0.822 | 0.873 |
| 20 | 0.920 | 0.860 | 0.885 |
| 40 | 0.916 | 0.893 | 0.911 |
| 75 | 0.935 | 0.927 | 0.930 |

Picks = ordinary, rejects = featured (a taste the general score gets
backwards: LAION alone 0.213):

| Picks and rejects each | Held-out | Blend 50 % with LAION |
|---|---|---|
| 10 | 0.822 | 0.611 |
| 40 | 0.893 | 0.747 |
| 75 | 0.927 | 0.835 |

Training on the largest set the app uses (1,000 + 1,000 examples, 5-fold
cross-validation and calibration included): 1.2 s on a Ryzen 7 PRO
7840HS, one thread.

- A few dozen decisions already beat the general score on its own
  benchmark; blending helps while there are few decisions.
- The model follows a taste the general score contradicts, so the blend
  weight is a setting: users whose taste differs from the crowd's can
  raise it.
- With 10 of each, cross-validation underestimates (folds of 4 photos);
  from 20 on it is close to, and slightly above, the held-out AUC.

**Decision:** learn from flags only, from 10 picks and 10 rejects, at most
1,000 of each; blend half and half by default.
