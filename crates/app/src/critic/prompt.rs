//! What the critic is asked and how its reply is read: the prompt, the
//! reply's JSON schema and its parser.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// What the model judges, unless the user wrote their own.
pub const DEFAULT_RUBRIC: &str = "\
- Composition: framing, balance, leading lines, distracting elements at the edges.
- Light: direction, quality and contrast of the light; exposure.
- Subject and moment: is the subject clear, is the moment or expression right?
- Colour and tone: harmony, white balance, saturation.
- Technique: focus on the subject, motion blur, noise.";

/// Bumped when the prompt changes enough to make earlier critiques
/// incomparable; it is part of the stored rubric key.
pub(crate) const PROMPT_VERSION: u32 = 3;

/// The key stored with a critique to recognize the prompt and rubric it
/// was written with.
pub(crate) fn rubric_key(rubric: &str) -> String {
    fingerprint(&format!("v{PROMPT_VERSION}\n{rubric}"))
}

/// The JSON the model is asked for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Critique {
    #[serde(default)]
    pub weaknesses: Vec<String>,
    #[serde(default)]
    pub strengths: Vec<String>,
    pub summary: String,
    /// 1–10.
    pub rating: f32,
}

/// Most strengths or weaknesses kept.
const MAX_POINTS: usize = 5;

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
        let tidy = |points: Vec<String>| -> Vec<String> {
            points
                .into_iter()
                .map(|p| p.trim().to_owned())
                .filter(|p| !p.is_empty())
                .take(MAX_POINTS)
                .collect()
        };
        c.strengths = tidy(c.strengths);
        c.weaknesses = tidy(c.weaknesses);
        (!c.summary.is_empty()).then_some(c)
    }

    /// The schema lists the keys in the order the model writes them
    /// (llama.cpp's grammar follows it; `serde_json` keeps the order): the
    /// flaws first, the rating last, so it follows from what was said.
    pub(super) fn schema() -> Value {
        let points = json!({ "type": "array", "items": { "type": "string" }, "maxItems": 3 });
        json!({
            "type": "object",
            "properties": {
                "weaknesses": points,
                "strengths": points,
                "summary": { "type": "string" },
                "rating": { "type": "number", "minimum": 1, "maximum": 10 },
            },
            "required": ["weaknesses", "strengths", "summary", "rating"],
        })
    }
}

/// 64-bit FNV-1a, stable across builds, to recognize the rubric a critique
/// was written with.
pub(crate) fn fingerprint(text: &str) -> String {
    let hash = text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    });
    format!("{hash:016x}")
}

/// The instructions for the model.
///
/// Evaluated on 25 Wikimedia Commons featured pictures against 25 random
/// camera photos (Qwen3-VL 4B): the flaws-first order and the grade anchors
/// gave a ranking AUC of 0.90 and a median rating of 4 for ordinary photos.
pub(super) fn system_prompt(rubric: &str) -> String {
    format!(
        "You are a professor of photography and a strict juror of a selective competition. \
Your verdict helps a photographer decide which photos to keep, so telling strong photos from \
ordinary ones matters more than encouragement.

Assess the photograph against this rubric:
{rubric}

Most photographs are ordinary snapshots. Look hard for flaws first: a missed focus, a tilted \
horizon, blown highlights, flat or harsh light, clutter or distracting elements at the edges, \
a subject that is unclear or unremarkable, a dull composition. List them in \"weaknesses\" \
(up to 3, concrete, naming the area of the frame), then the real strengths in \"strengths\" \
(up to 3, may be empty), then one sentence \"summary\", and only then the \"rating\".

Rating, 1 to 10, as an academic grade:
1-2 failed; 3 snapshot with clear flaws; 4 snapshot without visual intent; 5 competent but \
ordinary, the typical photograph; 6 above average, one clear strength; 7 strong, portfolio \
quality; 8 excellent, publishable; 9-10 exceptional, a competition winner.
The rating must reflect the weaknesses: if you listed two or more, give 5 or less. Give 7 or \
more only when the weaknesses are trivial and several strengths are outstanding. Write in a \
concise academic register: name formal qualities (visual weight, figure-ground separation, \
tonal range, leading lines, negative space, the decisive moment).

Reply with JSON only."
    )
}
