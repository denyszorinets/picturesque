-- Verified copies made out of a folder session (ADR-0014): the original
-- file, where its copy went and the copy's hash, so the folder can later
-- be cleared of exactly what was copied.

CREATE TABLE copy_record (
    photo_id     INTEGER NOT NULL REFERENCES photo (id) ON DELETE CASCADE,
    source       TEXT NOT NULL,
    destination  TEXT NOT NULL,
    hash         BLOB NOT NULL,
    size         INTEGER NOT NULL,
    copied_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (photo_id, source)
);
