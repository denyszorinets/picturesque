//! Path templates: where each photo goes inside the Clean library.
//!
//! A template is a `/`-separated path with tokens in braces, for example
//! `{year}/{date} {city}/{name}.{ext}`. It must end with `.{ext}` so the
//! files of one photo (RAW, JPEG, sidecars) share one base name.

use std::fmt;
use std::path::PathBuf;

use chrono::{Datelike, NaiveDateTime, Timelike};

use crate::metadata::{Camera, Place};

/// A token that can appear in a template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    Year,
    Month,
    Day,
    Date,
    Time,
    City,
    Region,
    Country,
    CountryCode,
    Camera,
    Make,
    Lens,
    Name,
    Seq,
    Ext,
}

impl Token {
    /// Every token, in documentation order.
    pub const ALL: [Token; 15] = [
        Token::Year,
        Token::Month,
        Token::Day,
        Token::Date,
        Token::Time,
        Token::City,
        Token::Region,
        Token::Country,
        Token::CountryCode,
        Token::Camera,
        Token::Make,
        Token::Lens,
        Token::Name,
        Token::Seq,
        Token::Ext,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Token::Year => "year",
            Token::Month => "month",
            Token::Day => "day",
            Token::Date => "date",
            Token::Time => "time",
            Token::City => "city",
            Token::Region => "region",
            Token::Country => "country",
            Token::CountryCode => "country_code",
            Token::Camera => "camera",
            Token::Make => "make",
            Token::Lens => "lens",
            Token::Name => "name",
            Token::Seq => "seq",
            Token::Ext => "ext",
        }
    }

    fn from_name(name: &str) -> Option<Token> {
        Token::ALL.into_iter().find(|t| t.name() == name)
    }
}

/// Why a template is invalid. The `Display` text is shown to users.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateError {
    Empty,
    UnknownToken(String),
    UnclosedBrace,
    UnexpectedClosingBrace,
    MustEndWithExt,
    ExtOutsideFileName,
    NotRelative,
    ParentSegment,
}

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("The template is empty."),
            Self::UnknownToken(t) => {
                let names: Vec<String> = Token::ALL.iter().map(|t| format!("{{{}}}", t.name())).collect();
                write!(f, "Unknown token `{{{t}}}`. Available tokens: {}.", names.join(", "))
            }
            Self::UnclosedBrace => f.write_str("A `{` is not closed. Tokens look like `{year}`."),
            Self::UnexpectedClosingBrace => f.write_str("A `}` has no matching `{`."),
            Self::MustEndWithExt => f.write_str(
                "The template must end with `.{ext}` so files keep their type and RAW+JPEG pairs keep matching names.",
            ),
            Self::ExtOutsideFileName => f.write_str("`{ext}` can only be used once, at the very end."),
            Self::NotRelative => f.write_str("The template must not start with `/`; it is relative to the library folder."),
            Self::ParentSegment => f.write_str("The template must not contain `..`."),
        }
    }
}

impl std::error::Error for TemplateError {}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    Literal(String),
    Token {
        token: Token,
        fallback: Option<String>,
    },
}

/// A parsed, valid path template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathTemplate {
    source: String,
    /// Folder segments, then the file stem (the last segment without `.{ext}`).
    segments: Vec<Vec<Piece>>,
}

/// Values a photo provides to a template.
#[derive(Debug, Clone, Copy, Default)]
pub struct TemplateValues<'a> {
    /// Local capture time.
    pub captured_at: Option<NaiveDateTime>,
    pub place: Option<&'a Place>,
    pub camera: Option<&'a Camera>,
    pub lens: Option<&'a str>,
    /// Original file name without extension.
    pub original_name: &'a str,
    /// Position of the photo in its import, starting at 1.
    pub seq: Option<u32>,
}

/// Where a photo goes: a folder relative to the library root and a file
/// stem. Each of the photo's files is `dir/stem.<its extension>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedPath {
    pub dir: PathBuf,
    pub stem: String,
}

impl RenderedPath {
    /// The path of one file of the photo, relative to the library root.
    pub fn file(&self, extension: &str) -> PathBuf {
        self.dir.join(format!("{}.{extension}", self.stem))
    }
}

