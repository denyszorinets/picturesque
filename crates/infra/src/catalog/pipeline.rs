//! Import sources, jobs and the pipeline log on SQLite.

use std::path::Path;

use picturesque_app::ports::catalog::CatalogError;
use picturesque_app::ports::pipeline::{
    ImportSource, Job, JobFile, JobStage, LogEntry, LogLevel, PipelineStore,
};
use picturesque_app::settings::DeleteAfterImport;
use picturesque_core::photo::PhotoId;
use rusqlite::{OptionalExtension, Row, params, params_from_iter};

use super::{SqliteCatalog, storage};

const NOW: &str = "strftime('%Y-%m-%dT%H:%M:%SZ', 'now')";

fn text(path: &Path) -> Result<&str, CatalogError> {
    path.to_str()
        .ok_or_else(|| storage(format!("path is not valid UTF-8: {}", path.display())))
}

fn source(row: &Row<'_>) -> rusqlite::Result<ImportSource> {
    Ok(ImportSource {
        id: row.get(0)?,
        path: row.get::<_, String>(1)?.into(),
        watch: row.get(2)?,
        scanned_at: row.get(3)?,
        scan_error: row.get(4)?,
        delete_originals: row
            .get::<_, Option<String>>(5)?
            .and_then(|name| DeleteAfterImport::parse(&name)),
    })
}

fn job(row: &Row<'_>) -> rusqlite::Result<Job> {
    let files: String = row.get(2)?;
    let stage: String = row.get(3)?;
    Ok(Job {
        id: row.get(0)?,
        source_id: row.get(1)?,
        files: serde_json::from_str(&files).unwrap_or_default(),
        stage: JobStage::from_name(&stage).unwrap_or(JobStage::Failed),
        photo_id: row.get::<_, Option<i64>>(4)?.map(PhotoId),
        error: row.get(5)?,
        updated_at: row.get(6)?,
        failed_stage: row
            .get::<_, Option<String>>(7)?
            .as_deref()
            .and_then(JobStage::from_name),
    })
}

const JOB_COLUMNS: &str = "id, source_id, files, stage, photo_id, error, updated_at, failed_stage";

fn rank(level: LogLevel) -> i64 {
    match level {
        LogLevel::Info => 0,
        LogLevel::Warn => 1,
        LogLevel::Error => 2,
    }
}

