//! Browsing queries and decisions on SQLite.

use picturesque_app::library::{
    CameraFacet, CameraKey, Counted, FlagCount, PhotoFacets, PhotoPage, PhotoQuery, PhotoSort,
    PhotoSummary, PlaceFacet,
};
use picturesque_app::ports::catalog::CatalogError;
use picturesque_core::decision::{Decision, Flag, Rating};
use picturesque_core::metadata::{Camera, Orientation};
use picturesque_core::photo::PhotoId;
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};

use super::{SqliteCatalog, storage};

/// SQL for the filters of `query`, with their parameters.
fn filters(query: &PhotoQuery) -> (Vec<String>, Vec<Value>) {
    let mut clauses = Vec::new();
    let mut values = Vec::new();
    let mut add = |clause: &str, vals: Vec<Value>| {
        clauses.push(clause.to_owned());
        values.extend(vals);
    };
    if let Some(from) = &query.from {
        add("p.captured_local >= ?", vec![Value::Text(from.clone())]);
    }
    if let Some(to) = &query.to {
        add(
            "p.captured_local < date(?, '+1 day')",
            vec![Value::Text(to.clone())],
        );
    }
    if let Some(cc) = &query.country_code {
        add("p.place_country_code = ?", vec![Value::Text(cc.clone())]);
    }
    if let Some(city) = &query.city {
        add("p.place_city = ?", vec![Value::Text(city.clone())]);
    }
    if let Some(camera) = &query.camera {
        add(
            "p.camera_make = ? AND p.camera_model = ?",
            vec![
                Value::Text(camera.make.clone()),
                Value::Text(camera.model.clone()),
            ],
        );
    }
    if let Some(lens) = &query.lens {
        add("p.lens = ?", vec![Value::Text(lens.clone())]);
    }
    if let Some(flags) = query.flags.as_ref().filter(|f| !f.is_empty()) {
        let marks = vec!["?"; flags.len()].join(", ");
        add(
            &format!("coalesce(d.flag, 'unflagged') IN ({marks})"),
            flags
                .iter()
                .map(|f| Value::Text(f.as_str().to_owned()))
                .collect(),
        );
    }
    if let Some(min) = query.min_rating {
        add(
            "coalesce(d.rating, 0) >= ?",
            vec![Value::Integer(i64::from(min))],
        );
    }
    (clauses, values)
}

/// A position in a result list: the last row of the previous page.
#[derive(Debug, PartialEq, Eq)]
enum Cursor {
    Dated { local: String, id: i64 },
    Undated { id: i64 },
    Imported { id: i64 },
}

impl Cursor {
    fn encode(&self) -> String {
        match self {
            Self::Dated { local, id } => format!("d|{local}|{id}"),
            Self::Undated { id } => format!("u|{id}"),
            Self::Imported { id } => format!("i|{id}"),
        }
    }

    fn decode(s: &str) -> Option<Self> {
        let mut parts = s.splitn(3, '|');
        match (parts.next()?, parts.next()?, parts.next()) {
            ("d", local, Some(id)) => Some(Self::Dated {
                local: local.to_owned(),
                id: id.parse().ok()?,
            }),
            ("u", id, None) => Some(Self::Undated {
                id: id.parse().ok()?,
            }),
            ("i", id, None) => Some(Self::Imported {
                id: id.parse().ok()?,
            }),
            _ => None,
        }
    }

    /// SQL that keeps rows after this cursor in `sort` order.
    fn after(&self, sort: PhotoSort) -> Result<(String, Vec<Value>), CatalogError> {
        let (cmp, id_cmp) = match sort {
            PhotoSort::CapturedDesc => ("<", "<"),
            PhotoSort::CapturedAsc => (">", ">"),
            PhotoSort::ImportedDesc => ("<", "<"),
        };
        Ok(match (self, sort) {
            (Self::Imported { id }, PhotoSort::ImportedDesc) => {
                ("p.id < ?".into(), vec![Value::Integer(*id)])
            }
            (Self::Dated { local, id }, PhotoSort::CapturedDesc | PhotoSort::CapturedAsc) => (
                format!(
                    "(p.captured_local IS NULL OR p.captured_local {cmp} ? OR (p.captured_local = ? AND p.id {id_cmp} ?))"
                ),
                vec![
                    Value::Text(local.clone()),
                    Value::Text(local.clone()),
                    Value::Integer(*id),
                ],
            ),
            (Self::Undated { id }, PhotoSort::CapturedDesc | PhotoSort::CapturedAsc) => (
                format!("(p.captured_local IS NULL AND p.id {id_cmp} ?)"),
                vec![Value::Integer(*id)],
            ),
            _ => {
                return Err(CatalogError::Storage(
                    "cursor does not match the sort order".into(),
                ));
            }
        })
    }
}

