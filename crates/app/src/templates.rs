//! Template presets and live previews for the template editor.

use chrono::NaiveDateTime;
use picturesque_core::metadata::{Camera, Place};
use picturesque_core::template::{PRESETS, PathTemplate, TemplateValues};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A built-in template, for the preset picker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct TemplatePresetView {
    pub id: String,
    pub label: String,
    pub template: String,
}

pub fn presets() -> Vec<TemplatePresetView> {
    PRESETS
        .iter()
        .map(|p| TemplatePresetView {
            id: p.id.into(),
            label: p.label.into(),
            template: p.template.into(),
        })
        .collect()
}

/// Arguments of `preview_template`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct PreviewTemplateArgs {
    pub template: String,
}

/// One sample photo rendered with a template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct TemplateExample {
    /// What the sample photo is, for example "RAW+JPEG taken in Lisbon".
    pub description: String,
    /// Resulting paths, `/`-separated, relative to the library folder.
    pub paths: Vec<String>,
}

/// Result of `preview_template`: an error message or example paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct TemplatePreview {
    pub error: Option<String>,
    pub examples: Vec<TemplateExample>,
}

/// Validates `template` and renders it for a few typical photos.
pub fn preview(template: &str) -> TemplatePreview {
    let template = match PathTemplate::parse(template) {
        Ok(t) => t,
        Err(e) => {
            return TemplatePreview {
                error: Some(e.to_string()),
                examples: Vec::new(),
            };
        }
    };
    let time = |s| NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok();
    let lisbon = Place {
        city: "Lisbon".into(),
        region: Some("Lisbon".into()),
        country: "Portugal".into(),
        country_code: "PT".into(),
    };
    let camera = Camera::new("Canon", "Canon EOS R5");
    let phone = Camera::new("Apple", "iPhone 15 Pro");
    let samples: [(&str, TemplateValues<'_>, &[&str]); 3] = [
        (
            "RAW+JPEG taken in Lisbon",
            TemplateValues {
                captured_at: time("2026-10-12 14:30:05"),
                place: Some(&lisbon),
                camera: Some(&camera),
                lens: Some("RF24-70mm F2.8 L IS USM"),
                original_name: "IMG_0042",
                seq: Some(1),
            },
            &["CR3", "JPG"],
        ),
        (
            "Phone photo without location",
            TemplateValues {
                captured_at: time("2025-07-04 09:15:00"),
                camera: Some(&phone),
                original_name: "IMG_7781",
                seq: Some(2),
                ..Default::default()
            },
            &["HEIC"],
        ),
        (
            "Scan with no date",
            TemplateValues {
                original_name: "scan_003",
                seq: Some(3),
                ..Default::default()
            },
            &["tif"],
        ),
    ];
    let examples = samples
        .into_iter()
        .map(|(description, values, extensions)| {
            let rendered = template.render(&values);
            TemplateExample {
                description: description.into(),
                paths: extensions
                    .iter()
                    .map(|ext| {
                        rendered
                            .file(ext)
                            .iter()
                            .map(|c| c.to_string_lossy())
                            .collect::<Vec<_>>()
                            .join("/")
                    })
                    .collect(),
            }
        })
        .collect();
    TemplatePreview {
        error: None,
        examples,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_typical_photos() {
        let p = preview("{year|Undated}/{date} {city}/{name}.{ext}");

        assert_eq!(p.error, None);
        assert_eq!(
            p.examples[0].paths,
            [
                "2026/2026-10-12 Lisbon/IMG_0042.CR3",
                "2026/2026-10-12 Lisbon/IMG_0042.JPG"
            ]
        );
        assert_eq!(p.examples[1].paths, ["2025/2025-07-04/IMG_7781.HEIC"]);
        assert_eq!(p.examples[2].paths, ["Undated/scan_003.tif"]);
    }

    #[test]
    fn reports_invalid_templates() {
        let p = preview("{year}/{name}");

        assert!(p.error.unwrap().contains(".{ext}"));
        assert!(p.examples.is_empty());
    }

    #[test]
    fn lists_every_preset() {
        assert_eq!(presets().len(), PRESETS.len());
        assert_eq!(
            presets()[0].template,
            crate::settings::DEFAULT_PATH_TEMPLATE
        );
    }
}
