//! Stored analysis results on SQLite.

use picturesque_app::ports::catalog::{AnalyzerId, CatalogError};
use picturesque_core::photo::PhotoId;
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter};

use super::{SqliteCatalog, storage};

/// `p.id` lacks a current result from at least one of `analyzers`.
fn pending_clause(analyzers: &[AnalyzerId]) -> (String, Vec<Value>) {
    if analyzers.is_empty() {
        return ("0".into(), Vec::new());
    }
    let mut values = Vec::new();
    let clauses: Vec<&str> = analyzers
        .iter()
        .map(|a| {
            values.push(Value::Text(a.name.to_owned()));
            values.push(Value::Integer(a.version.into()));
            "NOT EXISTS (SELECT 1 FROM analysis a WHERE a.photo_id = p.id AND a.analyzer = ? AND a.version = ? AND a.stale = 0)"
        })
        .collect();
    (format!("({})", clauses.join(" OR ")), values)
}

impl SqliteCatalog {
    pub(super) fn load_pending_analysis(
        &self,
        analyzers: &[AnalyzerId],
        limit: usize,
    ) -> Result<Vec<PhotoId>, CatalogError> {
        let (clause, mut values) = pending_clause(analyzers);
        values.push(Value::Integer(i64::try_from(limit).unwrap_or(i64::MAX)));
        let conn = self.conn();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT p.id FROM photo p WHERE {clause} ORDER BY p.captured_local IS NULL, p.captured_local, p.id LIMIT ?"
            ))
            .map_err(storage)?;
        stmt.query_map(params_from_iter(values), |row| row.get(0).map(PhotoId))
            .map_err(storage)?
            .collect::<Result<_, _>>()
            .map_err(storage)
    }

    pub(super) fn count_analysis(
        &self,
        analyzers: &[AnalyzerId],
    ) -> Result<(usize, usize), CatalogError> {
        let (clause, values) = pending_clause(analyzers);
        self.conn()
            .query_row(
                &format!("SELECT count(*) FILTER (WHERE {clause}), count(*) FROM photo p"),
                params_from_iter(values),
                |row| {
                    Ok((
                        row.get::<_, i64>(0)? as usize,
                        row.get::<_, i64>(1)? as usize,
                    ))
                },
            )
            .map_err(storage)
    }

    pub(super) fn current_analyzers(
        &self,
        id: PhotoId,
        analyzers: &[AnalyzerId],
    ) -> Result<Vec<AnalyzerId>, CatalogError> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare_cached(
                "SELECT version FROM analysis WHERE photo_id = ?1 AND analyzer = ?2 AND stale = 0",
            )
            .map_err(storage)?;
        let mut done = Vec::new();
        for analyzer in analyzers {
            let version: Option<i64> = stmt
                .query_row(params![id.0, analyzer.name], |row| row.get(0))
                .map(Some)
                .or_else(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => Ok(None),
                    e => Err(e),
                })
                .map_err(storage)?;
            if version == Some(analyzer.version.into()) {
                done.push(*analyzer);
            }
        }
        Ok(done)
    }

    pub(super) fn save_analysis(
        &self,
        id: PhotoId,
        analyzer: AnalyzerId,
        result: &serde_json::Value,
    ) -> Result<(), CatalogError> {
        self.conn()
            .execute(
                "INSERT INTO analysis (photo_id, analyzer, version, result) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (photo_id, analyzer) DO UPDATE SET
                     version = excluded.version, result = excluded.result,
                     analyzed_at = excluded.analyzed_at, stale = 0",
                params![id.0, analyzer.name, analyzer.version, result.to_string()],
            )
            .map_err(storage)?;
        Ok(())
    }

    pub(super) fn mark_stale(&self, ids: Option<&[PhotoId]>) -> Result<usize, CatalogError> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(storage)?;
        let photos = match ids {
            None => {
                tx.execute("UPDATE analysis SET stale = 1", [])
                    .map_err(storage)?;
                tx.query_row("SELECT count(*) FROM photo", [], |r| r.get::<_, i64>(0))
                    .map_err(storage)?
            }
            Some(ids) => {
                let mut photos = 0;
                let mut mark = tx
                    .prepare("UPDATE analysis SET stale = 1 WHERE photo_id = ?1")
                    .map_err(storage)?;
                let mut exists = tx
                    .prepare("SELECT count(*) FROM photo WHERE id = ?1")
                    .map_err(storage)?;
                for id in ids {
                    mark.execute([id.0]).map_err(storage)?;
                    photos += exists
                        .query_row([id.0], |r| r.get::<_, i64>(0))
                        .map_err(storage)?;
                }
                photos
            }
        };
        tx.commit().map_err(storage)?;
        Ok(photos as usize)
    }

    pub(super) fn load_analysis(
        &self,
        id: PhotoId,
    ) -> Result<Vec<(String, u32, serde_json::Value)>, CatalogError> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT analyzer, version, result FROM analysis WHERE photo_id = ?1 ORDER BY analyzer",
            )
            .map_err(storage)?;
        let rows = stmt
            .query_map(params![id.0], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(storage)?;
        rows.map(|r| {
            let (name, version, json) = r.map_err(storage)?;
            let value = serde_json::from_str(&json).map_err(storage)?;
            Ok((name, version, value))
        })
        .collect()
    }
}