impl PathTemplate {
    pub fn parse(source: &str) -> Result<Self, TemplateError> {
        let source = source.trim();
        if source.is_empty() {
            return Err(TemplateError::Empty);
        }
        if source.starts_with('/') || source.starts_with('\\') {
            return Err(TemplateError::NotRelative);
        }
        let Some(body) = source.strip_suffix(".{ext}") else {
            return Err(if source.contains("{ext}") {
                TemplateError::ExtOutsideFileName
            } else {
                TemplateError::MustEndWithExt
            });
        };
        let mut segments = Vec::new();
        for raw in body.split('/') {
            if raw.trim() == ".." {
                return Err(TemplateError::ParentSegment);
            }
            let pieces = parse_segment(raw)?;
            if pieces.iter().any(|p| {
                matches!(
                    p,
                    Piece::Token {
                        token: Token::Ext,
                        ..
                    }
                )
            }) {
                return Err(TemplateError::ExtOutsideFileName);
            }
            segments.push(pieces);
        }
        Ok(Self {
            source: source.to_owned(),
            segments,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.source
    }

    /// Renders the folder and file stem for a photo. Missing values render
    /// as their fallback, or as nothing (along with an adjacent separator);
    /// folders that end up empty are dropped. If the file stem would be
    /// empty, the original name is used.
    pub fn render(&self, values: &TemplateValues<'_>) -> RenderedPath {
        let (stem_pieces, dir_segments) = self
            .segments
            .split_last()
            .expect("at least the file segment");
        let dir: PathBuf = dir_segments
            .iter()
            .map(|pieces| sanitize_component(&render_segment(pieces, values)))
            .filter(|s| !s.is_empty())
            .collect();
        let mut stem = sanitize_component(&render_segment(stem_pieces, values));
        if stem.is_empty() {
            stem = sanitize_component(values.original_name);
        }
        if stem.is_empty() {
            stem = "photo".to_owned();
        }
        RenderedPath { dir, stem }
    }
}

impl fmt::Display for PathTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.source)
    }
}

fn parse_segment(raw: &str) -> Result<Vec<Piece>, TemplateError> {
    let mut pieces = Vec::new();
    let mut literal = String::new();
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        match c {
            '{' => {
                let mut inner = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some('{') | None => return Err(TemplateError::UnclosedBrace),
                        Some(c) => inner.push(c),
                    }
                }
                let (name, fallback) = match inner.split_once('|') {
                    Some((name, fallback)) => (name.trim(), Some(fallback.to_owned())),
                    None => (inner.trim(), None),
                };
                let token = Token::from_name(name)
                    .ok_or_else(|| TemplateError::UnknownToken(name.to_owned()))?;
                if !literal.is_empty() {
                    pieces.push(Piece::Literal(std::mem::take(&mut literal)));
                }
                pieces.push(Piece::Token { token, fallback });
            }
            '}' => return Err(TemplateError::UnexpectedClosingBrace),
            c => literal.push(c),
        }
    }
    if !literal.is_empty() {
        pieces.push(Piece::Literal(literal));
    }
    Ok(pieces)
}

fn token_value(token: Token, v: &TemplateValues<'_>) -> Option<String> {
    let t = v.captured_at;
    let place = v.place;
    let value = match token {
        Token::Year => t.map(|t| format!("{:04}", t.year())),
        Token::Month => t.map(|t| format!("{:02}", t.month())),
        Token::Day => t.map(|t| format!("{:02}", t.day())),
        Token::Date => t.map(|t| t.format("%Y-%m-%d").to_string()),
        Token::Time => t.map(|t| format!("{:02}{:02}{:02}", t.hour(), t.minute(), t.second())),
        Token::City => place.map(|p| p.city.clone()),
        Token::Region => place.and_then(|p| p.region.clone()),
        Token::Country => place.map(|p| p.country.clone()),
        Token::CountryCode => place.map(|p| p.country_code.clone()),
        Token::Camera => v.camera.map(Camera::display_name),
        Token::Make => v.camera.map(Camera::brand),
        Token::Lens => v.lens.map(str::to_owned),
        Token::Name => Some(v.original_name.to_owned()),
        Token::Seq => v.seq.map(|n| format!("{n:04}")),
        Token::Ext => None,
    };
    // A value must never create folders of its own.
    value
        .map(|s| s.replace(['/', '\\'], "-"))
        .filter(|s| !s.trim().is_empty())
}

const SEPARATORS: &[char] = &[' ', '-', '_', ',', '.'];

