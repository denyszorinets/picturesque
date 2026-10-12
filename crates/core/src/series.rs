//! Series: photos taken close together that show the same scene.
//!
//! Photos are walked in capture order. A photo joins the current series
//! when it was taken soon after the previous photo and looks like one of
//! the series' last few photos; otherwise it starts a new one. The user can
//! force a split or a join before any photo; those edits are applied on
//! every run, so they survive re-analysis.

use std::collections::HashMap;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::photo::PhotoId;
use crate::similarity::Signature;

/// How close photos must be to form a series.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SeriesThresholds {
    /// Longest pause between consecutive shots of a series.
    pub max_gap_seconds: u32,
    /// Most perceptual-hash bits a photo may differ from the series' recent
    /// photos (see `similarity`; unrelated photos differ by 18 or more).
    pub max_distance: u32,
}

impl Default for SeriesThresholds {
    fn default() -> Self {
        Self {
            max_gap_seconds: 30,
            max_distance: 16,
        }
    }
}

/// A user's correction of the boundary before a photo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeriesEdit {
    /// The photo starts a new series.
    Split,
    /// The photo belongs to the same series as the photo before it.
    Join,
}

impl SeriesEdit {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Split => "split",
            Self::Join => "join",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "split" => Some(Self::Split),
            "join" => Some(Self::Join),
            _ => None,
        }
    }
}

/// What series detection knows about a photo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeriesInput {
    pub id: PhotoId,
    pub captured: Option<NaiveDateTime>,
    /// `None` until analyzed (or if analysis failed); such photos are
    /// grouped by time alone.
    pub signature: Option<Signature>,
}

/// How many of a series' latest photos a new photo is compared with, so
/// one odd frame (someone walking past) does not break the series.
const LOOKBACK: usize = 3;

/// Groups photos into series of at least two, each in capture order, the
/// series ordered by their first photo. Undated photos are never in a
/// series. The result does not depend on the order of `photos`.
pub fn detect_series(
    photos: &[SeriesInput],
    edits: &HashMap<PhotoId, SeriesEdit>,
    thresholds: SeriesThresholds,
) -> Vec<Vec<PhotoId>> {
    let mut dated: Vec<(NaiveDateTime, &SeriesInput)> = photos
        .iter()
        .filter_map(|p| p.captured.map(|t| (t, p)))
        .collect();
    dated.sort_by_key(|(t, p)| (*t, p.id));

    let mut series: Vec<Vec<PhotoId>> = Vec::new();
    let mut current: Vec<&SeriesInput> = Vec::new();
    for (i, &(time, photo)) in dated.iter().enumerate() {
        let joins = match (i.checked_sub(1).map(|j| dated[j].0), edits.get(&photo.id)) {
            (None, _) => false,
            (Some(_), Some(SeriesEdit::Split)) => false,
            (Some(_), Some(SeriesEdit::Join)) => true,
            (Some(prev), None) => {
                let close_in_time = |a: NaiveDateTime, b: NaiveDateTime| {
                    (b - a).num_seconds() <= i64::from(thresholds.max_gap_seconds)
                };
                close_in_time(prev, time)
                    && (looks_alike(&current, photo, thresholds.max_distance)
                        // One odd frame (someone walking past) stays when
                        // the next shot looks like the series again.
                        || dated.get(i + 1).is_some_and(|&(next_time, next)| {
                            close_in_time(time, next_time)
                                && !edits.contains_key(&next.id)
                                && next.signature.is_some()
                                && looks_alike(&current, next, thresholds.max_distance)
                        }))
            }
        };
        if !joins {
            close(&mut series, std::mem::take(&mut current));
        }
        current.push(photo);
    }
    close(&mut series, current);
    series
}

/// Whether `photo` resembles one of the last few photos of `series`. Without
/// signatures to compare, time decides alone.
fn looks_alike(series: &[&SeriesInput], photo: &SeriesInput, max_distance: u32) -> bool {
    let Some(signature) = photo.signature else {
        return true;
    };
    let mut recent = series
        .iter()
        .rev()
        .take(LOOKBACK)
        .filter_map(|p| p.signature)
        .peekable();
    if recent.peek().is_none() {
        return true;
    }
    recent.any(|s| s.distance(&signature) <= max_distance)
}

