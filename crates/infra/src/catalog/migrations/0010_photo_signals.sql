-- What the library shows and filters about each photo's AI analysis,
-- derived with the combined score: the aesthetic score (0-1), whether
-- someone important has closed eyes (NULL when undecided or no faces) and
-- how many faces were found.

ALTER TABLE photo_score ADD COLUMN aesthetic REAL;
ALTER TABLE photo_score ADD COLUMN eyes_closed INTEGER;
ALTER TABLE photo_score ADD COLUMN faces INTEGER NOT NULL DEFAULT 0;
