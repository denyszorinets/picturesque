//! What to do about the recurring problems (`docs/features/insights.rst`).
//!
//! Plain rules over the numbers in `LibraryInsights`: a problem that comes
//! up often gets its remedy, and when it shows up more at one camera
//! setting the remedy says what to change there. Nothing is generated, so
//! the same library always reads the same.

use serde::Serialize;
use ts_rs::TS;

use super::{InsightSetting, IssueInsight, SettingHabit};
use crate::critic::prompt::Issue;

/// Fewest critiques that must name a problem before it is advised on.
const MIN_COUNT: u32 = 3;
/// Smallest recency-weighted share worth advice.
const MIN_SHARE: f32 = 0.10;
/// Most pieces of advice, so the page stays a short list of what matters.
const MAX_ADVICE: usize = 5;

/// One thing to work on.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct Advice {
    pub issue: Issue,
    /// For example "You tend to overexpose".
    pub title: String,
    /// What to do, in a few sentences.
    pub text: String,
    /// The numbers behind it.
    pub evidence: String,
    /// Photo ids showing the problem, the clearest first.
    #[ts(type = "number[]")]
    pub examples: Vec<i64>,
}

/// The title and the remedy for a problem; `None` for "other".
fn remedy(issue: Issue) -> Option<(&'static str, &'static str)> {
    Some(match issue {
        Issue::Overexposed => (
            "You tend to overexpose",
            "Dial in about -2/3 EV of exposure compensation, switch on the highlight warning or \
             the histogram, and expose for the brightest part that matters. Shoot RAW: it \
             recovers far more from a slightly dark frame than from blown highlights.",
        ),
        Issue::Underexposed => (
            "You tend to underexpose",
            "Try +2/3 EV, and check that the histogram does not pile up on the left. Raise the \
             ISO rather than accept a dark frame: lifting shadows afterwards adds more noise \
             than a higher ISO would.",
        ),
        Issue::MissedFocus => (
            "Focus often lands in the wrong place",
            "Use a single focus point, or eye and subject detection, on the subject itself. \
             Focus before you recompose, stop down a little for margin, and zoom in to check \
             sharpness on the shots that matter.",
        ),
        Issue::MotionBlur => (
            "Motion blur is costing you sharp shots",
            "Keep the shutter time at 1/focal length or faster (twice that on a high-resolution \
             body), 1/500 s or faster for moving subjects, and switch on stabilisation. Raise \
             the ISO to afford it.",
        ),
        Issue::SubjectPlacement => (
            "Your subject is often awkwardly placed",
            "Decide where the subject sits before you shoot: centre it when the scene is simple \
             or symmetrical, otherwise put it on a third line with room in the direction it \
             faces or moves. Step closer or change your position instead of fixing it with a \
             crop afterwards.",
        ),
        Issue::SubjectCutOff => (
            "The frame cuts through your subject",
            "Check the edges before you press the shutter. Leave room around heads, hands and \
             feet, or crop at a clear break, not at a joint. Zoom out or step back and crop \
             later if you cannot get it right in the camera.",
        ),
        Issue::BusyBackground => (
            "Backgrounds compete with your subject",
            "Take a step to the side to change what is behind the subject, get closer, or open \
             the aperture to blur the background. Look at the edges of the frame for bright \
             spots and lines before you shoot.",
        ),
        Issue::Noise => (
            "Noise shows in your photos",
            "Look for more light, open the aperture, or slow the shutter with support before \
             raising the ISO. Expose so the histogram reaches the right without clipping, so \
             you need not brighten later, and use noise reduction in the RAW editor.",
        ),
        Issue::Colour => (
            "Colour and white balance often miss",
            "Shoot RAW so white balance can be set afterwards, set it by hand in mixed light, \
             and use a grey card or a Kelvin preset when colour matters.",
        ),
        Issue::Horizon => (
            "Horizons are tilting",
            "Switch on the camera's grid or electronic level and line it up with a clear \
             horizontal. Straighten in the editor when the scene has no true horizontal.",
        ),
        Issue::Light => (
            "The light is often flat or harsh",
            "Shoot in the first and last hour of daylight. At midday move the subject into open \
             shade, and use side light to give it shape.",
        ),
        Issue::Timing => (
            "The moment is often missed",
            "Choose a spot and wait for the gesture instead of chasing it. Use burst mode and \
             shoot a few frames before and after the peak.",
        ),
        // Critiques from before prompt version 5 only say the coarse kind.
        Issue::Exposure => (
            "Exposure is a recurring problem",
            "Check the histogram and the highlight warning on important shots. Redo the \
             critiques to learn whether you tend to over- or underexpose.",
        ),
        Issue::Focus => (
            "Focus is a recurring problem",
            "Use a single focus point or subject detection, keep the shutter fast enough for the \
             focal length, and check sharpness before you move on. Redo the critiques to learn \
             whether the misses are focus or movement.",
        ),
        Issue::Composition => (
            "Composition is a recurring problem",
            "Decide what the picture is about before you shoot, place the subject on purpose, \
             and check the edges of the frame. Redo the critiques to learn what goes wrong most.",
        ),
        Issue::Other => return None,
    })
}