fn order_by(sort: PhotoSort) -> &'static str {
    match sort {
        PhotoSort::CapturedDesc => "p.captured_local IS NULL, p.captured_local DESC, p.id DESC",
        PhotoSort::CapturedAsc => "p.captured_local IS NULL, p.captured_local ASC, p.id ASC",
        PhotoSort::ImportedDesc => "p.id DESC",
    }
}

const FROM: &str = "FROM photo p LEFT JOIN decision d ON d.photo_id = p.id";

fn where_sql(clauses: &[String]) -> String {
    if clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", clauses.join(" AND "))
    }
}

fn query_page(conn: &Connection, query: &PhotoQuery) -> Result<PhotoPage, CatalogError> {
    let sort = query.sort.unwrap_or_default();
    let (mut clauses, mut values) = filters(query);

    let total: i64 = conn
        .query_row(
            &format!("SELECT count(*) {FROM} {}", where_sql(&clauses)),
            params_from_iter(values.iter()),
            |r| r.get(0),
        )
        .map_err(storage)?;

    if let Some(cursor) = &query.cursor {
        let cursor =
            Cursor::decode(cursor).ok_or_else(|| CatalogError::Storage("invalid cursor".into()))?;
        let (clause, vals) = cursor.after(sort)?;
        clauses.push(clause);
        values.extend(vals);
    }
    let limit = query.page_size();
    values.push(Value::Integer(i64::from(limit) + 1));

    let sql = format!(
        "SELECT p.id, p.captured_local, p.camera_make, p.camera_model, p.lens, p.place_city, p.place_country,
                coalesce(d.flag, 'unflagged'), coalesce(d.rating, 0), p.width, p.height, p.orientation,
                EXISTS (SELECT 1 FROM asset a WHERE a.photo_id = p.id AND a.role = 'raw'),
                (SELECT a.path FROM asset a WHERE a.photo_id = p.id AND a.role <> 'sidecar'
                 ORDER BY a.role = 'raw' LIMIT 1)
         {FROM} {} ORDER BY {} LIMIT ?",
        where_sql(&clauses),
        order_by(sort)
    );
    let mut stmt = conn.prepare_cached(&sql).map_err(storage)?;
    let mut photos = stmt
        .query_map(params_from_iter(values.iter()), |row| {
            let local: Option<String> = row.get(1)?;
            let make: Option<String> = row.get(2)?;
            let model: Option<String> = row.get(3)?;
            let flag: String = row.get(7)?;
            let (w, h): (Option<u32>, Option<u32>) = (row.get(9)?, row.get(10)?);
            let orientation = Orientation::from_exif(row.get(11)?);
            let (width, height) = if orientation.swaps_dimensions() {
                (h, w)
            } else {
                (w, h)
            };
            let path: String = row.get::<_, Option<String>>(13)?.unwrap_or_default();
            Ok(PhotoSummary {
                id: row.get(0)?,
                day: local.as_ref().map(|l| l[..10.min(l.len())].to_owned()),
                captured_at: local,
                camera: (make.is_some() || model.is_some()).then(|| {
                    Camera::new(make.unwrap_or_default(), model.unwrap_or_default()).display_name()
                }),
                lens: row.get(4)?,
                city: row.get(5)?,
                country: row.get(6)?,
                flag: Flag::from_name(&flag).unwrap_or_default(),
                rating: row.get(8)?,
                width,
                height,
                has_raw: row.get(12)?,
                file_name: path.rsplit('/').next().unwrap_or_default().to_owned(),
            })
        })
        .map_err(storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage)?;

    let next_cursor = if photos.len() > limit as usize {
        photos.truncate(limit as usize);
        let last = photos.last().expect("non-empty page");
        Some(
            match (sort, &last.captured_at) {
                (PhotoSort::ImportedDesc, _) => Cursor::Imported { id: last.id },
                (_, Some(local)) => Cursor::Dated {
                    local: local.clone(),
                    id: last.id,
                },
                (_, None) => Cursor::Undated { id: last.id },
            }
            .encode(),
        )
    } else {
        None
    };
    Ok(PhotoPage {
        photos,
        next_cursor,
        total: total as usize,
    })
}

