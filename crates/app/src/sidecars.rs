//! Keeping XMP sidecars in step with decisions (ADR-0009).
//!
//! Changing a decision marks it dirty in the catalog. A background worker
//! writes dirty decisions to sidecars shortly after changes stop (so rapid
//! culling writes each file once), and at startup.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use picturesque_core::grouping::base_name;
use picturesque_core::photo::Photo;

use crate::ports::catalog::PhotoCatalog;
use crate::ports::settings::SettingsStore;
use crate::ports::sidecars::SidecarStore;
use crate::settings::SidecarNaming;

/// How long to wait after the last change before writing.
pub const DEBOUNCE: Duration = Duration::from_millis(750);

/// Outcome of writing dirty decisions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlushReport {
    pub written: usize,
    /// Photos whose sidecar could not be written, with the reason.
    pub failed: Vec<(i64, String)>,
}

pub struct SidecarService {
    catalog: Arc<dyn PhotoCatalog>,
    settings: Arc<dyn SettingsStore>,
    store: Arc<dyn SidecarStore>,
    notify: Mutex<Option<Sender<()>>>,
}

impl std::fmt::Debug for SidecarService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SidecarService").finish_non_exhaustive()
    }
}

/// The sidecar to write for a photo: its existing sidecar, or a new one
/// named after its primary (RAW) file.
pub fn sidecar_path(photo: &Photo, naming: SidecarNaming) -> PathBuf {
    if let Some(existing) = photo.assets.sidecars().first() {
        return existing.path.clone();
    }
    let primary = &photo.assets.primary().path;
    let name = primary
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let file = match naming {
        SidecarNaming::FileExtension => format!("{name}.xmp"),
        SidecarNaming::BaseName => format!("{}.xmp", base_name(&name).unwrap_or(&name)),
    };
    primary.with_file_name(file)
}

impl SidecarService {
    pub fn new(
        catalog: Arc<dyn PhotoCatalog>,
        settings: Arc<dyn SettingsStore>,
        store: Arc<dyn SidecarStore>,
    ) -> Self {
        Self {
            catalog,
            settings,
            store,
            notify: Mutex::new(None),
        }
    }

    /// Writes every dirty decision now.
    pub fn flush(&self) -> FlushReport {
        let mut report = FlushReport::default();
        let (Ok(dirty), Ok(settings)) = (
            self.catalog.dirty_decisions(),
            self.settings.load_settings(),
        ) else {
            return report;
        };
        let Some(root) = settings.library_root else {
            return report;
        };
        for item in dirty {
            match self.write_one(&root, item.id, item.decision, settings.sidecar_naming) {
                Ok(()) => {
                    let _ = self.catalog.mark_sidecar_written(item.id, &item.token);
                    report.written += 1;
                }
                Err(reason) => report.failed.push((item.id.0, reason)),
            }
        }
        report
    }

    fn write_one(
        &self,
        root: &Path,
        id: picturesque_core::photo::PhotoId,
        decision: picturesque_core::decision::Decision,
        naming: SidecarNaming,
    ) -> Result<(), String> {
        let photo = self
            .catalog
            .photo(id)
            .map_err(|e| e.to_string())?
            .ok_or("photo no longer exists")?;
        let relative = sidecar_path(&photo, naming);
        let (hash, size) = self.store.write(&root.join(&relative), decision)?;
        self.catalog
            .upsert_sidecar(id, &relative, size, &hash)
            .map_err(|e| e.to_string())
    }

    /// Asks the background worker to write soon. Does nothing until
    /// [`SidecarService::start`] has been called.
    pub fn notify(&self) {
        if let Some(tx) = self.notify.lock().expect("notify lock").as_ref() {
            let _ = tx.send(());
        }
    }

    /// Starts the background worker; it flushes once immediately (to catch
    /// up after a restart), then after each burst of changes.
    pub fn start(self: &Arc<Self>) {
        let (tx, rx) = channel();
        *self.notify.lock().expect("notify lock") = Some(tx);
        let service = Arc::clone(self);
        std::thread::spawn(move || {
            service.flush();
            worker(&service, &rx);
        });
    }
}

fn worker(service: &SidecarService, rx: &Receiver<()>) {
    while rx.recv().is_ok() {
        // Wait until changes have stopped for DEBOUNCE.
        loop {
            match rx.recv_timeout(DEBOUNCE) {
                Ok(()) => continue,
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        service.flush();
    }
}

#[cfg(test)]
mod tests;
