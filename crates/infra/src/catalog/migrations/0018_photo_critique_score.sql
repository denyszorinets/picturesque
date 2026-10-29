-- The critique score of each photo (0-1, its rating's percentile among the
-- library's critiques; see core::critique), derived with the combined score
-- so the library can filter on it. NULL without a critique.

ALTER TABLE photo_score ADD COLUMN critique REAL;
