//! Counts for the library statistics.

use std::collections::HashMap;

use picturesque_app::library::{CameraKey, Counted};
use picturesque_app::ports::catalog::CatalogError;
use picturesque_app::stats::{CameraUsage, GearUsage, KeeperCounts, LensUsage};
use rusqlite::{Connection, Row};

use super::query::{AUTO_PICKED_SQL, FROM};
use super::{SqliteCatalog, storage};

const CAMERA_SQL: &str = "p.camera_make IS NOT NULL OR p.camera_model IS NOT NULL";

/// `count(*)` and decision counts of the grouped photos, as columns
/// `n`, `picks`, `auto`, `rejects`.
fn counts_sql() -> String {
    format!(
        "count(*) AS n,
         coalesce(sum(coalesce(d.flag, 'unflagged') = 'pick'), 0) AS picks,
         coalesce(sum({AUTO_PICKED_SQL}), 0) AS auto,
         coalesce(sum(coalesce(d.flag, 'unflagged') = 'reject'), 0) AS rejects"
    )
}

fn counts(row: &Row<'_>) -> rusqlite::Result<KeeperCounts> {
    let get = |name: &str| row.get::<_, i64>(name).map(|n| n as usize);
    Ok(KeeperCounts {
        photos: get("n")?,
        picks: get("picks")?,
        auto_picked: get("auto")?,
        rejects: get("rejects")?,
    })
}

fn rows<T>(
    conn: &Connection,
    sql: &str,
    map: impl Fn(&Row<'_>) -> rusqlite::Result<T>,
) -> Result<Vec<T>, CatalogError> {
    let mut stmt = conn.prepare(sql).map_err(storage)?;
    stmt.query_map([], |r| map(r))
        .map_err(storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage)
}

fn camera_key(row: &Row<'_>) -> rusqlite::Result<CameraKey> {
    Ok(CameraKey {
        make: row.get("make")?,
        model: row.get("model")?,
    })
}

fn gear_usage(conn: &Connection) -> Result<GearUsage, CatalogError> {
    let counted = counts_sql();
    let all = conn
        .query_row(&format!("SELECT {counted} {FROM}"), [], counts)
        .map_err(storage)?;

    let mut focal: HashMap<CameraKey, Vec<Counted<u32>>> = HashMap::new();
    for (key, mm, n) in rows(
        conn,
        &format!(
            "SELECT coalesce(p.camera_make, '') AS make, coalesce(p.camera_model, '') AS model,
                    CAST(round(p.focal_length_mm) AS INTEGER) AS mm, count(*) AS n
             FROM photo p WHERE ({CAMERA_SQL}) AND p.focal_length_mm >= 0.5
             GROUP BY make, model, mm"
        ),
        |r| {
            Ok((
                camera_key(r)?,
                r.get::<_, i64>("mm")?,
                r.get::<_, i64>("n")?,
            ))
        },
    )? {
        focal.entry(key).or_default().push(Counted {
            value: mm as u32,
            count: n as usize,
        });
    }

    let cameras = rows(
        conn,
        &format!(
            "SELECT coalesce(p.camera_make, '') AS make, coalesce(p.camera_model, '') AS model, {counted}
             {FROM} WHERE {CAMERA_SQL} GROUP BY make, model"
        ),
        |r| Ok((camera_key(r)?, counts(r)?)),
    )?
    .into_iter()
    .map(|(key, counts)| CameraUsage {
        focal_lengths: focal.remove(&key).unwrap_or_default(),
        key,
        counts,
    })
    .collect();

    let lenses = rows(
        conn,
        &format!(
            "SELECT p.lens AS lens, {counted} {FROM} WHERE p.lens IS NOT NULL GROUP BY p.lens"
        ),
        |r| {
            Ok(LensUsage {
                lens: r.get("lens")?,
                counts: counts(r)?,
            })
        },
    )?;

    Ok(GearUsage {
        all,
        cameras,
        lenses,
    })
}

impl SqliteCatalog {
    pub(super) fn load_gear_usage(&self) -> Result<GearUsage, CatalogError> {
        gear_usage(&self.conn())
    }
}
