-- Series (docs/features/series.rst). `series_member` is derived and
-- rewritten whenever series are recomputed; a series' id is its first
-- photo's id. `series_edit` holds the user's corrections, which are kept.

CREATE TABLE series_edit (
    photo_id    INTEGER PRIMARY KEY REFERENCES photo (id) ON DELETE CASCADE,
    kind        TEXT NOT NULL CHECK (kind IN ('split', 'join'))
);

CREATE TABLE series_member (
    photo_id    INTEGER PRIMARY KEY REFERENCES photo (id) ON DELETE CASCADE,
    series_id   INTEGER NOT NULL,
    position    INTEGER NOT NULL
);

CREATE INDEX series_member_series ON series_member (series_id, position);
