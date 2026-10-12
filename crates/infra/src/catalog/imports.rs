//! `ImportStore` on SQLite.

use std::path::{Path, PathBuf};

use picturesque_app::import::model::{
    FileStatus, ImportProblem, ImportRun, ImportState, PhotoStatus, PlannedFile, PlannedPhoto,
};
use picturesque_app::ports::catalog::CatalogError;
use picturesque_app::ports::imports::ImportStore;
use picturesque_core::hash::ContentHash;
use picturesque_core::media::MediaFormat;
use picturesque_core::photo::PhotoId;
use rusqlite::{OptionalExtension, params};

use super::photos::{from_library_path, to_library_path};
use super::{SqliteCatalog, storage};

fn path_text(path: &Path) -> Result<&str, CatalogError> {
    path.to_str()
        .ok_or_else(|| storage(format!("path is not valid UTF-8: {}", path.display())))
}

fn bad(e: impl std::fmt::Display) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, e.to_string().into())
}

impl ImportStore for SqliteCatalog {
    fn create_run(
        &self,
        library_root: &Path,
        template: &str,
        sources: &[PathBuf],
    ) -> Result<i64, CatalogError> {
        let sources: Vec<&str> = sources
            .iter()
            .map(|s| path_text(s))
            .collect::<Result<_, _>>()?;
        let conn = self.conn();
        conn.execute(
            "INSERT INTO import_run (state, library_root, template, sources) VALUES ('planning', ?1, ?2, ?3)",
            params![path_text(library_root)?, template, serde_json::to_string(&sources).map_err(storage)?],
        )
        .map_err(storage)?;
        Ok(conn.last_insert_rowid())
    }