impl PipelineStore for SqliteCatalog {
    fn add_source(&self, path: &Path, watch: bool) -> Result<ImportSource, CatalogError> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO import_source (path, watch) VALUES (?1, ?2) ON CONFLICT (path) DO NOTHING",
            params![text(path)?, watch],
        )
        .map_err(storage)?;
        conn.query_row(
            "SELECT id, path, watch, scanned_at, scan_error, delete_originals FROM import_source WHERE path = ?1",
            [text(path)?],
            source,
        )
        .map_err(storage)
    }

    fn sources(&self) -> Result<Vec<ImportSource>, CatalogError> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, path, watch, scanned_at, scan_error, delete_originals FROM import_source ORDER BY id",
            )
            .map_err(storage)?;
        stmt.query_map([], source)
            .map_err(storage)?
            .collect::<Result<_, _>>()
            .map_err(storage)
    }

    fn set_source_watch(&self, id: i64, watch: bool) -> Result<(), CatalogError> {
        self.conn()
            .execute(
                "UPDATE import_source SET watch = ?1 WHERE id = ?2",
                params![watch, id],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn set_source_delete_originals(
        &self,
        id: i64,
        choice: Option<DeleteAfterImport>,
    ) -> Result<(), CatalogError> {
        self.conn()
            .execute(
                "UPDATE import_source SET delete_originals = ?1 WHERE id = ?2",
                params![choice.map(DeleteAfterImport::as_str), id],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn remove_source(&self, id: i64) -> Result<(), CatalogError> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(storage)?;
        tx.execute(
            "DELETE FROM import_job WHERE source_id = ?1 AND stage IN ('queued', 'reading', 'copying')",
            [id],
        )
        .map_err(storage)?;
        tx.execute("DELETE FROM import_source WHERE id = ?1", [id])
            .map_err(storage)?;
        tx.commit().map_err(storage)
    }

    fn record_scan(&self, id: i64, error: Option<&str>) -> Result<(), CatalogError> {
        self.conn()
            .execute(
                &format!(
                    "UPDATE import_source SET scanned_at = {NOW}, scan_error = ?1 WHERE id = ?2"
                ),
                params![error, id],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn offer_job(
        &self,
        source_id: i64,
        key: &Path,
        files: &[JobFile],
    ) -> Result<Option<i64>, CatalogError> {
        let files = serde_json::to_string(files).map_err(storage)?;
        let conn = self.conn();
        let existing: Option<(i64, String, String)> = conn
            .query_row(
                "SELECT id, files, stage FROM import_job WHERE key = ?1",
                [text(key)?],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(storage)?;
        match existing {
            None => {
                conn.execute(
                    "INSERT INTO import_job (source_id, key, files, stage) VALUES (?1, ?2, ?3, 'queued')",
                    params![source_id, text(key)?, files],
                )
                .map_err(storage)?;
                Ok(Some(conn.last_insert_rowid()))
            }
            Some((id, old, stage)) => {
                let stage = JobStage::from_name(&stage);
                if old == files || matches!(stage, Some(JobStage::Reading | JobStage::Copying)) {
                    return Ok(None);
                }
                if stage == Some(JobStage::Queued) {
                    // Still waiting: import the files as they are now.
                    conn.execute(
                        "UPDATE import_job SET files = ?1, source_id = ?2 WHERE id = ?3",
                        params![files, source_id, id],
                    )
                    .map_err(storage)?;
                    return Ok(None);
                }
                conn.execute(
                    &format!(
                        "UPDATE import_job SET files = ?1, source_id = ?2, stage = 'queued', error = NULL,
                         source_removed_at = NULL, updated_at = {NOW} WHERE id = ?3"
                    ),
                    params![files, source_id, id],
                )
                .map_err(storage)?;
                Ok(Some(id))
            }
        }
    }

    fn next_job(&self) -> Result<Option<Job>, CatalogError> {
        self.conn()
            .query_row(
                &format!("SELECT {JOB_COLUMNS} FROM import_job WHERE stage = 'queued' ORDER BY id LIMIT 1"),
                [],
                job,
            )
            .optional()
            .map_err(storage)
    }

    fn set_job_stage(
        &self,
        id: i64,
        stage: JobStage,
        photo_id: Option<PhotoId>,
        error: Option<&str>,
    ) -> Result<(), CatalogError> {
        self.conn()
            .execute(
                &format!(
                    "UPDATE import_job SET stage = ?1, photo_id = coalesce(?2, photo_id), error = ?3,
                     failed_stage = CASE WHEN ?1 = 'failed' THEN stage END,
                     updated_at = {NOW} WHERE id = ?4"
                ),
                params![stage.as_str(), photo_id.map(|p| p.0), error, id],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn requeue_unfinished(&self) -> Result<usize, CatalogError> {
        self.conn()
            .execute(
                "UPDATE import_job SET stage = 'queued' WHERE stage IN ('reading', 'copying')",
                [],
            )
            .map_err(storage)
    }

    fn retry_failed(&self, ids: Option<&[i64]>) -> Result<usize, CatalogError> {
        let conn = self.conn();
        match ids {
            None => conn.execute(
                "UPDATE import_job SET stage = 'queued', error = NULL WHERE stage = 'failed'",
                [],
            ),
            Some(ids) => {
                let marks = vec!["?"; ids.len().max(1)].join(", ");
                conn.execute(
                    &format!(
                        "UPDATE import_job SET stage = 'queued', error = NULL
                         WHERE stage = 'failed' AND id IN ({marks})"
                    ),
                    params_from_iter(ids.iter()),
                )
            }
        }
        .map_err(storage)
    }

    fn job(&self, id: i64) -> Result<Option<Job>, CatalogError> {
        self.conn()
            .query_row(
                &format!("SELECT {JOB_COLUMNS} FROM import_job WHERE id = ?1"),
                [id],
                job,
            )
            .optional()
            .map_err(storage)
    }

    fn jobs(
        &self,
        stages: Option<&[JobStage]>,
        limit: usize,
        before_id: Option<i64>,
    ) -> Result<Vec<Job>, CatalogError> {
        let mut clauses = vec!["id < ?".to_owned()];
        let mut values: Vec<rusqlite::types::Value> = vec![rusqlite::types::Value::Integer(
            before_id.unwrap_or(i64::MAX),
        )];
        if let Some(stages) = stages {
            clauses.push(format!(
                "stage IN ({})",
                vec!["?"; stages.len().max(1)].join(", ")
            ));
            values.extend(
                stages
                    .iter()
                    .map(|s| rusqlite::types::Value::Text(s.as_str().into())),
            );
        }
        values.push(rusqlite::types::Value::Integer(limit as i64));
        let conn = self.conn();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {JOB_COLUMNS} FROM import_job WHERE {} ORDER BY id DESC LIMIT ?",
                clauses.join(" AND ")
            ))
            .map_err(storage)?;
        stmt.query_map(params_from_iter(values.iter()), job)
            .map_err(storage)?
            .collect::<Result<_, _>>()
            .map_err(storage)
    }

    fn cleanup_candidates(
        &self,
        source_id: i64,
        through_job_id: Option<i64>,
    ) -> Result<Vec<Job>, CatalogError> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {JOB_COLUMNS} FROM import_job
                 WHERE source_id = ?1 AND stage IN ('imported', 'duplicate')
                   AND photo_id IS NOT NULL AND source_removed_at IS NULL AND id <= ?2
                 ORDER BY id"
            ))
            .map_err(storage)?;
        stmt.query_map(params![source_id, through_job_id.unwrap_or(i64::MAX)], job)
            .map_err(storage)?
            .collect::<Result<_, _>>()
            .map_err(storage)
    }

    fn unfinished_jobs(&self, source_id: i64) -> Result<usize, CatalogError> {
        self.conn()
            .query_row(
                "SELECT count(*) FROM import_job
                 WHERE source_id = ?1 AND stage IN ('queued', 'reading', 'copying')",
                [source_id],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n as usize)
            .map_err(storage)
    }

    fn mark_source_removed(&self, job_id: i64) -> Result<(), CatalogError> {
        self.conn()
            .execute(
                &format!("UPDATE import_job SET source_removed_at = {NOW} WHERE id = ?1"),
                [job_id],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn job_counts(&self) -> Result<Vec<(JobStage, usize)>, CatalogError> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare("SELECT stage, count(*) FROM import_job GROUP BY stage")
            .map_err(storage)?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .map_err(storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage)?;
        Ok(rows
            .into_iter()
            .filter_map(|(s, n)| Some((JobStage::from_name(&s)?, n as usize)))
            .collect())
    }

    fn append_log(
        &self,
        level: LogLevel,
        job_id: Option<i64>,
        photo_id: Option<PhotoId>,
        message: &str,
    ) -> Result<(), CatalogError> {
        self.conn()
            .execute(
                "INSERT INTO pipeline_log (level, job_id, photo_id, message) VALUES (?1, ?2, ?3, ?4)",
                params![level.as_str(), job_id, photo_id.map(|p| p.0), message],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn log(
        &self,
        min_level: LogLevel,
        limit: usize,
        before_id: Option<i64>,
    ) -> Result<Vec<LogEntry>, CatalogError> {
        let levels: Vec<&str> = [LogLevel::Info, LogLevel::Warn, LogLevel::Error]
            .into_iter()
            .filter(|l| rank(*l) >= rank(min_level))
            .map(LogLevel::as_str)
            .collect();
        let conn = self.conn();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT id, at, level, job_id, photo_id, message FROM pipeline_log
                 WHERE id < ?1 AND level IN ({}) ORDER BY id DESC LIMIT ?2",
                levels
                    .iter()
                    .map(|l| format!("'{l}'"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
            .map_err(storage)?;
        stmt.query_map(params![before_id.unwrap_or(i64::MAX), limit as i64], |r| {
            let level: String = r.get(2)?;
            Ok(LogEntry {
                id: r.get(0)?,
                at: r.get(1)?,
                level: LogLevel::from_name(&level).unwrap_or(LogLevel::Info),
                job_id: r.get(3)?,
                photo_id: r.get(4)?,
                message: r.get(5)?,
            })
        })
        .map_err(storage)?
        .collect::<Result<_, _>>()
        .map_err(storage)
    }

    fn prune_log(&self, keep: usize) -> Result<(), CatalogError> {
        self.conn()
            .execute(
                "DELETE FROM pipeline_log WHERE id <= (SELECT id FROM pipeline_log ORDER BY id DESC LIMIT 1 OFFSET ?1)",
                [keep as i64],
            )
            .map_err(storage)?;
        Ok(())
    }
}
