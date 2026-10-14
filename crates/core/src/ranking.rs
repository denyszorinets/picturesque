//! Ranking the photos of a series to find the best one, and the combined
//! quality of single photos.
//!
//! Each photo gets a score from its technical scores and, once AI models
//! ran, its aesthetic score and faces, weighted by the user's preferences.
//! Sharpness counts relative to the sharpest photo of the series: in a
//! burst every frame may score as "sharp", and the one with the crispest
//! detail should still win; with faces, the face's sharpness counts as
//! much as the whole photo's. Closed eyes halve a photo's score. Rejected
//! photos rank last, photos not analyzed yet just before them. The user
//! can choose the best photo themselves; their latest choice in a series
//! wins.

use serde::{Deserialize, Serialize};

use crate::photo::PhotoId;
use crate::quality::TechnicalScores;

/// How much each score counts. Only the ratios matter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RankWeights {
    pub sharpness: f32,
    pub exposure: f32,
    pub noise: f32,
    /// The aesthetic score, when the CLIP model ran.
    pub aesthetic: f32,
}

impl Default for RankWeights {
    fn default() -> Self {
        Self {
            sharpness: 0.6,
            exposure: 0.3,
            noise: 0.1,
            aesthetic: 0.4,
        }
    }
}

impl RankWeights {
    /// Weights must be finite, non-negative and the technical ones not all
    /// zero.
    pub fn is_valid(&self) -> bool {
        let technical = [self.sharpness, self.exposure, self.noise];
        technical
            .iter()
            .chain([&self.aesthetic])
            .all(|w| w.is_finite() && *w >= 0.0)
            && technical.iter().sum::<f32>() > 0.0
    }
}

/// Closed eyes multiply a photo's score by this.
pub const CLOSED_EYES_FACTOR: f32 = 0.5;

/// What the face models say about a photo's people.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceSignal {
    /// Whether someone important (the largest face, or one at least half
    /// its size) has closed eyes; `None` when no eyes could be judged.
    pub eyes_closed: Option<bool>,
    /// Sharpness (0–1) of the largest face.
    pub sharpness: f32,
}

/// A photo of a series, as ranking sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    pub id: PhotoId,
    /// `None` until analyzed (or if analysis failed).
    pub scores: Option<TechnicalScores>,
    pub rejected: bool,
    /// Aesthetic score 0–1, once the CLIP model ran.
    pub aesthetic: Option<f32>,
    /// `None` without faces (or before the face models ran).
    pub face: Option<FaceSignal>,
}

/// A photo's combined quality, 0–1, for comparing photos across the
/// library: the technical score (scaled by the face's sharpness when
/// there are faces) averaged with the aesthetic score, halved when eyes
/// are closed. `None` before technical analysis.
pub fn photo_quality(c: &Candidate) -> Option<f32> {
    let scores = c.scores?;
    let mut technical = scores.overall;
    if let Some(face) = c.face {
        technical *= 0.5 + 0.5 * face.sharpness;
    }
    let mut quality = match c.aesthetic {
        Some(aesthetic) => (technical + aesthetic) / 2.0,
        None => technical,
    };
    if c.face.and_then(|f| f.eyes_closed) == Some(true) {
        quality *= CLOSED_EYES_FACTOR;
    }
    Some(quality.clamp(0.0, 1.0))
}

/// A photo's place in its series.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ranked {
    pub id: PhotoId,
    /// 0–1; `None` when not analyzed.
    pub score: Option<f32>,
}

/// Sharpness relative to the series' sharpest photo: 1 for the sharpest,
/// 0.25 less for each halving of detail (Laplacian variance), at least 0.
fn relative_sharpness(variance: f32, sharpest: f32) -> f32 {
    if sharpest <= 0.0 {
        return 1.0;
    }
    if variance <= 0.0 {
        return 0.0;
    }
    (1.0 + (variance / sharpest).log2() / 4.0).clamp(0.0, 1.0)
}

