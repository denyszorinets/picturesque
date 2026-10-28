//! Why a photo scored as it did, in words (`docs/features/ai.rst`).
//!
//! Turns a photo's analysis into a short list of reasons, good and bad,
//! for the loupe: only notable values are mentioned, so an unremarkable
//! photo gets few reasons.

use serde::Serialize;
use ts_rs::TS;

use crate::analysis::clip::AestheticView;
use crate::analysis::faces::{FaceView, signal};
use crate::analysis::technical::QualityView;

/// Whether a reason raises or lowers the score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Tone {
    Good,
    Bad,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ReasonView {
    pub tone: Tone,
    pub text: String,
}

/// What is known about one photo.
#[derive(Debug, Clone, Copy, Default)]
pub struct Signals<'a> {
    pub quality: Option<&'a QualityView>,
    pub aesthetic: Option<&'a AestheticView>,
    /// The personal taste score, 0–1.
    pub taste: Option<f32>,
    /// The critique's percentile among the library's critiques, 0–1.
    pub critique: Option<f32>,
    /// A RAW photo's critique rating and the potential rating it was raised
    /// toward, when the potential raised it.
    pub raw_potential: Option<(f32, f32)>,
    pub faces: Option<&'a [FaceView]>,
}

/// The photo's place in its series.
#[derive(Debug, Clone, Copy)]
pub struct SeriesPlace<'a> {
    /// 1 for the best-ranked photo.
    pub rank: usize,
    pub size: usize,
    pub is_best: bool,
    pub best_chosen: bool,
    /// The best photo's signals (when this photo is not the best).
    pub best: Signals<'a>,
}

/// A rating without a trailing `.0`.
fn rating(v: f32) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{:.0}", v.round())
    } else {
        format!("{v:.1}")
    }
}

fn percent(v: f32) -> String {
    format!("{}%", (v * 100.0).round())
}

fn reason(tone: Tone, text: impl Into<String>) -> ReasonView {
    ReasonView {
        tone,
        text: text.into(),
    }
}

fn eyes_closed(s: &Signals<'_>) -> Option<bool> {
    s.faces.and_then(signal).and_then(|f| f.eyes_closed)
}

