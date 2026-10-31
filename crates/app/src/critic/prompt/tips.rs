//! Checks the advice against the shot data. A small model sometimes
//! suggests the setting the photo was already taken with ("use f/4 at
//! 12 mm" for an f/4, 12 mm shot) or states the effect of the aperture the
//! wrong way round ("f/5.6 for a shallower depth of field"). Such a tip is
//! dropped rather than shown; precision matters more than keeping every
//! tip.

use picturesque_core::metadata::Exposure;

use super::{Critique, Scene};

/// Words that make a mention of a value a suggestion to use it.
const CHANGE_WORDS: [&str; 9] = [
    "use",
    "try",
    "set",
    "switch",
    "consider",
    "shoot",
    "choose",
    "open",
    "stop down",
];

/// Phrases that promise a shallower depth of field or more blur.
const SHALLOWER: [&str; 4] = [
    "shallow",
    "blurrier background",
    "more background blur",
    "separate the subject",
];

/// Phrases that promise a deeper depth of field.
const DEEPER: [&str; 5] = [
    "more depth of field",
    "deeper depth of field",
    "greater depth of field",
    "more of the scene in focus",
    "sharper throughout",
];

impl Critique {
    /// Removes the camera tips and improvements that contradict the shot or
    /// its kind of photo.
    pub(in crate::critic) fn check_against(&mut self, shot: &Exposure) {
        let context = self.scene.is_some_and(Scene::wants_context);
        let keep = |tip: &String| tip_fits(tip, shot) && !(context && blurs_background(tip, shot));
        self.camera_tips.retain(keep);
        self.improvements.retain(keep);
    }
}

fn tip_fits(tip: &str, shot: &Exposure) -> bool {
    let text = tip.to_lowercase();
    let suggests = CHANGE_WORDS.iter().any(|w| text.contains(w));
    let apertures = numbers_after(&text, "f/");
    let lengths = numbers_before(&text, "mm");
    let isos = numbers_after(&text, "iso ");

    if suggests
        && (all_equal(&apertures, shot.f_number)
            || all_equal(&lengths, shot.focal_length_mm)
            || all_equal(&isos, shot.iso.map(|i| i as f32)))
    {
        return false;
    }
    // The effect of the aperture, against the one in use.
    if let (Some(shot_f), [new_f]) = (shot.f_number, apertures.as_slice()) {
        let wants_shallower = SHALLOWER.iter().any(|p| text.contains(p));
        let wants_deeper = DEEPER.iter().any(|p| text.contains(p));
        if (wants_shallower && *new_f > shot_f + 0.05) || (wants_deeper && *new_f < shot_f - 0.05) {
            return false;
        }
    }
    // Without a number: "wider aperture" with "more depth of field".
    let wider = ["wider aperture", "open the aperture", "open up"];
    let narrower = ["narrower aperture", "smaller aperture", "stop down"];
    let says = |words: &[&str]| words.iter().any(|w| text.contains(w));
    !((says(&wider) && says(&DEEPER)) || (says(&narrower) && says(&SHALLOWER)))
}

/// Whether a tip asks for a blurred background or a wider aperture, which a
/// landscape, a building or a street scene rarely wants.
fn blurs_background(tip: &str, shot: &Exposure) -> bool {
    let text = tip.to_lowercase();
    let words = [
        "shallow",
        "background blur",
        "blur the background",
        "blurred background",
        "bokeh",
        "isolate",
        "wider aperture",
        "open up",
    ];
    if words.iter().any(|w| text.contains(w)) {
        return true;
    }
    // "f/2.8" when the shot was at f/5.6.
    match (shot.f_number, numbers_after(&text, "f/").as_slice()) {
        (Some(shot_f), [new_f]) => *new_f < shot_f - 0.05,
        _ => false,
    }
}

/// Whether the text names values and all of them are the shot's.
fn all_equal(found: &[f32], shot: Option<f32>) -> bool {
    shot.is_some_and(|s| !found.is_empty() && found.iter().all(|f| (f - s).abs() < 0.05))
}

/// Numbers that follow `marker`: "f/4.0" gives 4.0.
fn numbers_after(text: &str, marker: &str) -> Vec<f32> {
    text.match_indices(marker)
        .filter_map(|(at, m)| number(&text[at + m.len()..], false))
        .collect()
}

/// Numbers that end right before `unit`: "12mm" and "12 mm" give 12.
fn numbers_before(text: &str, unit: &str) -> Vec<f32> {
    text.match_indices(unit)
        .filter_map(|(at, _)| {
            let head = text[..at].trim_end();
            let start = head
                .rfind(|c: char| !(c.is_ascii_digit() || c == '.'))
                .map_or(0, |i| i + 1);
            number(&head[start..], true)
        })
        .collect()
}

fn number(text: &str, whole: bool) -> Option<f32> {
    let end = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(text.len());
    let digits = text[..end].trim_end_matches('.');
    if whole && end != text.len() {
        return None;
    }
    digits.parse().ok()
}

#[cfg(test)]
mod tests;