/// Orders `candidates` best first. Ties go to the earlier photo id, so the
/// result never depends on input order.
pub fn rank(candidates: &[Candidate], weights: RankWeights) -> Vec<Ranked> {
    let sharpest = candidates
        .iter()
        .filter_map(|c| c.scores.map(|s| s.laplacian_variance))
        .fold(0.0f32, f32::max);
    let sharpest_face = candidates
        .iter()
        .filter_map(|c| c.face.map(|f| f.sharpness))
        .fold(0.0f32, f32::max);
    let mut ranked: Vec<(&Candidate, Option<f32>)> = candidates
        .iter()
        .map(|c| {
            let score = c.scores.map(|s| {
                let mut sharpness = relative_sharpness(s.laplacian_variance, sharpest);
                if let Some(face) = c.face
                    && sharpest_face > 0.0
                {
                    sharpness = (sharpness + face.sharpness / sharpest_face) / 2.0;
                }
                let mut sum = weights.sharpness * sharpness
                    + weights.exposure * s.exposure
                    + weights.noise * s.noise;
                let mut total = weights.sharpness + weights.exposure + weights.noise;
                if let Some(aesthetic) = c.aesthetic {
                    sum += weights.aesthetic * aesthetic;
                    total += weights.aesthetic;
                }
                let mut score = if total > 0.0 { sum / total } else { 0.0 };
                if c.face.and_then(|f| f.eyes_closed) == Some(true) {
                    score *= CLOSED_EYES_FACTOR;
                }
                score
            });
            (c, score)
        })
        .collect();
    ranked.sort_by(|(a, sa), (b, sb)| {
        a.rejected
            .cmp(&b.rejected)
            .then(sa.is_none().cmp(&sb.is_none()))
            .then(sb.unwrap_or(0.0).total_cmp(&sa.unwrap_or(0.0)))
            .then(a.id.cmp(&b.id))
    });
    ranked
        .into_iter()
        .map(|(c, score)| Ranked { id: c.id, score })
        .collect()
}

/// The best photo: the user's latest choice among the members (`choices`
/// lists chosen photos, oldest choice first), else the top-ranked photo.
/// Returns the photo and whether the user chose it.
pub fn best(ranking: &[Ranked], choices: &[PhotoId]) -> Option<(PhotoId, bool)> {
    choices
        .iter()
        .rev()
        .find(|id| ranking.iter().any(|r| r.id == **id))
        .map(|id| (*id, true))
        .or_else(|| ranking.first().map(|r| (r.id, false)))
}

/// Where a photo stands in its series, for auto-pick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// Not in any series.
    Alone,
    /// The best photo of its series; `chosen` when the user chose it.
    Best { chosen: bool },
    /// Another photo of a series.
    Other,
}

