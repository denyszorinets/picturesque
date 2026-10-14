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
    api_with(
        Arc::new(SqliteCatalog::open_in_memory().expect("catalog")),
        std::env::temp_dir().join("picturesque-devserver-test-cache"),
    )
}

fn api_with(catalog: Arc<SqliteCatalog>, cache_dir: std::path::PathBuf) -> Arc<Api> {
    Arc::new(
        Api::new(AppContext {
            pipeline_store: catalog.clone(),
            settings_store: catalog.clone(),
            catalog: catalog.clone(),
            imports: catalog,
            fs: Arc::new(StdFileSystem),
            metadata: Arc::new(ExifMetadataReader),
            geocoder: Arc::new(GeoNamesGeocoder::default()),
            events: Arc::new(EventBus::default()),
            decoder: Arc::new(VipsDecoder),
            sidecars: Arc::new(picturesque_infra::xmp::XmpSidecars),
            pixels: Arc::new(picturesque_infra::images::VipsPixels),
            analyzers: Vec::new(),
            inference: Arc::new(picturesque_infra::inference::OrtEngine::new(1)),
            models: Vec::new(),
            model_store: Arc::new(picturesque_infra::models::HttpModelStore::new(
                cache_dir.join("models"),
            )),
            cache_dir,
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

mod images {
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use picturesque_app::ports::catalog::PhotoCatalog;
    use picturesque_app::ports::fs::FileSystem;
    use picturesque_app::ports::settings::SettingsStore;
    use picturesque_app::settings::Settings;
    use picturesque_core::media::MediaFormat;
    use picturesque_core::metadata::CaptureMetadata;
    use picturesque_core::photo::{Asset, NewPhoto, PhotoAssets};
    use picturesque_devserver::EventBus;
    use picturesque_infra::catalog::SqliteCatalog;
    use picturesque_infra::fs::StdFileSystem;
    use tower::ServiceExt;

    /// A library with one 40×30 JPEG photo; returns the router and photo id.
    fn library(dir: &std::path::Path) -> (axum::Router, i64) {
        let lib = dir.join("lib");
        std::fs::create_dir_all(&lib).unwrap();
        image::DynamicImage::new_rgb8(40, 30)
            .save(lib.join("a.jpg"))
            .unwrap();
        let catalog = Arc::new(SqliteCatalog::open_in_memory().unwrap());
        catalog
            .save_settings(&Settings {
                library_root: Some(lib.clone()),
                ..Default::default()
            })
            .unwrap();
        let (hash, size) = StdFileSystem.hash_file(&lib.join("a.jpg")).unwrap();
        let id = catalog
            .insert_photo(&NewPhoto {
                metadata: CaptureMetadata::default(),
                assets: PhotoAssets::new(vec![Asset {
                    format: MediaFormat::Jpeg,
                    path: "a.jpg".into(),
                    size,
                    hash,
                }])
                .unwrap(),
            })
            .unwrap();
        let api = super::api_with(catalog, dir.join("cache"));
        (
            picturesque_devserver::router(api, EventBus::default()),
            id.0,
        )
    }

    #[tokio::test]
    async fn serves_cached_jpegs_with_etag_revalidation() {
        let dir = tempfile::tempdir().unwrap();
        let (router, id) = library(dir.path());

        let response = router
            .clone()
            .oneshot(
                Request::get(format!("/api/images/grid/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/jpeg");
        let etag = response.headers()[header::ETAG]
            .to_str()
            .unwrap()
            .to_owned();
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        assert_eq!(image::load_from_memory(&body).unwrap().width(), 40);

        let again = router
            .oneshot(
                Request::get(format!("/api/images/grid/{id}"))
                    .header(header::IF_NONE_MATCH, &etag)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(again.status(), StatusCode::NOT_MODIFIED);
    }

    #[tokio::test]
    async fn rejects_unknown_photos_and_sizes() {
        let dir = tempfile::tempdir().unwrap();
        let (router, _) = library(dir.path());
        let status = |uri: &'static str| {
            let router = router.clone();
            async move {
                router
                    .oneshot(Request::get(uri).body(Body::empty()).unwrap())
                    .await
                    .unwrap()
                    .status()
            }
        };

        assert_eq!(status("/api/images/grid/999").await, StatusCode::NOT_FOUND);
        assert_eq!(status("/api/images/huge/1").await, StatusCode::BAD_REQUEST);
    }
}