fn counted<T>(
    conn: &Connection,
    sql: &str,
    map: impl Fn(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
) -> Result<Vec<Counted<T>>, CatalogError>
where
    T: ts_rs::TS,
{
    let mut stmt = conn.prepare(sql).map_err(storage)?;
    stmt.query_map([], |row| {
        Ok(Counted {
            value: map(row)?,
            count: row.get::<_, i64>("n")? as usize,
        })
    })
    .map_err(storage)?
    .collect::<Result<Vec<_>, _>>()
    .map_err(storage)
}

fn facets(conn: &Connection) -> Result<PhotoFacets, CatalogError> {
    let (total, first, last): (i64, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT count(*), substr(min(captured_local), 1, 10), substr(max(captured_local), 1, 10) FROM photo",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(storage)?;
    let days = counted(
        conn,
        "SELECT substr(captured_local, 1, 10) AS day, count(*) AS n FROM photo
         WHERE captured_local IS NOT NULL GROUP BY day ORDER BY day DESC",
        |r| r.get("day"),
    )?;
    let cameras = counted(
        conn,
        "SELECT coalesce(camera_make, '') AS make, coalesce(camera_model, '') AS model, count(*) AS n FROM photo
         WHERE camera_make IS NOT NULL OR camera_model IS NOT NULL GROUP BY make, model ORDER BY n DESC, make, model",
        |r| {
            let key = CameraKey { make: r.get("make")?, model: r.get("model")? };
            let label = Camera::new(&key.make, &key.model).display_name();
            Ok(CameraFacet { key, label })
        },
    )?;
    let lenses = counted(
        conn,
        "SELECT lens, count(*) AS n FROM photo WHERE lens IS NOT NULL GROUP BY lens ORDER BY n DESC, lens",
        |r| r.get("lens"),
    )?;
    let places = counted(
        conn,
        "SELECT place_city, place_country, place_country_code, count(*) AS n FROM photo
         WHERE place_city IS NOT NULL GROUP BY place_country_code, place_city ORDER BY n DESC, place_city",
        |r| Ok(PlaceFacet { city: r.get(0)?, country: r.get(1)?, country_code: r.get::<_, Option<String>>(2)?.unwrap_or_default() }),
    )?;
    let mut flags = Vec::new();
    let mut stmt = conn
        .prepare(&format!(
            "SELECT coalesce(d.flag, 'unflagged') AS f, count(*) {FROM} GROUP BY f ORDER BY f"
        ))
        .map_err(storage)?;
    for row in stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        .map_err(storage)?
    {
        let (f, n) = row.map_err(storage)?;
        if let Some(flag) = Flag::from_name(&f) {
            flags.push(FlagCount {
                flag,
                count: n as usize,
            });
        }
    }
    let mut ratings = vec![0usize; 6];
    let mut stmt = conn
        .prepare(&format!(
            "SELECT coalesce(d.rating, 0) AS r, count(*) {FROM} GROUP BY r"
        ))
        .map_err(storage)?;
    for row in stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
        .map_err(storage)?
    {
        let (r, n) = row.map_err(storage)?;
        if let Some(slot) = ratings.get_mut(r as usize) {
            *slot = n as usize;
        }
    }
    Ok(PhotoFacets {
        total: total as usize,
        first_day: first,
        last_day: last,
        days,
        cameras,
        lenses,
        places,
        flags,
        ratings,
    })
}

impl SqliteCatalog {
    pub(super) fn query_photos(&self, query: &PhotoQuery) -> Result<PhotoPage, CatalogError> {
        query_page(&self.conn(), query)
    }

    pub(super) fn photo_facets(&self) -> Result<PhotoFacets, CatalogError> {
        facets(&self.conn())
    }

    pub(super) fn load_decision(&self, id: PhotoId) -> Result<Decision, CatalogError> {
        let row: Option<(String, u8)> = self
            .conn()
            .query_row(
                "SELECT flag, rating FROM decision WHERE photo_id = ?1",
                [id.0],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(storage)?;
        Ok(row
            .map(|(flag, rating)| Decision {
                flag: Flag::from_name(&flag).unwrap_or_default(),
                rating: Rating::new(rating).unwrap_or_default(),
            })
            .unwrap_or_default())
    }

    pub(super) fn store_decision(
        &self,
        id: PhotoId,
        decision: Decision,
    ) -> Result<(), CatalogError> {
        self.conn()
            .execute(
                "INSERT INTO decision (photo_id, flag, rating) VALUES (?1, ?2, ?3)
                 ON CONFLICT (photo_id) DO UPDATE SET flag = excluded.flag, rating = excluded.rating,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
                params![id.0, decision.flag.as_str(), decision.rating.stars()],
            )
            .map_err(|e| match e.sqlite_error_code() {
                Some(rusqlite::ErrorCode::ConstraintViolation) => CatalogError::Storage(format!("no photo {}", id.0)),
                _ => storage(e),
            })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursors_round_trip() {
        for c in [
            Cursor::Dated {
                local: "2026-10-12T14:30:05.25".into(),
                id: 7,
            },
            Cursor::Undated { id: 3 },
            Cursor::Imported { id: 9 },
        ] {
            assert_eq!(Cursor::decode(&c.encode()), Some(c));
        }
        assert_eq!(Cursor::decode("x|1"), None);
        assert_eq!(Cursor::decode("d|2026|nope"), None);
    }
}