/// Whether the app suggests keeping a photo (*auto-pick*): the best of its
/// series, or a photo in none, with a combined quality (see
/// [`photo_quality`]) of at least `min_quality` and nobody important with
/// closed eyes. A best photo the user chose is always a keeper. Photos not
/// analyzed yet (`quality` is `None`) never are.
pub fn is_keeper(
    quality: Option<f32>,
    eyes_closed: Option<bool>,
    standing: Standing,
    min_quality: f32,
) -> bool {
    let Some(quality) = quality else {
        return false;
    };
    match standing {
        Standing::Other => false,
        Standing::Best { chosen: true } => true,
        Standing::Alone | Standing::Best { chosen: false } => {
            quality >= min_quality && eyes_closed != Some(true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scores(laplacian_variance: f32, exposure: f32, noise: f32) -> Option<TechnicalScores> {
        Some(TechnicalScores {
            sharpness: 1.0,
            exposure,
            noise,
            overall: 0.0,
            laplacian_variance,
            highlights_clipped: 0.0,
            shadows_clipped: 0.0,
            mean_brightness: 0.5,
            noise_sigma: 1.0,
        })
    }

    fn candidate(id: i64, scores: Option<TechnicalScores>) -> Candidate {
        Candidate {
            id: PhotoId(id),
            scores,
            rejected: false,
            aesthetic: None,
            face: None,
        }
    }

    fn order(ranking: &[Ranked]) -> Vec<i64> {
        ranking.iter().map(|r| r.id.0).collect()
    }

    #[test]
    fn the_sharpest_frame_of_a_sharp_burst_wins() {
        // All three are "sharp" in absolute terms; detail still differs.
        let burst = [
            candidate(1, scores(2_000.0, 1.0, 1.0)),
            candidate(2, scores(8_000.0, 1.0, 1.0)),
            candidate(3, scores(4_000.0, 1.0, 1.0)),
        ];
        let ranking = rank(&burst, RankWeights::default());
        assert_eq!(order(&ranking), [2, 3, 1]);
        assert_eq!(ranking[0].score, Some(1.0));
        // Half the detail costs a quarter of the sharpness weight.
        assert!((ranking[1].score.unwrap() - (1.0 - 0.6 * 0.25)).abs() < 1e-5);
    }

    #[test]
    fn weights_change_the_winner() {
        let photos = [
            candidate(1, scores(8_000.0, 0.5, 1.0)), // sharp, badly exposed
            candidate(2, scores(6_000.0, 1.0, 1.0)), // a bit softer, well exposed
        ];
        assert_eq!(order(&rank(&photos, RankWeights::default())), [2, 1]);
        let sharpness_only = RankWeights {
            sharpness: 1.0,
            exposure: 0.0,
            noise: 0.0,
            aesthetic: 0.0,
        };
        assert_eq!(order(&rank(&photos, sharpness_only)), [1, 2]);
    }

    #[test]
    fn rejected_and_unanalyzed_photos_rank_last() {
        let photos = [
            Candidate {
                rejected: true,
                ..candidate(1, scores(9_000.0, 1.0, 1.0))
            },
            candidate(2, None),
            candidate(3, scores(100.0, 0.2, 0.2)),
        ];
        let ranking = rank(&photos, RankWeights::default());
        assert_eq!(order(&ranking), [3, 2, 1]);
        assert_eq!(ranking[1].score, None);
    }

    #[test]
    fn ranking_is_deterministic() {
        let a = [
            candidate(5, scores(1_000.0, 1.0, 1.0)),
            candidate(2, scores(1_000.0, 1.0, 1.0)),
            candidate(9, None),
            candidate(7, None),
        ];
        let mut b = a;
        b.reverse();
        assert_eq!(order(&rank(&a, RankWeights::default())), [2, 5, 7, 9]);
        assert_eq!(
            rank(&a, RankWeights::default()),
            rank(&b, RankWeights::default())
        );
    }

    #[test]
    fn the_users_latest_choice_in_the_series_wins() {
        let ranking = rank(
            &[
                candidate(1, scores(9_000.0, 1.0, 1.0)),
                candidate(2, scores(1_000.0, 1.0, 1.0)),
                candidate(3, None),
            ],
            RankWeights::default(),
        );
        assert_eq!(best(&ranking, &[]), Some((PhotoId(1), false)));
        assert_eq!(
            best(&ranking, &[PhotoId(3), PhotoId(42), PhotoId(2)]),
            Some((PhotoId(2), true))
        );
        // Choices outside the series are ignored.
        assert_eq!(best(&ranking, &[PhotoId(42)]), Some((PhotoId(1), false)));
        assert_eq!(best(&[], &[PhotoId(1)]), None);
    }

    #[test]
    fn weights_are_validated() {
        assert!(RankWeights::default().is_valid());
        for bad in [
            RankWeights {
                sharpness: -1.0,
                ..Default::default()
            },
            RankWeights {
                sharpness: 0.0,
                exposure: 0.0,
                noise: 0.0,
                aesthetic: 1.0,
            },
            RankWeights {
                aesthetic: -1.0,
                ..Default::default()
            },
            RankWeights {
                noise: f32::NAN,
                ..Default::default()
            },
        ] {
            assert!(!bad.is_valid(), "{bad:?}");
        }
    }

    #[test]
    fn closed_eyes_and_soft_faces_lose_to_open_sharp_faces() {
        let face = |eyes_closed, sharpness| {
            Some(FaceSignal {
                eyes_closed: Some(eyes_closed),
                sharpness,
            })
        };
        let photos = [
            // The crispest frame, but a blink.
            Candidate {
                face: face(true, 0.9),
                ..candidate(1, scores(8_000.0, 1.0, 1.0))
            },
            // A bit softer overall, eyes open, face sharp.
            Candidate {
                face: face(false, 0.9),
                ..candidate(2, scores(6_000.0, 1.0, 1.0))
            },
            // Sharp background, soft face.
            Candidate {
                face: face(false, 0.3),
                ..candidate(3, scores(8_000.0, 1.0, 1.0))
            },
        ];
        assert_eq!(order(&rank(&photos, RankWeights::default())), [2, 3, 1]);
    }

    #[test]
    fn the_aesthetic_score_breaks_close_calls() {
        let photos = [
            Candidate {
                aesthetic: Some(0.3),
                ..candidate(1, scores(8_000.0, 1.0, 1.0))
            },
            Candidate {
                aesthetic: Some(0.8),
                ..candidate(2, scores(7_000.0, 1.0, 1.0))
            },
        ];
        assert_eq!(order(&rank(&photos, RankWeights::default())), [2, 1]);
        let technical_only = RankWeights {
            aesthetic: 0.0,
            ..Default::default()
        };
        assert_eq!(order(&rank(&photos, technical_only)), [1, 2]);
    }

    #[test]
    fn photo_quality_combines_every_signal() {
        let base = candidate(1, scores(1_000.0, 1.0, 1.0));
        let overall = |c: Candidate| photo_quality(&c).unwrap();
        let mut s = scores(1_000.0, 1.0, 1.0).unwrap();
        s.overall = 0.8;
        let c = Candidate {
            scores: Some(s),
            ..base
        };
        assert!((overall(c) - 0.8).abs() < 1e-6);
        let pretty = Candidate {
            aesthetic: Some(0.4),
            ..c
        };
        assert!((overall(pretty) - 0.6).abs() < 1e-6);
        let soft_face = Candidate {
            face: Some(FaceSignal {
                eyes_closed: Some(false),
                sharpness: 0.0,
            }),
            ..c
        };
        assert!((overall(soft_face) - 0.4).abs() < 1e-6);
        let blink = Candidate {
            face: Some(FaceSignal {
                eyes_closed: Some(true),
                sharpness: 1.0,
            }),
            ..c
        };
        assert!((overall(blink) - 0.4).abs() < 1e-6);
        assert_eq!(photo_quality(&candidate(2, None)), None);
    }

    #[test]
    fn relative_sharpness_handles_edges() {
        assert_eq!(relative_sharpness(0.0, 0.0), 1.0);
        assert_eq!(relative_sharpness(0.0, 100.0), 0.0);
        assert_eq!(relative_sharpness(1.0, 1_000_000.0), 0.0);
        assert_eq!(relative_sharpness(50.0, 100.0), 0.75);
    }

    #[test]
    fn keepers_are_good_best_or_single_photos() {
        let best = Standing::Best { chosen: false };
        assert!(is_keeper(Some(0.6), None, Standing::Alone, 0.5));
        assert!(is_keeper(Some(0.5), Some(false), best, 0.5));
        // Not good enough, closed eyes, or not the best of its series.
        assert!(!is_keeper(Some(0.49), None, Standing::Alone, 0.5));
        assert!(!is_keeper(Some(0.9), Some(true), best, 0.5));
        assert!(!is_keeper(Some(0.9), None, Standing::Other, 0.5));
        // Not analyzed yet.
        assert!(!is_keeper(None, None, Standing::Alone, 0.0));
        // The user's choice of best wins over scores.
        assert!(is_keeper(
            Some(0.1),
            Some(true),
            Standing::Best { chosen: true },
            0.5
        ));
    }
}
