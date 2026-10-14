//! SQLite implementation of the catalog ports (ADR-0004).

mod analysis;
mod migrations;
mod photos;
mod pipeline;
mod query;
mod rejects;
mod series;
mod settings;
mod stats;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use picturesque_app::ports::catalog::CatalogError;
use rusqlite::Connection;

pub use migrations::LATEST_VERSION as SCHEMA_VERSION;

/// The catalog database. One connection behind a mutex: the catalog is a
/// local, single-user store and writes are short.
#[derive(Debug)]
pub struct SqliteCatalog {
    conn: Mutex<Connection>,
}

impl SqliteCatalog {
    /// Opens (creating if needed) the catalog at `path` and migrates it.
    pub fn open(path: &Path) -> Result<Self, CatalogError> {
        let conn =
            Connection::open(path).map_err(|e| storage(format!("open {}: {e}", path.display())))?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(storage)?;
        Self::init(conn)
    }

    /// An empty in-memory catalog, for tests.
    pub fn open_in_memory() -> Result<Self, CatalogError> {
        Self::init(Connection::open_in_memory().map_err(storage)?)
    }

    fn init(mut conn: Connection) -> Result<Self, CatalogError> {
        conn.pragma_update(None, "foreign_keys", true)
            .map_err(storage)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(storage)?;
        migrations::migrate(&mut conn).map_err(CatalogError::Storage)?;
        register_functions(&conn).map_err(storage)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> MutexGuard<'_, Connection> {
        // A panic while holding the lock leaves SQLite itself consistent
        // (transactions roll back on drop), so the poison flag is ignored.
        self.conn
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Current schema version (`PRAGMA user_version`).
    pub fn schema_version(&self) -> Result<i64, CatalogError> {
        self.conn()
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(storage)
    }
}

/// SQL functions the queries use.
///
/// `camera_label(make, model)`: the camera's display name
/// (`Camera::display_name`), so spellings of one camera (`Canon`/`EOS R5`
/// and `Canon`/`Canon EOS R5`) match and count as one; NULL without either.
fn register_functions(conn: &Connection) -> rusqlite::Result<()> {
    use rusqlite::functions::FunctionFlags;
    conn.create_scalar_function(
        "camera_label",
        2,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            let make: Option<String> = ctx.get(0)?;
            let model: Option<String> = ctx.get(1)?;
            Ok((make.is_some() || model.is_some()).then(|| {
                picturesque_core::metadata::Camera::new(
                    make.as_deref().unwrap_or_default(),
                    model.as_deref().unwrap_or_default(),
                )
                .display_name()
            }))
        },
    )
}

fn storage(e: impl std::fmt::Display) -> CatalogError {
    let message = e.to_string();
    // SQLITE_FULL's message; also what rusqlite shows for it.
    if message.contains("database or disk is full") {
        CatalogError::Full
    } else {
        CatalogError::Storage(message)
    }
}
