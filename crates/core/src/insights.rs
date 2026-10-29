//! What a library's critiques say as a whole (`docs/features/insights.rst`).
//!
//! Pure aggregation of stored critiques, no model call. Recent work counts
//! more: a critique's weight halves every [`HALF_LIFE_DAYS`] days of age,
//! measured from the newest photo, so the result does not depend on the
//! clock.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{Datelike, NaiveDateTime};

use crate::metadata::Exposure;
use crate::photo::PhotoId;

/// A critique's weight halves every this many days of age.
pub const HALF_LIFE_DAYS: f64 = 365.0;
/// Fewest critiques for steady insights; the view says so below.
pub const MIN_CRITIQUES: usize = 10;
/// Fewest critiques in a settings bucket for a habit.
pub const MIN_BUCKET: usize = 8;
/// Fewest flagged critiques in a settings bucket for a habit.
pub const MIN_FLAGGED: usize = 3;
/// How many times more often than usual a problem must happen in a bucket.
pub const MIN_LIFT: f64 = 1.5;
/// Examples kept per issue.
pub const EXAMPLES: usize = 6;
/// Habits kept.
pub const MAX_HABITS: usize = 10;
/// Weight of a photo without a capture time.
pub const UNDATED_WEIGHT: f64 = 0.5;

/// One stored critique with what is known about its photo.
#[derive(Debug, Clone)]
pub struct Sample<K> {
    pub id: PhotoId,
    /// 1–10.
    pub rating: f32,
    /// The kinds of its weaknesses; `None` for critiques from before issues
    /// were asked for, which stay out of the issue statistics.
    pub issues: Option<Vec<K>>,
    pub captured: Option<NaiveDateTime>,
    pub exposure: Exposure,
}

/// A camera setting habits are looked for in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Setting {
    Iso,
    Shutter,
    Aperture,
    FocalLength,
}

/// How often one issue is flagged.
#[derive(Debug, Clone, PartialEq)]
pub struct IssueStat<K> {
    pub issue: K,
    pub count: usize,
    /// Of the critiques that have issues, 0–1.
    pub share: f32,
    /// Like `share`, with recent critiques counting more.
    pub weighted_share: f32,
    /// The photos that show it most, worst first.
    pub examples: Vec<PhotoId>,
}

/// The critiques of one calendar quarter.
#[derive(Debug, Clone, PartialEq)]
pub struct Quarter {
    pub year: i32,
    /// 1–4.
    pub quarter: u32,
    pub critiques: usize,
    pub mean_rating: f32,
}