/// What to change at the setting where the problem is most common; `None`
/// when there is nothing specific to say. `range` is the setting's bucket
/// from `core::insights`, for example `ISO above 3200`; a rule applies only
/// to the buckets its advice is true for.
fn at_setting(issue: Issue, setting: InsightSetting, range: &str) -> Option<String> {
    use InsightSetting::{Aperture, FocalLength, Iso, Shutter};
    let slow = ["1/30 s or longer", "between 1/250 and 1/30 s"];
    let advice = match (issue, setting) {
        (Issue::Noise, Iso) if ["ISO 801–3200", "ISO above 3200"].contains(&range) => {
            "Your noisy photos cluster at high ISO: before going there, try a wider aperture, a \
             slower shutter with support, or more light."
        }
        (Issue::MotionBlur, Shutter) if slow.contains(&range) => {
            "Most of them were shot with a slow shutter: use a faster one, or stabilisation and \
             a support."
        }
        (Issue::MissedFocus, Aperture) if range == "wider than f/2.8" => {
            "At wide apertures the zone of sharpness is thin: stop down towards f/4 to f/5.6 \
             when focus has to be exact."
        }
        (Issue::MissedFocus | Issue::MotionBlur, FocalLength) if range == "100 mm or longer" => {
            "With long lenses small shakes show: shoot faster than 1/focal length, or use a \
             support."
        }
        (Issue::Overexposed, Aperture) if range == "wider than f/2.8" => {
            "You overexpose most with wide apertures: in bright light use a faster shutter or a \
             lower ISO, or a neutral-density filter."
        }
        (Issue::Overexposed, Shutter) if range == "1/30 s or longer" => {
            "You overexpose most with slow shutter times: a faster shutter or a lower ISO keeps \
             the highlights."
        }
        (Issue::Underexposed, Iso) if range == "ISO 200 or lower" => {
            "You underexpose most at low ISO: allow a higher ISO or a longer shutter instead."
        }
        (Issue::Underexposed, Shutter) if range == "1/250 s or faster" => {
            "You underexpose most with fast shutter times, which leave too little light: raise \
             the ISO or open the aperture."
        }
        (Issue::SubjectCutOff, FocalLength) if range == "100 mm or longer" => {
            "With long lenses the frame is tight: step back, or zoom out and crop later."
        }
        (Issue::SubjectPlacement, FocalLength) if range == "under 24 mm" => {
            "Wide lenses push the subject towards the edges: step closer and place it on \
             purpose, centred or on a third."
        }
        (Issue::BusyBackground, Aperture)
            if ["f/5.6 to f/11", "f/11 or narrower"].contains(&range) =>
        {
            "With narrow apertures the background stays sharp: open up, or move the subject away \
             from what is behind it."
        }
        _ => return None,
    };
    Some(advice.to_owned())
}

fn percent(v: f32) -> u32 {
    (v * 100.0).round() as u32
}

/// The advice for the recurring problems, the most frequent first. `issues`
/// are ordered by frequency, as `LibraryInsights` has them.
pub(super) fn advise(issues: &[IssueInsight], habits: &[SettingHabit]) -> Vec<Advice> {
    issues
        .iter()
        .filter(|i| i.count >= MIN_COUNT && i.weighted_share >= MIN_SHARE)
        .filter_map(|i| {
            let (title, text) = remedy(i.issue)?;
            let habit = habits
                .iter()
                .filter(|h| h.issue == i.issue)
                .max_by(|a, b| a.lift.total_cmp(&b.lift));
            let mut evidence = format!(
                "Named in {}% of your recent critiques ({} photos)",
                percent(i.weighted_share),
                i.count
            );
            let mut text = text.to_owned();
            if let Some(h) = habit {
                evidence.push_str(&format!("; {:.1}× as often ({})", h.lift, h.label));
                if let Some(extra) = at_setting(i.issue, h.setting, &h.label) {
                    text.push(' ');
                    text.push_str(&extra);
                }
            }
            Some(Advice {
                issue: i.issue,
                title: title.to_owned(),
                text,
                evidence,
                examples: i.examples.clone(),
            })
        })
        .take(MAX_ADVICE)
        .collect()
}

#[cfg(test)]
mod tests;
