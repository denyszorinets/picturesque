use std::path::PathBuf;

use picturesque_core::media::MediaFormat;
use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets};
use serde_json::{Value, json};

use super::*;
use crate::api::{Api, ApiErrorCode};
use crate::ports::catalog::PhotoCatalog;
use crate::settings::CriticPreset;
use crate::settings::Settings;
use crate::testing::{FakeMetadata, MemFs, TestWorld, fake_hash};
use picturesque_core::photo::{Photo, PhotoId};

const REPLY: &str = r#"Sure! {"rating": 7.5, "summary": " Warm light on a calm lake. ",
    "strengths": ["golden light", " ", "clean horizon"], "weaknesses": ["kayaks too small"]}"#;

/// A library with one photo and a server that has `models`, critiques
/// from the server.
fn setup(enabled: bool, models: Option<&[&str]>) -> (TestWorld, Api, i64) {
    let critic = CriticSettings {
        enabled,
        backend: CriticEngine::Server,
        ..Default::default()
    };
    setup_with(critic, models, false)
}

/// A library with one photo, critiques from the built-in critic, whose
/// model is `downloaded` or not.
fn setup_builtin(downloaded: bool) -> (TestWorld, Api, i64) {
    let critic = CriticSettings {
        enabled: true,
        ..Default::default()
    };
    setup_with(critic, None, downloaded)
}

fn setup_with(
    critic: CriticSettings,
    models: Option<&[&str]>,
    downloaded: bool,
) -> (TestWorld, Api, i64) {
    let fs = MemFs::new().dir("/lib").file("/lib/IMG_1.JPG", b"one");
    let world = TestWorld::new(fs, FakeMetadata::default());
    world
        .settings
        .save_settings(&Settings {
            library_root: Some("/lib".into()),
            critic,
            ..Default::default()
        })
        .unwrap();
    let id = world
        .catalog
        .insert_photo(&NewPhoto {
            metadata: Default::default(),
            assets: PhotoAssets::new(vec![Asset {
                format: MediaFormat::Jpeg,
                path: PathBuf::from("IMG_1.JPG"),
                size: 3,
                hash: fake_hash(b"one"),
            }])
            .unwrap(),
        })
        .unwrap();
    *world.critic.models.lock().unwrap() =
        models.map(|m| m.iter().map(|s| (*s).to_owned()).collect());
    *world.critic.reply.lock().unwrap() = REPLY.into();
    let mut context = world.context();
    let spec = crate::models::catalog()
        .into_iter()
        .find(|m| m.id == CRITIC)
        .expect("the critic model is offered");
    let store = crate::testing::MemoryModelStore::default();
    if downloaded {
        store.install(&spec, &[]);
    }
    context.models = vec![spec];
    context.model_store = Arc::new(store);
    let api = Api::new(context).unwrap();
    (world, api, id.0)
}