/// An issue that is flagged unusually often at one camera setting.
#[derive(Debug, Clone, PartialEq)]
pub struct Habit<K> {
    pub issue: K,
    pub setting: Setting,
    pub label: &'static str,
    pub critiques: usize,
    /// Weighted share flagged within the bucket.
    pub share: f32,
    /// Weighted share flagged among all photos with this setting.
    pub baseline: f32,
    /// `share / baseline`.
    pub lift: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Insights<K> {
    pub critiques: usize,
    pub with_issues: usize,
    pub mean_rating: Option<f32>,
    pub weighted_rating: Option<f32>,
    /// Most often flagged first (recent critiques count more).
    pub issues: Vec<IssueStat<K>>,
    /// Oldest first.
    pub trend: Vec<Quarter>,
    /// Rating points per year, from three quarters on.
    pub trend_slope: Option<f32>,
    pub habits: Vec<Habit<K>>,
}

/// A critique's weight, 1 now and 0.5 after one half-life.
pub fn weight(age_days: f64, half_life_days: f64) -> f64 {
    0.5f64.powf(age_days.max(0.0) / half_life_days)
}

/// The insights of `samples`, with ages measured to `now`.
pub fn summarize<K: Copy + Eq + Ord>(
    samples: &[Sample<K>],
    now: NaiveDateTime,
    half_life_days: f64,
) -> Insights<K> {
    let weighted: Vec<(&Sample<K>, f64)> = samples
        .iter()
        .map(|s| {
            let w = s.captured.map_or(UNDATED_WEIGHT, |t| {
                weight((now - t).num_seconds() as f64 / 86_400.0, half_life_days)
            });
            (s, w)
        })
        .collect();
    let total: f64 = weighted.iter().map(|(_, w)| w).sum();
    let with_issues = weighted.iter().filter(|(s, _)| s.issues.is_some()).count();
    let (trend, trend_slope) = trend(samples);
    Insights {
        critiques: samples.len(),
        with_issues,
        mean_rating: (!samples.is_empty())
            .then(|| samples.iter().map(|s| s.rating).sum::<f32>() / samples.len() as f32),
        weighted_rating: (total > 0.0).then(|| {
            (weighted
                .iter()
                .map(|(s, w)| f64::from(s.rating) * w)
                .sum::<f64>()
                / total) as f32
        }),
        issues: issue_stats(&weighted),
        trend,
        trend_slope,
        habits: habits(&weighted),
    }
}

fn flagged<K: PartialEq>(s: &Sample<K>, issue: &K) -> bool {
    s.issues.as_ref().is_some_and(|i| i.contains(issue))
}

fn all_issues<K: Copy + Ord>(weighted: &[(&Sample<K>, f64)]) -> BTreeSet<K> {
    weighted
        .iter()
        .filter_map(|(s, _)| s.issues.as_ref())
        .flatten()
        .copied()
        .collect()
}

fn issue_stats<K: Copy + Eq + Ord>(weighted: &[(&Sample<K>, f64)]) -> Vec<IssueStat<K>> {
    let known: Vec<_> = weighted
        .iter()
        .filter(|(s, _)| s.issues.is_some())
        .collect();
    let n = known.len() as f64;
    let total: f64 = known.iter().map(|(_, w)| w).sum();
    let mut stats: Vec<IssueStat<K>> = all_issues(weighted)
        .into_iter()
        .map(|issue| {
            let mut hits: Vec<_> = known.iter().filter(|(s, _)| flagged(s, &issue)).collect();
            let count = hits.len();
            let weight_hit: f64 = hits.iter().map(|(_, w)| w).sum();
            // The worst, most recent photos make the best examples.
            let severity = |(s, w): &&(&Sample<K>, f64)| w * f64::from(10.0 - s.rating);
            hits.sort_by(|a, b| {
                severity(b)
                    .total_cmp(&severity(a))
                    .then(a.0.id.cmp(&b.0.id))
            });
            IssueStat {
                issue,
                count,
                share: (count as f64 / n) as f32,
                weighted_share: (weight_hit / total) as f32,
                examples: hits.iter().take(EXAMPLES).map(|(s, _)| s.id).collect(),
            }
        })
        .collect();
    stats.sort_by(|a, b| {
        b.weighted_share
            .total_cmp(&a.weighted_share)
            .then(a.issue.cmp(&b.issue))
    });
    stats
}

/// Mean rating per calendar quarter, and its slope in points per year.
fn trend<K>(samples: &[Sample<K>]) -> (Vec<Quarter>, Option<f32>) {
    let mut buckets: BTreeMap<(i32, u32), (usize, f32)> = BTreeMap::new();
    for s in samples {
        if let Some(t) = s.captured {
            let b = buckets.entry((t.year(), t.month0() / 3 + 1)).or_default();
            b.0 += 1;
            b.1 += s.rating;
        }
    }
    let trend: Vec<Quarter> = buckets
        .into_iter()
        .map(|((year, quarter), (n, sum))| Quarter {
            year,
            quarter,
            critiques: n,
            mean_rating: sum / n as f32,
        })
        .collect();
    (trend.clone(), slope(&trend))
}

/// Least squares of the quarters' means over the quarter index (gaps
/// count), each quarter weighing as many as it has critiques.
fn slope(trend: &[Quarter]) -> Option<f32> {
    if trend.len() < 3 {
        return None;
    }
    let point = |q: &Quarter| (f64::from(q.year * 4 + q.quarter as i32), q.critiques as f64);
    let total: f64 = trend.iter().map(|q| point(q).1).sum();
    let mean_x = trend.iter().map(|q| point(q).0 * point(q).1).sum::<f64>() / total;
    let mean_y = trend
        .iter()
        .map(|q| f64::from(q.mean_rating) * q.critiques as f64)
        .sum::<f64>()
        / total;
    let (mut covariance, mut variance) = (0.0, 0.0);
    for q in trend {
        let (x, n) = point(q);
        covariance += n * (x - mean_x) * (f64::from(q.mean_rating) - mean_y);
        variance += n * (x - mean_x).powi(2);
    }
    (variance > 0.0).then(|| (covariance / variance * 4.0) as f32)
}

/// Which bucket of `setting` an exposure falls in, with its label.
fn bucket(setting: Setting, e: &Exposure) -> Option<(usize, &'static str)> {
    match setting {
        Setting::Iso => {
            let iso = e.iso?;
            Some(match iso {
                0..=200 => (0, "ISO 200 or lower"),
                201..=800 => (1, "ISO 201–800"),
                801..=3200 => (2, "ISO 801–3200"),
                _ => (3, "ISO above 3200"),
            })
        }
        Setting::Shutter => {
            let s = e.exposure_time?.seconds();
            (s > 0.0).then_some(())?;
            Some(if s >= 1.0 / 30.0 {
                (0, "1/30 s or longer")
            } else if s > 1.0 / 250.0 {
                (1, "between 1/250 and 1/30 s")
            } else {
                (2, "1/250 s or faster")
            })
        }
        Setting::Aperture => {
            let f = e.f_number?;
            (f > 0.0).then_some(())?;
            Some(if f < 2.8 {
                (0, "wider than f/2.8")
            } else if f < 5.6 {
                (1, "f/2.8 to f/5.6")
            } else if f < 11.0 {
                (2, "f/5.6 to f/11")
            } else {
                (3, "f/11 or narrower")
            })
        }
        Setting::FocalLength => {
            let mm = e.focal_length_mm?;
            (mm > 0.0).then_some(())?;
            Some(if mm < 24.0 {
                (0, "under 24 mm")
            } else if mm < 50.0 {
                (1, "24 to 50 mm")
            } else if mm < 100.0 {
                (2, "50 to 100 mm")
            } else {
                (3, "100 mm or longer")
            })
        }
    }
}

const SETTINGS: [Setting; 4] = [
    Setting::Iso,
    Setting::Shutter,
    Setting::Aperture,
    Setting::FocalLength,
];

/// Issues flagged much more often than usual at one setting.
fn habits<K: Copy + Eq + Ord>(weighted: &[(&Sample<K>, f64)]) -> Vec<Habit<K>> {
    let mut out = Vec::new();
    for issue in all_issues(weighted) {
        for setting in SETTINGS {
            // (bucket) -> (label, critiques, flagged, weight, flagged weight)
            let mut buckets: BTreeMap<usize, (&'static str, usize, usize, f64, f64)> =
                BTreeMap::new();
            let (mut total, mut hit) = (0.0, 0.0);
            for (s, w) in weighted.iter().filter(|(s, _)| s.issues.is_some()) {
                let Some((i, label)) = bucket(setting, &s.exposure) else {
                    continue;
                };
                let is_flagged = flagged(s, &issue);
                let b = buckets.entry(i).or_insert((label, 0, 0, 0.0, 0.0));
                b.1 += 1;
                b.3 += w;
                total += w;
                if is_flagged {
                    b.2 += 1;
                    b.4 += w;
                    hit += w;
                }
            }
            let baseline = if total > 0.0 { hit / total } else { 0.0 };
            if baseline <= 0.0 {
                continue;
            }
            for (label, n, flagged, w, w_hit) in buckets.into_values() {
                let share = w_hit / w;
                let lift = share / baseline;
                if n >= MIN_BUCKET && flagged >= MIN_FLAGGED && lift >= MIN_LIFT {
                    out.push(Habit {
                        issue,
                        setting,
                        label,
                        critiques: n,
                        share: share as f32,
                        baseline: baseline as f32,
                        lift: lift as f32,
                    });
                }
            }
        }
    }
    out.sort_by(|a, b| {
        b.lift
            .total_cmp(&a.lift)
            .then(a.issue.cmp(&b.issue))
            .then(a.setting.cmp(&b.setting))
    });
    out.truncate(MAX_HABITS);
    out
}

#[cfg(test)]
mod tests;
