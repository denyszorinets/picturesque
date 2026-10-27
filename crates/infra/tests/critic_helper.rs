//! The built-in critic's process handling, with fake helpers (shell
//! scripts speaking the protocol), and once with the real helper and model.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use picturesque_app::ports::critic::{Critic, CriticBackend, CriticError, CriticRequest};
use picturesque_core::critic_protocol::HelperRequest;
use picturesque_infra::critic::{CriticAdapter, HelperCritic};
use serde_json::json;

/// A fake helper in `dir` that logs each start to `dir/starts` and runs
/// `body` for each request line.
fn fake(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("picturesque-critic");
    std::fs::write(
        &path,
        format!(
            "#!/bin/sh\necho start >> '{}'\nwhile IFS= read -r line; do\n{body}\ndone\n",
            dir.join("starts").display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn starts(dir: &Path) -> usize {
    std::fs::read_to_string(dir.join("starts")).map_or(0, |s| s.lines().count())
}

fn request() -> HelperRequest {
    HelperRequest {
        model: "m.gguf".into(),
        mmproj: "p.gguf".into(),
        gpu: false,
        system: String::new(),
        prompt: String::new(),
        image: String::new(),
        schema: "{}".into(),
        max_tokens: 0,
    }
}

const DONE: &str = r#"echo '{"status":"done","text":"nice","device":"CPU","seconds":0.1}'"#;

#[test]
fn keeps_one_helper_for_consecutive_critiques() {
    let dir = tempfile::tempdir().unwrap();
    let critic = HelperCritic::with_helper(Some(fake(dir.path(), DONE)), Duration::from_secs(60));
    assert_eq!(critic.problem(), None);
    assert_eq!(critic.ask(request()).unwrap(), "nice");
    assert_eq!(critic.ask(request()).unwrap(), "nice");
    assert_eq!(starts(dir.path()), 1);
    assert!(critic.is_running());
}

#[test]
fn passes_on_the_helpers_errors() {
    let dir = tempfile::tempdir().unwrap();
    let helper = fake(
        dir.path(),
        r#"echo '{"status":"failed","message":"cannot load the model: bad file"}'"#,
    );
    let critic = HelperCritic::with_helper(Some(helper), Duration::from_secs(60));
    let err = critic.ask(request()).unwrap_err();
    assert_eq!(
        err,
        CriticError::Server("cannot load the model: bad file".into())
    );
    // A failed critique is not a dead helper.
    assert!(critic.is_running());
}

#[test]
fn a_crash_fails_one_critique_and_the_next_starts_a_new_helper() {
    let dir = tempfile::tempdir().unwrap();
    // The first helper dies on its first request; later ones answer.
    let crash_once = format!(
        "if [ \"$(wc -l < '{}')\" -eq 1 ]; then exit 3; fi\n{DONE}",
        dir.path().join("starts").display()
    );
    let critic =
        HelperCritic::with_helper(Some(fake(dir.path(), &crash_once)), Duration::from_secs(60));
    let err = critic.ask(request()).unwrap_err();
    assert!(
        matches!(&err, CriticError::Server(m) if m.contains("stopped")),
        "{err:?}"
    );
    assert!(!critic.is_running());
    assert_eq!(critic.ask(request()).unwrap(), "nice");
    assert_eq!(starts(dir.path()), 2);
}

#[test]
fn stops_the_helper_when_idle() {
    let dir = tempfile::tempdir().unwrap();
    let critic =
        HelperCritic::with_helper(Some(fake(dir.path(), DONE)), Duration::from_millis(300));
    critic.ask(request()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while critic.is_running() {
        assert!(Instant::now() < deadline, "the helper was not stopped");
        std::thread::sleep(Duration::from_millis(50));
    }
    // And starts again when needed.
    assert_eq!(critic.ask(request()).unwrap(), "nice");
    assert_eq!(starts(dir.path()), 2);
}

#[test]
fn says_when_there_is_no_helper() {
    for helper in [
        None,
        Some(PathBuf::from("/does/not/exist/picturesque-critic")),
    ] {
        let critic = HelperCritic::with_helper(helper, Duration::from_secs(60));
        assert!(critic.problem().unwrap().contains("Reinstall"));
        let err = critic.ask(request()).unwrap_err();
        assert!(matches!(err, CriticError::Unreachable(_)), "{err:?}");
    }
}

/// Runs the real helper (`scripts/critic-libs.sh` puts it in
/// `target/<profile>/`) on the real model. Needs the model files:
///
///     PICTURESQUE_TEST_CRITIC_MODELS=<folder with both GGUF files> \
///         cargo test -p picturesque-infra --test critic_helper -- --ignored
#[test]
#[ignore = "needs the 2.9 GB model; see the doc comment"]
fn critiques_a_photo_with_the_real_model() {
    let dir = PathBuf::from(
        std::env::var_os("PICTURESQUE_TEST_CRITIC_MODELS").expect("PICTURESQUE_TEST_CRITIC_MODELS"),
    );
    let model = dir.join(picturesque_app::models::CRITIC_MODEL_FILE);
    let mmproj = dir.join(picturesque_app::models::CRITIC_MMPROJ_FILE);
    let photo = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/orient_1.jpg"
    ));
    let critic = CriticAdapter::new();
    assert_eq!(critic.builtin_problem(), None, "build the helper first");
    let schema = json!({
        "type": "object",
        "properties": {
            "rating": { "type": "number" },
            "summary": { "type": "string" },
        },
        "required": ["rating", "summary"],
    });
    let reply = critic
        .ask(&CriticRequest {
            backend: CriticBackend::Builtin {
                model: &model,
                mmproj: &mmproj,
                gpu: true,
            },
            system: "You are a photo editor. Reply with JSON only.",
            prompt: "Critique this photo.",
            image: photo,
            max_edge: 768,
            schema: &schema,
        })
        .unwrap();
    let critique: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert!(critique["rating"].is_number(), "{reply}");
    assert!(!critique["summary"].as_str().unwrap().is_empty(), "{reply}");
}
