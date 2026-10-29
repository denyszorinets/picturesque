//! Library insights: what all stored critiques say together, recent work
//! counting more (`docs/features/insights.rst`). The arithmetic is
//! `core::insights`; this maps stored critiques in and the result out.

use picturesque_core::insights::{self, HALF_LIFE_DAYS, MIN_CRITIQUES, Sample, summarize};
use serde::Serialize;
use ts_rs::TS;

use crate::critic::prompt::Issue;
use crate::ports::catalog::{CatalogError, PhotoCatalog};

/// What the critiques of the library say (`library_insights`).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct LibraryInsights {
    /// Stored critiques, whether current or outdated, rejects included.
    pub critiques: u32,
    /// Of those, the ones that name problem categories (made by prompt
    /// version 4 or later).
    pub with_issues: u32,
    /// Fewest critiques for steady insights.
    pub min_critiques: u32,
    /// A critique's weight halves every this many days of age.
    pub half_life_days: f32,
    /// The newest capture date among critiqued photos (`YYYY-MM-DD`); ages
    /// are measured to it.
    pub as_of: Option<String>,
    pub mean_rating: Option<f32>,
    /// The mean rating with recent critiques counting more.
    pub weighted_rating: Option<f32>,
    /// Most often flagged first.
    pub issues: Vec<IssueInsight>,
    /// Oldest quarter first.
    pub trend: Vec<TrendBucket>,
    /// Rating points per year, once there are three quarters.
    pub trend_slope: Option<f32>,
    pub habits: Vec<SettingHabit>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct IssueInsight {
    pub issue: Issue,
    pub count: u32,
    /// Of the critiques with problem categories, 0–1.
    pub share: f32,
    /// Like `share`, recent critiques counting more.
    pub weighted_share: f32,
    /// Photo ids, the clearest examples first.
    #[ts(type = "number[]")]
    pub examples: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct TrendBucket {
    /// For example `2026-Q3`.
    pub quarter: String,
    pub critiques: u32,
    pub mean_rating: f32,
}

/// A camera setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum InsightSetting {
    Iso,
    Shutter,
    Aperture,
    FocalLength,
}

/// A problem that happens unusually often at one camera setting.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct SettingHabit {
    pub issue: Issue,
    pub setting: InsightSetting,
    /// The range, for example `ISO above 3200`.
    pub label: String,
    pub critiques: u32,
    /// Share flagged within the range, 0–1.
    pub share: f32,
    /// Share flagged among all photos with this setting, 0–1.
    pub baseline: f32,
    /// `share / baseline`.
    pub lift: f32,
}

impl From<insights::Setting> for InsightSetting {
    fn from(s: insights::Setting) -> Self {
        match s {
            insights::Setting::Iso => Self::Iso,
            insights::Setting::Shutter => Self::Shutter,
            insights::Setting::Aperture => Self::Aperture,
            insights::Setting::FocalLength => Self::FocalLength,
        }
    }
}

/// The stored critique of a photo as a sample; `None` without a rating.
fn sample(
    id: picturesque_core::photo::PhotoId,
    result: &serde_json::Value,
    photo: Option<&picturesque_core::photo::Photo>,
) -> Option<Sample<Issue>> {
    let critique = &result["critique"];
    let rating = critique["rating"].as_f64()? as f32;
    // Critiques from before prompt version 4 have no `issues` key.
    let issues = critique["issues"].as_array().map(|values| {
        let mut issues: Vec<Issue> = Vec::new();
        for issue in values
            .iter()
            .filter_map(|v| serde_json::from_value(v.clone()).ok())
        {
            if !issues.contains(&issue) {
                issues.push(issue);
            }
        }
        issues
    });
    Some(Sample {
        id,
        rating,
        issues,
        captured: photo.and_then(|p| p.metadata.captured_at.map(|t| t.local)),
        exposure: photo.map(|p| p.metadata.exposure).unwrap_or_default(),
    })
}

/// The insights of every stored critique. It works whether or not
/// critiques are switched on: the stored ones stay readable.
pub fn library_insights(catalog: &dyn PhotoCatalog) -> Result<LibraryInsights, CatalogError> {
    let photos: std::collections::HashMap<_, _> = catalog
        .all_photos()?
        .into_iter()
        .map(|p| (p.id, p))
        .collect();
    let samples: Vec<Sample<Issue>> = catalog
        .analysis_results(crate::critic::ID)?
        .iter()
        .filter_map(|(id, result)| sample(*id, result, photos.get(id)))
        .collect();
    // Ages are measured to the newest capture; without any, no photo has an
    // age, so the date used does not matter.
    let now = samples
        .iter()
        .filter_map(|s| s.captured)
        .max()
        .unwrap_or_default();
    let result = summarize(&samples, now, HALF_LIFE_DAYS);
    Ok(LibraryInsights {
        critiques: result.critiques as u32,
        with_issues: result.with_issues as u32,
        min_critiques: MIN_CRITIQUES as u32,
        half_life_days: HALF_LIFE_DAYS as f32,
        as_of: samples
            .iter()
            .any(|s| s.captured.is_some())
            .then(|| now.format("%Y-%m-%d").to_string()),
        mean_rating: result.mean_rating,
        weighted_rating: result.weighted_rating,
        issues: result
            .issues
            .into_iter()
            .map(|s| IssueInsight {
                issue: s.issue,
                count: s.count as u32,
                share: s.share,
                weighted_share: s.weighted_share,
                examples: s.examples.into_iter().map(|id| id.0).collect(),
            })
            .collect(),
        trend: result
            .trend
            .into_iter()
            .map(|q| TrendBucket {
                quarter: format!("{}-Q{}", q.year, q.quarter),
                critiques: q.critiques as u32,
                mean_rating: q.mean_rating,
            })
            .collect(),
        trend_slope: result.trend_slope,
        habits: result
            .habits
            .into_iter()
            .map(|h| SettingHabit {
                issue: h.issue,
                setting: h.setting.into(),
                label: h.label.to_string(),
                critiques: h.critiques as u32,
                share: h.share,
                baseline: h.baseline,
                lift: h.lift,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests;