fn render_segment(pieces: &[Piece], values: &TemplateValues<'_>) -> String {
    // Each part is (text, kind). For each token that rendered empty, one
    // run of separators is removed from the nearest literal: the one before
    // it if there is text before, otherwise the one after. Values (like an
    // original name `_DSC0001`) are never trimmed.
    #[derive(PartialEq)]
    enum Kind {
        Literal,
        Value,
        Missing,
    }
    let mut parts: Vec<(String, Kind)> = pieces
        .iter()
        .map(|p| match p {
            Piece::Literal(s) => (s.clone(), Kind::Literal),
            Piece::Token { token, fallback } => {
                match token_value(*token, values).or_else(|| fallback.clone()) {
                    Some(v) => (v, Kind::Value),
                    None => (String::new(), Kind::Missing),
                }
            }
        })
        .collect();
    for i in 0..parts.len() {
        if parts[i].1 != Kind::Missing {
            continue;
        }
        let before = parts[..i].iter().rposition(|(s, _)| !s.is_empty());
        let after = parts[i + 1..]
            .iter()
            .position(|(s, _)| !s.is_empty())
            .map(|j| i + 1 + j);
        let target = match (before, after) {
            (Some(b), _) => Some((b, true)),
            (None, Some(a)) => Some((a, false)),
            (None, None) => None,
        };
        if let Some((index, trim_end)) = target {
            let (text, kind) = &mut parts[index];
            if *kind == Kind::Literal {
                *text = if trim_end {
                    text.trim_end_matches(SEPARATORS).to_owned()
                } else {
                    text.trim_start_matches(SEPARATORS).to_owned()
                };
            }
        }
    }
    parts.into_iter().map(|(s, _)| s).collect()
}

/// Characters not allowed in file names on at least one supported system.
const FORBIDDEN: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

/// Windows device names, which cannot be used as file names with any extension.
const RESERVED: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Longest file-name component, in bytes, leaving room for a collision
/// suffix and extensions within the usual 255-byte limit.
const MAX_COMPONENT_BYTES: usize = 200;