#[test]
fn parses_the_first_json_object_of_a_reply() {
    let c = Critique::parse(REPLY).unwrap();
    assert_eq!(c.rating, 7.5);
    assert_eq!(c.summary, "Warm light on a calm lake.");
    assert_eq!(c.strengths, ["golden light", "clean horizon"]);
    assert_eq!(c.weaknesses, ["kayaks too small"]);

    let thought = Critique::parse(r#"<think>{maybe} hmm</think>{"rating": 6, "summary": "Fine"}"#);
    assert_eq!(thought.unwrap().summary, "Fine");

    let high = Critique::parse(r#"{"rating": 12, "summary": "Wow"}"#).unwrap();
    assert_eq!((high.rating, high.strengths.len()), (10.0, 0));
    for bad in [
        "I cannot judge photos.",
        r#"{"rating": 5}"#,
        r#"{"rating": 5, "summary": "  "}"#,
        r#"{"rating": "good", "summary": "Nice"}"#,
    ] {
        assert_eq!(Critique::parse(bad), None, "{bad}");
    }
}

#[test]
fn recognizes_servers_on_this_computer() {
    for local in [
        "http://127.0.0.1:11434/v1",
        "http://localhost:8080/v1",
        "https://LOCALHOST/v1",
        "http://[::1]:11434/v1",
        "http://user@127.0.0.2/v1",
    ] {
        assert!(is_local(local), "{local}");
    }
    for remote in [
        "http://192.168.1.20:11434/v1",
        "https://api.example.com/v1",
        "http://localhost.example.com/v1",
        "http://127.0.0.1.example.com/v1",
    ] {
        assert!(!is_local(remote), "{remote}");
    }
}

#[test]
fn critiques_once_and_keeps_the_critique_for_the_model_and_rubric() {
    let (world, api, id) = setup(true, Some(&["qwen3-vl:4b"]));
    assert_eq!(
        api.call("get_critique", json!({ "photo_id": id })).unwrap(),
        Value::Null
    );

    let view = api
        .call("critique_photo", json!({ "photo_id": id }))
        .unwrap();
    assert_eq!(view["model"], "qwen3-vl:4b");
    assert_eq!(view["outdated"], false);
    assert_eq!(view["rating"], 7.5);
    assert!((view["score"].as_f64().unwrap() - 6.5 / 9.0).abs() < 1e-6);
    assert_eq!(view["strengths"], json!(["golden light", "clean horizon"]));
    let asked = world.critic.asked.lock().unwrap().clone();
    assert_eq!(asked.len(), 1);
    let q = &asked[0];
    assert_eq!(
        (q.target.as_str(), q.model.as_str(), q.gpu),
        (DEFAULT_SERVER, "qwen3-vl:4b", None)
    );
    assert!(q.system.contains("Composition: framing"));
    assert!(q.system.contains("professor of photography"));
    // The cached preview, shrunk by the adapter.
    assert!(q.image.starts_with("/cache"), "{:?}", q.image);
    assert_eq!(q.max_edge, MAX_EDGE);

    // Stored: shown again without asking.
    assert_eq!(
        api.call("get_critique", json!({ "photo_id": id })).unwrap(),
        view
    );
    api.call("critique_photo", json!({ "photo_id": id }))
        .unwrap();
    assert_eq!(world.critic.asked.lock().unwrap().len(), 1);
    // Unless asked to refresh.
    api.call("critique_photo", json!({ "photo_id": id, "refresh": true }))
        .unwrap();
    assert_eq!(world.critic.asked.lock().unwrap().len(), 2);

    // A critique written with the earlier prompt is shown as outdated.
    world
        .catalog
        .store_analysis(
            PhotoId(id),
            ID,
            &json!({ "model": "qwen3-vl:4b", "rubric": fingerprint(DEFAULT_RUBRIC),
                     "critique": { "rating": 7.0, "summary": "Old." } }),
        )
        .unwrap();
    let old = api.call("get_critique", json!({ "photo_id": id })).unwrap();
    assert_eq!(
        (&old["summary"], &old["outdated"]),
        (&json!("Old."), &json!(true))
    );
    api.call("critique_photo", json!({ "photo_id": id }))
        .unwrap();
    assert_eq!(world.critic.asked.lock().unwrap().len(), 3);

    // Another rubric makes the stored critique stale.
    api.call(
        "update_settings",
        json!({ "critic": { "enabled": true, "backend": "server", "server": DEFAULT_SERVER,
                            "model": "qwen3-vl:4b", "rubric": "Only the light." } }),
    )
    .unwrap();
    let old = api.call("get_critique", json!({ "photo_id": id })).unwrap();
    assert_eq!(old["outdated"], true);
    assert_eq!(old["model"], "qwen3-vl:4b");
    let fresh = api
        .call("critique_photo", json!({ "photo_id": id }))
        .unwrap();
    assert_eq!(fresh["outdated"], false);
    let asked = world.critic.asked.lock().unwrap();
    assert!(asked[3].system.contains("Only the light."));
}

#[test]
fn says_what_is_wrong() {
    let (_, api, id) = setup(false, Some(&["qwen3-vl:4b"]));
    let err = api
        .call("critique_photo", json!({ "photo_id": id }))
        .unwrap_err();
    assert_eq!(err.code, ApiErrorCode::InvalidArguments);
    assert!(err.message.contains("Settings"), "{}", err.message);

    let (_, api, id) = setup(true, None);
    let err = api
        .call("critique_photo", json!({ "photo_id": id }))
        .unwrap_err();
    assert_eq!(err.code, ApiErrorCode::Internal);
    assert!(
        err.message.contains("connection refused"),
        "{}",
        err.message
    );
    let err = api
        .call("critique_photo", json!({ "photo_id": 999 }))
        .unwrap_err();
    assert_eq!(err.code, ApiErrorCode::NotFound);

    let (world, api, id) = setup(true, Some(&["qwen3-vl:4b"]));
    *world.critic.reply.lock().unwrap() = "I'd rather not.".into();
    let err = api
        .call("critique_photo", json!({ "photo_id": id }))
        .unwrap_err();
    assert!(err.message.contains("I'd rather not."), "{}", err.message);
}

#[test]
fn status_checks_the_server_only_when_enabled() {
    let (world, api, _) = setup(false, None);
    let status = api.call("critic_status", Value::Null).unwrap();
    assert_eq!(status["enabled"], false);
    assert_eq!(status["local"], true);
    assert_eq!(status["problem"], Value::Null);
    assert_eq!(status["suggested"][1]["name"], "qwen3-vl:4b");
    assert_eq!(status["suggested"][1]["license"], "Apache-2.0");
    assert_eq!(status["default_rubric"], DEFAULT_RUBRIC);
    drop(world);

    let (_, api, _) = setup(true, None);
    let status = api.call("critic_status", Value::Null).unwrap();
    assert_eq!(status["reachable"], false);
    assert!(
        status["problem"]
            .as_str()
            .unwrap()
            .contains("Install Ollama")
    );

    let (_, api, _) = setup(true, Some(&["llava:7b"]));
    let status = api.call("critic_status", Value::Null).unwrap();
    assert_eq!(status["reachable"], true);
    assert_eq!(status["models"], json!(["llava:7b"]));
    assert_eq!(status["model_installed"], false);
    assert!(
        status["problem"]
            .as_str()
            .unwrap()
            .contains("ollama pull qwen3-vl:4b")
    );

    let (_, api, _) = setup(true, Some(&["qwen3-vl:4b"]));
    let status = api.call("critic_status", Value::Null).unwrap();
    assert_eq!(
        (&status["model_installed"], &status["problem"]),
        (&json!(true), &Value::Null)
    );
}

#[test]
fn the_builtin_critic_uses_the_downloaded_model() {
    let (_, api, id) = setup_builtin(false);
    let status = api.call("critic_status", Value::Null).unwrap();
    assert_eq!(status["backend"], "builtin");
    assert_eq!(status["model"], BUILTIN_MODEL);
    assert_eq!(status["local"], true);
    assert_eq!(status["model_installed"], false);
    assert!(
        status["problem"]
            .as_str()
            .unwrap()
            .contains("Settings → AI models")
    );
    let err = api
        .call("critique_photo", json!({ "photo_id": id }))
        .unwrap_err();
    assert!(err.message.contains("not downloaded"), "{}", err.message);

    let (world, api, id) = setup_builtin(true);
    let status = api.call("critic_status", Value::Null).unwrap();
    assert_eq!(
        (
            &status["reachable"],
            &status["model_installed"],
            &status["problem"]
        ),
        (&json!(true), &json!(true), &Value::Null)
    );
    let view = api
        .call("critique_photo", json!({ "photo_id": id }))
        .unwrap();
    assert_eq!(view["model"], BUILTIN_MODEL);
    let q = world.critic.asked.lock().unwrap()[0].clone();
    assert_eq!((q.target.as_str(), q.gpu), ("builtin", Some(true)));
    assert!(
        q.model.ends_with(crate::models::CRITIC_MODEL_FILE),
        "{}",
        q.model
    );
    // Stored under the built-in model: switching to a server marks it outdated.
    api.call(
        "update_settings",
        json!({ "critic": { "enabled": true, "backend": "server", "server": DEFAULT_SERVER,
                            "model": "qwen3-vl:4b", "rubric": null } }),
    )
    .unwrap();
    let old = api.call("get_critique", json!({ "photo_id": id })).unwrap();
    assert_eq!(
        (&old["model"], &old["outdated"]),
        (&json!(BUILTIN_MODEL), &json!(true))
    );
}

#[test]
fn the_builtin_critic_follows_the_acceleration_setting() {
    let (world, api, id) = setup_builtin(true);
    api.call("update_settings", json!({ "ai": { "provider": "cpu" } }))
        .unwrap();
    api.call("critique_photo", json!({ "photo_id": id }))
        .unwrap();
    assert_eq!(world.critic.asked.lock().unwrap()[0].gpu, Some(false));
}

#[test]
fn says_when_the_builtin_critic_is_not_installed() {
    let (world, api, id) = setup_builtin(true);
    *world.critic.builtin_problem.lock().unwrap() =
        Some("the critic helper is missing; reinstall Picturesque".into());
    let status = api.call("critic_status", Value::Null).unwrap();
    assert_eq!(status["reachable"], false);
    assert!(status["problem"].as_str().unwrap().contains("reinstall"));
    let err = api
        .call("critique_photo", json!({ "photo_id": id }))
        .unwrap_err();
    assert!(err.message.contains("reinstall"), "{}", err.message);
}

#[test]
fn schema_lists_the_flaws_first_and_the_rating_last() {
    let schema = Critique::schema();
    let keys: Vec<_> = schema["properties"].as_object().unwrap().keys().collect();
    assert_eq!(
        keys,
        [
            "weaknesses",
            "issues",
            "strengths",
            "summary",
            "rating",
            "potential",
            "fixable",
            "fix_steps",
            "improvements",
            "camera_tips"
        ]
    );
    let required: Vec<_> = schema["required"].as_array().unwrap().iter().collect();
    assert_eq!(required.len(), keys.len());
}

const FULL_REPLY: &str = r#"{"weaknesses": ["blown sky", "tilted"], "issues": ["exposure", "horizon", "exposure", "mood"],
    "strengths": ["colour"], "summary": "A tilted lake.", "rating": 4, "potential": 2,
    "fixable": "easy", "fix_steps": ["lower highlights", " ", "rotate 2 degrees"],
    "improvements": ["wait for clouds"], "camera_tips": ["use f/8", "ISO 100"]}"#;

#[test]
fn parses_the_advice_of_version_four() {
    let c = Critique::parse(FULL_REPLY).unwrap();
    // unknown category dropped, duplicate removed
    assert_eq!(c.issues, [Issue::Exposure, Issue::Horizon]);
    // never below the rating
    assert_eq!(c.potential, Some(4.0));
    assert_eq!(c.fixable, Some(Fixable::Easy));
    assert_eq!(c.fix_steps, ["lower highlights", "rotate 2 degrees"]);
    assert_eq!(c.improvements, ["wait for clouds"]);
    assert_eq!(c.camera_tips, ["use f/8", "ISO 100"]);
}

#[test]
fn clamps_and_tolerates_the_potential_and_the_verdict() {
    // (potential, fixable, expected potential, expected fixable)
    let cases = [
        (
            "9.5",
            "\"needs_skill\"",
            Some(9.5),
            Some(Fixable::NeedsSkill),
        ),
        (
            "15",
            "\"not_fixable\"",
            Some(10.0),
            Some(Fixable::NotFixable),
        ),
        ("3", "\"not_needed\"", Some(5.0), Some(Fixable::NotNeeded)),
        ("\"high\"", "\"maybe\"", None, None),
        ("null", "7", None, None),
    ];
    for (potential, fixable, want_potential, want_fixable) in cases {
        let reply = format!(
            r#"{{"rating": 5, "summary": "Ok", "potential": {potential}, "fixable": {fixable}}}"#
        );
        let c = Critique::parse(&reply).expect(&reply);
        assert_eq!(c.potential, want_potential, "{reply}");
        assert_eq!(c.fixable, want_fixable, "{reply}");
    }
}

#[test]
fn an_older_stored_critique_has_no_advice_and_is_outdated() {
    let (world, api, id) = setup(true, Some(&["qwen3-vl:4b"]));
    // Written by prompt version 3: the rubric key differs.
    let stored = json!({ "model": "qwen3-vl:4b", "rubric": fingerprint("v3\nx"), "critique": {
        "rating": 6.0, "summary": "Old.", "strengths": ["a"], "weaknesses": ["b"] } });
    world
        .catalog
        .store_analysis(PhotoId(id), ID, &stored)
        .unwrap();
    let view = api.call("get_critique", json!({ "photo_id": id })).unwrap();
    assert_eq!(view["outdated"], true);
    assert_eq!(view["summary"], "Old.");
    assert_eq!(view["issues"], json!([]));
    assert_eq!(view["potential"], Value::Null);
    assert_eq!(view["fixable"], Value::Null);
    assert_eq!(view["camera_tips"], json!([]));
}

#[test]
fn a_new_critique_keeps_its_advice_when_stored_and_viewed() {
    let (world, api, id) = setup(true, Some(&["qwen3-vl:4b"]));
    *world.critic.reply.lock().unwrap() = FULL_REPLY.into();
    let made = api
        .call("critique_photo", json!({ "photo_id": id }))
        .unwrap();
    let shown = api.call("get_critique", json!({ "photo_id": id })).unwrap();
    assert_eq!(made, shown);
    assert_eq!(shown["issues"], json!(["exposure", "horizon"]));
    assert_eq!(shown["potential"], 4.0);
    assert_eq!(shown["fixable"], "easy");
    assert_eq!(
        shown["fix_steps"],
        json!(["lower highlights", "rotate 2 degrees"])
    );
    assert_eq!(shown["camera_tips"], json!(["use f/8", "ISO 100"]));
}

#[test]
fn the_prompt_names_the_shot_data_and_the_format_only_when_known() {
    use picturesque_core::metadata::{Exposure, ExposureTime, GeoPoint};
    let photo = |exposure: Exposure, name: &str, format: MediaFormat| Photo {
        id: PhotoId(1),
        metadata: picturesque_core::metadata::CaptureMetadata {
            exposure,
            ..Default::default()
        },
        assets: PhotoAssets::new(vec![Asset {
            format,
            path: PathBuf::from(name),
            size: 1,
            hash: fake_hash(b"x"),
        }])
        .unwrap(),
    };
    let full = Exposure {
        iso: Some(800),
        f_number: Some(2.8),
        exposure_time: Some(ExposureTime {
            numerator: 1,
            denominator: 60,
        }),
        focal_length_mm: Some(35.0),
    };
    let cases = [
        (
            full,
            MediaFormat::Raw,
            "Critique this photo. Shot data: ISO 800, 1/60 s, f/2.8, 35 mm. \
             The original is a RAW file.",
        ),
        (
            Exposure {
                iso: Some(100),
                ..Default::default()
            },
            MediaFormat::Jpeg,
            "Critique this photo. Shot data: ISO 100. \
             The original is a JPEG; it has little room for recovery.",
        ),
        (
            Exposure::default(),
            MediaFormat::Jpeg,
            "Critique this photo. The original is a JPEG; it has little room for recovery.",
        ),
        (
            Exposure {
                exposure_time: Some(ExposureTime {
                    numerator: 1,
                    denominator: 2,
                }),
                f_number: Some(8.0),
                ..Default::default()
            },
            MediaFormat::Raw,
            "Critique this photo. Shot data: 0.5 s, f/8. The original is a RAW file.",
        ),
        (
            Exposure {
                exposure_time: Some(ExposureTime {
                    numerator: 2,
                    denominator: 1,
                }),
                focal_length_mm: Some(50.0),
                ..Default::default()
            },
            MediaFormat::Raw,
            "Critique this photo. Shot data: 2 s, 50 mm. The original is a RAW file.",
        ),
    ];
    for (exposure, format, want) in cases {
        let name = if format == MediaFormat::Raw {
            "secret-name.CR3"
        } else {
            "secret-name.JPG"
        };
        let mut p = photo(exposure, name, format);
        p.metadata.gps = Some(GeoPoint::new(38.7223, -9.1393).unwrap());
        let got = user_prompt(&p);
        assert_eq!(got, want);
        for leak in ["secret", "38.72", "9.13", "GPS"] {
            assert!(!got.contains(leak), "{got}");
        }
    }
}

#[test]
fn the_prompt_names_the_camera_and_lens_only_when_known() {
    use picturesque_core::metadata::{Camera, CaptureMetadata};
    // (camera make, model, lens, expected gear sentence)
    let cases = [
        (
            Some(("SONY", "ILCE-7M4")),
            Some("FE 35mm F1.8"),
            " Gear: Sony ILCE-7M4 with FE 35mm F1.8.",
        ),
        (
            Some(("Canon", "Canon EOS R5")),
            None,
            " Gear: Canon EOS R5.",
        ),
        (None, Some("  XF 23mm  "), " Gear: lens XF 23mm."),
        (Some(("", "")), Some(" "), ""),
        (None, Some("a\nb\0c   d"), " Gear: lens a b c d."),
        (None, None, ""),
    ];
    for (camera, lens, want) in cases {
        let photo = Photo {
            id: PhotoId(1),
            metadata: CaptureMetadata {
                camera: camera.map(|(make, model)| Camera::new(make, model)),
                lens: lens.map(str::to_owned),
                ..Default::default()
            },
            assets: PhotoAssets::new(vec![Asset {
                format: MediaFormat::Raw,
                path: PathBuf::from("a.CR3"),
                size: 1,
                hash: fake_hash(b"x"),
            }])
            .unwrap(),
        };
        assert_eq!(
            user_prompt(&photo),
            format!("Critique this photo.{want} The original is a RAW file."),
            "{camera:?} {lens:?}"
        );
    }
}

#[test]
fn a_long_lens_name_is_cut_before_it_reaches_the_model() {
    use picturesque_core::metadata::CaptureMetadata;
    let photo = Photo {
        id: PhotoId(1),
        metadata: CaptureMetadata {
            lens: Some("x".repeat(500)),
            ..Default::default()
        },
        assets: PhotoAssets::new(vec![Asset {
            format: MediaFormat::Raw,
            path: PathBuf::from("a.CR3"),
            size: 1,
            hash: fake_hash(b"x"),
        }])
        .unwrap(),
    };
    assert!(user_prompt(&photo).len() < 150);
}

#[test]
fn the_specific_issue_kinds_parse_and_the_schema_offers_only_them() {
    let c = Critique::parse(
        r#"{"summary": "A blurry, overexposed walk.", "rating": 3,
        "issues": ["overexposed", "subject_placement", "motion_blur", "exposure"]}"#,
    )
    .unwrap();
    assert_eq!(
        c.issues,
        [
            Issue::Overexposed,
            Issue::SubjectPlacement,
            Issue::MotionBlur,
            Issue::Exposure
        ]
    );
    let offered = Critique::schema()["properties"]["issues"]["items"]["enum"].clone();
    let offered: Vec<&str> = offered
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(offered.contains(&"underexposed") && offered.contains(&"busy_background"));
    // The coarse kinds are only read, never asked for.
    for old in ["exposure", "focus", "composition"] {
        assert!(!offered.contains(&old), "{old}");
    }
    for name in &offered {
        assert!(
            serde_json::from_value::<Issue>(json!(name)).is_ok(),
            "{name}"
        );
    }
}

