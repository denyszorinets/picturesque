-- Whether the app suggests keeping a photo (auto-pick), derived with the
-- combined score: the best of its series or in none, good enough and with
-- nobody's eyes closed (core::ranking::is_keeper). Shown as auto-picked
-- only while the photo has no flag of its own.

ALTER TABLE photo_score ADD COLUMN keeper INTEGER NOT NULL DEFAULT 0;
CREATE INDEX photo_score_keeper ON photo_score (keeper) WHERE keeper = 1;