    fn save_plan(
        &self,
        run_id: i64,
        photos: &mut [PlannedPhoto],
        problems: &[ImportProblem],
    ) -> Result<(), CatalogError> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(storage)?;
        for (ord, photo) in photos.iter_mut().enumerate() {
            tx.execute(
                "INSERT INTO import_photo (run_id, ord, metadata, status, reason, photo_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    run_id,
                    ord as i64,
                    serde_json::to_string(&photo.metadata).map_err(storage)?,
                    photo.status.as_str(),
                    photo.reason,
                    photo.photo_id.map(|p| p.0),
                ],
            )
            .map_err(storage)?;
            let photo_row = tx.last_insert_rowid();
            photo.id = Some(photo_row);
            for (file_ord, file) in photo.files.iter_mut().enumerate() {
                let target = if file.target.as_os_str().is_empty() {
                    String::new()
                } else {
                    to_library_path(&file.target)?
                };
                tx.execute(
                    "INSERT INTO import_file (import_photo_id, ord, source_path, format, size, modified_ns, hash,
                                              target_path, status, error)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![
                        photo_row,
                        file_ord as i64,
                        path_text(&file.source)?,
                        file.format.as_str(),
                        i64::try_from(file.size).map_err(storage)?,
                        file.modified_ns,
                        file.hash.as_ref().map(|h| h.as_bytes().to_vec()),
                        target,
                        file.status.as_str(),
                        file.error,
                    ],
                )
                .map_err(storage)?;
                file.id = Some(tx.last_insert_rowid());
            }
        }
        tx.execute(
            "UPDATE import_run SET state = 'planned', problems = ?2 WHERE id = ?1",
            params![run_id, serde_json::to_string(problems).map_err(storage)?],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)
    }

    fn load_run(&self, run_id: i64) -> Result<Option<ImportRun>, CatalogError> {
        let conn = self.conn();
        let head = conn
            .query_row(
                "SELECT state, library_root, template, sources, problems, error FROM import_run WHERE id = ?1",
                [run_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<String>>(5)?,
                    ))
                },
            )
            .optional()
            .map_err(storage)?;
        let Some((state, library_root, template, sources, problems, error)) = head else {
            return Ok(None);
        };

        let mut photo_stmt = conn
            .prepare("SELECT id, metadata, status, reason, photo_id FROM import_photo WHERE run_id = ?1 ORDER BY ord")
            .map_err(storage)?;
        let mut photos = photo_stmt
            .query_map([run_id], |row| {
                let metadata: String = row.get(1)?;
                let status: String = row.get(2)?;
                Ok(PlannedPhoto {
                    id: Some(row.get(0)?),
                    metadata: serde_json::from_str(&metadata).map_err(bad)?,
                    status: PhotoStatus::from_name(&status)
                        .ok_or_else(|| bad(format!("status {status}")))?,
                    reason: row.get(3)?,
                    photo_id: row.get::<_, Option<i64>>(4)?.map(PhotoId),
                    files: Vec::new(),
                })
            })
            .map_err(storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage)?;

        let mut file_stmt = conn
            .prepare(
                "SELECT id, source_path, format, size, modified_ns, hash, target_path, status, error
                 FROM import_file WHERE import_photo_id = ?1 ORDER BY ord",
            )
            .map_err(storage)?;
        for photo in &mut photos {
            photo.files = file_stmt
                .query_map([photo.id], |row| {
                    let format: String = row.get(2)?;
                    let hash: Option<Vec<u8>> = row.get(5)?;
                    let target: String = row.get(6)?;
                    let status: String = row.get(7)?;
                    Ok(PlannedFile {
                        id: Some(row.get(0)?),
                        source: PathBuf::from(row.get::<_, String>(1)?),
                        format: MediaFormat::from_name(&format)
                            .ok_or_else(|| bad(format!("format {format}")))?,
                        size: row.get::<_, i64>(3)? as u64,
                        modified_ns: row.get(4)?,
                        hash: hash
                            .map(|h| {
                                <[u8; 32]>::try_from(h)
                                    .map(ContentHash::from_bytes)
                                    .map_err(|_| bad("hash"))
                            })
                            .transpose()?,
                        target: if target.is_empty() {
                            PathBuf::new()
                        } else {
                            from_library_path(&target)
                        },
                        status: FileStatus::from_name(&status)
                            .ok_or_else(|| bad(format!("status {status}")))?,
                        error: row.get(8)?,
                    })
                })
                .map_err(storage)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(storage)?;
        }

        let sources: Vec<String> = serde_json::from_str(&sources).map_err(storage)?;
        Ok(Some(ImportRun {
            id: run_id,
            state: ImportState::from_name(&state)
                .ok_or_else(|| storage(format!("unknown state {state}")))?,
            library_root: PathBuf::from(library_root),
            template,
            sources: sources.into_iter().map(PathBuf::from).collect(),
            photos,
            problems: serde_json::from_str(&problems).map_err(storage)?,
            error,
        }))
    }

    fn set_state(
        &self,
        run_id: i64,
        state: ImportState,
        error: Option<&str>,
    ) -> Result<(), CatalogError> {
        self.conn()
            .execute(
                "UPDATE import_run SET state = ?2, error = ?3 WHERE id = ?1",
                params![run_id, state.as_str(), error],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn update_file(
        &self,
        file_id: i64,
        status: FileStatus,
        hash: Option<&ContentHash>,
        error: Option<&str>,
    ) -> Result<(), CatalogError> {
        self.conn()
            .execute(
                "UPDATE import_file SET status = ?2, hash = coalesce(?3, hash), error = ?4 WHERE id = ?1",
                params![file_id, status.as_str(), hash.map(|h| h.as_bytes().to_vec()), error],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn update_photo(
        &self,
        import_photo_id: i64,
        status: PhotoStatus,
        photo_id: Option<PhotoId>,
        reason: Option<&str>,
    ) -> Result<(), CatalogError> {
        self.conn()
            .execute(
                "UPDATE import_photo SET status = ?2, photo_id = ?3, reason = ?4 WHERE id = ?1",
                params![
                    import_photo_id,
                    status.as_str(),
                    photo_id.map(|p| p.0),
                    reason
                ],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn mark_unfinished_interrupted(&self) -> Result<(), CatalogError> {
        self.conn()
            .execute("UPDATE import_run SET state = 'interrupted' WHERE state IN ('planning', 'copying')", [])
            .map_err(storage)?;
        Ok(())
    }
}
