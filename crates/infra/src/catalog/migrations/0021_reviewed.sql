-- Human review progress, independent of decisions, AI labels and copies.
-- Folder catalogs containing these records are protected from expiry.
CREATE TABLE reviewed (
    photo_id INTEGER PRIMARY KEY REFERENCES photo(id) ON DELETE CASCADE,
    reviewed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
