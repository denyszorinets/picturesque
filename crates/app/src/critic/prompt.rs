//! What the critic is asked and how its reply is read: the prompt, the
//! reply's JSON schema and its parser.

use picturesque_core::media::MediaFormat;
use picturesque_core::metadata::CaptureMetadata;
use picturesque_core::photo::Photo;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value, json};
use ts_rs::TS;

use crate::settings::CriticPreset;

mod presets;
mod tips;
pub(super) use presets::system_prompt;

/// What the model judges, unless the user wrote their own.
pub const DEFAULT_RUBRIC: &str = "\
- Composition: framing, balance, leading lines, distracting elements at the edges.
- Light: direction, quality and contrast of the light; exposure.
- Subject and moment: is the subject clear, is the moment or expression right?
- Colour and tone: harmony, white balance, saturation.
- Technique: focus on the subject, motion blur, noise.";

/// Bumped when the prompt changes enough to make earlier critiques
/// incomparable; it is part of the stored rubric key.
pub(crate) const PROMPT_VERSION: u32 = 6;

/// The key stored with a critique to recognize the prompt, preset and
/// rubric it was written with. The Professional preset hashes what it did
/// before presets existed, so earlier critiques stay current.
pub(crate) fn rubric_key(preset: CriticPreset, rubric: &str) -> String {
    match preset {
        CriticPreset::Professional => fingerprint(&format!("v{PROMPT_VERSION}\n{rubric}")),
        other => fingerprint(&format!(
            "v{PROMPT_VERSION}\npreset:{}\n{rubric}",
            other.as_str()
        )),
    }
}

/// The kind of flaw a weakness is. Version 5 of the prompt asks for the
/// specific kinds (`ISSUES`); `Exposure`, `Focus` and `Composition` are what
/// earlier critiques said and stay readable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Issue {
    Exposure,
    Focus,
    Composition,
    Noise,
    Colour,
    Horizon,
    Light,
    Timing,
    Other,
    Overexposed,
    Underexposed,
    MissedFocus,
    MotionBlur,
    SubjectPlacement,
    SubjectCutOff,
    BusyBackground,
}

/// The kinds the model may name. The older, coarser ones are only read.
const ISSUES: [&str; 13] = [
    "overexposed",
    "underexposed",
    "missed_focus",
    "motion_blur",
    "subject_placement",
    "subject_cut_off",
    "busy_background",
    "noise",
    "colour",
    "horizon",
    "light",
    "timing",
    "other",
];

/// What kind of photo it is. The model names it before it gives advice, so
/// the advice can fit the genre: a blurred background suits a portrait, not
/// a landscape. Only the app reads it (it is stored, not shown).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scene {
    Portrait,
    Wildlife,
    Macro,
    Product,
    Landscape,
    Architecture,
    Street,
    Event,
    Other,
}

const SCENES: [&str; 9] = [
    "portrait",
    "wildlife",
    "macro",
    "product",
    "landscape",
    "architecture",
    "street",
    "event",
    "other",
];

impl Scene {
    /// Scenes where the surroundings are part of the picture, so a blurred
    /// background is rarely the advice.
    pub(super) fn wants_context(self) -> bool {
        matches!(self, Self::Landscape | Self::Architecture | Self::Street)
    }
}

/// Whether editing the original in a RAW editor would fix its flaws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Fixable {
    NotNeeded,
    Easy,
    NeedsSkill,
    NotFixable,
}

const FIXABLE: [&str; 4] = ["not_needed", "easy", "needs_skill", "not_fixable"];

/// Unknown categories are dropped, not a failed reply.
fn lenient_issues<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Issue>, D::Error> {
    let values = Vec::<Value>::deserialize(d).unwrap_or_default();
    let mut issues: Vec<Issue> = Vec::new();
    for issue in values
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
    {
        if !issues.contains(&issue) {
            issues.push(issue);
        }
    }
    Ok(issues)
}

fn lenient_fixable<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Fixable>, D::Error> {
    Ok(Value::deserialize(d)
        .ok()
        .and_then(|v| serde_json::from_value(v).ok()))
}

