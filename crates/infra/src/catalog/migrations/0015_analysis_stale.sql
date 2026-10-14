-- Results the user asked to analyze again. A stale result stays in use
-- (scores, series, filters) until the analyzer replaces it, but its photo
-- counts as pending, like a result from an older analyzer version.

ALTER TABLE analysis ADD COLUMN stale INTEGER NOT NULL DEFAULT 0;
