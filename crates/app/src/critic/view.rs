//! A critique as the API returns it.

use serde::Serialize;
use ts_rs::TS;

use super::prompt::{Critique, Fixable, Issue};

/// A critique, as `get_critique` and `critique_photo` return it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct CritiqueView {
    /// The model that wrote it.
    pub model: String,
    /// 1–10.
    pub rating: f32,
    /// The rating as 0–1.
    pub score: f32,
    pub summary: String,
    pub strengths: Vec<String>,
    pub weaknesses: Vec<String>,
    /// The kinds of the weaknesses; empty in critiques before prompt
    /// version 4.
    pub issues: Vec<Issue>,
    /// The rating after the best editing of a RAW file, 1–10; `None` in
    /// older critiques.
    pub potential: Option<f32>,
    /// Whether editing would fix the flaws; `None` in older critiques.
    /// Advisory only.
    pub fixable: Option<Fixable>,
    /// What to change when editing the photo.
    pub fix_steps: Vec<String>,
    /// How to make the photo itself better.
    pub improvements: Vec<String>,
    /// Camera settings and technique for the next shot.
    pub camera_tips: Vec<String>,
    /// Written by another model or rubric than the current settings; shown,
    /// but `critique_photo` makes a new one.
    pub outdated: bool,
}

impl CritiqueView {
    pub(super) fn new(model: &str, c: Critique) -> Self {
        Self {
            outdated: false,
            model: model.to_owned(),
            score: (c.rating - 1.0) / 9.0,
            rating: c.rating,
            summary: c.summary,
            strengths: c.strengths,
            weaknesses: c.weaknesses,
            issues: c.issues,
            potential: c.potential.map(|p| p.max(c.rating)),
            fixable: c.fixable,
            fix_steps: c.fix_steps,
            improvements: c.improvements,
            camera_tips: c.camera_tips,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn potential_is_never_below_the_rating() {
        let cases = [
            (Some(4.0), 6.0, Some(6.0)),
            (Some(8.0), 6.0, Some(8.0)),
            (None, 6.0, None),
        ];
        for (potential, rating, want) in cases {
            let c: Critique = serde_json::from_value(serde_json::json!({
                "summary": "s", "rating": rating, "potential": potential }))
            .unwrap();
            assert_eq!(CritiqueView::new("m", c).potential, want);
        }
    }
}