/// The reasons for a photo's score, most important first.
pub fn reasons(photo: Signals<'_>, series: Option<SeriesPlace<'_>>) -> Vec<ReasonView> {
    let mut out = Vec::new();
    if let Some(place) = series {
        if place.is_best {
            out.push(reason(
                Tone::Good,
                if place.best_chosen {
                    format!("You chose it as the best of its series of {}", place.size)
                } else {
                    format!("Best of its series of {}", place.size)
                },
            ));
        } else {
            out.push(reason(
                Tone::Info,
                format!("Ranked {} of {} in its series", place.rank, place.size),
            ));
            let best = place.best;
            if eyes_closed(&photo) == Some(true) && eyes_closed(&best) == Some(false) {
                out.push(reason(
                    Tone::Bad,
                    "The best photo of the series has open eyes",
                ));
            }
            if let (Some(q), Some(b)) = (photo.quality, best.quality)
                && b.sharpness > q.sharpness + 0.1
            {
                out.push(reason(Tone::Bad, "The best photo of the series is sharper"));
            }
            if let (Some(a), Some(b)) = (photo.aesthetic, best.aesthetic)
                && b.score > a.score + 0.1
            {
                out.push(reason(
                    Tone::Bad,
                    "The best photo of the series looks better",
                ));
            }
        }
    }
    if let Some(face) = photo.faces.and_then(signal) {
        match face.eyes_closed {
            Some(true) => out.push(reason(Tone::Bad, "Eyes closed: the score is halved")),
            Some(false) => out.push(reason(Tone::Good, "Eyes open")),
            None => {}
        }
        if face.sharpness >= 0.7 {
            out.push(reason(Tone::Good, "Face in focus"));
        } else if face.sharpness <= 0.35 {
            out.push(reason(
                Tone::Bad,
                format!("Face is soft ({})", percent(face.sharpness)),
            ));
        }
    }
    if let Some(q) = photo.quality {
        if q.sharpness >= 0.7 {
            out.push(reason(
                Tone::Good,
                format!("Sharp ({})", percent(q.sharpness)),
            ));
        } else if q.sharpness <= 0.35 {
            out.push(reason(
                Tone::Bad,
                format!(
                    "Blurry or out of focus (sharpness {})",
                    percent(q.sharpness)
                ),
            ));
        }
        if q.exposure >= 0.85 {
            out.push(reason(Tone::Good, "Well exposed"));
        } else if q.exposure < 0.6 {
            out.push(reason(
                Tone::Bad,
                if q.highlights_clipped > q.shadows_clipped {
                    "Overexposed: bright areas are blown out"
                } else {
                    "Underexposed: dark areas lose detail"
                },
            ));
        }
        if q.noise < 0.4 {
            out.push(reason(
                Tone::Bad,
                format!("Noisy (low noise {})", percent(q.noise)),
            ));
        }
    }
    if let Some(a) = photo.aesthetic {
        if a.score >= 0.6 {
            out.push(reason(
                Tone::Good,
                format!("Looks appealing (aesthetic {})", percent(a.score)),
            ));
        } else if a.score <= 0.25 {
            out.push(reason(
                Tone::Bad,
                format!("Looks plain (aesthetic {})", percent(a.score)),
            ));
        }
    }
    if let Some(t) = photo.taste {
        if t >= 0.7 {
            out.push(reason(
                Tone::Good,
                format!("Like the photos you pick (your taste {})", percent(t)),
            ));
        } else if t <= 0.3 {
            out.push(reason(
                Tone::Bad,
                format!("Like the photos you reject (your taste {})", percent(t)),
            ));
        }
    }
    if let Some(c) = photo.critique {
        if c >= 0.8 {
            out.push(reason(
                Tone::Good,
                format!(
                    "The critic ranks it among your best (top {}%)",
                    (((1.0 - c) * 100.0).round() as i32).max(1)
                ),
            ));
        } else if c <= 0.2 {
            out.push(reason(
                Tone::Bad,
                format!(
                    "The critic ranks it below most of your photos (bottom {}%)",
                    ((c * 100.0).round() as i32).max(1)
                ),
            ));
        }
    }
    if let Some((rated, potential)) = photo.raw_potential {
        out.push(reason(
            Tone::Info,
            format!(
                "Rated as a RAW file: the critic's {} is raised toward the {} it could reach after editing",
                rating(rated),
                rating(potential)
            ),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quality(sharpness: f32, exposure: f32, noise: f32) -> QualityView {
        QualityView {
            sharpness,
            exposure,
            noise,
            overall: 0.5,
            highlights_clipped: 0.2,
            shadows_clipped: 0.0,
        }
    }

    fn face(eyes_closed: Option<bool>, sharpness: f32) -> FaceView {
        FaceView {
            x: 0.3,
            y: 0.2,
            width: 0.3,
            height: 0.4,
            score: 0.9,
            eyes_closed,
            eye_openness: None,
            sharpness,
        }
    }

    fn texts(r: &[ReasonView]) -> Vec<(Tone, &str)> {
        r.iter().map(|r| (r.tone, r.text.as_str())).collect()
    }

    #[test]
    fn a_raw_photo_lifted_by_its_potential_says_so() {
        // (lift, expected reason)
        let cases = [
            (None, None),
            (
                Some((6.0, 8.0)),
                Some(
                    "Rated as a RAW file: the critic's 6 is raised toward the 8 it could reach after editing",
                ),
            ),
            (
                Some((6.5, 8.0)),
                Some(
                    "Rated as a RAW file: the critic's 6.5 is raised toward the 8 it could reach after editing",
                ),
            ),
        ];
        for (lift, expected) in cases {
            let r = reasons(
                Signals {
                    raw_potential: lift,
                    ..Default::default()
                },
                None,
            );
            let got: Vec<_> = texts(&r);
            match expected {
                Some(text) => assert_eq!(got, [(Tone::Info, text)]),
                None => assert!(got.is_empty()),
            }
        }
    }

    #[test]
    fn a_good_photo_lists_its_strengths() {
        let q = quality(0.9, 0.95, 0.9);
        let a = AestheticView {
            score: 0.72,
            rating: 5.9,
        };
        let faces = [face(Some(false), 0.8)];
        let r = reasons(
            Signals {
                quality: Some(&q),
                aesthetic: Some(&a),
                taste: Some(0.81),
                critique: Some(0.93),
                faces: Some(&faces),
                ..Default::default()
            },
            Some(SeriesPlace {
                rank: 1,
                size: 4,
                is_best: true,
                best_chosen: false,
                best: Signals::default(),
            }),
        );
        assert_eq!(
            texts(&r),
            [
                (Tone::Good, "Best of its series of 4"),
                (Tone::Good, "Eyes open"),
                (Tone::Good, "Face in focus"),
                (Tone::Good, "Sharp (90%)"),
                (Tone::Good, "Well exposed"),
                (Tone::Good, "Looks appealing (aesthetic 72%)"),
                (Tone::Good, "Like the photos you pick (your taste 81%)"),
                (Tone::Good, "The critic ranks it among your best (top 7%)"),
            ]
        );
    }

    #[test]
    fn a_weak_photo_says_what_is_wrong_and_why_another_won() {
        let q = quality(0.2, 0.4, 0.3);
        let a = AestheticView {
            score: 0.2,
            rating: 4.9,
        };
        let faces = [face(Some(true), 0.3)];
        let best_q = quality(0.8, 0.9, 0.9);
        let best_a = AestheticView {
            score: 0.6,
            rating: 5.7,
        };
        let best_faces = [face(Some(false), 0.8)];
        let r = reasons(
            Signals {
                quality: Some(&q),
                aesthetic: Some(&a),
                taste: Some(0.12),
                critique: Some(0.04),
                faces: Some(&faces),
                ..Default::default()
            },
            Some(SeriesPlace {
                rank: 3,
                size: 5,
                is_best: false,
                best_chosen: false,
                best: Signals {
                    quality: Some(&best_q),
                    aesthetic: Some(&best_a),
                    taste: None,
                    critique: None,
                    faces: Some(&best_faces),
                    ..Default::default()
                },
            }),
        );
        assert_eq!(
            texts(&r),
            [
                (Tone::Info, "Ranked 3 of 5 in its series"),
                (Tone::Bad, "The best photo of the series has open eyes"),
                (Tone::Bad, "The best photo of the series is sharper"),
                (Tone::Bad, "The best photo of the series looks better"),
                (Tone::Bad, "Eyes closed: the score is halved"),
                (Tone::Bad, "Face is soft (30%)"),
                (Tone::Bad, "Blurry or out of focus (sharpness 20%)"),
                (Tone::Bad, "Overexposed: bright areas are blown out"),
                (Tone::Bad, "Noisy (low noise 30%)"),
                (Tone::Bad, "Looks plain (aesthetic 20%)"),
                (Tone::Bad, "Like the photos you reject (your taste 12%)"),
                (
                    Tone::Bad,
                    "The critic ranks it below most of your photos (bottom 4%)",
                ),
            ]
        );
    }

    #[test]
    fn unremarkable_or_unanalyzed_photos_get_few_reasons() {
        let q = quality(0.5, 0.7, 0.6);
        assert!(
            reasons(
                Signals {
                    quality: Some(&q),
                    ..Default::default()
                },
                None
            )
            .is_empty()
        );
        assert!(reasons(Signals::default(), None).is_empty());
        let chosen = reasons(
            Signals::default(),
            Some(SeriesPlace {
                rank: 2,
                size: 3,
                is_best: true,
                best_chosen: true,
                best: Signals::default(),
            }),
        );
        assert_eq!(
            texts(&chosen),
            [(Tone::Good, "You chose it as the best of its series of 3")]
        );
    }
}