/// Makes `s` a file or folder name that is valid on Linux, macOS and
/// Windows, so a library on a portable drive works everywhere: forbidden
/// and control characters become `_`, whitespace is collapsed, trailing
/// dots and spaces are removed, device names get a `_` suffix and long
/// names are shortened.
pub fn sanitize_component(s: &str) -> String {
    let replaced: String = s
        .chars()
        .map(|c| {
            if FORBIDDEN.contains(&c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let mut out = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    while out.ends_with(['.', ' ']) {
        out.pop();
    }
    if out.chars().all(|c| c == '.') {
        out.clear();
    }
    if out.len() > MAX_COMPONENT_BYTES {
        let mut cut = MAX_COMPONENT_BYTES;
        while !out.is_char_boundary(cut) {
            cut -= 1;
        }
        out.truncate(cut);
        out = out.trim_end_matches(['.', ' ']).to_owned();
    }
    let base = out.split('.').next().unwrap_or("").to_ascii_lowercase();
    if RESERVED.contains(&base.as_str()) {
        out.push('_');
    }
    out
}

/// The first of `stem`, `stem (2)`, `stem (3)`, … for which `taken`
/// returns false. Callers check every file of the photo, so a RAW+JPEG
/// pair keeps matching names.
pub fn unique_stem(stem: &str, mut taken: impl FnMut(&str) -> bool) -> String {
    if !taken(stem) {
        return stem.to_owned();
    }
    (2u32..)
        .map(|n| format!("{stem} ({n})"))
        .find(|candidate| !taken(candidate))
        .expect("an unused name exists")
}

/// A ready-made template offered in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TemplatePreset {
    pub id: &'static str,
    pub label: &'static str,
    pub template: &'static str,
}

/// Built-in templates. The first is the default.
pub const PRESETS: &[TemplatePreset] = &[
    TemplatePreset {
        id: "date",
        label: "By date",
        template: "{year|Undated}/{date}/{name}.{ext}",
    },
    TemplatePreset {
        id: "date_place",
        label: "By date and place",
        template: "{year|Undated}/{date} {city}/{name}.{ext}",
    },
    TemplatePreset {
        id: "month",
        label: "By month",
        template: "{year|Undated}/{year}-{month}/{name}.{ext}",
    },
    TemplatePreset {
        id: "country_date",
        label: "By country, then date",
        template: "{country|Unknown place}/{year|Undated}/{date} {city}/{name}.{ext}",
    },
    TemplatePreset {
        id: "camera",
        label: "By date and camera",
        template: "{year|Undated}/{date}/{camera}/{name}.{ext}",
    },
    TemplatePreset {
        id: "renamed",
        label: "Renamed by capture time",
        template: "{year|Undated}/{date}/{date}_{time}_{seq}.{ext}",
    },
];

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn time() -> NaiveDateTime {
        NaiveDateTime::parse_from_str("2026-10-12 14:30:05", "%Y-%m-%d %H:%M:%S").unwrap()
    }

    fn lisbon() -> Place {
        Place {
            city: "Lisbon".into(),
            region: Some("Lisbon".into()),
            country: "Portugal".into(),
            country_code: "PT".into(),
        }
    }

    fn render(template: &str, values: TemplateValues<'_>) -> (PathBuf, String) {
        let r = PathTemplate::parse(template).unwrap().render(&values);
        (r.dir, r.stem)
    }

    fn full<'a>(place: &'a Place, camera: &'a Camera) -> TemplateValues<'a> {
        TemplateValues {
            captured_at: Some(time()),
            place: Some(place),
            camera: Some(camera),
            lens: Some("RF24-70mm F2.8 L IS USM"),
            original_name: "IMG_0042",
            seq: Some(7),
        }
    }

    fn dir(s: &str) -> PathBuf {
        s.split('/').collect()
    }

    #[test]
    fn renders_every_token() {
        let (place, camera) = (lisbon(), Camera::new("Canon", "Canon EOS R5"));
        let values = full(&place, &camera);

        assert_eq!(
            render("{year}/{month}/{day}/{date} {time}/{name}.{ext}", values),
            (dir("2026/10/12/2026-10-12 143005"), "IMG_0042".into())
        );
        assert_eq!(
            render(
                "{country}/{region}/{city} {country_code}/{seq}.{ext}",
                values
            ),
            (dir("Portugal/Lisbon/Lisbon PT"), "0007".into())
        );
        assert_eq!(
            render("{make}/{camera}/{lens}/{name}.{ext}", values),
            (
                dir("Canon/Canon EOS R5/RF24-70mm F2.8 L IS USM"),
                "IMG_0042".into()
            )
        );
    }

    #[test]
    fn rendered_path_joins_each_extension() {
        let r = PathTemplate::parse("{year}/{name}.{ext}")
            .unwrap()
            .render(&TemplateValues {
                captured_at: Some(time()),
                original_name: "IMG_1",
                ..Default::default()
            });

        assert_eq!(r.file("CR3"), Path::new("2026").join("IMG_1.CR3"));
        assert_eq!(r.file("CR3.xmp"), Path::new("2026").join("IMG_1.CR3.xmp"));
    }

    #[test]
    fn missing_values_drop_their_separator_and_empty_folders() {
        let values = TemplateValues {
            captured_at: Some(time()),
            original_name: "IMG_1",
            ..Default::default()
        };

        assert_eq!(
            render("{year}/{date} {city}/{name}.{ext}", values),
            (dir("2026/2026-10-12"), "IMG_1".into())
        );
        assert_eq!(
            render("{year}/{city}-{date}/{name}.{ext}", values),
            (dir("2026/2026-10-12"), "IMG_1".into())
        );
        assert_eq!(
            render("{year}/{camera}/{name}.{ext}", values),
            (dir("2026"), "IMG_1".into())
        );
    }

    #[test]
    fn fallbacks_replace_missing_values() {
        let values = TemplateValues {
            original_name: "IMG_1",
            ..Default::default()
        };

        assert_eq!(
            render("{year|Undated}/{date}/{name}.{ext}", values),
            (dir("Undated"), "IMG_1".into())
        );
        assert_eq!(
            render("{country|Unknown place}/{name}.{ext}", values),
            (dir("Unknown place"), "IMG_1".into())
        );
    }

    #[test]
    fn original_names_are_never_trimmed() {
        let values = TemplateValues {
            original_name: "_DSC0001",
            ..Default::default()
        };

        assert_eq!(render("{city}{name}.{ext}", values).1, "_DSC0001");
        assert_eq!(render("{name}.{ext}", values).1, "_DSC0001");
    }

    #[test]
    fn empty_file_stem_falls_back_to_the_original_name() {
        let values = TemplateValues {
            original_name: "IMG_9",
            ..Default::default()
        };

        assert_eq!(render("{date}_{time}.{ext}", values).1, "IMG_9");
    }

    #[test]
    fn values_cannot_create_folders() {
        let place = Place {
            city: "Paris/Île".into(),
            region: None,
            country: "France".into(),
            country_code: "FR".into(),
        };
        let values = TemplateValues {
            place: Some(&place),
            original_name: "a",
            ..Default::default()
        };

        assert_eq!(
            render("{city}/{name}.{ext}", values),
            (dir("Paris-Île"), "a".into())
        );
    }

    #[test]
    fn rejects_invalid_templates_with_helpful_errors() {
        let err = |t: &str| PathTemplate::parse(t).unwrap_err();

        assert_eq!(err(""), TemplateError::Empty);
        assert_eq!(err("{year}/{name}"), TemplateError::MustEndWithExt);
        assert_eq!(err("{ext}/{name}.{ext}"), TemplateError::ExtOutsideFileName);
        assert_eq!(
            err("{year}/{name}.{ext}.jpg"),
            TemplateError::ExtOutsideFileName
        );
        assert_eq!(
            err("{yaer}/{name}.{ext}"),
            TemplateError::UnknownToken("yaer".into())
        );
        assert_eq!(err("{year/{name}.{ext}"), TemplateError::UnclosedBrace);
        assert_eq!(
            err("year}/{name}.{ext}"),
            TemplateError::UnexpectedClosingBrace
        );
        assert_eq!(err("/photos/{name}.{ext}"), TemplateError::NotRelative);
        assert_eq!(err("{year}/../{name}.{ext}"), TemplateError::ParentSegment);
        assert!(err("{yaer}/{name}.{ext}").to_string().contains("{year}"));
    }

    #[test]
    fn sanitizes_names_for_every_system() {
        assert_eq!(sanitize_component("a<b>c:d\"e|f?g*h"), "a_b_c_d_e_f_g_h");
        assert_eq!(sanitize_component("tab\there\nnew"), "tab_here_new");
        assert_eq!(sanitize_component("  lots   of   space  "), "lots of space");
        assert_eq!(sanitize_component("ends with dots..."), "ends with dots");
        assert_eq!(sanitize_component("..."), "");
        assert_eq!(sanitize_component("CON"), "CON_");
        assert_eq!(sanitize_component("nul.txt"), "nul.txt_");
        assert_eq!(sanitize_component("console"), "console");
        assert_eq!(sanitize_component("São Paulo"), "São Paulo");
    }

    #[test]
    fn shortens_long_names_on_a_character_boundary() {
        let long = "é".repeat(150);

        let out = sanitize_component(&long);

        assert!(out.len() <= MAX_COMPONENT_BYTES);
        assert!(out.chars().all(|c| c == 'é'));
    }

    #[test]
    fn unique_stem_adds_a_counter() {
        let taken = ["IMG_1", "IMG_1 (2)"];

        assert_eq!(unique_stem("IMG_2", |s| taken.contains(&s)), "IMG_2");
        assert_eq!(unique_stem("IMG_1", |s| taken.contains(&s)), "IMG_1 (3)");
    }

    #[test]
    fn every_preset_is_valid_and_ids_are_unique() {
        for preset in PRESETS {
            PathTemplate::parse(preset.template).unwrap_or_else(|e| panic!("{}: {e}", preset.id));
        }
        let mut ids: Vec<_> = PRESETS.iter().map(|p| p.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), PRESETS.len());
    }

    /// Mirrors the presets table in docs/reference/path-templates.rst.
    #[test]
    fn presets_render_as_documented() {
        let (place, camera) = (lisbon(), Camera::new("Canon", "Canon EOS R5"));
        let values = full(&place, &camera);
        let expected = [
            ("date", "2026/2026-10-12/IMG_0042"),
            ("date_place", "2026/2026-10-12 Lisbon/IMG_0042"),
            ("month", "2026/2026-10/IMG_0042"),
            ("country_date", "Portugal/2026/2026-10-12 Lisbon/IMG_0042"),
            ("camera", "2026/2026-10-12/Canon EOS R5/IMG_0042"),
            ("renamed", "2026/2026-10-12/2026-10-12_143005_0007"),
        ];
        assert_eq!(expected.len(), PRESETS.len());
        for (id, path) in expected {
            let preset = PRESETS.iter().find(|p| p.id == id).unwrap();
            let r = PathTemplate::parse(preset.template)
                .unwrap()
                .render(&values);
            assert_eq!(
                r.file("CR3"),
                PathBuf::from(format!("{path}.CR3").replace('/', std::path::MAIN_SEPARATOR_STR)),
                "{id}"
            );
        }
    }

    #[test]
    fn date_presets_send_undated_photos_to_undated() {
        let values = TemplateValues {
            original_name: "IMG_0042",
            ..Default::default()
        };
        let preset = PRESETS.iter().find(|p| p.id == "date").unwrap();

        assert_eq!(
            render(preset.template, values),
            (dir("Undated"), "IMG_0042".into())
        );
    }
}