fn lenient_scene<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Scene>, D::Error> {
    Ok(Value::deserialize(d)
        .ok()
        .and_then(|v| serde_json::from_value(v).ok()))
}

fn lenient_number<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f32>, D::Error> {
    Ok(Value::deserialize(d)
        .ok()
        .and_then(|v| v.as_f64())
        .map(|n| n as f32))
}

/// The JSON the model is asked for. The fields after `rating` came with
/// prompt version 4; critiques stored before lack them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Critique {
    #[serde(default)]
    pub weaknesses: Vec<String>,
    #[serde(default)]
    pub strengths: Vec<String>,
    pub summary: String,
    /// 1–10.
    pub rating: f32,
    /// The kinds of the weaknesses.
    #[serde(default, deserialize_with = "lenient_issues")]
    pub issues: Vec<Issue>,
    /// The rating after the best editing of a RAW file, 1–10, at least
    /// `rating`.
    #[serde(default, deserialize_with = "lenient_number")]
    pub potential: Option<f32>,
    #[serde(default, deserialize_with = "lenient_fixable")]
    pub fixable: Option<Fixable>,
    /// The kind of photo; only the advice guard reads it.
    #[serde(
        default,
        deserialize_with = "lenient_scene",
        skip_serializing_if = "Option::is_none"
    )]
    pub scene: Option<Scene>,
    /// What to change when editing the photo.
    #[serde(default)]
    pub fix_steps: Vec<String>,
    /// How to make the photo itself better: framing, light, moment.
    #[serde(default)]
    pub improvements: Vec<String>,
    /// Camera settings and technique for the next shot.
    #[serde(default)]
    pub camera_tips: Vec<String>,
}

/// Most strengths or weaknesses kept.
const MAX_POINTS: usize = 5;
/// Most issues, fix steps, improvements or tips kept.
const MAX_ADVICE: usize = 4;

impl Critique {
    /// The first JSON object in a model's reply, tidied; `None` when there
    /// is none or it lacks a rating or summary.
    pub fn parse(reply: &str) -> Option<Self> {
        // Reasoning models think aloud first.
        let reply = reply.rsplit("</think>").next().unwrap_or(reply);
        let start = reply.find('{')?;
        let end = reply.rfind('}')?;
        let mut c: Self = serde_json::from_str(reply.get(start..=end)?).ok()?;
        if !c.rating.is_finite() {
            return None;
        }
        c.rating = c.rating.clamp(1.0, 10.0);
        c.summary = c.summary.trim().to_owned();
        let tidy = |points: Vec<String>, max: usize| -> Vec<String> {
            points
                .into_iter()
                .map(|p| p.trim().to_owned())
                .filter(|p| !p.is_empty())
                .take(max)
                .collect()
        };
        c.strengths = tidy(c.strengths, MAX_POINTS);
        c.weaknesses = tidy(c.weaknesses, MAX_POINTS);
        c.fix_steps = tidy(c.fix_steps, MAX_ADVICE);
        c.improvements = tidy(c.improvements, MAX_ADVICE);
        c.camera_tips = tidy(c.camera_tips, MAX_ADVICE);
        c.issues.truncate(MAX_ADVICE);
        c.potential = c
            .potential
            .filter(|p| p.is_finite())
            .map(|p| p.clamp(1.0, 10.0).max(c.rating));
        (!c.summary.is_empty()).then_some(c)
    }

