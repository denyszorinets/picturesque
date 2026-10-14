-- The continuous import pipeline (ADR-0012): folders imported from, one
-- job per source photo (queued until imported, a duplicate or failed) and
-- a log of what happened.

CREATE TABLE import_source (
    id         INTEGER PRIMARY KEY,
    path       TEXT NOT NULL UNIQUE,
    watch      INTEGER NOT NULL DEFAULT 1,
    added_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    scanned_at TEXT,
    scan_error TEXT
);

CREATE TABLE import_job (
    id         INTEGER PRIMARY KEY,
    source_id  INTEGER REFERENCES import_source (id) ON DELETE SET NULL,
    -- The photo's primary file in the source; one job per source photo.
    key        TEXT NOT NULL UNIQUE,
    -- The photo's files with size and modification time, as found.
    files      TEXT NOT NULL CHECK (json_valid(files)),
    stage      TEXT NOT NULL CHECK (stage IN ('queued', 'reading', 'copying', 'imported', 'duplicate', 'failed')),
    photo_id   INTEGER REFERENCES photo (id) ON DELETE SET NULL,
    error      TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX import_job_stage ON import_job (stage, id);

CREATE TABLE pipeline_log (
    id       INTEGER PRIMARY KEY,
    at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    level    TEXT NOT NULL CHECK (level IN ('info', 'warn', 'error')),
    job_id   INTEGER,
    photo_id INTEGER,
    message  TEXT NOT NULL
);
