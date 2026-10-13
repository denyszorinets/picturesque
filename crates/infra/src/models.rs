//! Downloaded AI models on disk: `<root>/<model id>/v<version>/<file>`.
//!
//! Files are downloaded over HTTPS to a temporary name next to their final
//! place while being hashed, and renamed into place only when their size
//! and SHA-256 match the manifest, so a stored file is always complete.

use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use picturesque_app::ports::models::{FetchError, ModelFile, ModelSpec, ModelStore};
use sha2::{Digest, Sha256};

/// Models stored under a folder and downloaded with HTTP(S).
#[derive(Debug)]
pub struct HttpModelStore {
    root: PathBuf,
    agent: ureq::Agent,
}

impl HttpModelStore {
    pub fn new(root: PathBuf) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(20)))
            // Per read, so slow but steady downloads of large models work.
            .timeout_recv_body(Some(Duration::from_secs(60)))
            .user_agent(concat!("picturesque/", env!("CARGO_PKG_VERSION")))
            .build()
            .into();
        Self { root, agent }
    }

    fn version_dir(&self, model: &ModelSpec) -> PathBuf {
        self.root
            .join(&model.id)
            .join(format!("v{}", model.version))
    }
}

/// `name` as a relative path that stays inside the model's folder.
fn relative(name: &str) -> Option<PathBuf> {
    let path = Path::new(name);
    path.components()
        .all(|c| matches!(c, Component::Normal(_)))
        .then(|| path.to_path_buf())
}

fn io(e: impl std::fmt::Display) -> FetchError {
    FetchError::Io(e.to_string())
}

impl ModelStore for HttpModelStore {
    fn path(&self, model: &ModelSpec, file: &ModelFile) -> PathBuf {
        // Unsafe names never match a stored file (see `fetch`).
        let name = relative(&file.name).unwrap_or_else(|| PathBuf::from("invalid-name"));
        self.version_dir(model).join(name)
    }

    fn has(&self, model: &ModelSpec, file: &ModelFile) -> bool {
        relative(&file.name).is_some()
            && fs::metadata(self.path(model, file))
                .is_ok_and(|m| m.is_file() && m.len() == file.size)
    }

    fn fetch(
        &self,
        model: &ModelSpec,
        file: &ModelFile,
        progress: &mut dyn FnMut(u64) -> bool,
    ) -> Result<(), FetchError> {
        if relative(&file.name).is_none() || relative(&model.id).is_none() {
            return Err(FetchError::Io(format!("unsafe file name `{}`", file.name)));
        }
        let dest = self.path(model, file);
        let dir = dest.parent().expect("files are inside a folder");
        fs::create_dir_all(dir).map_err(io)?;
        let part = dest.with_file_name(format!(
            ".{}.part",
            dest.file_name().expect("file name").to_string_lossy()
        ));

        let result = (|| {
            let response = self
                .agent
                .get(&file.url)
                .call()
                .map_err(|e| FetchError::Network(e.to_string()))?;
            let mut body = response.into_body();
            let mut reader = body.as_reader();
            let mut out = fs::File::create(&part).map_err(io)?;
            let mut hasher = Sha256::new();
            let mut received = 0u64;
            let mut buf = vec![0u8; 256 * 1024];
            loop {
                let n = reader
                    .read(&mut buf)
                    .map_err(|e| FetchError::Network(e.to_string()))?;
                if n == 0 {
                    break;
                }
                received += n as u64;
                if received > file.size {
                    return Err(FetchError::Corrupt(format!(
                        "{} is larger than the expected {} bytes",
                        file.name, file.size
                    )));
                }
                hasher.update(&buf[..n]);
                out.write_all(&buf[..n]).map_err(io)?;
                if !progress(received) {
                    return Err(FetchError::Cancelled);
                }
            }
            out.sync_all().map_err(io)?;
            if received != file.size {
                return Err(FetchError::Corrupt(format!(
                    "{} has {received} bytes, expected {}",
                    file.name, file.size
                )));
            }
            let digest = format!("{:x}", hasher.finalize());
            if !digest.eq_ignore_ascii_case(&file.sha256) {
                return Err(FetchError::Corrupt(format!(
                    "{} has checksum {digest}, expected {}",
                    file.name, file.sha256
                )));
            }
            fs::rename(&part, &dest).map_err(io)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&part);
        }
        result
    }

    fn read(&self, model: &ModelSpec, file: &ModelFile) -> Result<Vec<u8>, String> {
        if !self.has(model, file) {
            return Err(format!("{} is not downloaded", file.name));
        }
        fs::read(self.path(model, file)).map_err(|e| e.to_string())
    }

    fn prune(&self, model: &ModelSpec) -> Result<(), String> {
        let keep = format!("v{}", model.version);
        let Ok(entries) = fs::read_dir(self.root.join(&model.id)) else {
            return Ok(());
        };
        for entry in entries.flatten() {
            if entry.file_name() != keep.as_str() {
                fs::remove_dir_all(entry.path()).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    fn remove(&self, model: &ModelSpec) -> Result<(), String> {
        match fs::remove_dir_all(self.root.join(&model.id)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }
}
