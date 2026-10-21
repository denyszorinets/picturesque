//! Folder sessions (ADR-0013): culling one folder in place, with its own
//! catalog, or the library. [`Host`] owns the current session's [`Api`]
//! and replaces it when another folder (or the library) is opened.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use serde::Deserialize;
use serde_json::Value;

use crate::api::{Api, ApiError, ApiErrorCode, AppContext};
use crate::ports::catalog::CatalogError;
use crate::ports::events::{AppEvent, EventSink};
use crate::ports::settings::SettingsStore;
use crate::settings::{RecentFolder, Settings};

/// Folders not opened for this long are forgotten and their catalogs
/// deleted (30 days).
pub const PRUNE_AFTER_SECONDS: i64 = 30 * 24 * 60 * 60;

/// How many folders the recent list keeps.
const RECENT_LIMIT: usize = 10;

/// Builds what a session runs on (implemented by the entry points).
pub trait ContextFactory: Send + Sync {
    /// The context for the library (`None`) or for culling `folder` in
    /// place, with that folder's own catalog. Its settings store is the
    /// global one; [`Host`] adapts it for folders.
    fn context(&self, folder: Option<&Path>) -> Result<AppContext, String>;

    /// Deletes the catalogs of folder sessions other than `keep`.
    fn prune(&self, keep: &[PathBuf]);
}

/// Settings as a folder session sees them: the folder is the root. Saving
/// keeps the library folder and the folder list as they are.
struct SessionSettings {
    home: Arc<dyn SettingsStore>,
    folder: PathBuf,
}

impl SettingsStore for SessionSettings {
    fn load_settings(&self) -> Result<Settings, CatalogError> {
        let mut settings = self.home.load_settings()?;
        settings.library_root = Some(self.folder.clone());
        settings.open_folder = Some(self.folder.clone());
        Ok(settings)
    }

    fn save_settings(&self, settings: &Settings) -> Result<(), CatalogError> {
        let stored = self.home.load_settings()?;
        self.home.save_settings(&Settings {
            library_root: stored.library_root,
            open_folder: stored.open_folder,
            recent_folders: stored.recent_folders,
            ..settings.clone()
        })
    }
}

/// Passes events on until the session is replaced.
struct GatedEvents {
    inner: Arc<dyn EventSink>,
    open: AtomicBool,
}

impl EventSink for GatedEvents {
    fn emit(&self, event: &AppEvent) {
        if self.open.load(Ordering::Acquire) {
            self.inner.emit(event);
        }
    }
}

struct Session {
    api: Arc<Api>,
    events: Arc<GatedEvents>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenFolderArgs {
    path: PathBuf,
}

/// Runs the library or one folder session at a time.
pub struct Host {
    factory: Arc<dyn ContextFactory>,
    home: Arc<dyn SettingsStore>,
    current: RwLock<Session>,
    /// One switch at a time.
    switching: Mutex<()>,
    now: Box<dyn Fn() -> i64 + Send + Sync>,
}

impl std::fmt::Debug for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Host").finish_non_exhaustive()
    }
}

fn invalid(message: impl Into<String>) -> ApiError {
    ApiError::new(ApiErrorCode::InvalidArguments, message)
}

