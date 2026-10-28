//! The critique adapter against a stub OpenAI-compatible server.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::mpsc;

use picturesque_app::ports::critic::{Critic, CriticBackend, CriticError, CriticRequest};
use picturesque_core::base64;
use picturesque_infra::critic::CriticAdapter;
use serde_json::{Value, json};

/// A server answering `requests` requests with `(status, body)` by path,
/// sending back each request's method, path and body.
fn serve(
    replies: Vec<(&'static str, u16, Value)>,
) -> (String, mpsc::Receiver<(String, String, String)>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/v1", listener.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for _ in 0..replies.len() {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let mut parts = line.split_whitespace();
            let (method, path) = (
                parts.next().unwrap().to_owned(),
                parts.next().unwrap().to_owned(),
            );
            let mut length = 0;
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header.trim().is_empty() {
                    break;
                }
                if let Some((name, value)) = header.split_once(':')
                    && name.eq_ignore_ascii_case("content-length")
                {
                    length = value.trim().parse().unwrap();
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let (_, status, reply) = replies
                .iter()
                .find(|(p, _, _)| path.ends_with(p))
                .expect("a reply for the path");
            let reply = reply.to_string();
            let mut stream = reader.into_inner();
            write!(
                stream,
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                reply.len()
            )
            .unwrap();
            tx.send((method, path, String::from_utf8(body).unwrap()))
                .unwrap();
        }
    });
    (address, rx)
}

fn fixture() -> &'static Path {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/orient_6.jpg"
    ))
}

#[test]
fn lists_the_servers_models() {
    let (server, rx) = serve(vec![(
        "/v1/models",
        200,
        json!({ "object": "list", "data": [{ "id": "qwen3-vl:4b" }, { "id": "llava:7b" }] }),
    )]);
    let models = CriticAdapter::new().models(&server).unwrap();
    assert_eq!(models, ["llava:7b", "qwen3-vl:4b"]);
    let (method, path, _) = rx.recv().unwrap();
    assert_eq!((method.as_str(), path.as_str()), ("GET", "/v1/models"));
}

#[test]
fn sends_a_small_photo_without_metadata_and_returns_the_reply() {
    let content = r#"{"rating": 7, "summary": "Nice."}"#;
    let (server, rx) = serve(vec![(
        "/v1/chat/completions",
        200,
        json!({ "choices": [{ "message": { "role": "assistant", "content": content } }] }),
    )]);
    let schema = json!({ "type": "object" });
    let reply = CriticAdapter::new()
        .ask(&CriticRequest {
            backend: CriticBackend::Server {
                url: &format!("{server}/"),
                model: "qwen3-vl:4b",
            },
            background: None,
            system: "Judge it.",
            prompt: "Critique this photo.",
            image: fixture(),
            max_edge: 64,
            schema: &schema,
        })
        .unwrap();
    assert_eq!(reply, content);

    let (method, path, body) = rx.recv().unwrap();
    assert_eq!(
        (method.as_str(), path.as_str()),
        ("POST", "/v1/chat/completions")
    );
    let body: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(body["model"], "qwen3-vl:4b");
    assert_eq!(body["stream"], false);
    assert_eq!(body["messages"][0]["content"], "Judge it.");
    assert_eq!(body["response_format"]["json_schema"]["schema"], schema);
    let url = body["messages"][1]["content"][1]["image_url"]["url"]
        .as_str()
        .unwrap();
    let jpeg = base64::decode(url.strip_prefix("data:image/jpeg;base64,").unwrap()).unwrap();
    let size = imagesize::blob_size(&jpeg).unwrap();
    assert!(size.width.max(size.height) <= 64, "{size:?}");
    assert!(
        !jpeg.windows(4).any(|w| w == b"Exif"),
        "metadata must be stripped"
    );
}

#[test]
fn reports_server_errors_and_absent_servers() {
    let (server, _rx) = serve(vec![(
        "/v1/chat/completions",
        404,
        json!({ "error": { "message": "model \"nope\" not found, try pulling it first" } }),
    )]);
    let schema = json!({});
    let request = CriticRequest {
        backend: CriticBackend::Server {
            url: &server,
            model: "nope",
        },
        background: None,
        system: "",
        prompt: "",
        image: fixture(),
        max_edge: 64,
        schema: &schema,
    };
    let err = CriticAdapter::new().ask(&request).unwrap_err();
    assert!(
        matches!(&err, CriticError::Server(m) if m.contains("not found, try pulling")),
        "{err:?}"
    );

    // Nothing listens on a port just freed.
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let err = CriticAdapter::new()
        .models(&format!("http://127.0.0.1:{port}/v1"))
        .unwrap_err();
    assert!(matches!(err, CriticError::Unreachable(_)), "{err:?}");

    let missing = CriticRequest {
        image: Path::new("/does/not/exist.jpg"),
        ..request
    };
    let err = CriticAdapter::new().ask(&missing).unwrap_err();
    assert!(matches!(err, CriticError::Image(_)), "{err:?}");
}
