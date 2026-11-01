.. _features-insights:

Insights
========

The **Insights** tab summarizes what the critic has said about your photos
as a whole: which problems recur, at which camera settings, and whether
your ratings are rising or falling. It reads the critiques already stored
in the catalog (:doc:`ai`), so opening it never runs a model and it works
whether critiques are switched on or off. Rejected photos and outdated
critiques are included.

What the tab shows
------------------

- **What to work on.** The first section: up to five pieces of advice,
  most frequent problem first. Each has a title, a short remedy, an
  evidence line such as "Named in 35% of your recent critiques (14
  photos); 2.4x as often (1/30 s or longer)" and example photos. A
  problem is advised on when at least 3 critiques name it and its
  recent-weighted share is at least 10%. The remedy text is fixed per
  problem kind. When the problem is more common in one setting range
  (the strongest lift) and the range makes the advice true, a sentence
  about the settings is added: for example motion blur with a slow
  shutter suggests a faster shutter or stabilisation, noise at ISO 801
  and above suggests more light or a wider aperture, missed focus at
  wide apertures suggests stopping down. The rules are fixed and no model
  is involved, so the same library always reads the same. While old
  critiques are being redone, the counts are split between the older
  coarse kinds and the new specific ones, so advice may appear only once
  enough of them are redone. For critiques
  made before prompt version 5, which carry only the coarse exposure,
  focus and composition tags, the advice is generic, for example "Redo
  the critiques to learn whether you over- or underexpose".

- **Critiques** and **Average rating.** The number of stored critiques,
  the newest capture date they reach ("Up to 2026-09-30"), and the average
  rating with recent photos counting more. The plain average is shown next
  to it.
- **Recurring problems.** For each problem category (overexposed, underexposed, missed
  focus, motion blur, subject placement, subject cut off, busy background,
  noise, colour, horizon, light, timing, other; the older exposure, focus
  and composition still appear for earlier critiques) the
  share of critiques that name it, with recent critiques counting more, the
  number of critiques behind it, and up to six example photos. Examples are
  the photos that show it most clearly: the lowest rated and most recent
  first.
- **Settings habits.** Up to ten problems that show up much more often at
  one camera setting, for example "Noise shows up 2.1x as often at ISO
  above 3200".
- **Rating over time.** The average rating per calendar quarter, and from
  three quarters on a sentence such as "Your ratings are rising by about
  0.4 points a year." Below three quarters the tab says a trend appears
  once critiqued photos span three quarters.

Recent photos count more
------------------------

Each critique has a weight that halves every 365 days of age, so a photo
from a year ago counts half as much as a new one and one from two years
ago a quarter. The half-life is fixed; it is not a setting.

Age is measured from the **newest capture date among the critiqued
photos**, not from today's date. The same library therefore always gives
the same insights, and an old library is not judged as if all its photos
were ancient. Photos without a capture date weigh 0.5 and are left out of
the trend.

Thresholds
----------

- **Fewer than 10 critiques:** the tab says the insights are based on a
  few critiques and that 10 or more give steadier ones.
- **Problem statistics** use only critiques that name problem categories,
  which critiques made with prompt version 4 or later do (version 5 names
  the finer kinds). Older critiques
  count toward the ratings and the trend but not toward the problems. If
  none name problems, the tab says so and suggests redoing the critiques
  from the loupe or in the background.
- **A settings habit** needs a problem flagged at least 1.5 times as often
  within a range of ISO, shutter speed, aperture or focal length as across
  all photos with that setting. The range needs at least 8 photos, of which
  at least 3 are flagged.
  The ranges are:

  ======================= ===============================================
  ISO                     200 or lower, 201-800, 801-3200, above 3200
  Shutter speed           1/30 s or longer, between 1/250 and 1/30 s,
                          1/250 s or faster
  Aperture                wider than f/2.8, f/2.8 to f/5.6, f/5.6 to
                          f/11, f/11 or narrower
  Focal length            under 24 mm, 24 to 50 mm, 50 to 100 mm,
                          100 mm or longer
  ======================= ===============================================

Caveats
-------

- The critiques come from a small local model and are rough. Treat the
  insights as hints about habits, not as measurements.
- Few critiques give noisy insights: one bad quarter or a handful of
  flagged photos can swing a figure.
- A critique stores the model's problem categories; they are not checked
  against the photo.