#[test]
fn an_older_critique_stays_visible_and_is_redone_on_request() {
    // (stored model, stored rubric is the current one, outdated, asks again)
    let cases = [
        ("qwen3-vl:4b", true, false, false),
        ("other-model", true, true, true),
        ("qwen3-vl:4b", false, true, true),
    ];
    for (model, same_rubric, outdated, asks) in cases {
        let (world, api, id) = setup(true, Some(&["qwen3-vl:4b"]));
        let rubric = if same_rubric {
            rubric_key(CriticPreset::Professional, DEFAULT_RUBRIC)
        } else {
            "Only the light.".to_owned()
        };
        let stored = json!({ "model": model, "rubric": rubric, "critique": {
            "rating": 7.0, "summary": "Old.", "strengths": [], "weaknesses": [] } });
        world
            .catalog
            .store_analysis(PhotoId(id), ID, &stored)
            .unwrap();

        let view = api.call("get_critique", json!({ "photo_id": id })).unwrap();
        assert_eq!(view["outdated"], outdated, "{model} {same_rubric}");
        assert_eq!(view["model"], model);
        assert_eq!(view["summary"], "Old.");

        api.call("critique_photo", json!({ "photo_id": id }))
            .unwrap();
        let asked = world.critic.asked.lock().unwrap().len();
        assert_eq!(asked, usize::from(asks), "{model} {same_rubric}");
    }
}