fn close(series: &mut Vec<Vec<PhotoId>>, members: Vec<&SeriesInput>) {
    if members.len() >= 2 {
        series.push(members.iter().map(|p| p.id).collect());
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;
    use crate::similarity::Hash64;

    fn at(seconds: i64) -> Option<NaiveDateTime> {
        let start = NaiveDate::from_ymd_opt(2026, 10, 12)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();
        Some(start + chrono::Duration::seconds(seconds))
    }

    /// A signature `bits` bits away from scene `scene`'s base hash.
    fn look(scene: u64, bits: u32) -> Option<Signature> {
        let base = scene.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        let flip = if bits == 0 {
            0
        } else {
            u64::MAX >> (64 - bits)
        };
        Some(Signature {
            phash: Hash64(base ^ flip),
        })
    }

    fn photo(id: i64, seconds: i64, signature: Option<Signature>) -> SeriesInput {
        SeriesInput {
            id: PhotoId(id),
            captured: at(seconds),
            signature,
        }
    }

    fn ids(series: &[Vec<PhotoId>]) -> Vec<Vec<i64>> {
        series
            .iter()
            .map(|s| s.iter().map(|id| id.0).collect())
            .collect()
    }

    fn detect(photos: &[SeriesInput], edits: &[(i64, SeriesEdit)]) -> Vec<Vec<i64>> {
        let edits = edits.iter().map(|(id, e)| (PhotoId(*id), *e)).collect();
        ids(&detect_series(photos, &edits, SeriesThresholds::default()))
    }

    #[test]
    fn groups_similar_photos_taken_close_together() {
        let photos = [
            photo(1, 0, look(1, 0)),
            photo(2, 2, look(1, 4)),
            photo(3, 5, look(1, 8)),
            // A minute later: same scene, but a new series.
            photo(4, 65, look(1, 2)),
            photo(5, 70, look(1, 3)),
            // Alone.
            photo(6, 500, look(1, 0)),
        ];
        assert_eq!(detect(&photos, &[]), [vec![1, 2, 3], vec![4, 5]]);
    }

    #[test]
    fn a_scene_change_starts_a_new_series_even_within_the_gap() {
        let photos = [
            photo(1, 0, look(1, 0)),
            photo(2, 3, look(1, 2)),
            photo(3, 6, look(2, 0)),
            photo(4, 9, look(2, 1)),
        ];
        assert_eq!(detect(&photos, &[]), [vec![1, 2], vec![3, 4]]);
    }

    #[test]
    fn one_odd_frame_does_not_break_a_series() {
        let photos = [
            photo(1, 0, look(1, 0)),
            photo(2, 1, look(1, 1)),
            photo(3, 2, look(9, 0)), // someone walked through the frame
            photo(4, 3, look(1, 2)),
            photo(5, 4, look(1, 1)),
        ];
        assert_eq!(detect(&photos, &[]), [vec![1, 2, 3, 4, 5]]);

        // Two different frames in a row are a new scene.
        let photos = [
            photo(1, 0, look(1, 0)),
            photo(2, 1, look(1, 1)),
            photo(3, 2, look(9, 0)),
            photo(4, 3, look(9, 1)),
        ];
        assert_eq!(detect(&photos, &[]), [vec![1, 2], vec![3, 4]]);
    }

    #[test]
    fn photos_without_signatures_are_grouped_by_time() {
        let photos = [
            photo(1, 0, None),
            photo(2, 10, look(1, 0)),
            photo(3, 20, None),
            photo(4, 100, None),
        ];
        assert_eq!(detect(&photos, &[]), [vec![1, 2, 3]]);
    }

    #[test]
    fn undated_photos_are_never_in_a_series() {
        let undated = SeriesInput {
            id: PhotoId(9),
            captured: None,
            signature: look(1, 0),
        };
        let photos = [photo(1, 0, look(1, 0)), undated, photo(2, 1, look(1, 0))];
        assert_eq!(
            detect(&photos, &[(9, SeriesEdit::Join), (2, SeriesEdit::Join)]),
            [vec![1, 2]]
        );
    }

    #[test]
    fn user_edits_split_and_join_and_survive_reanalysis() {
        let mut photos = vec![
            photo(1, 0, look(1, 0)),
            photo(2, 2, look(1, 1)),
            photo(3, 4, look(1, 2)),
            photo(4, 300, look(5, 0)),
            photo(5, 302, look(5, 1)),
        ];
        let edits = [(3, SeriesEdit::Split), (4, SeriesEdit::Join)];
        assert_eq!(detect(&photos, &edits), [vec![1, 2], vec![3, 4, 5]]);

        // New signatures (a new analyzer version) and a newly imported
        // photo: the edits still apply.
        for p in &mut photos {
            p.signature = look(7, 0);
        }
        photos.push(photo(6, 3, look(7, 0)));
        assert_eq!(detect(&photos, &edits), [vec![1, 2, 6], vec![3, 4, 5]]);
    }

    #[test]
    fn result_is_deterministic_regardless_of_input_order() {
        let photos = vec![
            photo(3, 0, look(1, 0)),
            photo(1, 0, look(1, 0)),
            photo(2, 1, look(1, 1)),
        ];
        let mut reversed = photos.clone();
        reversed.reverse();
        assert_eq!(detect(&photos, &[]), [vec![1, 3, 2]]);
        assert_eq!(detect(&reversed, &[]), detect(&photos, &[]));
    }

    #[test]
    fn thresholds_are_adjustable() {
        let photos = [photo(1, 0, look(1, 0)), photo(2, 45, look(1, 14))];
        let edits = HashMap::new();
        let strict = SeriesThresholds::default();
        let loose = SeriesThresholds {
            max_gap_seconds: 60,
            max_distance: 16,
        };
        let tight_look = SeriesThresholds {
            max_gap_seconds: 60,
            max_distance: 10,
        };
        assert!(detect_series(&photos, &edits, strict).is_empty());
        assert_eq!(ids(&detect_series(&photos, &edits, loose)), [vec![1, 2]]);
        assert!(detect_series(&photos, &edits, tight_look).is_empty());
    }

    #[test]
    fn edit_names_round_trip() {
        for e in [SeriesEdit::Split, SeriesEdit::Join] {
            assert_eq!(SeriesEdit::from_name(e.as_str()), Some(e));
        }
        assert_eq!(SeriesEdit::from_name("other"), None);
    }
}
