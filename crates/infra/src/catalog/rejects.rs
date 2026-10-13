//! Moving assets and remembering moved rejects.

use std::path::PathBuf;

use picturesque_app::ports::catalog::CatalogError;
use picturesque_core::photo::PhotoId;
use rusqlite::{OptionalExtension, params};

use super::photos::to_library_path;
use super::{SqliteCatalog, storage};

impl SqliteCatalog {
    pub(super) fn update_asset_paths(
        &self,
        id: PhotoId,
        moves: &[(PathBuf, PathBuf)],
    ) -> Result<(), CatalogError> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(storage)?;
        for (old, new) in moves {
            let changed = tx
                .execute(
                    "UPDATE asset SET path = ?1 WHERE photo_id = ?2 AND path = ?3",
                    params![to_library_path(new)?, id.0, to_library_path(old)?],
                )
                .map_err(storage)?;
            if changed != 1 {
                return Err(CatalogError::Storage(format!(
                    "photo {} has no file {}",
                    id.0,
                    old.display()
                )));
            }
        }
        tx.commit().map_err(storage)
    }

    pub(super) fn save_reject_move(&self, batch: i64, ids: &[PhotoId]) -> Result<(), CatalogError> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(storage)?;
        for id in ids {
            tx.execute(
                "INSERT INTO reject_move (photo_id, batch) VALUES (?1, ?2)
                 ON CONFLICT (photo_id) DO UPDATE SET batch = excluded.batch",
                params![id.0, batch],
            )
            .map_err(storage)?;
        }
        tx.commit().map_err(storage)
    }

    pub(super) fn load_last_reject_move(
        &self,
    ) -> Result<Option<(i64, Vec<PhotoId>)>, CatalogError> {
        let conn = self.conn();
        let Some(batch): Option<i64> = conn
            .query_row("SELECT max(batch) FROM reject_move", [], |r| r.get(0))
            .optional()
            .map_err(storage)?
            .flatten()
        else {
            return Ok(None);
        };
        let mut stmt = conn
            .prepare("SELECT photo_id FROM reject_move WHERE batch = ?1 ORDER BY photo_id")
            .map_err(storage)?;
        let ids = stmt
            .query_map([batch], |r| r.get(0).map(PhotoId))
            .map_err(storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage)?;
        Ok(Some((batch, ids)))
    }

    pub(super) fn delete_reject_move(&self, ids: &[PhotoId]) -> Result<(), CatalogError> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(storage)?;
        for id in ids {
            tx.execute("DELETE FROM reject_move WHERE photo_id = ?1", [id.0])
                .map_err(storage)?;
        }
        tx.commit().map_err(storage)
    }
}