#[test]
fn changing_the_preset_makes_a_critique_outdated() {
    // (stored preset, current preset, outdated)
    let cases = [
        (
            CriticPreset::Professional,
            CriticPreset::Professional,
            false,
        ),
        (CriticPreset::Professional, CriticPreset::Amateur, true),
        (CriticPreset::Amateur, CriticPreset::Amateur, false),
        (CriticPreset::Amateur, CriticPreset::Beginner, true),
    ];
    for (was, now, outdated) in cases {
        let critic = CriticSettings {
            enabled: true,
            backend: CriticEngine::Server,
            preset: now,
            ..Default::default()
        };
        let (world, api, id) = setup_with(critic, Some(&["qwen3-vl:4b"]), false);
        let stored = json!({ "model": "qwen3-vl:4b", "rubric": rubric_key(was, DEFAULT_RUBRIC),
            "critique": { "rating": 7.0, "summary": "Old.", "strengths": [], "weaknesses": [] } });
        world
            .catalog
            .store_analysis(PhotoId(id), ID, &stored)
            .unwrap();
        let view = api.call("get_critique", json!({ "photo_id": id })).unwrap();
        assert_eq!(view["outdated"], outdated, "{was:?} -> {now:?}");
    }
}

#[test]
fn a_new_critique_is_written_with_the_presets_prompt_and_key() {
    for preset in [CriticPreset::Amateur, CriticPreset::Beginner] {
        let critic = CriticSettings {
            enabled: true,
            backend: CriticEngine::Server,
            preset,
            ..Default::default()
        };
        let (world, api, id) = setup_with(critic, Some(&["qwen3-vl:4b"]), false);
        api.call("critique_photo", json!({ "photo_id": id }))
            .unwrap();
        let asked = world.critic.asked.lock().unwrap();
        assert!(
            asked[0]
                .system
                .contains(&system_prompt(preset, DEFAULT_RUBRIC))
        );
        drop(asked);
        let view = api.call("get_critique", json!({ "photo_id": id })).unwrap();
        assert_eq!(view["outdated"], false);
    }
}
