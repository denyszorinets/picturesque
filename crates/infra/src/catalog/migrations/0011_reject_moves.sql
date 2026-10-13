-- Photos moved to the _Rejected folder, by batch (one "Move rejects"),
-- so the latest move can be undone.

CREATE TABLE reject_move (
    photo_id INTEGER PRIMARY KEY REFERENCES photo (id) ON DELETE CASCADE,
    batch    INTEGER NOT NULL
);

CREATE INDEX reject_move_batch ON reject_move (batch);