    /// The schema lists the keys in the order the model writes them
    /// (llama.cpp's grammar follows it; `serde_json` keeps the order): the
    /// flaws first, the rating after them so it follows from what was
    /// said, the advice last.
    pub(super) fn schema() -> Value {
        let points =
            |max: u32| json!({ "type": "array", "items": { "type": "string" }, "maxItems": max });
        json!({
            "type": "object",
            "properties": {
                "weaknesses": points(3),
                "issues": {
                    "type": "array",
                    "items": { "type": "string", "enum": ISSUES },
                    "maxItems": 4,
                },
                "strengths": points(3),
                // A minimum length: without it the model sometimes answers with an
                // empty object.
                "summary": { "type": "string", "minLength": 20 },
                "rating": { "type": "number", "minimum": 1, "maximum": 10 },
                "potential": { "type": "number", "minimum": 1, "maximum": 10 },
                "fixable": { "type": "string", "enum": FIXABLE },
                "scene": { "type": "string", "enum": SCENES },
                "fix_steps": points(3),
                "improvements": points(3),
                "camera_tips": points(3),
            },
            "required": ["weaknesses", "issues", "strengths", "summary", "rating",
                "potential", "fixable", "scene", "fix_steps", "improvements", "camera_tips"],
        })
    }
}

/// What the model is told about this photo: the camera and lens, how it
/// was shot and whether the original is a RAW file. Empty parts are left
/// out.
pub(super) fn user_prompt(photo: &Photo) -> String {
    let mut prompt = String::from("Critique this photo.");
    if let Some(gear) = gear(&photo.metadata) {
        prompt.push_str(&format!(" Gear: {gear}."));
    }
    if let Some(shot) = shot_data(&photo.metadata) {
        prompt.push_str(&format!(" Shot data: {shot}."));
    }
    if photo.assets.raw().is_some() {
        prompt.push_str(" The original is a RAW file.");
    } else if photo
        .assets
        .image()
        .is_some_and(|a| a.format == MediaFormat::Jpeg)
    {
        prompt.push_str(" The original is a JPEG; it has little room for recovery.");
    }
    prompt
}

/// Longest camera or lens name sent to the model.
const MAX_GEAR_CHARS: usize = 64;

/// EXIF text made safe to put in the prompt: control characters become
/// spaces, whitespace collapses and the length is capped.
fn tidy_gear(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let words: Vec<&str> = cleaned.split_whitespace().collect();
    words
        .join(" ")
        .chars()
        .take(MAX_GEAR_CHARS)
        .collect::<String>()
        .trim_end()
        .to_owned()
}

/// "Sony ILCE-7M4 with FE 35mm F1.8", with what is known.
fn gear(m: &CaptureMetadata) -> Option<String> {
    let camera = m.camera.as_ref().map(|c| {
        let model = &tidy_gear(&c.model);
        let brand = tidy_gear(&c.brand());
        // Many models repeat the make ("Canon EOS R5").
        if model.to_lowercase().starts_with(&brand.to_lowercase()) {
            model.to_owned()
        } else {
            format!("{brand} {model}")
        }
    });
    let lens = m.lens.as_deref().map(tidy_gear).filter(|l| !l.is_empty());
    match (camera.filter(|c| !c.trim().is_empty()), lens) {
        (Some(c), Some(l)) => Some(format!("{c} with {l}")),
        (Some(c), None) => Some(c),
        (None, Some(l)) => Some(format!("lens {l}")),
        (None, None) => None,
    }
}

/// "ISO 800, 1/60 s, f/2.8, 35 mm", with what is known.
fn shot_data(m: &CaptureMetadata) -> Option<String> {
    let e = &m.exposure;
    let mut parts = Vec::new();
    if let Some(iso) = e.iso {
        parts.push(format!("ISO {iso}"));
    }
    if let Some(t) = e.exposure_time {
        let seconds = t.seconds();
        if seconds > 0.0 && seconds < 0.5 {
            parts.push(format!("1/{} s", (1.0 / seconds).round()));
        } else if seconds > 0.0 {
            parts.push(format!("{seconds} s"));
        }
    }
    if let Some(f) = e.f_number {
        parts.push(format!("f/{f}"));
    }
    if let Some(mm) = e.focal_length_mm {
        parts.push(format!("{mm} mm"));
    }
    (!parts.is_empty()).then(|| parts.join(", "))
}

/// 64-bit FNV-1a, stable across builds, to recognize the rubric a critique
/// was written with.
pub(crate) fn fingerprint(text: &str) -> String {
    let hash = text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    });
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests;
