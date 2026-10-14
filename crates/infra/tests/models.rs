//! Model downloads against a local HTTP server.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::Arc;

use picturesque_app::ports::models::{FetchError, ModelFile, ModelSpec, ModelStore};
use picturesque_infra::models::HttpModelStore;
use sha2::{Digest, Sha256};

/// Serves `files` (path → body) until the test ends; returns the base URL.
fn serve(files: HashMap<&'static str, Vec<u8>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let files = Arc::new(files);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let files = Arc::clone(&files);
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request = String::new();
                reader.read_line(&mut request).unwrap();
                let path = request.split_whitespace().nth(1).unwrap_or("/").to_owned();
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap() <= 2 {
                        break;
                    }
                }
                let response = match files.get(path.as_str()) {
                    Some(body) => {
                        let mut r = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        )
                        .into_bytes();
                        r.extend_from_slice(body);
                        r
                    }
                    None => {
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            .to_vec()
                    }
                };
                let _ = stream.write_all(&response);
            });
        }
    });
    base
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn model(version: u32, files: Vec<ModelFile>) -> ModelSpec {
    ModelSpec {
        id: "clip".into(),
        name: "CLIP".into(),
        purpose: "Search".into(),
        version,
        license: "MIT".into(),
        license_url: String::new(),
        homepage: String::new(),
        files,
    }
}

fn file(base: &str, name: &str, path: &str, body: &[u8]) -> ModelFile {
    ModelFile {
        name: name.into(),
        url: format!("{base}{path}"),
        sha256: sha256(body),
        size: body.len() as u64,
    }
}

fn leftovers(dir: &std::path::Path) -> Vec<String> {
    walk(dir)
        .into_iter()
        .filter(|p| p.ends_with(".part"))
        .collect()
}

fn walk(dir: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            if e.path().is_dir() {
                out.extend(walk(&e.path()));
            } else {
                out.push(e.path().display().to_string());
            }
        }
    }
    out
}

const WEIGHTS: &[u8] = &[7u8; 700_000];

#[test]
fn downloads_and_verifies_files() {
    let base = serve(HashMap::from([("/w.onnx", WEIGHTS.to_vec())]));
    let dir = tempfile::tempdir().unwrap();
    let store = HttpModelStore::new(dir.path().to_path_buf());
    let f = file(&base, "visual/model.onnx", "/w.onnx", WEIGHTS);
    let m = model(1, vec![f.clone()]);
    assert!(!store.has(&m, &f));

    let mut seen = Vec::new();
    store
        .fetch(&m, &f, &mut |n| {
            seen.push(n);
            true
        })
        .unwrap();
    assert!(store.has(&m, &f));
    assert_eq!(
        store.path(&m, &f),
        dir.path().join("clip/v1/visual/model.onnx")
    );
    assert_eq!(std::fs::read(store.path(&m, &f)).unwrap(), WEIGHTS);
    assert_eq!(seen.last(), Some(&(WEIGHTS.len() as u64)));
    assert!(seen.windows(2).all(|w| w[0] < w[1]));
    assert!(leftovers(dir.path()).is_empty());
}

#[test]
fn corrupt_or_failed_downloads_leave_nothing() {
    let base = serve(HashMap::from([
        ("/w.onnx", WEIGHTS.to_vec()),
        ("/short.onnx", WEIGHTS[..1000].to_vec()),
    ]));
    let dir = tempfile::tempdir().unwrap();
    let store = HttpModelStore::new(dir.path().to_path_buf());

    // Right size, wrong checksum.
    let mut wrong = file(&base, "a.onnx", "/w.onnx", WEIGHTS);
    wrong.sha256 = sha256(b"something else");
    let m = model(1, vec![wrong.clone()]);
    let err = store.fetch(&m, &wrong, &mut |_| true).unwrap_err();
    assert!(
        matches!(err, FetchError::Corrupt(ref e) if e.contains("checksum")),
        "{err}"
    );
    assert!(!store.has(&m, &wrong));

    // Truncated and oversized downloads.
    let mut short = file(&base, "b.onnx", "/short.onnx", WEIGHTS);
    short.sha256 = sha256(WEIGHTS);
    assert!(matches!(
        store.fetch(&m, &short, &mut |_| true),
        Err(FetchError::Corrupt(_))
    ));
    let mut long = file(&base, "c.onnx", "/w.onnx", &WEIGHTS[..10]);
    long.size = 10;
    assert!(matches!(
        store.fetch(&m, &long, &mut |_| true),
        Err(FetchError::Corrupt(_))
    ));

    // Missing on the server; cancelled half-way.
    let missing = file(&base, "d.onnx", "/nope", b"x");
    assert!(matches!(
        store.fetch(&m, &missing, &mut |_| true),
        Err(FetchError::Network(_))
    ));
    let cancelled = file(&base, "e.onnx", "/w.onnx", WEIGHTS);
    assert_eq!(
        store.fetch(&m, &cancelled, &mut |n| n < 300_000),
        Err(FetchError::Cancelled)
    );

    assert!(walk(dir.path()).is_empty(), "{:?}", walk(dir.path()));
}

#[test]
fn prunes_old_versions_and_removes_models() {
    let base = serve(HashMap::from([("/w.onnx", WEIGHTS.to_vec())]));
    let dir = tempfile::tempdir().unwrap();
    let store = HttpModelStore::new(dir.path().to_path_buf());
    let f = file(&base, "model.onnx", "/w.onnx", WEIGHTS);
    let v1 = model(1, vec![f.clone()]);
    let v2 = model(2, vec![f.clone()]);
    store.fetch(&v1, &f, &mut |_| true).unwrap();
    store.fetch(&v2, &f, &mut |_| true).unwrap();
    store.prune(&v2).unwrap();
    assert!(!store.has(&v1, &f));
    assert!(store.has(&v2, &f));

    store.remove(&v2).unwrap();
    assert!(!dir.path().join("clip").exists());
    // Removing again (or pruning a model never downloaded) is fine.
    store.remove(&v2).unwrap();
    store.prune(&v2).unwrap();
}

#[test]
fn file_names_cannot_escape_the_model_folder() {
    let dir = tempfile::tempdir().unwrap();
    let store = HttpModelStore::new(dir.path().join("models"));
    for name in ["../evil.onnx", "/etc/passwd", "a/../../b"] {
        let f = ModelFile {
            name: name.into(),
            url: "http://127.0.0.1:9/x".into(),
            sha256: String::new(),
            size: 1,
        };
        let m = model(1, vec![f.clone()]);
        assert!(matches!(
            store.fetch(&m, &f, &mut |_| true),
            Err(FetchError::Io(_))
        ));
        assert!(!store.has(&m, &f));
        assert!(store.path(&m, &f).starts_with(dir.path().join("models")));
    }
}
