# Text search: results

2026-10-13, #45. OpenAI CLIP ViT-L/14 image and text encoders on the 150
Wikimedia Commons *Featured pictures* of `spikes/aesthetic/sample/featured`,
with `crates/infra/examples/search_sample.rs`. Top five per query, with
cosine similarity and the Commons file name; "median" is the similarity
of the median photo.

| Query | Median | Top results |
|---|---|---|
| a bird | 0.131 | 0.250 motmot, 0.240 long-tailed tit, 0.231 tree sparrow, 0.221 warbler, 0.221 bee-eater |
| flowers | 0.113 | 0.223 mock-orange flower, 0.219 Chinese quince flowers, 0.214 columbine, 0.208 geranium, 0.207 bugloss |
| a church interior | 0.086 | 0.221 cathedral in Chișinău, 0.220 altar in Brescia cathedral, 0.217 Jesuit church Vienna, 0.209 Albi cathedral choir, 0.198 Notre-Dame Montreal |
| mountains with snow | 0.100 | 0.225 Bürstegg in winter, 0.217 Byron Glacier, 0.216 Bow Lake, 0.206 Valle de la Luna, 0.184 Swiss mountain hike |
| a city at night | 0.102 | 0.202 Château Frontenac at night, 0.196 Ghent at dusk, 0.177 fountain in the evening, 0.169 sculpture in Lisbon, 0.163 Notre-Dame Montreal |
| people at the beach | 0.100 | 0.196 beach in Mar del Plata, 0.185 Borkum beach, 0.178 surfer, 0.172 pier at sunrise, 0.169 tram |
| a dog (none in the sample) | 0.126 | 0.194 jackal, then flowers, mountains and a cactus at 0.16 |

- Matches that fit the query score about 0.19–0.25; unrelated photos stay
  below about 0.17, apart from near misses such as the jackal for "a dog".
- **Decision:** results are ranked by similarity and cut at 0.17
  (`MIN_SIMILARITY`), which keeps the relevant photos and drops the
  unrelated tail.
