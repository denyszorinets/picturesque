//! Series and their inputs on SQLite.

use chrono::NaiveDateTime;
use picturesque_app::ports::catalog::{AnalyzerId, CatalogError, PhotoScore, StoredSeries};
use picturesque_core::photo::PhotoId;
use picturesque_core::ranking::Ranked;
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

    pub(super) fn save_series(&self, series: &[StoredSeries]) -> Result<(), CatalogError> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(storage)?;
        tx.execute("DELETE FROM series_member", [])
            .map_err(storage)?;
        {
            let mut insert = tx
                .prepare(
                    "INSERT INTO series_member (photo_id, series_id, position, rank, score, best)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )
                .map_err(storage)?;
            for s in series {
                let Some(first) = s.members.first() else {
                    continue;
                };
                for (position, id) in s.members.iter().enumerate() {
                    let rank = s.ranking.iter().position(|r| r.id == *id);
                    let score = rank.and_then(|r| s.ranking[r].score);
                    let best =
                        (s.best == *id).then_some(if s.best_chosen { "user" } else { "auto" });
                    insert
                        .execute(params![
                            id.0,
                            first.0,
                            position as i64,
                            rank.unwrap_or(position) as i64,
                            score,
                            best
                        ])
                        .map_err(storage)?;
                }
            }
        }
        tx.commit().map_err(storage)
    }

    pub(super) fn load_series_of(
        &self,
        id: PhotoId,
    ) -> Result<Option<(PhotoId, StoredSeries)>, CatalogError> {
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
            .prepare(
                "SELECT photo_id, rank, score, best FROM series_member WHERE series_id = ?1 ORDER BY position",
            )
            .map_err(storage)?;
        let rows: Vec<(PhotoId, i64, Option<f32>, Option<String>)> = stmt
            .query_map(params![series], |row| {
                Ok((PhotoId(row.get(0)?), row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .map_err(storage)?
            .collect::<Result<_, _>>()
            .map_err(storage)?;
        let mut ranked: Vec<&(PhotoId, i64, Option<f32>, Option<String>)> = rows.iter().collect();
        ranked.sort_by_key(|(id, rank, _, _)| (*rank, *id));
        let (best, best_chosen) = rows
            .iter()
            .find_map(|(id, _, _, best)| best.as_deref().map(|b| (*id, b == "user")))
            .ok_or_else(|| CatalogError::Storage(format!("series {series} has no best photo")))?;
        Ok(Some((
            PhotoId(series),
            StoredSeries {
                members: rows.iter().map(|(id, ..)| *id).collect(),
                ranking: ranked
                    .into_iter()
                    .map(|(id, _, score, _)| Ranked {
                        id: *id,
                        score: *score,
                    })
                    .collect(),
                best,
                best_chosen,
            },
        )))
    }

    pub(super) fn load_best_choices(&self) -> Result<Vec<PhotoId>, CatalogError> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare("SELECT photo_id FROM best_choice ORDER BY seq")
            .map_err(storage)?;
        stmt.query_map([], |row| row.get(0).map(PhotoId))
            .map_err(storage)?
            .collect::<Result<_, _>>()
            .map_err(storage)
    }

    pub(super) fn save_best_choice(&self, id: PhotoId, chosen: bool) -> Result<(), CatalogError> {
        let conn = self.conn();
        if chosen {
            conn.execute(
                "INSERT INTO best_choice (photo_id, seq)
                 VALUES (?1, (SELECT coalesce(max(seq), 0) + 1 FROM best_choice))
                 ON CONFLICT (photo_id) DO UPDATE SET seq = excluded.seq",
                params![id.0],
            )
        } else {
            conn.execute("DELETE FROM best_choice WHERE photo_id = ?1", params![id.0])
        }
        .map_err(storage)?;
        Ok(())
    }

    pub(super) fn save_scores(&self, scores: &[PhotoScore]) -> Result<(), CatalogError> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(storage)?;
        tx.execute("DELETE FROM photo_score", []).map_err(storage)?;
        {
            let mut insert = tx
                .prepare(
                    "INSERT INTO photo_score (photo_id, score, aesthetic, eyes_closed, faces, keeper)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )
                .map_err(storage)?;
            for s in scores {
                insert
                    .execute(params![
                        s.id.0,
                        f64::from(s.score),
                        s.aesthetic.map(f64::from),
                        s.eyes_closed,
                        s.faces,
                        s.keeper
                    ])
                    .map_err(storage)?;
            }
        }
        tx.commit().map_err(storage)
    }

    pub(super) fn load_rejected(&self) -> Result<Vec<PhotoId>, CatalogError> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare("SELECT photo_id FROM decision WHERE flag = 'reject' ORDER BY photo_id")
            .map_err(storage)?;
        stmt.query_map([], |row| row.get(0).map(PhotoId))
            .map_err(storage)?
            .collect::<Result<_, _>>()
            .map_err(storage)
    }
}
