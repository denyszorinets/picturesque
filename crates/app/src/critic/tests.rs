use std::path::PathBuf;

use picturesque_core::media::MediaFormat;
use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets};
use serde_json::{Value, json};

use super::*;
use crate::api::{Api, ApiErrorCode};
use crate::settings::Settings;
use crate::testing::{FakeMetadata, MemFs, TestWorld, fake_hash};

const REPLY: &str = r#"Sure! {"rating": 7.5, "summary": " Warm light on a calm lake. ",
    "strengths": ["golden light", " ", "clean horizon"], "weaknesses": ["kayaks too small"]}"#;

/// A library with one photo and a server that has `models`.
fn setup(enabled: bool, models: Option<&[&str]>) -> (TestWorld, Api, i64) {
    let fs = MemFs::new().dir("/lib").file("/lib/IMG_1.JPG", b"one");
    let world = TestWorld::new(fs, FakeMetadata::default());
    world
        .settings
        .save_settings(&Settings {
            library_root: Some("/lib".into()),
            critic: CriticSettings {
                enabled,
                ..Default::default()
            },
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
    let api = Api::new(world.context()).unwrap();
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
    assert_eq!(view["rating"], 7.5);
    assert!((view["score"].as_f64().unwrap() - 6.5 / 9.0).abs() < 1e-6);
    assert_eq!(view["strengths"], json!(["golden light", "clean horizon"]));
    let asked = world.critic.asked.lock().unwrap().clone();
    assert_eq!(asked.len(), 1);
    let (server, model, system, image, max_edge) = &asked[0];
    assert_eq!(
        (server.as_str(), model.as_str()),
        (DEFAULT_SERVER, "qwen3-vl:4b")
    );
    assert!(system.contains("Composition: framing"));
    // The cached preview, shrunk by the adapter.
    assert!(image.starts_with("/cache"), "{image:?}");
    assert_eq!(*max_edge, MAX_EDGE);

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

    // Another rubric makes the stored critique stale.
    api.call(
        "update_settings",
        json!({ "critic": { "enabled": true, "server": DEFAULT_SERVER,
                            "model": "qwen3-vl:4b", "rubric": "Only the light." } }),
    )
    .unwrap();
    assert_eq!(
        api.call("get_critique", json!({ "photo_id": id })).unwrap(),
        Value::Null
    );
    api.call("critique_photo", json!({ "photo_id": id }))
        .unwrap();
    let asked = world.critic.asked.lock().unwrap();
    assert!(asked[2].2.contains("Only the light."));
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
