//! Series and their inputs on SQLite.

use chrono::NaiveDateTime;
use picturesque_app::ports::catalog::{AnalyzerId, CatalogError};
use picturesque_core::photo::PhotoId;
use picturesque_core::series::SeriesEdit;
use rusqlite::{OptionalExtension, params};

use super::photos::TIME_FORMAT;
use super::{SqliteCatalog, storage};

impl SqliteCatalog {
    pub(super) fn load_capture_times(
        &self,
    ) -> Result<Vec<(PhotoId, Option<NaiveDateTime>)>, CatalogError> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare("SELECT id, captured_local FROM photo ORDER BY id")
            .map_err(storage)?;
        stmt.query_map([], |row| {
            let local: Option<String> = row.get(1)?;
            Ok((
                PhotoId(row.get(0)?),
                local.and_then(|l| NaiveDateTime::parse_from_str(&l, TIME_FORMAT).ok()),
            ))
        })
        .map_err(storage)?
        .collect::<Result<_, _>>()
        .map_err(storage)
    }

    pub(super) fn load_analysis_results(
        &self,
        analyzer: AnalyzerId,
    ) -> Result<Vec<(PhotoId, serde_json::Value)>, CatalogError> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT photo_id, result FROM analysis WHERE analyzer = ?1 AND version = ?2 ORDER BY photo_id",
            )
            .map_err(storage)?;
        let rows = stmt
            .query_map(params![analyzer.name, analyzer.version], |row| {
                Ok((PhotoId(row.get(0)?), row.get::<_, String>(1)?))
            })
            .map_err(storage)?;
        rows.map(|r| {
            let (id, json) = r.map_err(storage)?;
            Ok((id, serde_json::from_str(&json).map_err(storage)?))
        })
        .collect()
    }

    pub(super) fn load_series_edits(&self) -> Result<Vec<(PhotoId, SeriesEdit)>, CatalogError> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare("SELECT photo_id, kind FROM series_edit ORDER BY photo_id")
            .map_err(storage)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((PhotoId(row.get(0)?), row.get::<_, String>(1)?))
            })
            .map_err(storage)?;
        rows.map(|r| {
            let (id, kind) = r.map_err(storage)?;
            let edit = SeriesEdit::from_name(&kind)
                .ok_or_else(|| CatalogError::Storage(format!("unknown series edit `{kind}`")))?;
            Ok((id, edit))
        })
        .collect()
    }

    pub(super) fn save_series_edit(
        &self,
        id: PhotoId,
        edit: Option<SeriesEdit>,
    ) -> Result<(), CatalogError> {
        let conn = self.conn();
        match edit {
            Some(edit) => conn.execute(
                "INSERT INTO series_edit (photo_id, kind) VALUES (?1, ?2)
                 ON CONFLICT (photo_id) DO UPDATE SET kind = excluded.kind",
                params![id.0, edit.as_str()],
            ),
            None => conn.execute("DELETE FROM series_edit WHERE photo_id = ?1", params![id.0]),
        }
        .map_err(storage)?;
        Ok(())
    }

    pub(super) fn save_series(&self, series: &[Vec<PhotoId>]) -> Result<(), CatalogError> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(storage)?;
        tx.execute("DELETE FROM series_member", [])
            .map_err(storage)?;
        {
            let mut insert = tx
                .prepare(
                    "INSERT INTO series_member (photo_id, series_id, position) VALUES (?1, ?2, ?3)",
                )
                .map_err(storage)?;
            for members in series {
                let Some(first) = members.first() else {
                    continue;
                };
                for (position, id) in members.iter().enumerate() {
                    insert
                        .execute(params![id.0, first.0, position as i64])
                        .map_err(storage)?;
                }
            }
        }
        tx.commit().map_err(storage)
    }

    pub(super) fn load_series_of(
        &self,
        id: PhotoId,
    ) -> Result<Option<(PhotoId, Vec<PhotoId>)>, CatalogError> {
        let conn = self.conn();
        let Some(series) = conn
            .query_row(
                "SELECT series_id FROM series_member WHERE photo_id = ?1",
                params![id.0],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(storage)?
        else {
            return Ok(None);
        };
        let mut stmt = conn
            .prepare("SELECT photo_id FROM series_member WHERE series_id = ?1 ORDER BY position")
            .map_err(storage)?;
        let members = stmt
            .query_map(params![series], |row| row.get(0).map(PhotoId))
            .map_err(storage)?
            .collect::<Result<_, _>>()
            .map_err(storage)?;
        Ok(Some((PhotoId(series), members)))
    }
}
