-- Combined quality score of each analyzed photo (technical, aesthetic and
-- faces; see core::ranking::photo_quality), recomputed with the series.
-- Kept in a table so the library can sort by it.

CREATE TABLE photo_score (
    photo_id INTEGER PRIMARY KEY REFERENCES photo (id) ON DELETE CASCADE,
    score    REAL NOT NULL
);
