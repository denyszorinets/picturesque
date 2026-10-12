-- Import runs, so plans can be reviewed and interrupted imports resumed
-- (ADR-0008).

CREATE TABLE import_run (
    id              INTEGER PRIMARY KEY,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    state           TEXT NOT NULL CHECK (state IN
                        ('planning', 'planned', 'copying', 'completed', 'cancelled', 'failed', 'interrupted')),
    library_root    TEXT NOT NULL,
    template        TEXT NOT NULL,
    -- JSON array of absolute source folders.
    sources         TEXT NOT NULL CHECK (json_valid(sources)),
    -- JSON array of {path, reason}.
    problems        TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(problems)),
    error           TEXT
);

CREATE TABLE import_photo (
    id          INTEGER PRIMARY KEY,
    run_id      INTEGER NOT NULL REFERENCES import_run (id) ON DELETE CASCADE,
    ord         INTEGER NOT NULL,
    -- CaptureMetadata as JSON; inserted into the catalog when imported.
    metadata    TEXT NOT NULL CHECK (json_valid(metadata)),
    status      TEXT NOT NULL CHECK (status IN ('planned', 'duplicate', 'imported', 'failed')),
    reason      TEXT,
    photo_id    INTEGER REFERENCES photo (id) ON DELETE SET NULL
);

CREATE INDEX import_photo_run ON import_photo (run_id, ord);

CREATE TABLE import_file (
    id              INTEGER PRIMARY KEY,
    import_photo_id INTEGER NOT NULL REFERENCES import_photo (id) ON DELETE CASCADE,
    ord             INTEGER NOT NULL,
    source_path     TEXT NOT NULL,
    format          TEXT NOT NULL,
    size            INTEGER NOT NULL CHECK (size >= 0),
    modified_ns     INTEGER,
    hash            BLOB CHECK (hash IS NULL OR length(hash) = 32),
    -- Relative to the library root, '/'-separated.
    target_path     TEXT NOT NULL,
    status          TEXT NOT NULL CHECK (status IN ('planned', 'verified', 'failed')),
    error           TEXT
);

CREATE INDEX import_file_photo ON import_file (import_photo_id, ord);
