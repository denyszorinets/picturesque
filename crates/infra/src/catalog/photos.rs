//! `PhotoCatalog` on SQLite: mapping between rows and domain types.

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use chrono::{FixedOffset, NaiveDateTime};
use picturesque_app::ports::catalog::{AnalyzerId, CatalogError, PhotoCatalog, StoredSeries};
use picturesque_core::hash::ContentHash;
use picturesque_core::media::MediaFormat;
use picturesque_core::metadata::{
    Camera, CaptureMetadata, CaptureTime, Exposure, ExposureTime, GeoPoint, Orientation, Place,
};
use picturesque_core::photo::{Asset, NewPhoto, Photo, PhotoAssets, PhotoId};
use picturesque_core::series::SeriesEdit;
use rusqlite::{ErrorCode, OptionalExtension, Row, params};

use super::{SqliteCatalog, storage};

pub(super) const TIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%.f";

impl PhotoCatalog for SqliteCatalog {
    fn insert_photo(&self, photo: &NewPhoto) -> Result<PhotoId, CatalogError> {
        let m = &photo.metadata;
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(storage)?;
        tx.execute(
            "INSERT INTO photo (
                captured_local, captured_offset_s, camera_make, camera_model, lens,
                iso, f_number, exposure_num, exposure_den, focal_length_mm,
                gps_lat, gps_lon, place_city, place_region, place_country, place_country_code,
                orientation, width, height
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
            params![
                m.captured_at.map(|t| t.local.format(TIME_FORMAT).to_string()),
                m.captured_at.and_then(|t| t.offset).map(|o| o.local_minus_utc()),
                m.camera.as_ref().map(|c| &c.make),
                m.camera.as_ref().map(|c| &c.model),
                m.lens,
                m.exposure.iso,
                m.exposure.f_number,
                m.exposure.exposure_time.map(|t| t.numerator),
                m.exposure.exposure_time.map(|t| t.denominator),
                m.exposure.focal_length_mm,
                m.gps.map(|g| g.latitude()),
                m.gps.map(|g| g.longitude()),
                m.place.as_ref().map(|p| &p.city),
                m.place.as_ref().and_then(|p| p.region.as_ref()),
                m.place.as_ref().map(|p| &p.country),
                m.place.as_ref().map(|p| &p.country_code),
                m.orientation.to_exif(),
                m.dimensions.map(|d| d.0),
                m.dimensions.map(|d| d.1),
            ],
        )
        .map_err(storage)?;
        let id = tx.last_insert_rowid();

        for asset in photo.assets.iter() {
            let path = to_library_path(&asset.path)?;
            tx.execute(
                "INSERT INTO asset (photo_id, role, format, path, size, hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    id,
                    asset.role().as_str(),
                    asset.format.as_str(),
                    path,
                    i64::try_from(asset.size).map_err(storage)?,
                    asset.hash.as_bytes().as_slice(),
                ],
            )
            .map_err(|e| match e.sqlite_error_code() {
                Some(ErrorCode::ConstraintViolation) => {
                    CatalogError::Duplicate(format!("{} ({})", path, asset.hash))
                }
                _ => storage(e),
            })?;
        }
        tx.commit().map_err(storage)?;
        Ok(PhotoId(id))
    }

    fn photo(&self, id: PhotoId) -> Result<Option<Photo>, CatalogError> {
        let conn = self.conn();
        let metadata = conn
            .query_row(
                "SELECT captured_local, captured_offset_s, camera_make, camera_model, lens,
                        iso, f_number, exposure_num, exposure_den, focal_length_mm,
                        gps_lat, gps_lon, place_city, place_region, place_country, place_country_code,
                        orientation, width, height
                 FROM photo WHERE id = ?1",
                [id.0],
                metadata_from_row,
            )
            .optional()
            .map_err(storage)?;
        let Some(metadata) = metadata else {
            return Ok(None);
        };

        let mut stmt = conn
            .prepare("SELECT format, path, size, hash FROM asset WHERE photo_id = ?1 ORDER BY id")
            .map_err(storage)?;
        let assets = stmt
            .query_map([id.0], asset_from_row)
            .map_err(storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage)?;
        let assets =
            PhotoAssets::new(assets).map_err(|e| storage(format!("photo {}: {e}", id.0)))?;
        Ok(Some(Photo {
            id,
            metadata,
            assets,
        }))
    }

    fn photo_with_media_hash(&self, hash: &ContentHash) -> Result<Option<PhotoId>, CatalogError> {
        self.conn()
            .query_row(
                "SELECT photo_id FROM asset WHERE hash = ?1 AND role <> 'sidecar'",
                [hash.as_bytes().as_slice()],
                |row| row.get(0).map(PhotoId),
            )
            .optional()
            .map_err(storage)
    }

    fn all_photos(&self) -> Result<Vec<Photo>, CatalogError> {
        let conn = self.conn();
        let mut assets: HashMap<i64, Vec<Asset>> = HashMap::new();
        let mut stmt = conn
            .prepare("SELECT photo_id, format, path, size, hash FROM asset ORDER BY id")
            .map_err(storage)?;
        let rows = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                // asset_from_row reads columns 0..4; shift by one.
                let format: String = row.get(1)?;
                let hash: Vec<u8> = row.get(4)?;
                let hash: [u8; 32] = hash
                    .try_into()
                    .map_err(|_| conversion(4, "hash is not 32 bytes"))?;
                Ok((
                    id,
                    Asset {
                        format: MediaFormat::from_name(&format)
                            .ok_or_else(|| conversion(1, format!("unknown format {format}")))?,
                        path: from_library_path(&row.get::<_, String>(2)?),
                        size: row.get::<_, i64>(3)? as u64,
                        hash: ContentHash::from_bytes(hash),
                    },
                ))
            })
            .map_err(storage)?;
        for row in rows {
            let (id, asset) = row.map_err(storage)?;
            assets.entry(id).or_default().push(asset);
        }

        let mut stmt = conn
            .prepare(
                "SELECT captured_local, captured_offset_s, camera_make, camera_model, lens,
                        iso, f_number, exposure_num, exposure_den, focal_length_mm,
                        gps_lat, gps_lon, place_city, place_region, place_country, place_country_code,
                        orientation, width, height, id
                 FROM photo ORDER BY captured_local IS NULL, captured_local, id",
            )
            .map_err(storage)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, i64>(19)?, metadata_from_row(row)?))
            })
            .map_err(storage)?;
        let mut photos = Vec::new();
        for row in rows {
            let (id, metadata) = row.map_err(storage)?;
            let assets = PhotoAssets::new(assets.remove(&id).unwrap_or_default())
                .map_err(|e| storage(format!("photo {id}: {e}")))?;
            photos.push(Photo {
                id: PhotoId(id),
                metadata,
                assets,
            });
        }
        Ok(photos)
    }

    fn query(
        &self,
        query: &picturesque_app::library::PhotoQuery,
    ) -> Result<picturesque_app::library::PhotoPage, CatalogError> {
        self.query_photos(query)
    }

    fn facets(
        &self,
        filter: Option<&picturesque_app::library::PhotoQuery>,
    ) -> Result<picturesque_app::library::PhotoFacets, CatalogError> {
        self.photo_facets(filter)
    }

    fn gear_usage(&self) -> Result<picturesque_app::stats::GearUsage, CatalogError> {
        self.load_gear_usage()
    }

    fn decision(&self, id: PhotoId) -> Result<picturesque_core::decision::Decision, CatalogError> {
        self.load_decision(id)
    }

    fn set_decision(
        &self,
        id: PhotoId,
        decision: picturesque_core::decision::Decision,
    ) -> Result<(), CatalogError> {
        self.store_decision(id, decision)
    }

    fn set_decision_saved(
        &self,
        id: PhotoId,
        decision: picturesque_core::decision::Decision,
    ) -> Result<(), CatalogError> {
        self.store_decision(id, decision)?;
        self.conn()
            .execute(
                "UPDATE decision SET sidecar_revision = revision,
                     sidecar_written_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE photo_id = ?1",
                [id.0],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn dirty_decisions(
        &self,
    ) -> Result<Vec<picturesque_app::ports::catalog::DirtyDecision>, CatalogError> {
        self.load_dirty()
    }

    fn remove_photo(&self, id: PhotoId) -> Result<(), CatalogError> {
        self.conn()
            .execute("DELETE FROM photo WHERE id = ?1", [id.0])
            .map_err(storage)?;
        Ok(())
    }

    fn record_copies(
        &self,
        copies: &[picturesque_app::ports::catalog::CopyRecord],
    ) -> Result<(), CatalogError> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(storage)?;
        for copy in copies {
            tx.execute(
                "INSERT INTO copy_record (photo_id, source, destination, hash, size)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (photo_id, source) DO UPDATE SET destination = excluded.destination,
                     hash = excluded.hash, size = excluded.size, copied_at = excluded.copied_at",
                params![
                    copy.photo_id.0,
                    to_library_path(&copy.source)?,
                    copy.destination
                        .to_str()
                        .ok_or_else(|| storage("copy path is not UTF-8"))?,
                    copy.hash.as_bytes().as_slice(),
                    i64::try_from(copy.size).map_err(storage)?
                ],
            )
            .map_err(storage)?;
        }
        tx.commit().map_err(storage)
    }

    fn copies(&self) -> Result<Vec<picturesque_app::ports::catalog::CopyRecord>, CatalogError> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT photo_id, source, destination, hash, size FROM copy_record
                 ORDER BY photo_id, source",
            )
            .map_err(storage)?;
        stmt.query_map([], |r| {
            let hash: Vec<u8> = r.get(3)?;
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                hash,
                r.get::<_, i64>(4)?,
            ))
        })
        .map_err(storage)?
        .map(|row| {
            let (id, source, destination, hash, size) = row.map_err(storage)?;
            let hash: [u8; 32] = hash
                .try_into()
                .map_err(|_| storage("a copy's hash is not 32 bytes"))?;
            Ok(picturesque_app::ports::catalog::CopyRecord {
                photo_id: PhotoId(id),
                source: from_library_path(&source),
                destination: PathBuf::from(destination),
                hash: ContentHash::from_bytes(hash),
                size: u64::try_from(size).map_err(storage)?,
            })
        })
        .collect()
    }

    fn auto_labeled(&self) -> Result<HashSet<PhotoId>, CatalogError> {
        self.load_auto_labeled()
    }

    fn set_auto_labeled(&self, ids: &[PhotoId]) -> Result<usize, CatalogError> {
        self.store_auto_labeled(ids)
    }

    fn mark_sidecar_written(&self, id: PhotoId, token: &str) -> Result<(), CatalogError> {
        self.mark_written(id, token)
    }

    fn upsert_sidecar(
        &self,
        id: PhotoId,
        path: &Path,
        size: u64,
        hash: &ContentHash,
    ) -> Result<(), CatalogError> {
        let path = to_library_path(path)?;
        self.conn()
            .execute(
                "INSERT INTO asset (photo_id, role, format, path, size, hash) VALUES (?1, 'sidecar', 'xmp', ?2, ?3, ?4)
                 ON CONFLICT (path) DO UPDATE SET size = excluded.size, hash = excluded.hash
                 WHERE asset.photo_id = excluded.photo_id",
                params![id.0, path, i64::try_from(size).map_err(storage)?, hash.as_bytes().as_slice()],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn set_dimensions(&self, id: PhotoId, width: u32, height: u32) -> Result<(), CatalogError> {
        self.conn()
            .execute(
                "UPDATE photo SET width = ?2, height = ?3 WHERE id = ?1",
                params![id.0, width, height],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn media_sizes(&self) -> Result<HashSet<u64>, CatalogError> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare("SELECT DISTINCT size FROM asset WHERE role <> 'sidecar'")
            .map_err(storage)?;
        let sizes = stmt
            .query_map([], |row| row.get::<_, i64>(0))
            .map_err(storage)?
            .map(|r| r.map(|n| n as u64))
            .collect::<Result<HashSet<_>, _>>()
            .map_err(storage)?;
        Ok(sizes)
    }

    fn pending_analysis(
        &self,
        analyzers: &[AnalyzerId],
        limit: usize,
    ) -> Result<Vec<PhotoId>, CatalogError> {
        self.load_pending_analysis(analyzers, limit)
    }

    fn analysis_progress(&self, analyzers: &[AnalyzerId]) -> Result<(usize, usize), CatalogError> {
        self.count_analysis(analyzers)
    }

    fn analyzed(
        &self,
        id: PhotoId,
        analyzers: &[AnalyzerId],
    ) -> Result<Vec<AnalyzerId>, CatalogError> {
        self.current_analyzers(id, analyzers)
    }

    fn store_analysis(
        &self,
        id: PhotoId,
        analyzer: AnalyzerId,
        result: &serde_json::Value,
    ) -> Result<(), CatalogError> {
        self.save_analysis(id, analyzer, result)
    }

    fn mark_analysis_stale(&self, ids: Option<&[PhotoId]>) -> Result<usize, CatalogError> {
        self.mark_stale(ids)
    }

    fn analysis(&self, id: PhotoId) -> Result<Vec<(String, u32, serde_json::Value)>, CatalogError> {
        self.load_analysis(id)
    }

    fn capture_times(&self) -> Result<Vec<(PhotoId, Option<NaiveDateTime>)>, CatalogError> {
        self.load_capture_times()
    }

    fn analysis_results(
        &self,
        analyzer: AnalyzerId,
    ) -> Result<Vec<(PhotoId, serde_json::Value)>, CatalogError> {
        self.load_analysis_results(analyzer)
    }

    fn series_edits(&self) -> Result<Vec<(PhotoId, SeriesEdit)>, CatalogError> {
        self.load_series_edits()
    }

    fn set_series_edit(&self, id: PhotoId, edit: Option<SeriesEdit>) -> Result<(), CatalogError> {
        self.save_series_edit(id, edit)
    }

    fn replace_series(&self, series: &[StoredSeries]) -> Result<(), CatalogError> {
        self.save_series(series)
    }

    fn series_of(&self, id: PhotoId) -> Result<Option<(PhotoId, StoredSeries)>, CatalogError> {
        self.load_series_of(id)
    }

    fn best_choices(&self) -> Result<Vec<PhotoId>, CatalogError> {
        self.load_best_choices()
    }

    fn set_best_choice(&self, id: PhotoId, chosen: bool) -> Result<(), CatalogError> {
        self.save_best_choice(id, chosen)
    }

    fn rejected_photos(&self) -> Result<Vec<PhotoId>, CatalogError> {
        self.load_flagged("reject")
    }

    fn picked_photos(&self) -> Result<Vec<PhotoId>, CatalogError> {
        self.load_flagged("pick")
    }

    fn move_assets(
        &self,
        id: PhotoId,
        moves: &[(std::path::PathBuf, std::path::PathBuf)],
    ) -> Result<(), CatalogError> {
        self.update_asset_paths(id, moves)
    }

    fn record_reject_move(&self, batch: i64, ids: &[PhotoId]) -> Result<(), CatalogError> {
        self.save_reject_move(batch, ids)
    }

    fn last_reject_move(&self) -> Result<Option<(i64, Vec<PhotoId>)>, CatalogError> {
        self.load_last_reject_move()
    }

    fn forget_reject_move(&self, ids: &[PhotoId]) -> Result<(), CatalogError> {
        self.delete_reject_move(ids)
    }

    fn replace_scores(
        &self,
        scores: &[picturesque_app::ports::catalog::PhotoScore],
    ) -> Result<(), CatalogError> {
        self.save_scores(scores)
    }

    fn photo_count(&self) -> Result<u64, CatalogError> {
        self.conn()
            .query_row("SELECT count(*) FROM photo", [], |row| row.get::<_, i64>(0))
            .map(|n| n as u64)
            .map_err(storage)
    }
}

fn metadata_from_row(row: &Row<'_>) -> rusqlite::Result<CaptureMetadata> {
    let local: Option<String> = row.get(0)?;
    let captured_at = local
        .map(|s| NaiveDateTime::parse_from_str(&s, TIME_FORMAT).map_err(|e| conversion(0, e)))
        .transpose()?
        .map(|local| {
            let offset = row
                .get::<_, Option<i32>>(1)?
                .and_then(FixedOffset::east_opt);
            Ok::<_, rusqlite::Error>(CaptureTime::new(local, offset))
        })
        .transpose()?;
    let camera = match (
        row.get::<_, Option<String>>(2)?,
        row.get::<_, Option<String>>(3)?,
    ) {
        (None, None) => None,
        (make, model) => Some(Camera::new(
            make.unwrap_or_default(),
            model.unwrap_or_default(),
        )),
    };
    let exposure_time = match (row.get::<_, Option<u32>>(7)?, row.get::<_, Option<u32>>(8)?) {
        (Some(numerator), Some(denominator)) => Some(ExposureTime {
            numerator,
            denominator,
        }),
        _ => None,
    };
    let gps = match (
        row.get::<_, Option<f64>>(10)?,
        row.get::<_, Option<f64>>(11)?,
    ) {
        (Some(lat), Some(lon)) => GeoPoint::new(lat, lon),
        _ => None,
    };
    let place = match (
        row.get::<_, Option<String>>(12)?,
        row.get::<_, Option<String>>(14)?,
    ) {
        (Some(city), Some(country)) => Some(Place {
            city,
            region: row.get(13)?,
            country,
            country_code: row.get::<_, Option<String>>(15)?.unwrap_or_default(),
        }),
        _ => None,
    };
    let dimensions = match (
        row.get::<_, Option<u32>>(17)?,
        row.get::<_, Option<u32>>(18)?,
    ) {
        (Some(w), Some(h)) => Some((w, h)),
        _ => None,
    };
    Ok(CaptureMetadata {
        captured_at,
        camera,
        lens: row.get(4)?,
        exposure: Exposure {
            iso: row.get(5)?,
            f_number: row.get(6)?,
            exposure_time,
            focal_length_mm: row.get(9)?,
        },
        gps,
        place,
        orientation: Orientation::from_exif(row.get(16)?),
        dimensions,
    })
}

fn asset_from_row(row: &Row<'_>) -> rusqlite::Result<Asset> {
    let format: String = row.get(0)?;
    let hash: Vec<u8> = row.get(3)?;
    let hash: [u8; 32] = hash
        .try_into()
        .map_err(|_| conversion(3, "hash is not 32 bytes"))?;
    Ok(Asset {
        format: MediaFormat::from_name(&format)
            .ok_or_else(|| conversion(0, format!("unknown format {format}")))?,
        path: from_library_path(&row.get::<_, String>(1)?),
        size: row.get::<_, i64>(2)? as u64,
        hash: ContentHash::from_bytes(hash),
    })
}

fn conversion(column: usize, e: impl ToString) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        column,
        rusqlite::types::Type::Text,
        e.to_string().into(),
    )
}

