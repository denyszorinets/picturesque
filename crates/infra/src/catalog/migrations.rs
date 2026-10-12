//! Schema migrations, embedded in the binary and applied at startup.
//!
//! `PRAGMA user_version` records how many migrations have run. Migrations
//! are append-only: never edit one that has been merged; add a new one.

use rusqlite::Connection;

/// Ordered migrations; index + 1 is the schema version.
const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_photos", include_str!("migrations/0001_photos.sql")),
    (
        "0002_settings",
        include_str!("migrations/0002_settings.sql"),
    ),
    ("0003_imports", include_str!("migrations/0003_imports.sql")),
    (
        "0004_decisions",
        include_str!("migrations/0004_decisions.sql"),
    ),
];

/// Schema version after all migrations have run.
pub const LATEST_VERSION: i64 = MIGRATIONS.len() as i64;

/// Brings the database up to [`LATEST_VERSION`], one transaction per
/// migration. Fails if the database is newer than this build.
pub fn migrate(conn: &mut Connection) -> Result<(), String> {
    let current: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| format!("read schema version: {e}"))?;
    if current > LATEST_VERSION {
        return Err(format!(
            "catalog schema version {current} is newer than this version of Picturesque supports ({LATEST_VERSION})"
        ));
    }
    for (index, (name, sql)) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let version = index as i64 + 1;
        let tx = conn
            .transaction()
            .map_err(|e| format!("begin migration {name}: {e}"))?;
        tx.execute_batch(sql)
            .map_err(|e| format!("migration {name}: {e}"))?;
        tx.pragma_update(None, "user_version", version)
            .map_err(|e| format!("record migration {name}: {e}"))?;
        tx.commit()
            .map_err(|e| format!("commit migration {name}: {e}"))?;
    }
    Ok(())
}
