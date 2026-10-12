use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use picturesque_app::api::{Api, AppContext};
use picturesque_devserver::EventBus;
use picturesque_infra::catalog::SqliteCatalog;
use picturesque_infra::fs::StdFileSystem;
use picturesque_infra::geocode::GeoNamesGeocoder;
use picturesque_infra::images::VipsDecoder;
use picturesque_infra::metadata::ExifMetadataReader;
use serde_json::{Value, json};
use tower::ServiceExt;

fn api() -> Arc<Api> {
    let catalog = Arc::new(SqliteCatalog::open_in_memory().expect("catalog"));
    Arc::new(
        Api::new(AppContext {
            settings_store: catalog.clone(),
            catalog: catalog.clone(),
            imports: catalog,
            fs: Arc::new(StdFileSystem),
            metadata: Arc::new(ExifMetadataReader),
            geocoder: Arc::new(GeoNamesGeocoder::default()),
            events: Arc::new(EventBus::default()),
            decoder: Arc::new(VipsDecoder),
            cache_dir: std::env::temp_dir().join("picturesque-devserver-test-cache"),
        })
        .expect("api"),
    )
}

async fn send(request: Request<Body>) -> (StatusCode, Vec<u8>) {
    let response = picturesque_devserver::router(api(), EventBus::default())
        .oneshot(request)
        .await
        .expect("router responds");
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    (status, body.to_vec())
}

fn post(path: &str, body: &str) -> Request<Body> {
    Request::post(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .expect("valid request")
}

fn json(body: &[u8]) -> Value {
    serde_json::from_slice(body).expect("JSON body")
}

#[tokio::test]
async fn health_returns_ok() {
    let (status, body) = send(Request::get("/api/health").body(Body::empty()).unwrap()).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, b"ok");
}

#[tokio::test]
async fn command_without_body_returns_result() {
    let (status, body) = send(post("/api/commands/app_info", "")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json(&body)["name"], "Picturesque");
}

#[tokio::test]
async fn unknown_command_returns_404_with_error() {
    let (status, body) = send(post("/api/commands/nope", "")).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json(&body)["code"], "unknown_command");
}

#[tokio::test]
async fn invalid_json_returns_400() {
    let (status, body) = send(post("/api/commands/app_info", "{not json")).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json(&body)["code"], "invalid_arguments");
}

#[tokio::test]
async fn rejected_arguments_return_400() {
    let (status, body) = send(post(
        "/api/commands/app_info",
        &json!({ "x": 1 }).to_string(),
    ))
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json(&body)["code"], "invalid_arguments");
}

#[tokio::test]
async fn settings_round_trip_over_http() {
    let (status, body) = send(post("/api/commands/get_settings", "")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json(&body)["library_root"], Value::Null);
}

#[tokio::test]
async fn invalid_settings_return_400_with_reason() {
    let (status, body) = send(post(
        "/api/commands/update_settings",
        r#"{"library_root":"/definitely/not/here"}"#,
    ))
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        json(&body)["message"]
            .as_str()
            .unwrap()
            .contains("does not exist")
    );
}

#[tokio::test]
async fn events_are_streamed_as_server_sent_events() {
    use picturesque_app::import::ImportStatus;
    use picturesque_app::import::model::ImportState;
    use picturesque_app::ports::events::{AppEvent, EventSink};

    let bus = EventBus::default();
    let response = picturesque_devserver::router(api(), bus.clone())
        .oneshot(Request::get("/api/events").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.headers()["content-type"], "text/event-stream");

    bus.emit(&AppEvent::ImportProgress(ImportStatus {
        run_id: 7,
        state: ImportState::Copying,
        phase: None,
        done: 1,
        total: 2,
        summary: None,
        error: None,
    }));
    let mut body = response.into_body();
    let frame = body.frame().await.unwrap().unwrap().into_data().unwrap();
    let text = String::from_utf8(frame.to_vec()).unwrap();

    assert!(text.starts_with("data: "), "{text}");
    let event: Value = serde_json::from_str(text.trim_start_matches("data: ").trim()).unwrap();
    assert_eq!(event["type"], "import_progress");
    assert_eq!(event["run_id"], 7);
}
