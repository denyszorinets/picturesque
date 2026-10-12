-- The best photo of each series (docs/features/series.rst). Ranking
-- columns are derived like the rest of `series_member`; `best_choice`
-- holds the user's choices, latest `seq` first.

ALTER TABLE series_member ADD COLUMN rank INTEGER NOT NULL DEFAULT 0;
ALTER TABLE series_member ADD COLUMN score REAL;
-- NULL for other members; 'auto' when ranked best, 'user' when chosen.
ALTER TABLE series_member ADD COLUMN best TEXT CHECK (best IN ('auto', 'user'));

CREATE TABLE best_choice (
    photo_id    INTEGER PRIMARY KEY REFERENCES photo (id) ON DELETE CASCADE,
    seq         INTEGER NOT NULL
);