fn storage(e: CatalogError) -> ApiError {
    ApiError::internal(e.to_string())
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

impl Host {
    /// Forgets folders not opened for a while, reopens the folder that was
    /// open last (or the library, if it is gone) and starts its background
    /// work.
    pub fn start(
        factory: Arc<dyn ContextFactory>,
        home: Arc<dyn SettingsStore>,
    ) -> Result<Self, ApiError> {
        Self::start_at(factory, home, unix_now)
    }

    /// [`Host::start`] with a clock, for tests.
    pub fn start_at(
        factory: Arc<dyn ContextFactory>,
        home: Arc<dyn SettingsStore>,
        now: impl Fn() -> i64 + Send + Sync + 'static,
    ) -> Result<Self, ApiError> {
        let mut settings = home.load_settings().map_err(storage)?;
        let cutoff = now() - PRUNE_AFTER_SECONDS;
        settings.recent_folders.retain(|f| f.opened_at >= cutoff);
        home.save_settings(&settings).map_err(storage)?;
        let keep: Vec<PathBuf> = settings
            .recent_folders
            .iter()
            .map(|f| f.path.clone())
            .collect();
        factory.prune(&keep);

        let session = match settings.open_folder.clone() {
            Some(folder) => match Self::build(&factory, &home, Some(&folder)) {
                Ok(session) => session,
                // The folder is gone (a card taken out): back to the library.
                Err(_) => {
                    let mut settings = home.load_settings().map_err(storage)?;
                    settings.open_folder = None;
                    home.save_settings(&settings).map_err(storage)?;
                    Self::build(&factory, &home, None)?
                }
            },
            None => Self::build(&factory, &home, None)?,
        };
        session.api.start_background_work();
        Ok(Self {
            factory,
            home,
            current: RwLock::new(session),
            switching: Mutex::new(()),
            now: Box::new(now),
        })
    }

    /// Builds a session's `Api` without starting its background work.
    fn build(
        factory: &Arc<dyn ContextFactory>,
        home: &Arc<dyn SettingsStore>,
        folder: Option<&Path>,
    ) -> Result<Session, ApiError> {
        let mut ctx = factory.context(folder).map_err(ApiError::internal)?;
        if let Some(folder) = folder {
            ctx.fs.list_dir(folder).map_err(|reason| {
                invalid(format!("Cannot read `{}`: {reason}.", folder.display()))
            })?;
            ctx.settings_store = Arc::new(SessionSettings {
                home: Arc::clone(home),
                folder: folder.to_path_buf(),
            });
        }
        let events = Arc::new(GatedEvents {
            inner: Arc::clone(&ctx.events),
            open: AtomicBool::new(true),
        });
        ctx.events = Arc::clone(&events) as Arc<dyn EventSink>;
        let api = Arc::new(Api::new(ctx)?);
        if let Some(folder) = folder {
            let sources = api.call("list_import_sources", Value::Null)?;
            if sources.as_array().is_none_or(Vec::is_empty) {
                api.call(
                    "add_import_source",
                    serde_json::json!({ "path": folder, "watch": true }),
                )?;
            }
        }
        Ok(Session { api, events })
    }

    /// The current session's `Api`.
    pub fn api(&self) -> Arc<Api> {
        Arc::clone(&self.current.read().expect("session lock").api)
    }

    /// Runs a command: opening a folder or the library here, everything
    /// else in the current session.
    pub fn call(&self, command: &str, args: Value) -> Result<Value, ApiError> {
        match command {
            "open_folder" => {
                let OpenFolderArgs { path } = serde_json::from_value(args)
                    .map_err(|e| invalid(format!("invalid arguments for `open_folder`: {e}")))?;
                self.open(Some(&path))
            }
            "open_library" => {
                if !args.is_null() {
                    return Err(invalid("`open_library` takes no arguments"));
                }
                self.open(None)
            }
            _ => self.api().call(command, args),
        }
    }

    fn check_folder(&self, folder: &Path) -> Result<(), ApiError> {
        if !crate::ports::fs::is_absolute(folder) {
            return Err(invalid(format!(
                "The folder must be an absolute path, got `{}`.",
                folder.display()
            )));
        }
        if folder.parent().is_none() {
            return Err(invalid("Choose a folder, not a whole disk."));
        }
        let library = self.home.load_settings().map_err(storage)?.library_root;
        if let Some(library) = library
            && (folder.starts_with(&library) || library.starts_with(folder))
        {
            return Err(invalid(format!(
                "`{}` holds or is inside your library; its photos are culled in the library.",
                folder.display()
            )));
        }
        Ok(())
    }

    /// Replaces the current session with `folder`'s (or the library's) and
    /// returns its settings.
    fn open(&self, folder: Option<&Path>) -> Result<Value, ApiError> {
        let _switching = self.switching.lock().expect("switch lock");
        if let Some(folder) = folder {
            self.check_folder(folder)?;
        }
        let session = Self::build(&self.factory, &self.home, folder)?;

        let mut settings = self.home.load_settings().map_err(storage)?;
        settings.open_folder = folder.map(Path::to_path_buf);
        if let Some(folder) = folder {
            settings.recent_folders.retain(|f| f.path != folder);
            settings.recent_folders.insert(
                0,
                RecentFolder {
                    path: folder.to_path_buf(),
                    opened_at: (self.now)(),
                },
            );
            settings.recent_folders.truncate(RECENT_LIMIT);
        }
        self.home.save_settings(&settings).map_err(storage)?;

        let old = std::mem::replace(&mut *self.current.write().expect("session lock"), session);
        old.events.open.store(false, Ordering::Release);
        old.api.shutdown();
        let api = self.api();
        api.start_background_work();
        api.call("get_settings", Value::Null)
    }

    /// Writes pending sidecars and stops background work (on exit).
    pub fn shutdown(&self) {
        let current = self.current.read().expect("session lock");
        current.api.shutdown();
    }
}

#[cfg(test)]
mod tests;