/// Stores library-relative paths with `/` separators, so a library on an
/// external drive reads the same from every operating system.
pub(super) fn to_library_path(path: &Path) -> Result<String, CatalogError> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                parts.push(part.to_str().ok_or_else(|| {
                    storage(format!("path is not valid UTF-8: {}", path.display()))
                })?)
            }
            _ => {
                return Err(storage(format!(
                    "not a relative library path: {}",
                    path.display()
                )));
            }
        }
    }
    if parts.is_empty() {
        return Err(storage("empty library path"));
    }
    Ok(parts.join("/"))
}

pub(super) fn from_library_path(stored: &str) -> PathBuf {
    stored.split('/').collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_paths_use_forward_slashes() {
        let path: PathBuf = ["2026", "2026-10-12 Lisbon", "IMG_1.CR3"].iter().collect();

        assert_eq!(
            to_library_path(&path).unwrap(),
            "2026/2026-10-12 Lisbon/IMG_1.CR3"
        );
        assert_eq!(from_library_path("2026/2026-10-12 Lisbon/IMG_1.CR3"), path);
    }

    #[test]
    fn rejects_absolute_and_parent_paths() {
        assert!(to_library_path(Path::new("/abs/a.jpg")).is_err());
        assert!(to_library_path(Path::new("../a.jpg")).is_err());
        assert!(to_library_path(Path::new("")).is_err());
    }
}
